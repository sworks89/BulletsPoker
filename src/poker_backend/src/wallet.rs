//! Money movements with the ICP ledger, made safe against ambiguous outcomes.
//!
//! Every deposit, sweep and withdrawal is first recorded as a `LedgerOp`, then
//! sent to the ledger with a fixed `created_at_time` and a memo carrying the op
//! id, which turns on the ledger's deduplication. The result is classified as:
//!
//! - `Completed`: the transfer happened (including a `Duplicate` reply on retry);
//! - `Failed`: it definitely did not happen, so any debit is refunded;
//! - `Unknown`: we can't tell (e.g. the reply could not be decoded). Money stays
//!   where it is and the op is re-sent, unchanged, by the retry loop. The ledger
//!   either executes it now or answers `Duplicate`, so it can never run twice.
//!   A retry that fails does NOT settle it: the ledger may reject a retry (busy,
//!   upgrading) without checking for the original, which may have gone through.
//!   Only a success/`Duplicate` reply, or an admin after the ledger's 24h dedup
//!   window has closed, settles an unknown op.
//!
//! Also here: the emergency pause switches and the solvency record.

use crate::state::{State, R};
use candid::{CandidType, Deserialize, Principal};
use icrc_ledger_types::icrc1::account::{Account, Subaccount};

const NANOS: u64 = 1_000_000_000;
/// Unknown ops are retried while the ledger can still deduplicate them.
pub const RETRY_WINDOW_NS: u64 = 23 * 60 * 60 * NANOS;
pub const RETRY_EVERY_NS: u64 = 60 * NANOS;
/// Admins may settle an unknown op by hand only once the ledger can no longer
/// execute it: its 24h dedup window plus the allowed clock drift, with margin.
pub const RESOLVE_AFTER_NS: u64 = (24 * 60 + 10) * 60 * NANOS;
/// Recent op ids kept per player for their wallet history.
const MY_OPS: usize = 20;
const HOUR_NS: u64 = 3600 * NANOS;
/// Failed deposits or sweeps one player may cause per hour.
pub const USER_FAILS_PER_HOUR: u32 = 5;
/// Failed deposit records across all players per hour.
pub const TOTAL_FAILS_PER_HOUR: u32 = 500;
/// Failed deposit/sweep records are kept this long.
pub const PRUNE_AFTER_NS: u64 = 30 * 24 * 3600 * NANOS;
/// Start of the reason on ops an admin settled; those records are never pruned.
const ADMIN_RESOLVED: &str = "Resolved by admin";
/// Most records removed per prune run.
pub const PRUNE_BATCH: usize = 500;
/// Most records looked at per prune run.
const PRUNE_SCAN: usize = 2000;

/// Failures counted in a one-hour window that opens at the first of them.
#[derive(CandidType, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct FailWindow {
    pub start: u64,
    pub count: u32,
}

#[derive(CandidType, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum OpKind {
    /// Player withdrawal from their poker balance.
    Withdraw,
    /// ICRC-2 transfer_from the player's wallet into the canister.
    Deposit,
    /// Sweep of a player's deposit subaccount into the main account.
    Sweep,
    /// Controller withdrawal from the house treasury.
    Treasury,
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub enum OpStatus {
    Pending,
    Completed { block: u64 },
    Failed { reason: String },
    Unknown { reason: String, attempts: u32 },
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct LedgerOp {
    pub id: u64,
    pub kind: OpKind,
    /// The player (or the controller for treasury withdrawals).
    pub user: Principal,
    /// Amount moved on the ledger, excluding the fee.
    pub amount: u64,
    pub fee: u64,
    pub from: Option<Account>,
    pub from_subaccount: Option<Subaccount>,
    pub to: Account,
    pub created_at_time: u64,
    pub status: OpStatus,
    pub updated_at: u64,
}

impl LedgerOp {
    /// Ledger memo: identifies the op so retries are deduplicated.
    pub fn memo(&self) -> Vec<u8> {
        let mut m = b"bp-op:".to_vec();
        m.extend_from_slice(&self.id.to_le_bytes());
        m
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self.status, OpStatus::Unknown { .. })
    }
}

/// How a ledger call ended.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Ok(u64),
    Failed(String),
    Unknown(String),
}

