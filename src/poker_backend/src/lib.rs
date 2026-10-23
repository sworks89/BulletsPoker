//! BulletsPoker backend canister: Texas Hold'em with ICP buy-ins, a cosmetic
//! item store, table gifts, chat and friends.

pub mod cards;
pub mod fair;
pub mod jackpot;
pub mod leaderboards;
pub mod ledger;
pub mod medals;
pub mod metrics;
pub mod moderation;
pub mod ratelimit;
pub mod referrals;
pub mod rewards;
pub mod signup;
pub mod state;
pub mod store;
pub mod table;
pub mod tournament;
pub mod wallet;
pub mod welcome;

use candid::{CandidType, Deserialize, Principal};
use ic_cdk::{init, inspect_message, post_upgrade, pre_upgrade, query, update};
use icrc_ledger_types::icrc1::account::Account;
use sha2::{Digest, Sha256};
use state::*;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::time::Duration;
use jackpot::JackpotInfo;
use medals::MedalDef;
use moderation::{AuditEntry, ModAction, ModPlayerView, Report, ReportPage, ReportReason};
use table::{Action, Currency, TableConfig};
use wallet::{LedgerOp, OpKind, OpStatus, PauseFlags, SolvencyReport};
use tournament::{TournamentDetail, TournamentSummary, TournamentTemplate};

/// Mainnet ICP ledger.
const ICP_LEDGER: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";
const ICP_FEE: u64 = 10_000;
/// Most tables one client can fetch in a single `get_tables` call.
const MAX_OPEN_TABLES: usize = 8;
const SOLVENCY_EVERY_SECS: u64 = 600;
const ENTROPY_EVERY_SECS: u64 = 300;

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    /// Tables with a seed request (`raw_rand`) in flight; one per table at most.
    static PENDING_SEEDS: RefCell<BTreeSet<u64>> = const { RefCell::new(BTreeSet::new()) };
    static RETRYING: Cell<bool> = const { Cell::new(false) };
    /// Users with a wallet operation in flight.
    static WALLET_LOCKS: RefCell<BTreeSet<Principal>> = const { RefCell::new(BTreeSet::new()) };
    /// Per-caller request budgets (not persisted: they refill in seconds anyway).
    static LIMITER: RefCell<ratelimit::Limiter> = RefCell::new(ratelimit::Limiter::default());
}

fn with<T>(f: impl FnOnce(&State) -> T) -> T {
    STATE.with(|s| f(s.borrow().as_ref().expect("state initialized")))
}

fn with_mut<T>(f: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|s| f(s.borrow_mut().as_mut().expect("state initialized")))
}

fn now() -> u64 {
    ic_cdk::api::time()
}

/// The signed-in caller; anonymous callers may only use queries. Every
/// update spends from the caller's request budget.
fn caller() -> R<Principal> {
    let p = ic_cdk::api::msg_caller();
    if p == Principal::anonymous() {
        return Err("Please sign in first".into());
    }
    if !LIMITER.with(|l| l.borrow_mut().take(p, now())) {
        return Err("Too many requests. Please slow down.".into());
    }
    with_mut(|s| s.touch(&p, now()));
    Ok(p)
}

/// A signed-in caller who isn't suspended.
fn active_caller() -> R<Principal> {
    let p = caller()?;
    with(|s| s.check_not_banned(&p, now()))?;
    Ok(p)
}

/// A signed-in caller who may post to chat (gifts and throws are announced there).
fn chatty_caller() -> R<Principal> {
    let p = caller()?;
    with(|s| s.check_can_chat(&p, now()))?;
    Ok(p)
}

fn admin() -> R<Principal> {
    let p = ic_cdk::api::msg_caller();
    if !ic_cdk::api::is_controller(&p) {
        return Err("Admins only".into());
    }
    Ok(p)
}

/// Controllers and appointed moderators (a suspended moderator can't moderate).
fn moderator() -> R<Principal> {
    let p = ic_cdk::api::msg_caller();
    if ic_cdk::api::is_controller(&p) {
        return Ok(p);
    }
    if !with(|s| s.is_moderator(&p)) {
        return Err("Moderators only".into());
    }
    with(|s| s.check_not_banned(&p, now()))?;
    Ok(p)
}

/// A moderator acting on `target`: see `State::check_mod_target`.
fn moderator_for(target: Principal) -> R<Principal> {
    let p = moderator()?;
    let (caller_admin, target_admin) = (ic_cdk::api::is_controller(&p), ic_cdk::api::is_controller(&target));
    with(|s| s.check_mod_target(&p, caller_admin, &target, target_admin))?;
    Ok(p)
}

/// Largest argument any public method needs (chat, notes and reports are a few KB).
const MAX_ARG_BYTES: usize = 16 * 1024;

