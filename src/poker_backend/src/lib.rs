//! BulletsPoker backend canister: Texas Hold'em with ICP buy-ins, a cosmetic
//! item store, table gifts, chat and friends.

pub mod cards;
pub mod jackpot;
pub mod ledger;
pub mod medals;
pub mod state;
pub mod store;
pub mod table;
pub mod tournament;
pub mod wallet;

use candid::{CandidType, Deserialize, Principal};
use ic_cdk::{init, post_upgrade, pre_upgrade, query, update};
use icrc_ledger_types::icrc1::account::Account;
use sha2::{Digest, Sha256};
use state::*;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::time::Duration;
use jackpot::JackpotInfo;
use medals::MedalDef;
use table::{Action, Currency, TableConfig};
use wallet::{LedgerOp, OpKind, OpStatus, PauseFlags, SolvencyReport};
use tournament::{TournamentDetail, TournamentSummary, TournamentTemplate};

/// Mainnet ICP ledger.
const ICP_LEDGER: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";
const ICP_FEE: u64 = 10_000;
/// Most tables one client can fetch in a single `get_tables` call.
const MAX_OPEN_TABLES: usize = 8;
const SOLVENCY_EVERY_SECS: u64 = 600;

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static DEALING: Cell<bool> = const { Cell::new(false) };
    static RETRYING: Cell<bool> = const { Cell::new(false) };
    /// Users with a wallet operation in flight.
    static WALLET_LOCKS: RefCell<BTreeSet<Principal>> = const { RefCell::new(BTreeSet::new()) };
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

/// The signed-in caller; anonymous callers may only use queries.
fn caller() -> R<Principal> {
    let p = ic_cdk::api::msg_caller();
    if p == Principal::anonymous() {
        return Err("Please sign in first".into());
    }
    with_mut(|s| s.touch(&p, now()));
    Ok(p)
}

fn admin() -> R<Principal> {
    let p = ic_cdk::api::msg_caller();
    if !ic_cdk::api::is_controller(&p) {
        return Err("Admins only".into());
    }
    Ok(p)
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
    STATE.with(|s| *s.borrow_mut() = Some(state));
    start_timers();
}

fn start_timers() {
    ic_cdk_timers::set_timer_interval(Duration::from_secs(1), || async { tick().await });
    ic_cdk_timers::set_timer_interval(Duration::from_secs(60), || async { retry_unknown_ops().await });
    ic_cdk_timers::set_timer_interval(Duration::from_secs(SOLVENCY_EVERY_SECS), || async {
        let _ = check_solvency().await;
    });
}

/// Re-sends ledger ops whose outcome is unknown. The ledger either executes
/// them now or reports them as duplicates of the original, so nothing runs twice.
async fn retry_unknown_ops() {
    if RETRYING.get() {
        return;
    }
    RETRYING.set(true);
    for op in with(|s| s.retryable_ops(now())) {
        let _ = run_op(op).await;
    }
    RETRYING.set(false);
}

async fn check_solvency() -> R<SolvencyReport> {
    let ledger = with(|s| s.ledger);
    // Snapshot liabilities before reading the balance: anything settling during
    // the call can only make the result look better, never trigger a false alarm.
    let liabilities = with(|s| s.house_stats().liabilities_total);
    let balance = ledger::balance_of(ledger, main_account()).await?;
    Ok(with_mut(|s| s.record_solvency(balance, liabilities, now())))
}

/// Runs every second: enforces action clocks and deals new hands.
async fn tick() {
    let ready = with_mut(|s| s.tick(now()));
    if ready.is_empty() || DEALING.get() {
        return;
    }
    DEALING.set(true);
    let rand = ledger::raw_rand().await;
    DEALING.set(false);
    let Ok(rand) = rand else { return };
    for id in ready {
        // Fresh, unpredictable deck per table and hand.
        let hand_no = with(|s| s.tables.get(&id).map_or(0, |t| t.hand_no));
        let mut h = Sha256::new();
        h.update(&rand);
        h.update(id.to_le_bytes());
        h.update(hand_no.to_le_bytes());
        let seed: [u8; 32] = h.finalize().into();
        with_mut(|s| s.start_hand(id, seed, now()));
    }
}