#[derive(CandidType, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct PauseFlags {
    pub deposits: bool,
    pub withdrawals: bool,
    /// No new hands are dealt (hands in progress finish).
    pub dealing: bool,
    /// No new seats, rooms or tournament entries.
    pub joins: bool,
    pub reason: Option<String>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct SolvencyReport {
    pub checked_at: u64,
    pub ledger_balance: u64,
    pub liabilities: u64,
    /// Ledger balance minus liabilities (negative means insolvent).
    pub surplus: i128,
    pub ok: bool,
    pub unresolved_ops: u64,
}

impl State {
    fn new_op(&mut self, kind: OpKind, user: Principal, amount: u64, to: Account, now: u64) -> LedgerOp {
        let id = self.next_op_id;
        self.next_op_id += 1;
        let op = LedgerOp {
            id,
            kind,
            user,
            amount,
            fee: self.ledger_fee,
            from: None,
            from_subaccount: None,
            to,
            created_at_time: now,
            status: OpStatus::Pending,
            updated_at: now,
        };
        self.stores.ledger_ops.insert(id, op.clone());
        self.unresolved.insert(id);
        if let Some(mut u) = self.users.get_mut(&user) {
            u.ops.push(id);
            let excess = u.ops.len().saturating_sub(MY_OPS);
            u.ops.drain(..excess);
        }
        op
    }

    /// Starts a player withdrawal: debits amount + fee and records the op.
    pub fn begin_withdraw(&mut self, user: Principal, to: Account, amount: u64, now: u64) -> R<LedgerOp> {
        if self.pause.withdrawals {
            return Err(self.paused_msg("Withdrawals"));
        }
        if amount == 0 {
            return Err("Enter an amount".into());
        }
        let total = amount.checked_add(self.ledger_fee).ok_or("Amount too large")?;
        self.debit(&user, total)?;
        Ok(self.new_op(OpKind::Withdraw, user, amount, to, now))
    }

    /// Starts a treasury withdrawal by a controller.
    pub fn begin_treasury_withdraw(&mut self, admin: Principal, to: Account, amount: u64, now: u64) -> R<LedgerOp> {
        let total = amount.checked_add(self.ledger_fee).ok_or("Amount too large")?;
        if self.treasury < total {
            return Err("Treasury too small".into());
        }
        self.treasury -= total;
        Ok(self.new_op(OpKind::Treasury, admin, amount, to, now))
    }

    /// Records an ICRC-2 deposit about to be pulled from the player's wallet.
    pub fn begin_deposit(&mut self, user: Principal, from: Account, to: Account, amount: u64, now: u64) -> R<LedgerOp> {
        if self.pause.deposits {
            return Err(self.paused_msg("Deposits"));
        }
        if amount == 0 {
            return Err("Enter an amount".into());
        }
        // Without a profile the pulled ICP would have nowhere to go.
        self.user(&user)?;
        self.check_not_banned(&user, now)?;
        self.check_user_fails(&user, now)?;
        // Only accounts that never deposited are held back, so cheap accounts
        // burning the budget can't lock out established players.
        if self.deposit_fail_budget.is_some_and(|w| now < w.start.saturating_add(HOUR_NS) && w.count >= TOTAL_FAILS_PER_HOUR)
            && !self.has_deposited(&user)
        {
            return Err("Deposits are busy right now, try again shortly".into());
        }
        let mut op = self.new_op(OpKind::Deposit, user, amount, to, now);
        op.from = Some(from);
        self.stores.ledger_ops.insert(op.id, op.clone());
        Ok(op)
    }

    /// Records a sweep of `amount` from the player's deposit subaccount.
    pub fn begin_sweep(&mut self, user: Principal, sub: Subaccount, to: Account, amount: u64, now: u64) -> R<LedgerOp> {
        if self.pause.deposits {
            return Err(self.paused_msg("Deposits"));
        }
        self.user(&user)?;
        self.check_user_fails(&user, now)?;
        let mut op = self.new_op(OpKind::Sweep, user, amount, to, now);
        op.from_subaccount = Some(sub);
        self.stores.ledger_ops.insert(op.id, op.clone());
        Ok(op)
    }

    /// Has this player ever had a deposit or sweep complete? Accounts from before
    /// the flag existed are judged by their recent ops.
    fn has_deposited(&self, user: &Principal) -> bool {
        let Some(u) = self.users.get(user) else { return false };
        u.has_deposited.unwrap_or_else(|| {
            u.ops.iter().filter_map(|id| self.stores.ledger_ops.get(id)).any(|op| {
                matches!(op.kind, OpKind::Deposit | OpKind::Sweep) && matches!(op.status, OpStatus::Completed { .. })
            })
        })
    }