/// Drops obviously bad update calls before they cost the canister cycles:
/// anonymous or unregistered callers, oversized arguments, privileged methods
/// from the wrong principal, and callers who are over their request budget.
/// This only runs for ingress messages (replicas may skip it), so every
/// endpoint still checks for itself.
#[inspect_message]
fn inspect_message() {
    let p = ic_cdk::api::msg_caller();
    let method = ic_cdk::api::msg_method_name();
    if p == Principal::anonymous() || ic_cdk::api::msg_arg_data().len() > MAX_ARG_BYTES {
        return;
    }
    let allowed = if method.starts_with("admin_") {
        ic_cdk::api::is_controller(&p)
    } else if method.starts_with("mod_") {
        ic_cdk::api::is_controller(&p) || with(|s| s.is_moderator(&p))
    } else {
        (method == "register" || with(|s| s.users.contains_key(&p))) && LIMITER.with(|l| l.borrow().has_budget(&p, now()))
    };
    if allowed {
        ic_cdk::api::accept_message();
    }
}

/// Releases a wallet lock when dropped (including when a call traps).
struct WalletLock(Principal);

impl WalletLock {
    fn acquire(p: Principal) -> R<Self> {
        WALLET_LOCKS.with(|l| {
            if l.borrow_mut().insert(p) {
                Ok(WalletLock(p))
            } else {
                Err("Another wallet operation is in progress".into())
            }
        })
    }
}

impl Drop for WalletLock {
    fn drop(&mut self) {
        WALLET_LOCKS.with(|l| l.borrow_mut().remove(&self.0));
    }
}

// --------------------------------------------------------------- lifecycle

#[derive(CandidType, Deserialize)]
pub struct InitArgs {
    pub ledger: Option<Principal>,
    pub ledger_fee: Option<u64>,
}

#[init]
fn init(args: Option<InitArgs>) {
    let args = args.unwrap_or(InitArgs {
        ledger: None,
        ledger_fee: None,
    });
    let ledger = args
        .ledger
        .unwrap_or_else(|| Principal::from_text(ICP_LEDGER).unwrap());
    let state = State::new_in(ic_stable_structures::DefaultMemoryImpl::default(), ledger, args.ledger_fee.unwrap_or(ICP_FEE));
    STATE.with(|s| *s.borrow_mut() = Some(state));
    start_timers();
}

#[pre_upgrade]
fn pre_upgrade() {
    // Users, hands and the ledger log already live in stable memory; only the
    // bounded heap state is written here.
    with(|s| s.save());
}

#[post_upgrade]
fn post_upgrade() {
    let mut state = State::restore(ic_stable_structures::DefaultMemoryImpl::default());
    // Calls in flight during the upgrade never reported back: retry them.
    state.mark_interrupted_ops(now());
    // House ICP tables deployed before the stake ladder move to the new blinds.
    state.migrate_icp_stakes();
    state.migrate_icp_tournament_prices();
    // Players who deposited before the welcome bonus existed are not eligible.
    state.mark_existing_depositors();
    state.seed_banked_ev();
    STATE.with(|s| *s.borrow_mut() = Some(state));
    start_timers();
}

fn start_timers() {
    ic_cdk_timers::set_timer_interval(Duration::from_secs(1), || async { tick().await });
    ic_cdk_timers::set_timer_interval(Duration::from_secs(60), || async { retry_unknown_ops().await });
    ic_cdk_timers::set_timer_interval(Duration::from_secs(SOLVENCY_EVERY_SECS), || async {
        let _ = check_solvency().await;
    });
    // Keep the secret entropy fresh even when no hands are being dealt.
    ic_cdk_timers::set_timer(Duration::ZERO, refresh_entropy());
    ic_cdk_timers::set_timer_interval(Duration::from_secs(ENTROPY_EVERY_SECS), || async { refresh_entropy().await });
}

async fn refresh_entropy() {
    if let Ok(rand) = ledger::raw_rand().await {
        with_mut(|s| s.mix_entropy(&rand));
    }
}

/// Re-sends ledger ops whose outcome is unknown. The ledger either executes
/// them now or reports them as duplicates of the original, so nothing runs twice.
async fn retry_unknown_ops() {
    with_mut(|s| s.prune_failed_deposits(now()));
    if RETRYING.get() {
        return;
    }
    // Cleared on drop, so a trap mid-loop can't leave retries switched off.
    let _running = FlagGuard::set(&RETRYING);
    for op in with(|s| s.retryable_ops(now())) {
        let _ = run_op(op).await;
    }
}

