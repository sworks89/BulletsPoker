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
//!   After the ledger's 24h dedup window an admin resolves it by hand.
//!
//! Also here: the emergency pause switches and the solvency record.

use crate::state::{State, R};
use candid::{CandidType, Deserialize, Principal};
use icrc_ledger_types::icrc1::account::{Account, Subaccount};

const NANOS: u64 = 1_000_000_000;
/// Unknown ops are retried while the ledger can still deduplicate them.
pub const RETRY_WINDOW_NS: u64 = 23 * 60 * 60 * NANOS;
pub const RETRY_EVERY_NS: u64 = 60 * NANOS;
/// Recent op ids kept per player for their wallet history.
const MY_OPS: usize = 20;

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
        self.user(&user)?;
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
        let mut op = self.new_op(OpKind::Sweep, user, amount, to, now);
        op.from_subaccount = Some(sub);
        self.stores.ledger_ops.insert(op.id, op.clone());
        Ok(op)
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
        op.status = match outcome {
            Outcome::Ok(block) => {
                self.apply_success(&op);
                OpStatus::Completed { block }
            }
            Outcome::Failed(reason) => {
                self.apply_failure(&op);
                OpStatus::Failed { reason }
            }
            Outcome::Unknown(reason) => OpStatus::Unknown { reason, attempts: attempts + 1 },
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
    fn apply_success(&mut self, op: &LedgerOp) {
        match op.kind {
            // Already debited when the op started.
            OpKind::Withdraw | OpKind::Treasury => {}
            OpKind::Deposit | OpKind::Sweep => {
                if self.credit(&op.user, op.amount).is_err() {
                    // Should be impossible (users are never deleted); keep the funds visible.
                    self.treasury += op.amount;
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
        let op = self.stores.ledger_ops.get(&id).ok_or("No such operation")?;
        if !op.is_unknown() {
            return Err("Only operations with an unknown outcome can be resolved".into());
        }
        let outcome = if executed {
            Outcome::Ok(block.ok_or("Give the ledger block index of the transfer")?)
        } else {
            Outcome::Failed("Resolved by admin: the transfer did not happen".into())
        };
        self.finish_op(id, outcome, now)
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
        assert!(s.resolve_op(op.id, true, None, 2).is_err(), "needs a block index");
        s.resolve_op(op.id, false, None, 2).unwrap();
        assert_eq!(balance(&s), 10 * E8S, "refunded after the admin confirmed it never happened");
        assert!(s.resolve_op(op.id, false, None, 3).is_err());
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
        s.finish_op(op.id, Outcome::Failed("InsufficientFunds".into()), 11).unwrap();
        assert!(s.unresolved_ops().is_empty());
        assert_eq!(balance(&s), 10 * E8S);
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
}