    /// Refuses a deposit or sweep from a player with too many recent failures.
    fn check_user_fails(&self, user: &Principal, now: u64) -> R<()> {
        let w = self.users.get(user).and_then(|u| u.deposit_fails);
        if let Some(w) = w {
            let until = w.start.saturating_add(HOUR_NS);
            if now < until && w.count >= USER_FAILS_PER_HOUR {
                let mins = (until - now).div_ceil(60 * NANOS);
                return Err(format!("Too many failed deposits — try again in {mins} minutes"));
            }
        }
        Ok(())
    }

    /// Counts a failed deposit or sweep against the player and the hourly total.
    fn note_deposit_failure(&mut self, op: &LedgerOp, now: u64) {
        if !matches!(op.kind, OpKind::Deposit | OpKind::Sweep) {
            return;
        }
        fn bump(w: Option<FailWindow>, now: u64) -> FailWindow {
            match w {
                Some(w) if now < w.start.saturating_add(HOUR_NS) => FailWindow { start: w.start, count: w.count.saturating_add(1) },
                _ => FailWindow { start: now, count: 1 },
            }
        }
        self.deposit_fail_budget = Some(bump(self.deposit_fail_budget, now));
        if let Some(mut u) = self.users.get_mut(&op.user) {
            u.deposit_fails = Some(bump(u.deposit_fails, now));
        }
    }

    /// Retention: removes settled-as-failed deposit and sweep records older than
    /// 30 days (they moved no money) and drops their ids from the player's
    /// history. Pending, unknown, completed and withdrawal records are never
    /// touched. Op ids grow with time, so a cursor walks the old records a slice
    /// at a time and wraps once it reaches records that are too young.
    /// Returns how many were removed.
    pub fn prune_failed_deposits(&mut self, now: u64) -> usize {
        let cutoff = now.saturating_sub(PRUNE_AFTER_NS);
        let start = self.prune_cursor.unwrap_or(0);
        let mut doomed: Vec<LedgerOp> = vec![];
        let mut next = None;
        let mut scanned = 0;
        for e in self.stores.ledger_ops.range(start..) {
            let op = e.value();
            if op.created_at_time >= cutoff {
                break; // this and every later record is too young
            }
            if scanned == PRUNE_SCAN || doomed.len() == PRUNE_BATCH {
                next = Some(op.id);
                break;
            }
            scanned += 1;
            let money_free = matches!(op.kind, OpKind::Deposit | OpKind::Sweep);
            // An admin's decision is kept as a record.
            if money_free && matches!(&op.status, OpStatus::Failed { reason } if !reason.starts_with(ADMIN_RESOLVED)) {
                doomed.push(op);
            }
        }
        self.prune_cursor = next;
        for op in &doomed {
            self.stores.ledger_ops.remove(&op.id);
            self.unresolved.remove(&op.id);
            if let Some(mut u) = self.users.get_mut(&op.user) {
                u.ops.retain(|id| *id != op.id);
            }
        }
        doomed.len()
    }

    /// Applies a ledger outcome to an op exactly once.
    pub fn finish_op(&mut self, id: u64, outcome: Outcome, now: u64) -> R<LedgerOp> {
        let mut op = self.stores.ledger_ops.get(&id).ok_or("No such operation")?;
        if !matches!(op.status, OpStatus::Pending | OpStatus::Unknown { .. }) {
            return Ok(op); // already settled
        }
        let attempts = match &op.status {
            OpStatus::Unknown { attempts, .. } => *attempts,
            _ => 0,
        };
        let was_unknown = op.is_unknown();
        let outcome = match outcome {
            // A failed retry proves nothing about the original attempt.
            Outcome::Failed(reason) if was_unknown => Outcome::Unknown(format!("Retry failed: {reason}")),
            o => o,
        };
        op.status = match outcome {
            Outcome::Ok(block) => {
                self.apply_success(&op, now);
                if matches!(op.kind, OpKind::Deposit | OpKind::Sweep) {
                    if let Some(mut u) = self.users.get_mut(&op.user) {
                        u.deposit_fails = None;
                        u.has_deposited = Some(true);
                    }
                }
                OpStatus::Completed { block }
            }
            Outcome::Failed(reason) => {
                self.apply_failure(&op);
                self.counters.ledger_ops_failed += 1;
                self.note_deposit_failure(&op, now);
                OpStatus::Failed { reason }
            }
            Outcome::Unknown(reason) => {
                if !was_unknown {
                    self.counters.ledger_ops_unknown += 1;
                }
                OpStatus::Unknown { reason, attempts: attempts + 1 }
            }
        };
        op.updated_at = now;
        if op.is_unknown() {
            self.unresolved.insert(id);
        } else {
            self.unresolved.remove(&id);
        }
        self.stores.ledger_ops.insert(id, op.clone());
        Ok(op)
    }