/// Sets a thread-local flag and clears it when dropped (ic-cdk drops a
/// trapped call's locals in its cleanup step).
struct FlagGuard(&'static std::thread::LocalKey<Cell<bool>>);

impl FlagGuard {
    fn set(flag: &'static std::thread::LocalKey<Cell<bool>>) -> Self {
        flag.set(true);
        FlagGuard(flag)
    }
}

impl Drop for FlagGuard {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

async fn check_solvency() -> R<SolvencyReport> {
    let ledger = with(|s| s.ledger);
    let mut attempt = 0;
    loop {
        attempt += 1;
        // Transfers can settle while we wait for the balance: a withdrawal that
        // executes counts in the "before" figure but not the balance, and a
        // deposit credited before the reply counts "after" but not in the
        // balance. Compare against the smaller of the two snapshots...
        let before = with(|s| s.house_stats().liabilities_total);
        let balance = ledger::balance_of(ledger, main_account()).await?;
        let liabilities = with(|s| s.house_stats().liabilities_total).min(before);
        // ...and only pause withdrawals if a second, independent check agrees.
        let last = attempt >= 2 || balance >= liabilities;
        if last {
            return Ok(with_mut(|s| s.record_solvency(balance, liabilities, now())));
        }
    }
}

/// Runs every second: enforces action clocks and deals new hands.
async fn tick() {
    LIMITER.with(|l| l.borrow_mut().prune(now()));
    let ready = with_mut(|s| s.tick(now()));
    // One fresh `raw_rand` per hand (owner decision D2a), requested
    // concurrently; a table with a request already in flight is skipped.
    let requests: Vec<_> = ready
        .into_iter()
        .filter_map(|id| {
            let hand_no = with(|s| s.tables.get(&id).map(|t| t.hand_no))?;
            Some(seed_and_deal(SeedRequest::claim(id)?, hand_no))
        })
        .collect();
    if !requests.is_empty() {
        futures_util::future::join_all(requests).await;
    }
}

/// Fetches a seed for one table's next hand and deals it. A failed or
/// malformed `raw_rand` means no hand this tick: there is no fallback.
async fn seed_and_deal(request: SeedRequest, hand_no: u64) {
    let Ok(rand) = ledger::raw_rand().await else { return };
    let Ok(seed) = <[u8; 32]>::try_from(rand.as_slice()) else { return };
    // The secret entropy gets fresh randomness too, domain-separated from the
    // seed (which is revealed after the hand).
    let mut h = Sha256::new();
    h.update(b"BulletsPoker/entropy/v1");
    h.update(seed);
    let fresh = h.finalize();
    with_mut(|s| {
        s.mix_entropy(&fresh);
        // Refused when the table changed during the await: the seed is dropped unseen.
        let _ = s.deal_with_seed(request.0, hand_no, seed, ic_cdk::api::canister_self(), now());
    });
}

/// A table's place in `PENDING_SEEDS`, released on drop (also when a trapped
/// call is cleaned up), so a table can't have two seed requests in flight.
struct SeedRequest(u64);

impl SeedRequest {
    fn claim(table: u64) -> Option<Self> {
        PENDING_SEEDS.with(|p| p.borrow_mut().insert(table)).then_some(SeedRequest(table))
    }
}

impl Drop for SeedRequest {
    fn drop(&mut self) {
        PENDING_SEEDS.with(|p| p.borrow_mut().remove(&self.0));
    }
}

// ----------------------------------------------------------------- profile

#[query]
fn whoami() -> Principal {
    ic_cdk::api::msg_caller()
}

/// How much sign-up work a new account needs right now (see `register`).
#[query]
fn get_signup_challenge() -> signup::SignupChallenge {
    signup::SignupChallenge { difficulty: with(|s| s.signup_difficulty(now())) }
}

/// Creates your profile. `pow` is a nonce solving `get_signup_challenge`.
#[update]
fn register(username: String, pow: Option<u64>) -> R<Profile> {
    let p = caller()?;
    with(|s| s.check_signup_pow(&p, pow.unwrap_or(0), now()))?;
    with_mut(|s| s.register(p, username, now()))
}

/// Attaches a referrer (another player's username) within your first 24 hours.
#[update]
fn set_referrer(code: String) -> R<()> {
    let p = active_caller()?;
    with_mut(|s| s.set_referrer(p, &code, now()))
}

/// Every table where the caller holds a seat (cash or tournament), so a client
/// can find seats it doesn't know about: tournament moves, another device.
#[query]
fn get_my_seats() -> Vec<SeatRef> {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.my_seats(&p))
}

#[query]
fn get_my_profile() -> Option<Profile> {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.profile(&p))
}

#[query]
fn get_profile(p: Principal) -> Option<PublicProfile> {
    with(|s| s.public_profile(&p, now()))
}

#[query]
fn find_user(username: String) -> Option<PublicProfile> {
    with(|s| s.find_user(&username).and_then(|p| s.public_profile(&p, now())))
}

#[update]
fn set_status(status: String) -> R<()> {
    let p = active_caller()?;
    with_mut(|s| s.set_status(p, status))
}

// ------------------------------------------------------------------ wallet

#[derive(CandidType, Deserialize)]
pub struct DepositInfo {
    /// ICRC-1 account to send ICP to (this canister + your deposit subaccount).
    pub account: Account,
    /// Legacy account identifier for exchanges / older wallets.
    pub account_id: String,
    pub ledger: Principal,
    pub fee: u64,
}

#[query]
fn get_deposit_info() -> DepositInfo {
    let p = ic_cdk::api::msg_caller();
    let sub = ledger::deposit_subaccount(&p);
    let me = ic_cdk::api::canister_self();
    DepositInfo {
        account: Account {
            owner: me,
            subaccount: Some(sub),
        },
        account_id: ledger::account_identifier(&me, &sub),
        ledger: with(|s| s.ledger),
        fee: with(|s| s.ledger_fee),
    }
}

fn main_account() -> Account {
    Account {
        owner: ic_cdk::api::canister_self(),
        subaccount: None,
    }
}

