//! End-to-end tests: the poker canister against the real ICP ledger canister
//! on a PocketIC replica. Run with `./scripts/integration-test.sh`, which
//! downloads PocketIC and the ledger wasm and sets the env vars below.
//! Without them (a plain `cargo test`), these tests are skipped.

use candid::{decode_one, encode_args, encode_one, CandidType, Deserialize, Nat, Principal};
use icrc_ledger_types::icrc1::account::Account;
use pocket_ic::{PocketIc, PocketIcBuilder};
use poker_backend::state::{HouseStats, LeaveResult, Profile, StoreItem, TableView};
use poker_backend::table::{Action, Currency};
use poker_backend::wallet::{LedgerOp, OpStatus, PauseFlags, SolvencyReport};
use std::time::Duration;

const E8S: u64 = 100_000_000;
const FEE: u64 = 10_000;

// ------------------------------------------------------------ ledger init

#[derive(CandidType)]
struct Tokens {
    e8s: u64,
}

#[derive(CandidType)]
struct FeatureFlags {
    icrc2: bool,
}

#[derive(CandidType)]
struct LedgerInit {
    minting_account: String,
    icrc1_minting_account: Option<Account>,
    initial_values: Vec<(String, Tokens)>,
    max_message_size_bytes: Option<u64>,
    transaction_window: Option<u64>,
    archive_options: Option<()>,
    send_whitelist: Vec<Principal>,
    transfer_fee: Option<Tokens>,
    token_symbol: Option<String>,
    token_name: Option<String>,
    feature_flags: Option<FeatureFlags>,
}

#[derive(CandidType)]
enum LedgerArg {
    Init(LedgerInit),
}

#[derive(CandidType)]
struct InitArgs {
    ledger: Option<Principal>,
    ledger_fee: Option<u64>,
}

#[derive(CandidType)]
struct ApproveArgs {
    from_subaccount: Option<[u8; 32]>,
    spender: Account,
    amount: Nat,
    expected_allowance: Option<Nat>,
    expires_at: Option<u64>,
    fee: Option<Nat>,
    memo: Option<Vec<u8>>,
    created_at_time: Option<u64>,
}

#[derive(CandidType)]
struct TransferArgs {
    from_subaccount: Option<[u8; 32]>,
    to: Account,
    amount: Nat,
    fee: Option<Nat>,
    memo: Option<Vec<u8>>,
    created_at_time: Option<u64>,
}

#[derive(CandidType, Deserialize, Debug)]
struct DepositInfo {
    account: Account,
    account_id: String,
    ledger: Principal,
    fee: u64,
}

type R<T> = Result<T, String>;

fn principal(n: u8) -> Principal {
    Principal::self_authenticating([n; 32])
}

fn account(p: Principal) -> Account {
    Account { owner: p, subaccount: None }
}

struct Env {
    pic: PocketIc,
    ledger: Principal,
    poker: Principal,
    admin: Principal,
    wasm: Vec<u8>,
}

impl Env {
    /// None when the integration-test binaries aren't available.
    fn new() -> Option<Env> {
        Env::starting_from(None)
    }

    /// Like `new`, but installs the module at `first` (another release) instead
    /// of BACKEND_WASM; `wasm` is still BACKEND_WASM, ready to upgrade to.
    fn starting_from(first: Option<String>) -> Option<Env> {
        let (Ok(ledger_wasm), Ok(backend_wasm)) = (std::env::var("LEDGER_WASM"), std::env::var("BACKEND_WASM")) else {
            eprintln!("skipping: run ./scripts/integration-test.sh");
            return None;
        };
        let pic = PocketIcBuilder::new().with_application_subnet().build();
        let admin = principal(200);
        let minter = principal(201);

        let ledger = pic.create_canister_with_settings(Some(admin), None);
        pic.add_cycles(ledger, 10_000_000_000_000);
        let id = |p: Principal| poker_backend::ledger::account_identifier(&p, &[0; 32]);
        let init = LedgerArg::Init(LedgerInit {
            minting_account: id(minter),
            icrc1_minting_account: None,
            initial_values: (1..=3).map(|k| (id(principal(k)), Tokens { e8s: 100 * E8S })).collect(),
            max_message_size_bytes: None,
            transaction_window: None,
            archive_options: None,
            send_whitelist: vec![],
            transfer_fee: Some(Tokens { e8s: FEE }),
            token_symbol: Some("ICP".into()),
            token_name: Some("Internet Computer".into()),
            feature_flags: Some(FeatureFlags { icrc2: true }),
        });
        pic.install_canister(ledger, std::fs::read(ledger_wasm).unwrap(), encode_one(init).unwrap(), Some(admin));

        let poker = pic.create_canister_with_settings(Some(admin), None);
        pic.add_cycles(poker, 10_000_000_000_000);
        let wasm = std::fs::read(backend_wasm).unwrap();
        let installed = first.map_or_else(|| wasm.clone(), |path| std::fs::read(path).unwrap());
        let args = Some(InitArgs { ledger: Some(ledger), ledger_fee: Some(FEE) });
        pic.install_canister(poker, installed, encode_one(args).unwrap(), Some(admin));
        Some(Env { pic, ledger, poker, admin, wasm })
    }

    fn update<T: CandidType + for<'de> Deserialize<'de>>(&self, who: Principal, method: &str, args: Vec<u8>) -> T {
        let bytes = self.pic.update_call(self.poker, who, method, args).unwrap_or_else(|e| panic!("{method}: {e:?}"));
        decode_one(&bytes).unwrap_or_else(|e| panic!("{method}: {e}"))
    }

    fn query<T: CandidType + for<'de> Deserialize<'de>>(&self, who: Principal, method: &str, args: Vec<u8>) -> T {
        let bytes = self.pic.query_call(self.poker, who, method, args).unwrap_or_else(|e| panic!("{method}: {e:?}"));
        decode_one(&bytes).unwrap_or_else(|e| panic!("{method}: {e}"))
    }