    /// Money effects once a transfer is known to have happened.
    fn apply_success(&mut self, op: &LedgerOp, now: u64) {
        match op.kind {
            // Already debited when the op started.
            OpKind::Withdraw | OpKind::Treasury => {}
            OpKind::Deposit | OpKind::Sweep => {
                if self.credit(&op.user, op.amount).is_err() {
                    // Should be impossible (users are never deleted); keep the funds visible.
                    self.treasury += op.amount;
                } else {
                    self.grant_welcome(&op.user, op.amount, now);
                }
            }
        }
    }

    /// Money effects once a transfer is known not to have happened.
    fn apply_failure(&mut self, op: &LedgerOp) {
        let total = op.amount + op.fee;
        match op.kind {
            OpKind::Withdraw => {
                if self.credit(&op.user, total).is_err() {
                    self.treasury += total;
                }
            }
            OpKind::Treasury => self.treasury += total,
            OpKind::Deposit | OpKind::Sweep => {}
        }
    }

    /// Ops still unresolved and young enough for the ledger to deduplicate a retry.
    pub fn retryable_ops(&self, now: u64) -> Vec<LedgerOp> {
        self.unresolved_ops()
            .into_iter()
            .filter(|op| now.saturating_sub(op.created_at_time) < RETRY_WINDOW_NS)
            .filter(|op| now.saturating_sub(op.updated_at) >= RETRY_EVERY_NS)
            .collect()
    }

    /// After an upgrade, ops that were still waiting on the ledger never got a
    /// reply; mark them unknown so the retry loop settles them.
    pub fn mark_interrupted_ops(&mut self, now: u64) {
        let pending: Vec<u64> = self
            .unresolved
            .iter()
            .copied()
            .filter(|id| self.stores.ledger_ops.get(id).is_some_and(|op| op.status == OpStatus::Pending))
            .collect();
        for id in pending {
            let _ = self.finish_op(id, Outcome::Unknown("interrupted by a canister upgrade".into()), now);
        }
    }

    pub fn unresolved_ops(&self) -> Vec<LedgerOp> {
        self.unresolved.iter().filter_map(|id| self.stores.ledger_ops.get(id)).filter(|op| op.is_unknown()).collect()
    }

    /// Admin resolution of an op the ledger can no longer deduplicate: the admin
    /// checked the ledger and says whether the transfer happened.
    pub fn resolve_op(&mut self, id: u64, executed: bool, block: Option<u64>, now: u64) -> R<LedgerOp> {
        let mut op = self.stores.ledger_ops.get(&id).ok_or("No such operation")?;
        if !op.is_unknown() {
            return Err("Only operations with an unknown outcome can be resolved".into());
        }
        // Before then a retry (automatic or admin) could still execute it.
        let ready_at = op.created_at_time.saturating_add(RESOLVE_AFTER_NS);
        if now < ready_at {
            let mins = (ready_at - now).div_ceil(60 * NANOS);
            return Err(format!("The ledger can still execute this operation; settle it by hand in {mins} minutes"));
        }
        if executed {
            return self.finish_op(id, Outcome::Ok(block.ok_or("Give the ledger block index of the transfer")?), now);
        }
        // finish_op keeps a failed *unknown* op unknown, so settle it directly.
        self.apply_failure(&op);
        self.counters.ledger_ops_failed += 1;
        op.status = OpStatus::Failed { reason: format!("{ADMIN_RESOLVED}: the transfer did not happen") };
        op.updated_at = now;
        self.unresolved.remove(&id);
        self.stores.ledger_ops.insert(id, op.clone());
        Ok(op)
    }

    /// ICP debited for outgoing transfers the ledger hasn't confirmed yet. It is
    /// still in the canister's ledger account until the transfer executes.
    pub fn outgoing_in_flight(&self) -> u64 {
        self.unresolved
            .iter()
            .filter_map(|id| self.stores.ledger_ops.get(id))
            .filter(|op| matches!(op.kind, OpKind::Withdraw | OpKind::Treasury))
            .map(|op| op.amount + op.fee)
            .sum()
    }