/// Sends a recorded ledger op and settles it. Deposits pull with ICRC-2;
/// everything else is a plain ICRC-1 transfer.
async fn run_op(op: LedgerOp) -> R<LedgerOp> {
    let ledger = with(|s| s.ledger);
    let outcome = match op.kind {
        OpKind::Deposit => ledger::transfer_from_op(ledger, &op).await,
        _ => ledger::transfer_op(ledger, &op).await,
    };
    with_mut(|s| s.finish_op(op.id, outcome, now()))
}

/// Turns a settled op into the endpoint's reply.
fn op_reply(op: LedgerOp, ok: impl FnOnce(u64) -> u64) -> R<u64> {
    match op.status {
        OpStatus::Completed { block } => Ok(ok(block)),
        OpStatus::Failed { reason } => Err(reason),
        _ => Err(format!(
            "The ledger hasn't confirmed this yet (operation #{}). It will be settled automatically; check your wallet history.",
            op.id
        )),
    }
}

/// Deposit via ICRC-2: the caller first approves this canister for
/// `amount + fee` on the ICP ledger, then calls this. Returns the new balance.
#[update]
async fn deposit(amount: u64) -> R<u64> {
    let p = caller()?;
    let _lock = WalletLock::acquire(p)?;
    let from = Account { owner: p, subaccount: None };
    let op = with_mut(|s| s.begin_deposit(p, from, main_account(), amount, now()))?;
    let op = run_op(op).await?;
    op_reply(op, |_| with(|s| s.users.get(&p).map_or(0, |u| u.balance)))
}

/// Deposit via plain transfer: send ICP to your deposit account (see
/// `get_deposit_info`) from any wallet or exchange, then call this. Returns the amount credited.
#[update]
async fn claim_deposit() -> R<u64> {
    let p = caller()?;
    with(|s| s.profile(&p)).ok_or("Create a profile first")?;
    let _lock = WalletLock::acquire(p)?;
    let (ledger, fee) = with(|s| (s.ledger, s.ledger_fee));
    let sub = ledger::deposit_subaccount(&p);
    let bal = ledger::balance_of(ledger, Account { owner: ic_cdk::api::canister_self(), subaccount: Some(sub) }).await?;
    if bal <= fee {
        return Err("No new deposit found".into());
    }
    let amount = bal - fee;
    let op = with_mut(|s| s.begin_sweep(p, sub, main_account(), amount, now()))?;
    let op = run_op(op).await?;
    op_reply(op, |_| amount)
}

/// Sends `amount` e8s to `to`. The ledger fee is charged on top. Returns the block index.
#[update]
async fn withdraw(to: Account, amount: u64) -> R<u64> {
    let p = caller()?;
    let _lock = WalletLock::acquire(p)?;
    let op = with_mut(|s| s.begin_withdraw(p, to, amount, now()))?;
    let op = run_op(op).await?;
    op_reply(op, |block| block)
}

/// Your recent deposits and withdrawals, newest first.
#[query]
fn get_my_ledger_ops() -> Vec<LedgerOp> {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.my_ops(p, 20))
}

/// Public status: what's paused and the latest solvency check.
#[derive(CandidType, Deserialize)]
pub struct ServiceStatus {
    pub pause: PauseFlags,
    pub solvency: Option<SolvencyReport>,
}

#[query]
fn get_status() -> ServiceStatus {
    with(|s| ServiceStatus {
        pause: s.pause.clone(),
        solvency: s.last_solvency.clone(),
    })
}

// ------------------------------------------------------------------- store

#[query]
fn list_store() -> Vec<StoreItem> {
    with(|s| s.items.values().filter(|i| i.available).cloned().collect())
}

#[update]
fn buy_item(id: u32, currency: Currency) -> R<()> {
    let p = active_caller()?;
    with_mut(|s| s.buy_item(p, id, currency))
}

#[update]
fn equip(id: u32) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.equip(p, id))
}

#[update]
fn send_gift(table_id: u64, to: Principal, item_id: u32, currency: Currency) -> R<()> {
    let p = chatty_caller()?;
    with_mut(|s| s.send_gift(p, table_id, to, item_id, currency, now()))
}

#[update]
fn throw_item(table_id: u64, to: Principal, item_id: u32, currency: Currency) -> R<()> {
    let p = chatty_caller()?;
    with_mut(|s| s.throw_item(p, table_id, to, item_id, currency, now()))
}

// ------------------------------------------------------- chips & settings

/// Claims the daily free play-chip bonus (bigger on consecutive days).
#[update]
fn claim_daily_chips() -> R<u64> {
    let p = active_caller()?;
    with_mut(|s| s.claim_daily(p, now()))
}

/// Spins the oldest banked Fish Buffet spin for ICP cashback.
#[update]
fn spin_wheel() -> R<rewards::SpinRecord> {
    let p = active_caller()?;
    with_mut(|s| s.spin_wheel(p, now()))
}

/// The caller's Fish Buffet standing, spins and history.
#[query]
fn get_rewards() -> rewards::RewardsView {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.rewards_view(&p, now()))
}

/// The caller's referral earnings and friends, `offset` friends in (newest first).
#[query]
fn get_referrals(offset: u32) -> referrals::ReferralView {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.referral_view(&p, offset, now()))
}