    fn ledger_balance(&self, acct: Account) -> u64 {
        let bytes = self.pic.query_call(self.ledger, Principal::anonymous(), "icrc1_balance_of", encode_one(acct).unwrap()).unwrap();
        let n: Nat = decode_one(&bytes).unwrap();
        n.0.try_into().unwrap()
    }

    fn ledger_update<T: CandidType + for<'de> Deserialize<'de>>(&self, who: Principal, method: &str, arg: impl CandidType) -> T {
        let bytes = self.pic.update_call(self.ledger, who, method, encode_one(arg).unwrap()).unwrap();
        decode_one(&bytes).unwrap()
    }

    fn register(&self, who: Principal, name: &str) {
        let challenge: poker_backend::signup::SignupChallenge = self.query(who, "get_signup_challenge", encode_args(()).unwrap());
        let nonce = poker_backend::signup::solve(&who, challenge.difficulty);
        // Without the proof-of-work, sign-up is refused. The profile isn't read, so this
        // also works against an older release whose profile has fewer fields.
        let r: R<candid::Reserved> = self.update(who, "register", encode_args((name.to_string(), None::<u64>)).unwrap());
        assert!(r.is_err() || challenge.difficulty == 0);
        let r: R<candid::Reserved> = self.update(who, "register", encode_args((name.to_string(), Some(nonce))).unwrap());
        r.unwrap();
    }

    fn balance(&self, who: Principal) -> u64 {
        let p: Option<Profile> = self.query(who, "get_my_profile", encode_args(()).unwrap());
        p.unwrap().balance
    }

    fn approve_and_deposit(&self, who: Principal, amount: u64) -> R<u64> {
        let approve = ApproveArgs {
            from_subaccount: None,
            spender: account(self.poker),
            amount: Nat::from(amount + FEE),
            expected_allowance: None,
            expires_at: None,
            fee: None,
            memo: None,
            created_at_time: None,
        };
        let r: Result<Nat, candid::Reserved> = self.ledger_update(who, "icrc2_approve", approve);
        r.map_err(|_| "approve failed".to_string())?;
        self.update(who, "deposit", encode_one(amount).unwrap())
    }

    fn solvency(&self) -> SolvencyReport {
        let r: R<SolvencyReport> = self.update(self.admin, "admin_check_solvency", encode_args(()).unwrap());
        r.unwrap()
    }

    /// Steps the clock a second at a time, letting due timers run.
    fn run_timers(&self, secs: u64) {
        for _ in 0..secs {
            self.pic.advance_time(Duration::from_secs(1));
            self.pic.tick();
            self.pic.tick();
        }
    }

    fn table(&self, who: Principal, id: u64) -> TableView {
        let v: Option<TableView> = self.query(who, "get_table", encode_one(id).unwrap());
        v.unwrap()
    }
}

/// The IC's module hash is the SHA-256 of the exact bytes installed (for the
/// reproducible build, the .wasm.gz), which is what releases publish.
#[test]
fn module_hash_is_the_sha256_of_the_installed_bytes() {
    use sha2::{Digest, Sha256};
    let Some(env) = Env::new() else { return };
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let want = hex(&Sha256::digest(&env.wasm));
    let status = env.pic.canister_status(env.poker, Some(env.admin)).unwrap();
    let got = hex(&status.module_hash.expect("installed canister has a module hash"));
    eprintln!("installed module sha256 {want}, module_hash {got}");
    assert_eq!(got, want);
    // integration-test.sh sets this from the build's .sha256 file, so the
    // module under test is the one that was published.
    if let Ok(published) = std::env::var("BACKEND_WASM_SHA256") {
        assert_eq!(got, published.trim());
    }
}

#[test]
fn deposits_withdrawals_and_solvency() {
    let Some(env) = Env::new() else { return };
    let alice = principal(1);
    env.register(alice, "alice");

    // ICRC-2 deposit: approve, then the canister pulls it.
    assert_eq!(env.approve_and_deposit(alice, 5 * E8S).unwrap(), 5 * E8S);
    assert_eq!(env.ledger_balance(account(alice)), 100 * E8S - 5 * E8S - 2 * FEE);
    assert_eq!(env.ledger_balance(account(env.poker)), 5 * E8S);

    // Plain-transfer deposit to the personal subaccount, then claim it.
    let info: DepositInfo = env.query(alice, "get_deposit_info", encode_args(()).unwrap());
    let t = TransferArgs { from_subaccount: None, to: info.account, amount: Nat::from(E8S), fee: None, memo: None, created_at_time: None };
    let r: Result<Nat, candid::Reserved> = env.ledger_update(alice, "icrc1_transfer", t);
    assert!(r.is_ok());
    let claimed: R<u64> = env.update(alice, "claim_deposit", encode_args(()).unwrap());
    assert_eq!(claimed.unwrap(), E8S - FEE);
    assert_eq!(env.balance(alice), 6 * E8S - FEE);
    let again: R<u64> = env.update(alice, "claim_deposit", encode_args(()).unwrap());
    assert!(again.is_err(), "nothing left to claim");

    // Withdraw back to the wallet.
    let before = env.ledger_balance(account(alice));
    let r: R<u64> = env.update(alice, "withdraw", encode_args((account(alice), 2 * E8S)).unwrap());
    r.unwrap();
    assert_eq!(env.ledger_balance(account(alice)), before + 2 * E8S);
    assert_eq!(env.balance(alice), 4 * E8S - 2 * FEE);

    // Over-withdrawing fails cleanly with nothing debited.
    let r: R<u64> = env.update(alice, "withdraw", encode_args((account(alice), 50 * E8S)).unwrap());
    assert!(r.is_err());
    assert_eq!(env.balance(alice), 4 * E8S - 2 * FEE);

    // Every op is recorded and settled.
    let ops: Vec<LedgerOp> = env.query(alice, "get_my_ledger_ops", encode_args(()).unwrap());
    assert_eq!(ops.len(), 3);
    assert!(ops.iter().all(|o| matches!(o.status, OpStatus::Completed { .. })));

    // The canister holds exactly what it owes.
    let report = env.solvency();
    assert!(report.ok);
    assert_eq!(report.surplus, 0);
    assert_eq!(report.ledger_balance, env.ledger_balance(account(env.poker)));
    let stats: R<HouseStats> = env.query(env.admin, "admin_house_stats", encode_args(()).unwrap());
    assert_eq!(stats.unwrap().liabilities_total, report.liabilities);
}