// ----------------------------------------------------------------- profile

#[query]
fn whoami() -> Principal {
    ic_cdk::api::msg_caller()
}

#[update]
fn register(username: String) -> R<Profile> {
    let p = caller()?;
    with_mut(|s| s.register(p, username, now()))
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
    let p = caller()?;
    with_mut(|s| s.set_status(p, status))
}

#[update]
fn ping() {
    let _ = caller();
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
    let p = caller()?;
    with_mut(|s| s.buy_item(p, id, currency))
}

#[update]
fn equip(id: u32) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.equip(p, id))
}

#[update]
fn send_gift(table_id: u64, to: Principal, item_id: u32, currency: Currency) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.send_gift(p, table_id, to, item_id, currency, now()))
}

#[update]
fn throw_item(table_id: u64, to: Principal, item_id: u32, currency: Currency) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.throw_item(p, table_id, to, item_id, currency, now()))
}

// ------------------------------------------------------- chips & settings

/// Claims the daily free play-chip bonus (bigger on consecutive days).
#[update]
fn claim_daily_chips() -> R<u64> {
    let p = caller()?;
    with_mut(|s| s.claim_daily(p, now()))
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

/// Creates your own table. Hosts of ICP tables earn 20% of the rake it generates.
#[update]
fn create_room(args: RoomArgs) -> R<u64> {
    let p = caller()?;
    with_mut(|s| s.create_room(p, args, now()))
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
fn leave_table(id: u64) -> R<u64> {
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
    with_mut(|s| s.sit_out(p, id, out))
}

#[update]
fn act(id: u64, action: Action) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.act(p, id, action, now()))
}

// -------------------------------------------------------------- chat/social

#[update]
fn send_chat(channel: Channel, text: String) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.send_chat(p, channel, text, now()))
}

#[query]
fn get_chat(channel: Channel, since: u64) -> Vec<ChatMessage> {
    with(|s| s.chat_since(channel, since))
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
    let p = caller()?;
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
    admin()?;
    with_mut(|s| s.create_table(cfg))
}

/// Adds a recurring tournament (a sit-and-go or a scheduled event).
#[update]
fn admin_add_tournament(template: TournamentTemplate) -> R<u32> {
    admin()?;
    with_mut(|s| s.add_template(template))
}

#[update]
fn admin_add_item(item: StoreItem) -> R<u32> {
    admin()?;
    Ok(with_mut(|s| s.add_item(item)))
}

#[update]
fn admin_set_item_available(id: u32, available: bool) -> R<()> {
    admin()?;
    with_mut(|s| {
        let item = s.items.get_mut(&id).ok_or("No such item")?;
        item.available = available;
        Ok(())
    })
}

/// Moves ICP from the house treasury into the bad-beat jackpot.
#[update]
fn admin_seed_jackpot(amount: u64) -> R<()> {
    admin()?;
    with_mut(|s| s.seed_jackpot(amount))
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
    let op = with_mut(|s| s.begin_treasury_withdraw(p, to, amount, now()))?;
    let op = run_op(op).await?;
    op_reply(op, |block| block)
}

/// Emergency switches: pause deposits, withdrawals, dealing and/or new joins.
#[update]
fn admin_set_pause(flags: PauseFlags) -> R<()> {
    admin()?;
    with_mut(|s| s.set_pause(flags));
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
    admin()?;
    with_mut(|s| s.resolve_op(id, executed, block, now()))
}

/// Re-sends an unknown op right away (it's deduplicated by the ledger).
#[update]
async fn admin_retry_op(id: u64) -> R<LedgerOp> {
    admin()?;
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

ic_cdk::export_candid!();

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