/// The caller's welcome bonus: the offer, or its progress.
#[query]
fn get_welcome() -> welcome::WelcomeView {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.welcome_view(&p, now()))
}

/// The four weekly and monthly leaderboards, with the caller's own row.
#[query]
fn get_leaderboards() -> Vec<leaderboards::BoardView> {
    let p = ic_cdk::api::msg_caller();
    let me = (p != candid::Principal::anonymous()).then_some(p);
    with(|s| s.leaderboards(me, now()))
}

#[update]
fn update_settings(settings: Settings) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.update_settings(p, settings))
}

// ------------------------------------------------------------ hand history

/// Your most recent hands, newest first.
#[query]
fn get_hand_history(limit: u32) -> Vec<HandSummary> {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.hand_history(p, limit.min(50) as usize))
}

/// Full replay of a hand you played in.
#[query]
fn get_hand(id: u64) -> Option<HandRecord> {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.hand(p, id))
}

// ------------------------------------------------------------------ tables

#[query]
fn list_tables() -> Vec<TableSummary> {
    with(|s| s.list_tables())
}

#[query]
fn get_table(id: u64) -> Option<TableView> {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.table_view(id, Some(p), now()))
}

/// `password` is only needed for password-protected player rooms.
/// Views of several tables at once (for multi-tabling); unknown ids are skipped.
#[query]
fn get_tables(ids: Vec<u64>) -> Vec<TableView> {
    let p = ic_cdk::api::msg_caller();
    let now = now();
    with(|s| ids.iter().take(MAX_OPEN_TABLES).filter_map(|id| s.table_view(*id, Some(p), now)).collect())
}

/// `password` is only needed for password-protected player rooms.
#[update]
fn join_table(id: u64, seat: u8, buy_in: u64, password: Option<String>) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.join_table(p, id, seat, buy_in, password, now()))
}

/// Joins a game: a free seat at its fullest table, or a new copy of it when every
/// table is full. Returns the id of the table you were seated at.
#[update]
fn join_game(game: u64, buy_in: u64) -> R<u64> {
    let p = caller()?;
    with_mut(|s| s.join_game(p, game, buy_in, now()))
}

/// Waits for a seat at a table: you're seated automatically, with this buy-in,
/// when one opens. `None` leaves the line. Keep calling `auto_sit_ping` while
/// you watch, or you're dropped from the line.
#[update]
fn auto_sit(table: u64, buy_in: Option<u64>) -> R<AutoSitStatus> {
    let p = caller()?;
    with_mut(|s| s.auto_sit(p, table, buy_in, now()))
}

/// Keeps you in a table's auto-sit line and reports whether you were seated or dropped.
#[update]
fn auto_sit_ping(table: u64) -> R<AutoSitStatus> {
    let p = caller()?;
    Ok(with_mut(|s| s.auto_sit_ping(p, table, now())))
}

/// Creates your own table. Hosts of ICP tables earn 20% of the rake it generates.
#[update]
fn create_room(args: RoomArgs) -> R<u64> {
    let p = caller()?;
    with_mut(|s| s.create_room(p, args, now()))
}

/// Enters a private room's password so you can watch it and chat there.
#[update]
fn unlock_room(id: u64, password: String) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.unlock_room(p, id, password))
}

#[update]
fn close_room(id: u64) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.close_room(p, id))
}

// ------------------------------------------------------------- tournaments

#[query]
fn list_tournaments() -> Vec<TournamentSummary> {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.list_tournaments(Some(p)))
}

#[query]
fn get_tournament(id: u64) -> Option<TournamentDetail> {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.tournament_detail(id, Some(p)))
}

#[update]
fn register_tournament(id: u64) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.register_tournament(p, id, now()))
}

#[update]
fn unregister_tournament(id: u64) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.unregister_tournament(p, id))
}

/// Buys a fresh starting stack during a tournament's rebuy period (buy-in only, no fee).
#[update]
fn rebuy_tournament(id: u64) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.rebuy(p, id, now()))
}

/// Takes the one-time add-on at the end of a tournament's rebuy period.
#[update]
fn addon_tournament(id: u64) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.addon(p, id, now()))
}

// ------------------------------------------------------------- player notes

/// Saves a private note about a player. Empty text and no color deletes it.
#[update]
fn set_note(player: Principal, text: String, color: Option<NoteColor>) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.set_note(p, player, text, color, now()))
}

/// All your notes, most recently updated first.
#[query]
fn get_notes() -> Vec<NoteView> {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.notes(p))
}

/// Current bad-beat jackpots, the rules and recent hits.
#[query]
fn get_jackpots() -> JackpotInfo {
    with(|s| s.jackpot_info())
}

#[query]
fn list_medals() -> Vec<MedalDef> {
    medals::medal_defs()
}

#[update]
fn leave_table(id: u64) -> R<LeaveResult> {
    let p = caller()?;
    with_mut(|s| s.leave_table(p, id, now()))
}

#[update]
fn top_up(id: u64, amount: u64) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.top_up(p, id, amount))
}

#[update]
fn sit_out(id: u64, out: bool) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.sit_out(p, id, out, now()))
}

/// Shows your cards after winning a hand uncontested, until the next hand is dealt.
#[update]
fn show_cards(id: u64) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.show_cards(p, id, now()))
}