    /// The caller's most recent ledger operations, newest first.
    pub fn my_ops(&self, user: Principal, limit: usize) -> Vec<LedgerOp> {
        let ids = self.users.get(&user).map(|u| u.ops).unwrap_or_default();
        ids.iter().rev().take(limit).filter_map(|id| self.stores.ledger_ops.get(id)).collect()
    }

    // ------------------------------------------------------------- pause

    pub fn paused_msg(&self, what: &str) -> String {
        match &self.pause.reason {
            Some(r) => format!("{what} are paused: {r}"),
            None => format!("{what} are temporarily paused"),
        }
    }

    pub fn set_pause(&mut self, flags: PauseFlags) {
        self.pause = flags;
    }

    // ---------------------------------------------------------- solvency

    /// Compares the canister's ledger balance with what it owes. On a
    /// shortfall, withdrawals are paused automatically.
    pub fn record_solvency(&mut self, ledger_balance: u64, liabilities: u64, now: u64) -> SolvencyReport {
        let surplus = ledger_balance as i128 - liabilities as i128;
        let report = SolvencyReport {
            checked_at: now,
            ledger_balance,
            liabilities,
            surplus,
            ok: surplus >= 0,
            unresolved_ops: self.unresolved_ops().len() as u64,
        };
        if !report.ok && !self.pause.withdrawals {
            self.pause.withdrawals = true;
            self.pause.reason = Some("automatic solvency check failed; the team has been alerted".into());
        }
        self.last_solvency = Some(report.clone());
        report
    }
}

#[cfg(test)]
mod tests {
    use crate::state::{State, E8S};
    use super::*;

    fn p(n: u8) -> Principal {
        Principal::from_slice(&[n; 10])
    }

    fn acct(n: u8) -> Account {
        Account { owner: p(n), subaccount: None }
    }

    fn setup() -> State {
        let mut s = State::new(Principal::management_canister(), 10_000);
        s.register(p(1), "alice".into(), 0).unwrap();
        s.credit(&p(1), 10 * E8S).unwrap();
        s
    }

    fn balance(s: &State) -> u64 {
        s.users.get(&p(1)).unwrap().balance
    }

    #[test]
    fn withdraw_success_keeps_debit() {
        let mut s = setup();
        let op = s.begin_withdraw(p(1), acct(9), E8S, 5).unwrap();
        assert_eq!(balance(&s), 9 * E8S - 10_000);
        assert_eq!(op.memo(), [b"bp-op:".as_slice(), &op.id.to_le_bytes()].concat());
        let done = s.finish_op(op.id, Outcome::Ok(77), 6).unwrap();
        assert_eq!(done.status, OpStatus::Completed { block: 77 });
        assert_eq!(balance(&s), 9 * E8S - 10_000);
        // Settling twice has no effect.
        s.finish_op(op.id, Outcome::Failed("late".into()), 7).unwrap();
        assert_eq!(balance(&s), 9 * E8S - 10_000);
    }

    #[test]
    fn withdraw_failure_refunds() {
        let mut s = setup();
        let op = s.begin_withdraw(p(1), acct(9), E8S, 5).unwrap();
        s.finish_op(op.id, Outcome::Failed("InsufficientFunds".into()), 6).unwrap();
        assert_eq!(balance(&s), 10 * E8S);
    }

    #[test]
    fn unknown_withdrawal_is_never_refunded_until_resolved() {
        let mut s = setup();
        let op = s.begin_withdraw(p(1), acct(9), E8S, 0).unwrap();
        s.finish_op(op.id, Outcome::Unknown("could not decode reply".into()), 1).unwrap();
        assert_eq!(balance(&s), 9 * E8S - 10_000, "money stays debited");
        assert_eq!(s.unresolved_ops().len(), 1);
        // Retried after a minute, within the dedup window.
        assert!(s.retryable_ops(30 * NANOS).is_empty());
        assert_eq!(s.retryable_ops(2 * RETRY_EVERY_NS).len(), 1);
        // The retry hits the ledger's dedup: the first transfer did happen.
        s.finish_op(op.id, Outcome::Ok(123), 2 * RETRY_EVERY_NS).unwrap();
        assert_eq!(balance(&s), 9 * E8S - 10_000);
        assert!(s.unresolved_ops().is_empty());
    }