#[test]
fn pause_switches_block_money_movement() {
    let Some(env) = Env::new() else { return };
    let alice = principal(1);
    env.register(alice, "alice");
    env.approve_and_deposit(alice, 3 * E8S).unwrap();

    // Refused by inspect_message before it ever runs.
    assert!(env.pic.update_call(env.poker, alice, "admin_set_pause", encode_one(PauseFlags::default()).unwrap()).is_err());

    let flags = PauseFlags { withdrawals: true, deposits: true, reason: Some("maintenance".into()), ..Default::default() };
    let r: R<()> = env.update(env.admin, "admin_set_pause", encode_one(flags).unwrap());
    r.unwrap();
    let w: R<u64> = env.update(alice, "withdraw", encode_args((account(alice), E8S)).unwrap());
    assert!(w.unwrap_err().contains("maintenance"));
    assert!(env.approve_and_deposit(alice, E8S).unwrap_err().contains("maintenance"));
    assert_eq!(env.balance(alice), 3 * E8S);

    let r: R<()> = env.update(env.admin, "admin_set_pause", encode_one(PauseFlags::default()).unwrap());
    r.unwrap();
    let w: R<u64> = env.update(alice, "withdraw", encode_args((account(alice), E8S)).unwrap());
    w.unwrap();
}

#[test]
fn icp_hands_and_an_upgrade_mid_game() {
    let Some(env) = Env::new() else { return };
    let (alice, bob) = (principal(1), principal(2));
    env.register(alice, "alice");
    env.register(bob, "bob");
    env.approve_and_deposit(alice, 2 * E8S).unwrap();
    env.approve_and_deposit(bob, 2 * E8S).unwrap();

    // Penny Lane (ICP, 0.01/0.02): the timer deals once two players sit.
    let table = 5u64;
    for (k, who) in [alice, bob].into_iter().enumerate() {
        let r: R<()> = env.update(who, "join_table", encode_args((table, k as u8, 40_000_000u64, None::<String>)).unwrap());
        r.unwrap();
    }
    env.run_timers(3);
    let view = env.table(alice, table);
    assert!(view.hand_no >= 1, "a hand was dealt using on-chain randomness");
    assert_eq!(view.seats[view.my_seat.unwrap() as usize].as_ref().unwrap().cards.len(), 2);
    let other = env.table(bob, table);
    let alice_seat = view.my_seat.unwrap() as usize;
    assert!(other.seats[alice_seat].as_ref().unwrap().cards.is_empty(), "opponents can't see your cards");

    // Upgrade the canister in the middle of the hand.
    let hand = view.hand_no;
    env.pic.upgrade_canister(env.poker, env.wasm.clone(), encode_args(()).unwrap(), Some(env.admin)).unwrap();
    let after = env.table(alice, table);
    assert_eq!(after.hand_no, hand);
    assert_eq!(after.seats[alice_seat].as_ref().unwrap().cards, view.seats[alice_seat].as_ref().unwrap().cards);
    assert_eq!(env.balance(alice), 2 * E8S - 40_000_000);

    // Play the hand out: whoever is to act folds.
    let to_act = after.to_act.expect("someone to act");
    let actor = after.seats[to_act as usize].as_ref().unwrap().player;
    let r: R<()> = env.update(actor, "act", encode_args((table, Action::Fold)).unwrap());
    r.unwrap();
    // Timers survived the upgrade: the next hand gets dealt.
    env.run_timers(8);
    assert!(env.table(alice, table).hand_no > hand);

    // Cash out and check the books still balance.
    for who in [alice, bob] {
        let r: R<LeaveResult> = env.update(who, "leave_table", encode_one(table).unwrap());
        r.unwrap();
    }
    let report = env.solvency();
    assert!(report.ok, "{report:?}");
    assert_eq!(report.surplus, 0);
}

/// Upgrading from the previous release (PREVIOUS_WASM, e.g. v0.1.7's
/// poker_backend.wasm.gz), which ticked every second whatever was going on: a
/// hand in progress keeps its clock with nobody calling, what players bought
/// and hold is still theirs, and the game then settles to the slow tick.
#[test]
fn upgrading_from_the_previous_release_keeps_a_live_game_going() {
    let Ok(previous) = std::env::var("PREVIOUS_WASM") else {
        eprintln!("skipping: set PREVIOUS_WASM to the previous release's module");
        return;
    };
    let Some(env) = Env::starting_from(Some(previous)) else { return };
    let (alice, _, table) = env.seat_two();
    let store: Vec<StoreItem> = env.query(alice, "list_store", encode_args(()).unwrap());
    let cowboy = store.iter().find(|i| i.name == "Cowboy").unwrap().id;
    let r: R<()> = env.update(alice, "buy_item", encode_args((cowboy, Currency::Icp)).unwrap());
    r.unwrap();
    env.run_timers(3);
    let dealt = env.table(alice, table);
    assert_eq!(dealt.hand_no, 1);
    let seat = dealt.my_seat.unwrap() as usize;
    let balance = env.balance(alice);

    env.pic.upgrade_canister(env.poker, env.wasm.clone(), encode_args(()).unwrap(), Some(env.admin)).unwrap();
    env.run_timers(1);
    assert!(env.tick_fast(), "a hand is in progress");
    let after = env.table(alice, table);
    assert_eq!(after.hand_no, 1);
    assert_eq!(after.seats[seat].as_ref().unwrap().cards, dealt.seats[seat].as_ref().unwrap().cards);
    assert_eq!(env.balance(alice), balance);
    let profile: Option<Profile> = env.query(alice, "get_my_profile", encode_args(()).unwrap());
    assert!(profile.unwrap().inventory.contains(&cowboy));

    env.wait_out_hand(alice, table);
    env.run_timers(2);
    assert!(!env.tick_fast(), "one player left in: nothing to run a clock for");
    let report = env.solvency();
    assert!(report.ok, "{report:?}");
}