#[update]
fn act(id: u64, action: Action, expected: Option<u64>) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.act_for(p, id, action, now(), expected))
}

// -------------------------------------------------------------- chat/social

#[update]
fn send_chat(channel: Channel, text: String) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.send_chat(p, channel, text, now()))
}

#[query]
fn get_chat(channel: Channel, since: u64) -> Vec<ChatMessage> {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.chat_for(p, channel, since))
}

#[update]
fn send_dm(to: Principal, text: String) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.send_dm(p, to, text, now()))
}

#[query]
fn get_dms(with_user: Principal, since: u64) -> Vec<ChatMessage> {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.dms_with(p, with_user, since))
}

#[update]
fn send_friend_request(to: Principal) -> R<()> {
    let p = active_caller()?;
    with_mut(|s| s.friend_request(p, to))
}

#[update]
fn respond_friend_request(from: Principal, accept: bool) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.respond_friend(p, from, accept))
}

#[update]
fn remove_friend(other: Principal) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.remove_friend(p, other))
}

#[query]
fn get_friends() -> Vec<FriendInfo> {
    let p = ic_cdk::api::msg_caller();
    with(|s| s.friends(p, now()))
}

#[query]
fn get_leaderboard() -> Vec<LeaderboardEntry> {
    with(|s| s.leaderboard(50))
}

// ------------------------------------------------------------------- admin

#[update]
fn admin_create_table(cfg: TableConfig) -> R<u64> {
    let p = admin()?;
    with_mut(|s| {
        let name = cfg.name.clone();
        let id = s.create_table(cfg)?;
        s.audit(p, format!("Created table #{id} ({name})"), now());
        Ok(id)
    })
}

/// Adds a recurring tournament (a sit-and-go or a scheduled event).
#[update]
fn admin_add_tournament(template: TournamentTemplate) -> R<u32> {
    let p = admin()?;
    with_mut(|s| {
        let name = template.name.clone();
        let id = s.add_template(template)?;
        s.audit(p, format!("Added tournament template {id} ({name})"), now());
        Ok(id)
    })
}

#[update]
fn admin_add_item(item: StoreItem) -> R<u32> {
    let p = admin()?;
    Ok(with_mut(|s| {
        let name = item.name.clone();
        let id = s.add_item(item);
        s.audit(p, format!("Added store item {id} ({name})"), now());
        id
    }))
}

#[update]
fn admin_set_item_available(id: u32, available: bool) -> R<()> {
    let p = admin()?;
    with_mut(|s| {
        let item = s.items.get_mut(&id).ok_or("No such item")?;
        item.available = available;
        s.audit(p, format!("Store item {id} available: {available}"), now());
        Ok(())
    })
}

/// Moves ICP from the house treasury into the bad-beat jackpot.
#[update]
fn admin_seed_jackpot(amount: u64) -> R<()> {
    let p = admin()?;
    with_mut(|s| {
        s.seed_jackpot(amount)?;
        s.audit(p, format!("Seeded the ICP jackpot with {amount} e8s"), now());
        Ok(())
    })
}

#[query]
fn admin_house_stats() -> R<HouseStats> {
    admin()?;
    Ok(with(|s| s.house_stats()))
}

/// Withdraws house revenue (rake and store sales) from the treasury.
#[update]
async fn admin_withdraw_treasury(to: Account, amount: u64) -> R<u64> {
    let p = admin()?;
    let op = with_mut(|s| {
        let op = s.begin_treasury_withdraw(p, to, amount, now())?;
        s.audit(p, format!("Treasury withdrawal of {amount} e8s to {} (op #{})", to.owner.to_text(), op.id), now());
        Ok::<_, String>(op)
    })?;
    let op = run_op(op).await?;
    op_reply(op, |block| block)
}

/// Emergency switches: pause deposits, withdrawals, dealing and/or new joins.
#[update]
fn admin_set_pause(flags: PauseFlags) -> R<()> {
    let p = admin()?;
    with_mut(|s| {
        s.audit(p, format!("Set pause flags: {flags:?}"), now());
        s.set_pause(flags)
    });
    Ok(())
}

/// Compares the canister's ICP balance with everything it owes. A shortfall
/// pauses withdrawals automatically. Also runs every 10 minutes on its own.
#[update]
async fn admin_check_solvency() -> R<SolvencyReport> {
    admin()?;
    check_solvency().await
}

/// Ledger operations whose outcome is unknown and need attention.
#[query]
fn admin_unresolved_ops() -> R<Vec<LedgerOp>> {
    admin()?;
    Ok(with(|s| s.unresolved_ops()))
}

/// Settles an unknown op after checking the ledger by hand (beyond the 24h retry window).
#[update]
fn admin_resolve_op(id: u64, executed: bool, block: Option<u64>) -> R<LedgerOp> {
    let p = admin()?;
    with_mut(|s| {
        let op = s.resolve_op(id, executed, block, now())?;
        s.audit(p, format!("Resolved ledger op #{id}: executed={executed}, block={block:?}"), now());
        Ok(op)
    })
}

