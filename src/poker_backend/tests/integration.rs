//! End-to-end tests: the poker canister against the real ICP ledger canister
//! on a PocketIC replica. Run with `./scripts/integration-test.sh`, which
//! downloads PocketIC and the ledger wasm and sets the env vars below.
//! Without them (a plain `cargo test`), these tests are skipped.

use candid::{decode_one, encode_args, encode_one, CandidType, Deserialize, Nat, Principal};
use icrc_ledger_types::icrc1::account::Account;
use pocket_ic::{PocketIc, PocketIcBuilder};
use poker_backend::state::{HouseStats, Profile, TableView};
use poker_backend::table::Action;
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
        let args = Some(InitArgs { ledger: Some(ledger), ledger_fee: Some(FEE) });
        pic.install_canister(poker, wasm.clone(), encode_one(args).unwrap(), Some(admin));
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
        let r: R<Profile> = self.update(who, "register", encode_one(name.to_string()).unwrap());
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

    /// Lets the 1-second timer run a few times.
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

    // Penny Lane (ICP, 0.0001/0.0002): the timer deals once two players sit.
    let table = 5u64;
    for (k, who) in [alice, bob].into_iter().enumerate() {
        let r: R<()> = env.update(who, "join_table", encode_args((table, k as u8, 1_000_000u64, None::<String>)).unwrap());
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
    assert_eq!(env.balance(alice), 2 * E8S - 1_000_000);

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
        let r: R<u64> = env.update(who, "leave_table", encode_one(table).unwrap());
        r.unwrap();
    }
    let report = env.solvency();
    assert!(report.ok, "{report:?}");
    assert_eq!(report.surplus, 0);
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
    assert!(!text.contains("bp_heap_bytes 0\n"));
    assert_eq!(get("/health").status_code, 200);
    assert_eq!(get("/nope").status_code, 404);
    let log: R<Vec<poker_backend::moderation::AuditEntry>> = env.query(env.admin, "admin_audit_log", encode_one(10u32).unwrap());
    assert_eq!(log.unwrap().len(), 2, "moderator appointment and the mute");
}