#[test]
fn ingress_filter_moderation_and_metrics() {
    let Some(env) = Env::new() else { return };
    let (alice, bob, stranger) = (principal(1), principal(2), principal(9));
    env.register(alice, "alice");
    env.register(bob, "bob");

    // Unregistered and anonymous callers can only register.
    assert!(env.pic.update_call(env.poker, stranger, "claim_daily_chips", encode_args(()).unwrap()).is_err());
    assert!(env.pic.update_call(env.poker, Principal::anonymous(), "register", encode_one("anon".to_string()).unwrap()).is_err());
    // Oversized arguments are dropped.
    assert!(env.pic.update_call(env.poker, alice, "set_status", encode_one("x".repeat(20_000)).unwrap()).is_err());
    // Moderator methods need the role.
    assert!(env.pic.update_call(env.poker, alice, "mod_mute", encode_args((bob, 1u32)).unwrap()).is_err());
    let r: R<()> = env.update(env.admin, "admin_set_moderator", encode_args((alice, true)).unwrap());
    r.unwrap();
    let r: R<()> = env.update(alice, "mod_mute", encode_args((bob, 1u32)).unwrap());
    r.unwrap();
    let roles: poker_backend::Roles = env.query(alice, "get_my_roles", encode_args(()).unwrap());
    assert!(roles.moderator && !roles.admin);
    let roles: poker_backend::Roles = env.query(env.admin, "get_my_roles", encode_args(()).unwrap());
    assert!(roles.admin && roles.moderator);
    let view: R<poker_backend::moderation::ModPlayerView> = env.query(alice, "mod_get_player", encode_one(bob).unwrap());
    assert!(view.unwrap().chat_muted_until > 0);
    let denied: R<poker_backend::moderation::ModPlayerView> = env.query(bob, "mod_get_player", encode_one(alice).unwrap());
    assert!(denied.is_err(), "queries check the role too");
    // Moderators can't act on themselves or other staff.
    let r: R<()> = env.update(env.admin, "admin_set_moderator", encode_args((bob, true)).unwrap());
    r.unwrap();
    let r: R<()> = env.update(alice, "mod_ban", encode_args((bob, 0u32, "rival".to_string())).unwrap());
    assert!(r.unwrap_err().contains("Only admins"));
    let r: R<()> = env.update(alice, "mod_unban", encode_one(alice).unwrap());
    assert!(r.unwrap_err().contains("yourself"));
    let r: R<()> = env.update(env.admin, "admin_set_moderator", encode_args((bob, false)).unwrap());
    r.unwrap();
    // A suspended player can't use the social or store endpoints either.
    let r: R<()> = env.update(env.admin, "mod_ban", encode_args((bob, 24u32, "spam".to_string())).unwrap());
    r.unwrap();
    let r: R<u64> = env.update(bob, "claim_daily_chips", encode_args(()).unwrap());
    assert!(r.unwrap_err().contains("suspended"));
    let r: R<()> = env.update(bob, "send_friend_request", encode_one(alice).unwrap());
    assert!(r.unwrap_err().contains("suspended"));
    let r: R<()> = env.update(env.admin, "mod_unban", encode_one(bob).unwrap());
    r.unwrap();
    let r: R<()> = env.update(bob, "send_chat", encode_args((poker_backend::state::Channel::Lobby, "hi".to_string())).unwrap());
    assert!(r.unwrap_err().contains("muted"));

    // Request budget: a burst of 20, then refused until it refills.
    let mut refused = 0;
    for _ in 0..30 {
        if env.pic.update_call(env.poker, alice, "set_status", encode_one("spam".to_string()).unwrap()).is_err() {
            refused += 1;
        }
    }
    assert!(refused >= 5, "refused {refused}");
    env.pic.advance_time(Duration::from_secs(5));
    env.pic.tick(); // inspect_message sees the time of the last executed round
    let r: R<()> = env.update(alice, "set_status", encode_one("back".to_string()).unwrap());
    r.unwrap();

    // Metrics over HTTP.
    use poker_backend::metrics::{HttpRequest, HttpResponse};
    let get = |url: &str| -> HttpResponse {
        let req = HttpRequest { method: "GET".into(), url: url.into(), headers: vec![], body: Default::default() };
        env.query(Principal::anonymous(), "http_request", encode_one(req).unwrap())
    };
    let m = get("/metrics");
    assert_eq!(m.status_code, 200);
    let text = String::from_utf8(m.body.into_vec()).unwrap();
    assert!(text.contains("bp_users 2\n"), "{text}");
    assert!(text.contains("bp_cycles "));
    assert!(!text.contains("treasury") && !text.contains("liabilities"), "no money figures in public metrics");
    assert!(!text.contains("bp_heap_bytes 0\n"));
    assert_eq!(get("/health").status_code, 200);
    assert_eq!(get("/nope").status_code, 404);
    let log: R<Vec<poker_backend::moderation::AuditEntry>> = env.query(env.admin, "admin_audit_log", encode_one(10u32).unwrap());
    assert_eq!(log.unwrap().len(), 6, "moderator changes, the mute, the suspension and its lifting");
}

// ------------------------------------------------------------ provably fair