/// Re-sends an unknown op right away (it's deduplicated by the ledger).
#[update]
async fn admin_retry_op(id: u64) -> R<LedgerOp> {
    let p = admin()?;
    with_mut(|s| s.audit(p, format!("Retried ledger op #{id}"), now()));
    let op = with(|s| s.stores.ledger_ops.get(&id)).ok_or("No such operation")?;
    if !op.is_unknown() {
        return Err("Only operations with an unknown outcome can be retried".into());
    }
    run_op(op).await
}

/// Recomputes the sum of ICP balances by scanning every user, to verify the running total.
#[query]
fn admin_audit_wallet_total() -> R<(u64, u64)> {
    admin()?;
    Ok(with(|s| (s.wallet_total, s.recount_wallet_total())))
}

// -------------------------------------------------------------- moderation

/// Blocks (or unblocks) a player: hides their chat and stops DMs, friend
/// requests, gifts and throws between you.
#[update]
fn block_player(other: Principal, block: bool) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.block(p, other, block))
}

/// Reports a player to the moderators. With a channel, their recent messages
/// there are attached as evidence. Returns the report id.
#[update]
fn report_player(target: Principal, reason: ReportReason, details: String, channel: Option<Channel>) -> R<u64> {
    let p = active_caller()?;
    with_mut(|s| s.report(p, target, reason, details, channel, now()))
}

/// What the signed-in caller may administer (drives the admin page).
#[derive(CandidType, Deserialize)]
pub struct Roles {
    pub admin: bool,
    pub moderator: bool,
}

#[query]
fn get_my_roles() -> Roles {
    let p = ic_cdk::api::msg_caller();
    let admin = p != Principal::anonymous() && ic_cdk::api::is_controller(&p);
    Roles { admin, moderator: admin || with(|s| s.is_moderator(&p)) }
}

#[query]
fn mod_get_player(p: Principal) -> R<ModPlayerView> {
    moderator()?;
    with(|s| s.mod_player_view(&p)).ok_or_else(|| "No such player".into())
}

/// The latest reports, open or resolved.
#[query]
fn mod_recent_reports(limit: u32) -> R<Vec<Report>> {
    moderator()?;
    Ok(with(|s| s.recent_reports(limit as usize)))
}

/// The open-report queue, oldest first: up to `limit` (max 50) after report `after`.
#[query]
fn mod_open_reports(after: Option<u64>, limit: u32) -> R<ReportPage> {
    moderator()?;
    Ok(with(|s| s.open_reports(after, limit as usize)))
}

#[update]
fn mod_resolve_report(id: u64, action: ModAction, note: String) -> R<()> {
    let target = with(|s| s.stores.reports.get(&id).map(|r| r.target)).ok_or("No such report")?;
    let p = moderator_for(target)?;
    with_mut(|s| s.resolve_report(p, id, action, note, now()))
}

/// Mutes a player in every chat for `hours` (0 lifts the mute).
#[update]
fn mod_mute(target: Principal, hours: u32) -> R<()> {
    let p = moderator_for(target)?;
    with_mut(|s| s.mute(p, target, hours, now()))
}

/// Suspends a player for `hours` (0 = permanent). They keep access to withdrawals.
#[update]
fn mod_ban(target: Principal, hours: u32, reason: String) -> R<()> {
    let p = moderator_for(target)?;
    with_mut(|s| s.ban(p, target, hours, &reason, now()))
}

#[update]
fn mod_unban(target: Principal) -> R<()> {
    let p = moderator_for(target)?;
    with_mut(|s| s.unban(p, target, now()))
}

#[update]
fn admin_set_moderator(p: Principal, enabled: bool) -> R<()> {
    let a = admin()?;
    with_mut(|s| {
        if enabled {
            s.moderators.insert(p);
        } else {
            s.moderators.remove(&p);
        }
        let who = s.who(&p);
        s.audit(a, format!("Moderator {who}: {enabled}"), now());
    });
    Ok(())
}

/// Settings an admin can change from the admin page.
#[derive(CandidType, Deserialize)]
pub struct AdminConfig {
    /// Moderators with their usernames (empty if they have no profile).
    pub moderators: Vec<(Principal, String)>,
    pub banned_words: Vec<String>,
    pub signups_per_hour: u32,
    pub signup_pow_bits: u8,
}

#[query]
fn admin_get_config() -> R<AdminConfig> {
    admin()?;
    Ok(with(|s| AdminConfig {
        moderators: s.moderators.iter().map(|p| (*p, s.users.get(p).map(|u| u.username).unwrap_or_default())).collect(),
        banned_words: s.banned_words.clone(),
        signups_per_hour: s.signups_per_hour,
        signup_pow_bits: s.signup_pow_bits(),
    }))
}

/// Replaces the chat/username word filter (whole words, case-insensitive).
#[update]
fn admin_set_banned_words(words: Vec<String>) -> R<()> {
    let a = admin()?;
    if words.len() > 1_000 {
        return Err("At most 1000 words".into());
    }
    if words.iter().any(|w| w.chars().count() > moderation::MAX_BANNED_WORD) {
        return Err(format!("Words are at most {} characters", moderation::MAX_BANNED_WORD));
    }
    with_mut(|s| {
        s.banned_words = words.into_iter().map(|w| w.trim().to_lowercase()).filter(|w| !w.is_empty()).collect();
        s.audit(a, format!("Set {} banned words", s.banned_words.len()), now());
    });
    Ok(())
}