    #[test]
    fn stale_unknown_ops_need_an_admin() {
        let mut s = setup();
        let op = s.begin_withdraw(p(1), acct(9), E8S, 0).unwrap();
        s.finish_op(op.id, Outcome::Unknown("timeout".into()), 1).unwrap();
        assert!(s.retryable_ops(RETRY_WINDOW_NS + 1).is_empty(), "outside the dedup window");
        assert!(s.resolve_op(op.id, false, None, RETRY_WINDOW_NS + 1).unwrap_err().contains("still execute"), "too early");
        let late = RESOLVE_AFTER_NS;
        assert!(s.resolve_op(op.id, true, None, late).is_err(), "needs a block index");
        s.resolve_op(op.id, false, None, late).unwrap();
        assert_eq!(balance(&s), 10 * E8S, "refunded after the admin confirmed it never happened");
        assert!(s.resolve_op(op.id, false, None, late + 1).is_err());
    }

    #[test]
    fn deposits_credit_only_on_success() {
        let mut s = setup();
        let me = Account { owner: Principal::anonymous(), subaccount: None };
        let op = s.begin_deposit(p(1), acct(1), me, 3 * E8S, 0).unwrap();
        assert_eq!(balance(&s), 10 * E8S);
        s.finish_op(op.id, Outcome::Unknown("?".into()), 1).unwrap();
        assert_eq!(balance(&s), 10 * E8S);
        s.finish_op(op.id, Outcome::Ok(5), 2).unwrap();
        assert_eq!(balance(&s), 13 * E8S);
        let op = s.begin_sweep(p(1), [1; 32], me, E8S, 3).unwrap();
        s.finish_op(op.id, Outcome::Failed("no funds".into()), 4).unwrap();
        assert_eq!(balance(&s), 13 * E8S);
        assert_eq!(s.my_ops(p(1), 10).len(), 2);
    }

    #[test]
    fn ops_interrupted_by_an_upgrade_get_retried() {
        let mut s = setup();
        let op = s.begin_withdraw(p(1), acct(9), E8S, 0).unwrap();
        assert!(s.unresolved_ops().is_empty(), "pending, call in flight");
        s.mark_interrupted_ops(10);
        assert_eq!(s.unresolved_ops().len(), 1);
        assert_eq!(s.retryable_ops(10 + RETRY_EVERY_NS).len(), 1);
        s.finish_op(op.id, Outcome::Ok(9), 11).unwrap();
        assert!(s.unresolved_ops().is_empty());
        assert_eq!(balance(&s), 9 * E8S - 10_000);
    }

    #[test]
    fn a_failed_retry_never_refunds_an_unknown_withdrawal() {
        let mut s = setup();
        let op = s.begin_withdraw(p(1), acct(9), E8S, 0).unwrap();
        assert_eq!(s.outgoing_in_flight(), E8S + 10_000);
        s.finish_op(op.id, Outcome::Unknown("reply lost".into()), 1).unwrap();
        // The retry hits a busy or upgrading ledger: the first send may still have executed.
        let after = s.finish_op(op.id, Outcome::Failed("Ledger rejected the call: CanisterError".into()), 2).unwrap();
        assert!(after.is_unknown(), "still unknown, not failed");
        assert_eq!(balance(&s), 9 * E8S - 10_000, "no refund");
        assert_eq!(s.unresolved_ops().len(), 1);
        assert_eq!(s.counters.ledger_ops_unknown, 1, "counted once");
        // A later retry learns it went through.
        s.finish_op(op.id, Outcome::Ok(42), 3).unwrap();
        assert_eq!(balance(&s), 9 * E8S - 10_000);
        assert_eq!(s.outgoing_in_flight(), 0);
    }

    #[test]
    fn deposits_need_a_profile_and_an_amount() {
        let mut s = setup();
        let me = Account { owner: Principal::anonymous(), subaccount: None };
        assert!(s.begin_deposit(p(7), acct(7), me, E8S, 0).is_err(), "no profile");
        assert!(s.begin_deposit(p(1), acct(1), me, 0, 0).is_err(), "zero");
        assert!(s.my_ops(p(1), 10).is_empty(), "nothing recorded");
    }

    #[test]
    fn pause_switches() {
        let mut s = setup();
        s.set_pause(PauseFlags { withdrawals: true, reason: Some("maintenance".into()), ..Default::default() });
        let e = s.begin_withdraw(p(1), acct(9), E8S, 0).unwrap_err();
        assert!(e.contains("maintenance"));
        assert_eq!(balance(&s), 10 * E8S);
        let me = Account { owner: Principal::anonymous(), subaccount: None };
        assert!(s.begin_deposit(p(1), acct(1), me, E8S, 0).is_ok(), "deposits still open");
    }