/// True when `needle` occurs anywhere in `hay`.
fn contains_bytes(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Raw candid reply bytes of the public table views, as seen by `who`.
fn raw_views(env: &Env, who: Principal, table: u64) -> Vec<Vec<u8>> {
    let q = |m: &str, a: Vec<u8>| env.pic.query_call(env.poker, who, m, a).unwrap();
    vec![
        q("get_table", encode_one(table).unwrap()),
        q("get_tables", encode_one(vec![table]).unwrap()),
        q("list_tables", encode_args(()).unwrap()),
    ]
}

fn hand_record(env: &Env, who: Principal, id: u64) -> Option<poker_backend::state::HandRecord> {
    env.query(who, "get_hand", encode_one(id).unwrap())
}

#[test]
fn fair_hand_is_verifiable_and_the_seed_stays_hidden_until_reveal() {
    use poker_backend::fair::{shuffle_deck, verify_commitment, HandId};
    use poker_backend::state::HandSummary;
    use poker_backend::table::Phase;

    let Some(env) = Env::new() else { return };
    let (alice, bob, carol) = (principal(1), principal(2), principal(3));
    for (p, n) in [(alice, "alice"), (bob, "bob"), (carol, "carol")] {
        env.register(p, n);
    }
    env.approve_and_deposit(alice, 2 * E8S).unwrap();
    env.approve_and_deposit(bob, 2 * E8S).unwrap();

    let table = 5u64;
    for (k, who) in [alice, bob].into_iter().enumerate() {
        let r: R<()> = env.update(who, "join_table", encode_args((table, k as u8, 40_000_000u64, None::<String>)).unwrap());
        r.unwrap();
    }
    env.run_timers(3);
    let first = env.table(alice, table);
    assert_eq!(first.hand_no, 1, "the first hand is dealt");
    // The live hand's commitment is in the table view, the same for a watcher.
    let shown = first.commitment.clone().expect("the commitment is shown while the hand is played");
    assert_eq!(env.table(carol, table).commitment.as_ref(), Some(&shown));

    // Check the hand down to a showdown, capturing every public view on the way.
    let mut captured: Vec<Vec<u8>> = vec![];
    for step in 0..40 {
        for who in [alice, bob, carol] {
            captured.extend(raw_views(&env, who, table));
        }
        let v = env.table(alice, table);
        if v.hand_no != 1 || matches!(v.phase, Phase::HandOver | Phase::Waiting) {
            break;
        }
        let seat = v.to_act.expect("someone to act") as usize;
        let s = v.seats[seat].as_ref().unwrap();
        let action = if s.bet < v.current_bet { Action::Call } else { Action::Check };
        let r: R<()> = env.update(s.player, "act", encode_args((table, action)).unwrap());
        r.unwrap();
        assert!(step < 39, "hand did not finish");
    }

    let history: Vec<HandSummary> = env.query(alice, "get_hand_history", encode_one(5u32).unwrap());
    let id = history.first().expect("the hand is in the history").id;

    // Before the reveal: participants get the evidence but not the seed; others get nothing.
    let rec = hand_record(&env, alice, id).expect("participant sees the hand");
    let fair = rec.fair.clone().expect("fair evidence");
    assert!(fair.revealed_seed.is_none(), "seed must be absent before reveal_at");
    assert_eq!(fair.commitment.len(), 32);
    assert_eq!(fair.outcome, "settled");
    assert_eq!(fair.canister, env.poker);
    assert!(fair.reveal_at >= fair.settled_at + 24 * 3600 * 1_000_000_000);
    assert!(hand_record(&env, bob, id).unwrap().fair.unwrap().revealed_seed.is_none());
    assert!(hand_record(&env, carol, id).is_none(), "non-participants get no record");
    assert_eq!(rec.board.len(), 5, "checked down to the river");
    assert_eq!(fair.burns, 3);
    assert_eq!(fair.commitment, shown, "the record publishes the commitment shown at the table");
    // The hand is listed as verifiable for its players only, pending until reveal_at.
    let listed: Vec<HandSummary> = env.query(alice, "get_verifiable_hands", encode_args((0u32, 500u32)).unwrap());
    assert_eq!(listed.iter().map(|h| h.id).collect::<Vec<_>>(), vec![id]);
    assert_eq!(listed[0].reveal_at, Some(fair.reveal_at));
    let none: Vec<HandSummary> = env.query(carol, "get_verifiable_hands", encode_args((0u32, 500u32)).unwrap());
    assert!(none.is_empty());

    // 24 hours and a second later the seed is revealed to participants only.
    env.pic.advance_time(Duration::from_secs(24 * 3600 + 1));
    env.pic.tick();
    env.pic.tick();
    assert!(hand_record(&env, carol, id).is_none());
    let revealed = hand_record(&env, alice, id).unwrap().fair.unwrap();
    let seed: [u8; 32] = revealed.revealed_seed.clone().expect("seed revealed after reveal_at").try_into().expect("32 bytes");
    assert_eq!(hand_record(&env, bob, id).unwrap().fair.unwrap().revealed_seed.as_deref(), Some(&seed[..]));

    // Rebuild the deck from the evidence alone.
    let hand = HandId { canister: revealed.canister, table_id: revealed.table_id, hand_no: revealed.hand_no };
    let commitment: [u8; 32] = revealed.commitment.clone().try_into().unwrap();
    assert!(verify_commitment(&seed, &hand, revealed.version, &commitment));
    let mut wrong = seed;
    wrong[0] ^= 1;
    assert!(!verify_commitment(&wrong, &hand, revealed.version, &commitment));
    let mut deck = shuffle_deck(&seed, &hand, revealed.version).unwrap();

    let mut hole: std::collections::BTreeMap<u8, Vec<u8>> = Default::default();
    for &seat in &revealed.dealt_order {
        hole.entry(seat).or_default().push(deck.pop().unwrap());
    }
    assert_eq!(revealed.dealt_order.len(), 4);
    let record = hand_record(&env, alice, id).unwrap();
    let mut matched = 0;
    for p in &record.players {
        if !p.cards.is_empty() {
            assert_eq!(hole[&p.seat], p.cards, "hole cards of seat {}", p.seat);
            matched += 1;
        }
    }
    assert!(matched >= 1, "at least the viewer's own cards were checked");
    // Board: a burn before the flop, the turn and the river.
    let mut board = vec![];
    for n in [3, 1, 1] {
        deck.pop();
        for _ in 0..n {
            board.push(deck.pop().unwrap());
        }
    }
    assert_eq!(revealed.burns, 3);
    assert_eq!(record.board, board);

    // While the hand was live, no public view carried the seed (raw or hex).
    assert!(captured.len() >= 9);
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    for (i, bytes) in captured.iter().enumerate() {
        assert!(!contains_bytes(bytes, &seed), "view {i} leaked the seed");
        assert!(!contains_bytes(bytes, hex.as_bytes()), "view {i} leaked the seed in hex");
    }
    // Sanity: the check is able to see the seed when it is there.
    let bytes = env.pic.query_call(env.poker, alice, "get_hand", encode_one(id).unwrap()).unwrap();
    assert!(contains_bytes(&bytes, &seed));
}

/// Cycles per hand, measured and printed (never asserted: PocketIC pricing
/// is not mainnet pricing).
#[test]
fn cycle_cost_per_hand() {
    let Some(env) = Env::new() else { return };
    const N: u64 = 25;
    let (alice, bob) = (principal(1), principal(2));
    env.register(alice, "alice");
    env.register(bob, "bob");
    env.approve_and_deposit(alice, 2 * E8S).unwrap();
    env.approve_and_deposit(bob, 2 * E8S).unwrap();
    let table = 5u64;
    for (k, who) in [alice, bob].into_iter().enumerate() {
        let r: R<()> = env.update(who, "join_table", encode_args((table, k as u8, 40_000_000u64, None::<String>)).unwrap());
        r.unwrap();
    }

    let t0 = env.pic.get_time().as_nanos_since_unix_epoch();
    let c0 = env.pic.cycle_balance(env.poker);
    let mut steps = 0u64;
    let mut dealt = 0u64;
    while dealt < N {
        env.run_timers(1);
        steps += 1;
        assert!(steps < 100 * N, "hands are not being dealt");
        let v = env.table(alice, table);
        if v.hand_no > dealt {
            if let Some(seat) = v.to_act {
                dealt = v.hand_no;
                let who = v.seats[seat as usize].as_ref().unwrap().player;
                let r: R<()> = env.update(who, "act", encode_args((table, Action::Fold)).unwrap());
                r.unwrap();
            }
        }
    }
    let c1 = env.pic.cycle_balance(env.poker);
    let elapsed = env.pic.get_time().as_nanos_since_unix_epoch() - t0;
    let played = c0 - c1;

    // Idle: the same number of one-second steps, nobody seated.
    for who in [alice, bob] {
        let r: R<LeaveResult> = env.update(who, "leave_table", encode_one(table).unwrap());
        r.unwrap();
    }
    env.run_timers(2);
    let i0 = env.pic.cycle_balance(env.poker);
    env.run_timers(steps);
    let i1 = env.pic.cycle_balance(env.poker);
    let idle = i0 - i1;

    eprintln!("CYCLES hands={N} steps={steps} simulated_ns={elapsed}");
    eprintln!("CYCLES play_total={played} idle_same_steps={idle}");
    eprintln!("CYCLES per_hand_net={} per_hand_gross={}", played.saturating_sub(idle) / N as u128, played / N as u128);
}

// -------------------------------------------------------------- adaptive tick

/// Longest a hand at Penny Lane can last with nobody acting: the 25-second
/// clock, the 30-second time bank, the 5-second results pause and some slack.
const UNATTENDED_HAND_SECS: u64 = 25 + 30 + 5 + 5;

impl Env {
    /// Whether the game tick is running every second, from the public metrics.
    fn tick_fast(&self) -> bool {
        use poker_backend::metrics::{HttpRequest, HttpResponse};
        let req = HttpRequest { method: "GET".into(), url: "/metrics".into(), headers: vec![], body: Default::default() };
        let m: HttpResponse = self.query(Principal::anonymous(), "http_request", encode_one(req).unwrap());
        let text = String::from_utf8(m.body.into_vec()).unwrap();
        text.lines().find_map(|l| l.strip_prefix("bp_tick_fast ")).expect("the metric is published") == "1"
    }

    /// Two funded players sit down at Penny Lane.
    fn seat_two(&self) -> (Principal, Principal, u64) {
        let (alice, bob) = (principal(1), principal(2));
        self.register(alice, "alice");
        self.register(bob, "bob");
        self.approve_and_deposit(alice, 2 * E8S).unwrap();
        self.approve_and_deposit(bob, 2 * E8S).unwrap();
        let table = 5u64;
        for (k, who) in [alice, bob].into_iter().enumerate() {
            let r: R<()> = self.update(who, "join_table", encode_args((table, k as u8, 40_000_000u64, None::<String>)).unwrap());
            r.unwrap();
        }
        (alice, bob, table)
    }

    /// With no call from anyone, steps the clock until the hand in play ends
    /// and the table is back to waiting. Returns the seconds it took.
    fn wait_out_hand(&self, who: Principal, table: u64) -> u64 {
        use poker_backend::table::Phase;
        for secs in 1..=UNATTENDED_HAND_SECS {
            self.run_timers(1);
            if self.table(who, table).phase == Phase::Waiting {
                return secs;
            }
        }
        panic!("the hand did not finish in {UNATTENDED_HAND_SECS} seconds with nobody acting");
    }

    fn tournament_named(&self, name: &str) -> poker_backend::tournament::TournamentSummary {
        let all: Vec<poker_backend::tournament::TournamentSummary> = self.query(principal(1), "list_tournaments", encode_args(()).unwrap());
        all.into_iter().filter(|t| t.name == name).max_by_key(|t| t.id).unwrap_or_else(|| panic!("no {name}"))
    }

    /// Moves the clock to just before `start_at` the way an idle canister
    /// lives through it (a tick per simulated minute, no calls), then a second
    /// at a time until `id` has left `Registering`. Returns how long after
    /// `start_at` that was, in seconds.
    fn seconds_late(&self, id: u64, start_at: u64) -> u64 {
        use poker_backend::tournament::{TournamentStatus, TournamentSummary};
        let now = |env: &Env| env.pic.get_time().as_nanos_since_unix_epoch();
        while start_at.saturating_sub(now(self)) > 90 * 1_000_000_000 {
            self.pic.advance_time(Duration::from_secs(60));
            self.pic.tick();
            self.pic.tick();
        }
        for _ in 0..300 {
            self.run_timers(1);
            let all: Vec<TournamentSummary> = self.query(principal(1), "list_tournaments", encode_args(()).unwrap());
            let waiting = all.iter().any(|t| t.id == id && t.status == TournamentStatus::Registering);
            if !waiting {
                return now(self).saturating_sub(start_at) / 1_000_000_000;
            }
        }
        panic!("tournament {id} never left registration");
    }
}

/// Nobody playing: the tick drops to its slow pace on its own, and the burn
/// with it. A player's first call brings it straight back.
#[test]
fn the_tick_slows_when_idle_and_wakes_for_players() {
    let Some(env) = Env::new() else { return };
    env.run_timers(3);
    assert!(!env.tick_fast(), "a new canister with no players settles to the slow tick");

    // Long enough for the balance to reflect the burn, not the cycles a call in
    // flight has reserved. Starts past the first entropy and solvency calls.
    const IDLE_SECS: u64 = 1800;
    env.run_timers(10);
    let c0 = env.pic.cycle_balance(env.poker);
    env.run_timers(IDLE_SECS);
    let idle = c0.saturating_sub(env.pic.cycle_balance(env.poker)) / IDLE_SECS as u128;
    assert!(!env.tick_fast());

    // A cold start: the first hand still comes at once, from the join alone.
    let (alice, _, table) = env.seat_two();
    env.run_timers(3);
    assert!(env.tick_fast());
    assert!(env.table(alice, table).hand_no >= 1, "a hand is dealt within seconds of the second player sitting");

    let c0 = env.pic.cycle_balance(env.poker);
    env.run_timers(30);
    let playing = c0.saturating_sub(env.pic.cycle_balance(env.poker)) / 30;
    eprintln!("TICK idle_per_s={idle} playing_per_s={playing}");
    assert!(idle > 0 && idle * 10 < playing, "idle should cost a small fraction of play, got {idle} vs {playing} per second");
}

/// Nobody acts and nobody calls: the action clock still runs out on time, and
/// once the hand has ended the tick goes back to its slow pace.
#[test]
fn action_clocks_run_out_with_nobody_calling() {
    let Some(env) = Env::new() else { return };
    let (alice, bob, table) = env.seat_two();
    env.run_timers(3);
    let dealt = env.table(alice, table);
    assert_eq!(dealt.hand_no, 1);
    let actor = dealt.to_act.expect("someone to act") as usize;

    let secs = env.wait_out_hand(alice, table);
    assert!(secs >= 25, "the clock was cut short: {secs}s");
    let after = env.table(alice, table);
    assert!(after.seats[actor].as_ref().unwrap().sitting_out, "the player who ran out of time sits out");
    env.run_timers(2);
    assert!(!env.tick_fast(), "one player left in: nothing to run a clock for");

    // They come back: a wake from the call alone, with the slow tick not due.
    let them = dealt.seats[actor].as_ref().unwrap().player;
    let r: R<()> = env.update(them, "sit_out", encode_args((table, false)).unwrap());
    r.unwrap();
    env.run_timers(3);
    assert_eq!(env.table(bob, table).hand_no, 2);
}

/// Upgrades drop every timer. A hand in progress keeps its clock with no call
/// from anyone, and an empty game still gets back to the slow pace.
#[test]
fn an_upgrade_mid_hand_keeps_the_clock_running() {
    let Some(env) = Env::new() else { return };
    let (alice, _, table) = env.seat_two();
    env.run_timers(3);
    assert_eq!(env.table(alice, table).hand_no, 1);
    env.pic.upgrade_canister(env.poker, env.wasm.clone(), encode_args(()).unwrap(), Some(env.admin)).unwrap();
    env.wait_out_hand(alice, table);
    env.run_timers(2);
    assert!(!env.tick_fast());

    env.pic.upgrade_canister(env.poker, env.wasm.clone(), encode_args(()).unwrap(), Some(env.admin)).unwrap();
    env.run_timers(3);
    assert!(!env.tick_fast(), "nothing to do after this upgrade");
}

impl Env {
    /// One figure from the public metrics.
    fn metric(&self, name: &str) -> u64 {
        use poker_backend::metrics::{HttpRequest, HttpResponse};
        let req = HttpRequest { method: "GET".into(), url: "/metrics".into(), headers: vec![], body: Default::default() };
        let m: HttpResponse = self.query(Principal::anonymous(), "http_request", encode_one(req).unwrap());
        let text = String::from_utf8(m.body.into_vec()).unwrap();
        let prefix = format!("bp_{name} ");
        text.lines().find_map(|l| l.strip_prefix(prefix.as_str())).unwrap_or_else(|| panic!("no {name}")).parse().unwrap()
    }

    /// Settings as on mainnet, with the ledger standing in for Internet Identity:
    /// it refuses the send, which leaves everything up to the send to observe.
    fn configure_alerts(&self) {
        let settings = pocket_ic::CanisterSettings {
            environment_variables: Some(vec![
                pocket_ic::EnvironmentVariable { name: "notification_sender".into(), value: self.ledger.to_text() },
                pocket_ic::EnvironmentVariable { name: "notification_origin".into(), value: "https://byggy-dyaaa-aaaad-aha7q-cai.icp0.io".into() },
            ]),
            ..Default::default()
        };
        self.pic.update_canister_settings(self.poker, Some(self.admin), settings).unwrap();
    }

    /// Jumps the clock to `at` (nanoseconds) and lets the timers due by then run.
    fn jump_to(&self, at: u64) {
        let now = self.pic.get_time().as_nanos_since_unix_epoch();
        self.pic.advance_time(Duration::from_nanos(at.saturating_sub(now)));
        self.run_timers(3);
    }
}

/// Registered players are alerted once, a few minutes before the start, and an
/// upgrade neither repeats the alert nor disturbs what's in stable memory.
#[test]
fn registered_players_get_one_alert_before_a_scheduled_start() {
    use poker_backend::tournament::START_ALERT_LEAD_NS;
    let Some(env) = Env::new() else { return };
    env.configure_alerts();
    let (alice, bob, carol) = (principal(1), principal(2), principal(3));
    env.register(alice, "alice");
    env.register(bob, "bob");
    env.register(carol, "carol");
    env.run_timers(3);
    let t = env.tournament_named("Hourly Freeroll");
    for who in [alice, bob] {
        let r: R<()> = env.update(who, "register_tournament", encode_one(t.id).unwrap());
        r.unwrap();
    }
    const MINUTE: u64 = 60_000_000_000;

    env.jump_to(t.start_at - START_ALERT_LEAD_NS - 2 * MINUTE);
    assert_eq!(env.metric("alerts_queued"), 0, "too early");

    env.jump_to(t.start_at - START_ALERT_LEAD_NS + MINUTE);
    assert_eq!(env.metric("alerts_queued"), 2, "alice and bob, and not carol");
    assert_eq!(env.metric("alerts_misconfigured_since"), 0);

    let before = env.balance(alice);
    env.pic.upgrade_canister(env.poker, env.wasm.clone(), encode_args(()).unwrap(), Some(env.admin)).unwrap();
    env.jump_to(t.start_at - MINUTE);
    assert_eq!(env.metric("alerts_queued"), 2, "only once, even across an upgrade");
    assert_eq!(env.balance(alice), before, "the game's stable memory is untouched");
}

/// A canister nobody has given the settings to still plays, and says so.
#[test]
fn missing_alert_settings_show_up_in_the_metrics() {
    use poker_backend::tournament::START_ALERT_LEAD_NS;
    let Some(env) = Env::new() else { return };
    let alice = principal(1);
    env.register(alice, "alice");
    env.run_timers(3);
    let t = env.tournament_named("Hourly Freeroll");
    let r: R<()> = env.update(alice, "register_tournament", encode_one(t.id).unwrap());
    r.unwrap();
    assert_eq!(env.metric("alerts_misconfigured_since"), 0);
    env.jump_to(t.start_at - START_ALERT_LEAD_NS + 60_000_000_000);
    assert_eq!(env.metric("alerts_queued"), 1);
    assert_eq!(env.metric("alerts_dropped"), 1);
    assert!(env.metric("alerts_misconfigured_since") > 0);
}

/// Internet Identity's service worker calls as a principal of its own, never a
/// registered player, and its receipts must get past the ingress filter.
#[test]
fn delivery_receipts_are_accepted_from_unregistered_callers() {
    let Some(env) = Env::new() else { return };
    let stranger = principal(99);
    for method in ["_internet_identity_notification_received", "_internet_identity_notification_opened"] {
        env.pic.update_call(env.poker, stranger, method, encode_one(1u64).unwrap()).unwrap_or_else(|e| panic!("{method}: {e:?}"));
    }
    assert!(env.pic.update_call(env.poker, stranger, "claim_daily_chips", encode_args(()).unwrap()).is_err(), "the filter still stops strangers elsewhere");
}

/// An idle canister starts a scheduled tournament on time: nothing calls it in
/// the minutes before, and only the slow tick is running.
#[test]
fn a_scheduled_tournament_starts_on_time_while_idle() {
    use poker_backend::tournament::TournamentStatus;
    let Some(env) = Env::new() else { return };
    let (alice, bob) = (principal(1), principal(2));
    env.register(alice, "alice");
    env.register(bob, "bob");
    env.run_timers(3);
    let t = env.tournament_named("Hourly Freeroll");
    for who in [alice, bob] {
        let r: R<()> = env.update(who, "register_tournament", encode_one(t.id).unwrap());
        r.unwrap();
    }
    env.run_timers(3);
    assert!(!env.tick_fast(), "registering alone runs no clock");

    let late = env.seconds_late(t.id, t.start_at);
    assert!(late <= 2, "started {late}s late");
    let started = env.tournament_named("Hourly Freeroll");
    assert_eq!(started.status, TournamentStatus::Registering, "the next hour's event opened too");
    let all: Vec<poker_backend::tournament::TournamentSummary> = env.query(alice, "list_tournaments", encode_args(()).unwrap());
    assert_eq!(all.iter().find(|x| x.id == t.id).unwrap().status, TournamentStatus::Running);
    assert!(env.tick_fast(), "the tournament has clocks to run");
}

/// An event nobody registered for closes on time too, and the next one opens,
/// so the lobby never shows a countdown stuck at zero.
#[test]
fn an_empty_scheduled_tournament_rolls_over_on_time() {
    let Some(env) = Env::new() else { return };
    env.register(principal(1), "alice");
    env.run_timers(3);
    let t = env.tournament_named("Hourly Freeroll");
    let late = env.seconds_late(t.id, t.start_at);
    assert!(late <= 2, "rolled over {late}s late");
    assert!(env.tournament_named("Hourly Freeroll").id > t.id);
    env.run_timers(2);
    assert!(!env.tick_fast(), "and the game goes back to idling");
}
