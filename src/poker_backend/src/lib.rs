//! BulletsPoker backend canister: Texas Hold'em with ICP buy-ins, a cosmetic
//! item store, table gifts, chat and friends.

pub mod cards;
pub mod ledger;
pub mod medals;
pub mod state;
pub mod table;
pub mod tournament;

use candid::{CandidType, Deserialize, Principal};
use ic_cdk::{init, post_upgrade, pre_upgrade, query, update};
use icrc_ledger_types::icrc1::account::Account;
use sha2::{Digest, Sha256};
use state::*;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::time::Duration;
use medals::MedalDef;
use table::{Action, Currency, TableConfig};
use tournament::{TournamentDetail, TournamentSummary, TournamentTemplate};

/// Mainnet ICP ledger.
const ICP_LEDGER: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";
const ICP_FEE: u64 = 10_000;
/// Most tables one client can fetch in a single `get_tables` call.
const MAX_OPEN_TABLES: usize = 8;

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static DEALING: Cell<bool> = const { Cell::new(false) };
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
    STATE.with(|s| *s.borrow_mut() = Some(State::new(ledger, args.ledger_fee.unwrap_or(ICP_FEE))));
    start_timers();
}

#[pre_upgrade]
fn pre_upgrade() {
    STATE.with(|s| ic_cdk::storage::stable_save((s.borrow().as_ref().unwrap(),)).expect("save state"));
}

#[post_upgrade]
fn post_upgrade() {
    let (state,): (State,) = ic_cdk::storage::stable_restore().expect("restore state");
    STATE.with(|s| *s.borrow_mut() = Some(state));
    start_timers();
}

fn start_timers() {
    ic_cdk_timers::set_timer_interval(Duration::from_secs(1), || async { tick().await });
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

/// Deposit via ICRC-2: the caller first approves this canister for
/// `amount + fee` on the ICP ledger, then calls this.
#[update]
async fn deposit(amount: u64) -> R<u64> {
    let p = caller()?;
    with(|s| s.profile(&p)).ok_or("Create a profile first")?;
    let _lock = WalletLock::acquire(p)?;
    let (ledger, fee) = with(|s| (s.ledger, s.ledger_fee));
    let from = Account {
        owner: p,
        subaccount: None,
    };
    let to = Account {
        owner: ic_cdk::api::canister_self(),
        subaccount: None,
    };
    ledger::transfer_from(ledger, from, to, amount, fee).await?;
    with_mut(|s| s.credit(&p, amount))?;
    Ok(with(|s| s.profile(&p).map_or(0, |u| u.balance)))
}

/// Deposit via plain transfer: send ICP to your deposit account (see
/// `get_deposit_info`) from any wallet or exchange, then call this.
#[update]
async fn claim_deposit() -> R<u64> {
    let p = caller()?;
    with(|s| s.profile(&p)).ok_or("Create a profile first")?;
    let _lock = WalletLock::acquire(p)?;
    let (ledger, fee) = with(|s| (s.ledger, s.ledger_fee));
    let me = ic_cdk::api::canister_self();
    let sub = ledger::deposit_subaccount(&p);
    let bal = ledger::balance_of(ledger, Account { owner: me, subaccount: Some(sub) }).await?;
    if bal <= fee {
        return Err("No new deposit found".into());
    }
    let amount = bal - fee;
    let main = Account {
        owner: me,
        subaccount: None,
    };
    ledger::transfer(ledger, Some(sub), main, amount, fee, b"sweep").await?;
    with_mut(|s| s.credit(&p, amount))?;
    Ok(amount)
}

/// Sends `amount` e8s to `to`. The ledger fee is charged on top.
#[update]
async fn withdraw(to: Account, amount: u64) -> R<u64> {
    let p = caller()?;
    let _lock = WalletLock::acquire(p)?;
    let (ledger, fee) = with(|s| (s.ledger, s.ledger_fee));
    if amount == 0 {
        return Err("Enter an amount".into());
    }
    let total = amount.checked_add(fee).ok_or("Amount too large")?;
    // Debit first so the balance can't be spent twice while the transfer is in flight.
    with_mut(|s| s.debit(&p, total))?;
    match ledger::transfer(ledger, None, to, amount, fee, b"withdraw").await {
        Ok(block) => Ok(block),
        Err(e) => {
            with_mut(|s| s.credit(&p, total))?;
            Err(e)
        }
    }
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

/// Buys a fresh starting stack during a tournament's rebuy period.
#[update]
fn rebuy_tournament(id: u64) -> R<()> {
    let p = caller()?;
    with_mut(|s| s.rebuy(p, id, now()))
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

#[query]
fn admin_house_stats() -> R<HouseStats> {
    admin()?;
    Ok(with(|s| s.house_stats()))
}

/// Withdraws house revenue (rake and store sales) from the treasury.
#[update]
async fn admin_withdraw_treasury(to: Account, amount: u64) -> R<u64> {
    admin()?;
    let (ledger, fee) = with(|s| (s.ledger, s.ledger_fee));
    let total = amount.checked_add(fee).ok_or("Amount too large")?;
    with_mut(|s| {
        if s.treasury < total {
            return Err("Treasury too small".to_string());
        }
        s.treasury -= total;
        Ok(())
    })?;
    match ledger::transfer(ledger, None, to, amount, fee, b"treasury").await {
        Ok(block) => Ok(block),
        Err(e) => {
            with_mut(|s| s.treasury += total);
            Err(e)
        }
    }
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