    #[test]
    fn solvency_shortfall_pauses_withdrawals() {
        let mut s = setup();
        let liab = s.house_stats().liabilities_total;
        let ok = s.record_solvency(10 * E8S, liab, 1);
        assert!(ok.ok && ok.surplus == 0);
        assert!(!s.pause.withdrawals);
        let bad = s.record_solvency(10 * E8S - 1, liab, 2);
        assert!(!bad.ok && bad.surplus == -1);
        assert!(s.pause.withdrawals);
        assert!(s.begin_withdraw(p(1), acct(9), E8S, 3).is_err());
    }

    fn fail_deposit(s: &mut State, who: u8, now: u64) {
        let me = Account { owner: Principal::anonymous(), subaccount: None };
        let op = s.begin_deposit(p(who), acct(who), me, E8S, now).unwrap();
        s.finish_op(op.id, Outcome::Failed("InsufficientAllowance".into()), now).unwrap();
    }

    #[test]
    fn repeated_failed_deposits_back_off() {
        let mut s = setup();
        let me = Account { owner: Principal::anonymous(), subaccount: None };
        for i in 0..5 {
            fail_deposit(&mut s, 1, i);
        }
        let e = s.begin_deposit(p(1), acct(1), me, E8S, 10 * NANOS).unwrap_err();
        assert!(e.contains("Too many failed deposits") && e.contains("minutes"), "{e}");
        assert!(s.begin_sweep(p(1), [1; 32], me, E8S, 10 * NANOS).unwrap_err().contains("Too many failed deposits"));
        // An hour after the window opened, they may try again.
        assert!(s.begin_deposit(p(1), acct(1), me, E8S, HOUR_NS + 1).is_ok());
    }

    #[test]
    fn a_successful_deposit_resets_the_failure_count() {
        let mut s = setup();
        let me = Account { owner: Principal::anonymous(), subaccount: None };
        for i in 0..4 {
            fail_deposit(&mut s, 1, i);
        }
        let ok = s.begin_deposit(p(1), acct(1), me, E8S, 5).unwrap();
        s.finish_op(ok.id, Outcome::Ok(1), 6).unwrap();
        for i in 7..11 {
            fail_deposit(&mut s, 1, i);
        }
        assert!(s.begin_deposit(p(1), acct(1), me, E8S, 12).is_ok(), "count started over");
    }

    #[test]
    fn aggregate_failure_budget() {
        let mut s = setup();
        let me = Account { owner: Principal::anonymous(), subaccount: None };
        // 100 users x 5 failures = 500 failed records this hour.
        for n in 0..100u8 {
            let who = 100 + n;
            s.register(p(who), format!("user{who}"), 0).unwrap();
            for i in 0..5 {
                fail_deposit(&mut s, who, i);
            }
        }
        let e = s.begin_deposit(p(1), acct(1), me, E8S, 20).unwrap_err();
        assert!(e.contains("busy"), "{e}");
        // The budget resets with the hour.
        assert!(s.begin_deposit(p(1), acct(1), me, E8S, HOUR_NS + 1).is_ok());
    }

    fn age_failed(s: &mut State, kind_deposit: bool, created: u64) -> u64 {
        let me = Account { owner: Principal::anonymous(), subaccount: None };
        let op = if kind_deposit {
            s.begin_deposit(p(1), acct(1), me, E8S, created).unwrap()
        } else {
            s.begin_sweep(p(1), [1; 32], me, E8S, created).unwrap()
        };
        s.finish_op(op.id, Outcome::Failed("x".into()), created).unwrap();
        s.users.get_mut(&p(1)).unwrap().deposit_fails = None;
        op.id
    }

    #[test]
    fn prune_removes_only_old_failed_deposits() {
        let mut s = setup();
        let old_dep = age_failed(&mut s, true, 0);
        let old_sweep = age_failed(&mut s, false, 1);
        let recent = age_failed(&mut s, true, PRUNE_AFTER_NS);
        let now = PRUNE_AFTER_NS + 10;
        assert_eq!(s.prune_failed_deposits(now), 2);
        assert!(s.stores.ledger_ops.get(&old_dep).is_none());
        assert!(s.stores.ledger_ops.get(&old_sweep).is_none());
        assert!(s.stores.ledger_ops.get(&recent).is_some());
        let ops = s.users.get(&p(1)).unwrap().ops;
        assert_eq!(ops, vec![recent], "pruned ids leave the player's history");
        assert_eq!(s.prune_failed_deposits(now), 0);
    }