/// Base sign-up proof-of-work, in leading zero bits (default 12, max 22).
#[update]
fn admin_set_signup_pow(bits: u8) -> R<()> {
    let a = admin()?;
    with_mut(|s| {
        s.set_signup_pow_bits(bits)?;
        s.audit(a, format!("Sign-up proof-of-work: {bits} bits"), now());
        Ok(())
    })
}

/// Hourly sign-up target: past it, sign-ups need more proof-of-work (0 closes sign-ups).
#[update]
fn admin_set_signup_limit(per_hour: u32) -> R<()> {
    let a = admin()?;
    with_mut(|s| {
        s.signups_per_hour = per_hour;
        s.audit(a, format!("Sign-up limit: {per_hour}/hour"), now());
    });
    Ok(())
}

/// Privileged actions (admins and moderators), newest first.
#[query]
fn admin_audit_log(limit: u32) -> R<Vec<AuditEntry>> {
    admin()?;
    Ok(with(|s| s.audit_log.iter().take(limit as usize).cloned().collect()))
}

// ---------------------------------------------------------------- metrics

#[query]
fn admin_get_metrics() -> R<metrics::Metrics> {
    admin()?;
    Ok(collect_metrics())
}

fn collect_metrics() -> metrics::Metrics {
    let now = now();
    let (active, limited) = LIMITER.with(|l| {
        let l = l.borrow();
        (l.active_since(now.saturating_sub(5 * 60 * 1_000_000_000)), l.rejected)
    });
    with(|s| {
        let mut m = metrics::Metrics::from_state(s, now);
        m.active_players_5m = active as u64;
        m.rate_limited_calls = limited;
        m.cycles = ic_cdk::api::canister_cycle_balance() as u64;
        m.heap_bytes = metrics::heap_bytes();
        m.stable_bytes = ic_cdk::stable::stable_size() * 65_536;
        m
    })
}

/// `GET /metrics` (Prometheus text, without the house's money figures) and
/// `GET /health` on the canister's raw domain. Responses are not certified, so
/// they're for monitoring only.
#[query]
fn http_request(req: metrics::HttpRequest) -> metrics::HttpResponse {
    let path = req.url.split('?').next().unwrap_or_default();
    match path {
        "/metrics" => metrics::HttpResponse::text(200, "text/plain; version=0.0.4", collect_metrics().to_prometheus(true)),
        "/health" => {
            let paused = with(|s| s.pause.withdrawals || s.pause.deposits || s.pause.dealing || s.pause.joins);
            metrics::HttpResponse::text(200, "text/plain", if paused { "ok (partially paused)".into() } else { "ok".into() })
        }
        _ => metrics::HttpResponse::text(404, "text/plain", "not found".into()),
    }
}

ic_cdk::export_candid!();

#[cfg(test)]
mod seed_request_tests {
    use super::*;

    #[test]
    fn a_table_has_one_seed_request_in_flight() {
        let a = SeedRequest::claim(7).expect("first request");
        assert!(SeedRequest::claim(7).is_none(), "second request while the first is pending");
        let b = SeedRequest::claim(8).expect("other tables are independent");
        drop(a);
        let again = SeedRequest::claim(7).expect("released when the request ends");
        drop((b, again));
        assert!(PENDING_SEEDS.with(|p| p.borrow().is_empty()));
    }
}

#[cfg(test)]
mod candid_tests {
    /// Keeps `poker_backend.did` in sync with the Rust code.
    /// Run `UPDATE_CANDID=1 cargo test` after changing the API.
    #[test]
    fn candid_interface_is_up_to_date() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/poker_backend.did");
        let generated = super::__export_service();
        if std::env::var("UPDATE_CANDID").is_ok() {
            std::fs::write(path, &generated).unwrap();
        }
        let on_disk = std::fs::read_to_string(path).unwrap_or_default();
        assert_eq!(on_disk, generated, "run `UPDATE_CANDID=1 cargo test` to regenerate the .did file");

        // Frontend bindings, as `dfx generate` would produce them.
        let (env, actor, prog) = candid_parser::check_file(std::path::Path::new(path)).unwrap();
        let out = concat!(env!("CARGO_MANIFEST_DIR"), "/../poker_frontend/src/declarations/poker_backend");
        let js = candid_parser::bindings::javascript::compile(&env, &actor);
        let ts = candid_parser::bindings::typescript::compile(&env, &actor, &prog);
        if std::env::var("UPDATE_CANDID").is_ok() {
            std::fs::create_dir_all(out).unwrap();
            std::fs::write(format!("{out}/poker_backend.did.js"), &js).unwrap();
            std::fs::write(format!("{out}/poker_backend.did.d.ts"), &ts).unwrap();
        }
        assert_eq!(std::fs::read_to_string(format!("{out}/poker_backend.did.js")).unwrap_or_default(), js);
        assert_eq!(std::fs::read_to_string(format!("{out}/poker_backend.did.d.ts")).unwrap_or_default(), ts);
    }
}