    #[test]
    fn prune_keeps_unresolved_and_completed() {
        let mut s = setup();
        let me = Account { owner: Principal::anonymous(), subaccount: None };
        let done = s.begin_deposit(p(1), acct(1), me, E8S, 0).unwrap();
        s.finish_op(done.id, Outcome::Ok(1), 1).unwrap();
        let unknown = s.begin_deposit(p(1), acct(1), me, E8S, 2).unwrap();
        s.finish_op(unknown.id, Outcome::Unknown("?".into()), 3).unwrap();
        let pending = s.begin_sweep(p(1), [1; 32], me, E8S, 4).unwrap();
        // A failed withdrawal (refunded) is a money record and stays.
        let w = s.begin_withdraw(p(1), acct(9), E8S, 5).unwrap();
        s.finish_op(w.id, Outcome::Failed("x".into()), 6).unwrap();
        s.treasury = 10 * E8S;
        let t = s.begin_treasury_withdraw(Principal::management_canister(), acct(9), 1, 7).unwrap();
        s.finish_op(t.id, Outcome::Failed("x".into()), 8).unwrap();
        let failed = age_failed(&mut s, true, 8);
        let far = 100 * PRUNE_AFTER_NS;
        assert_eq!(s.prune_failed_deposits(far), 1);
        for id in [done.id, unknown.id, pending.id, w.id, t.id] {
            assert!(s.stores.ledger_ops.get(&id).is_some(), "op {id} must stay");
        }
        assert!(s.stores.ledger_ops.get(&failed).is_none());
        assert_eq!(s.unresolved_ops().len(), 1);
    }

    fn exhaust_budget(s: &mut State) {
        for n in 0..100u8 {
            let who = 100 + n;
            s.register(p(who), format!("user{who}"), 0).unwrap();
            for i in 0..5 {
                fail_deposit(s, who, i);
            }
        }
    }

    #[test]
    fn established_depositors_are_not_blocked_by_the_budget() {
        let mut s = setup();
        let me = Account { owner: Principal::anonymous(), subaccount: None };
        let done = s.begin_deposit(p(1), acct(1), me, E8S, 0).unwrap();
        s.finish_op(done.id, Outcome::Ok(1), 1).unwrap();
        // An older account: no flag, but a completed deposit among its ops.
        s.register(p(2), "bob".into(), 0).unwrap();
        let old = s.begin_deposit(p(2), acct(2), me, E8S, 0).unwrap();
        s.finish_op(old.id, Outcome::Ok(2), 1).unwrap();
        s.users.get_mut(&p(2)).unwrap().has_deposited = None;
        s.register(p(3), "carol".into(), 0).unwrap();
        exhaust_budget(&mut s);
        assert!(s.begin_deposit(p(1), acct(1), me, E8S, 20).is_ok(), "flagged depositor");
        assert!(s.begin_deposit(p(2), acct(2), me, E8S, 20).is_ok(), "older depositor");
        assert!(s.begin_deposit(p(3), acct(3), me, E8S, 20).unwrap_err().contains("busy"), "new account");
    }

    #[test]
    fn prune_keeps_admin_decisions() {
        let mut s = setup();
        let me = Account { owner: Principal::anonymous(), subaccount: None };
        let op = s.begin_deposit(p(1), acct(1), me, E8S, 0).unwrap();
        s.finish_op(op.id, Outcome::Unknown("?".into()), 1).unwrap();
        s.resolve_op(op.id, false, None, RESOLVE_AFTER_NS).unwrap();
        assert_eq!(s.prune_failed_deposits(RESOLVE_AFTER_NS + 100 * PRUNE_AFTER_NS), 0);
        assert!(s.stores.ledger_ops.get(&op.id).is_some());
    }

    #[test]
    fn prune_takes_several_runs_for_many_records() {
        let mut s = setup();
        for i in 0..(PRUNE_BATCH as u64 + 10) {
            let me = Account { owner: Principal::anonymous(), subaccount: None };
            let op = s.begin_deposit(p(1), acct(1), me, E8S, i).unwrap();
            s.finish_op(op.id, Outcome::Failed("x".into()), i).unwrap();
            s.users.get_mut(&p(1)).unwrap().deposit_fails = None;
            s.deposit_fail_budget = None;
        }
        let far = 100 * PRUNE_AFTER_NS;
        assert_eq!(s.prune_failed_deposits(far), PRUNE_BATCH);
        assert_eq!(s.prune_failed_deposits(far), 10);
        assert_eq!(s.prune_failed_deposits(far), 0);
        assert!(s.stores.ledger_ops.iter().next().is_none());
    }
}
