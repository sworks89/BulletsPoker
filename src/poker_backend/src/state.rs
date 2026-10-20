//! Canister state and the business logic around it: profiles, wallets, the
//! item store, gifts, chat, friends and tables. Everything here is pure (the
//! caller and the current time are passed in) so it can be unit tested
//! natively; `lib.rs` wraps it in canister endpoints.

use crate::table::{Action, AutoSitEntry, Currency, HandResult, LogEntry, Phase, Table, TableConfig};
use crate::jackpot::JackpotHit;
use crate::store::{load_blob, save_blob, DmThread, Stores, UserMut, Users};
use crate::moderation::{AuditEntry, Ban};
use crate::wallet::{PauseFlags, SolvencyReport};
use ic_stable_structures::DefaultMemoryImpl;
use crate::tournament::{Tournament, TournamentBadge, TournamentTemplate};
use sha2::{Digest, Sha256};
use candid::{CandidType, Deserialize, Principal};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const E8S: u64 = 100_000_000;
const CHAT_HISTORY: usize = 100;
const DM_HISTORY: usize = 200;
const MAX_CHAT_LEN: usize = 280;
const CHAT_COOLDOWN_NS: u64 = 700_000_000;
const GIFT_DURATION_NS: u64 = 10 * 60 * 1_000_000_000;
const ONLINE_WINDOW_NS: u64 = 5 * 60 * 1_000_000_000;
const DAY_NS: u64 = 24 * 60 * 60 * 1_000_000_000;
const THROW_COOLDOWN_NS: u64 = 3_000_000_000;
/// How long a throw stays in the table view so every client sees it.
const THROW_VISIBLE_NS: u64 = 8_000_000_000;
const HISTORY_PER_USER: usize = 50;
const HISTORY_MAX: usize = 20_000;
pub const SIGNUP_CHIPS: u64 = 20_000;
pub const DAILY_CHIPS: u64 = 5_000;
const MAX_STREAK: u32 = 7;
/// Share of the rake paid to the creator of a player-created ICP room.
pub const HOST_RAKE_SHARE_BPS: u64 = 2_000;
const ROOM_IDLE_CLOSE_NS: u64 = 30 * 60 * 1_000_000_000;
/// An empty copy of a full game closes after this long (the original never does).
pub const COPY_IDLE_CLOSE_NS: u64 = 60 * 1_000_000_000;
/// Most copies one game can have open, and in total per currency (separate
/// budgets so a rush on free tables can't block real-money ones).
pub const MAX_COPIES_PER_GAME: usize = 50;
pub const MAX_CHIP_COPIES: usize = 200;
pub const MAX_ICP_COPIES: usize = 100;
/// Auto-sit: a waiting player whose browser hasn't checked in for this long is
/// dropped, and one is only seated if it checked in within `AUTO_SIT_LIVE_NS`.
pub const AUTO_SIT_EXPIRE_NS: u64 = 60 * 1_000_000_000;
pub const AUTO_SIT_LIVE_NS: u64 = 25 * 1_000_000_000;
pub const AUTO_SIT_MAX_QUEUE: usize = 100;
const MAX_ROOMS_PER_USER: usize = 2;
const MAX_NOTE_LEN: usize = 500;
const LEADERS: usize = 100;
/// Cash-table seats one player may hold at once.
/// Leaving a cash table and coming back within this time requires buying back
/// in for at least the stack you left with (no "ratholing" winnings).
pub const RETURN_WINDOW_NS: u64 = 30 * 60 * 1_000_000_000;
pub const MAX_CASH_SEATS: usize = 4;
/// Players sitting out this long at a cash table are stood up.
pub const IDLE_STAND_NS: u64 = 10 * 60 * 1_000_000_000;
/// Hosting a table needs this level, and only this many rooms can exist at once.
pub const HOST_MIN_LEVEL: u32 = 2;
const MAX_ROOMS: usize = 200;
const MAX_FRIENDS: usize = 500;
const MAX_FRIEND_REQUESTS: usize = 100;
pub const DEFAULT_SIGNUPS_PER_HOUR: u32 = 1_000;
/// Bad-beat jackpot drop on cash tables: 1% of the pot after rake, capped at one big blind.
pub const JACKPOT_DROP_BPS: u16 = 100;
const MAX_NOTES: usize = 2_000;

pub type R<T> = Result<T, String>;

#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct Stats {
    pub hands_played: u64,
    pub hands_won: u64,
    /// Largest ICP pot won, in e8s.
    pub biggest_pot_won: u64,
    /// Net ICP won or lost at ICP tables, in e8s.
    pub net_winnings: i64,
    /// Net play chips won or lost at chip tables.
    pub chip_net: i64,
}

/// Color label for a player note (a quick read on a player's style).
#[derive(CandidType, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum NoteColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
}

/// A private note one player keeps about another. Only its author can see it.
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct PlayerNote {
    pub text: String,
    pub color: Option<NoteColor>,
    pub updated_at: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct NoteView {
    pub player: Principal,
    pub username: String,
    pub avatar: String,
    pub text: String,
    pub color: Option<NoteColor>,
    pub updated_at: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Settings {
    pub four_color_deck: bool,
    /// Don't show losing hands at showdown.
    pub auto_muck: bool,
    /// Refill your stack to the table maximum after each hand when it drops below half.
    pub auto_top_up: bool,
    pub sound: bool,
    pub show_throwables: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            four_color_deck: false,
            auto_muck: false,
            auto_top_up: false,
            sound: true,
            show_throwables: true,
        }
    }
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct ActiveGift {
    pub item: u32,
    pub from: String,
    pub expires_at: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct User {
    pub username: String,
    pub status: String,
    /// Wallet balance in e8s (ICP not sitting on a table).
    pub balance: u64,
    /// Free play chips not sitting on a table.
    pub chips: u64,
    pub settings: Settings,
    /// Hand ids of this player's most recent hands, oldest first.
    pub history: VecDeque<u64>,
    pub daily_streak: u32,
    pub last_daily_at: u64,
    pub last_throw_at: u64,
    /// Medal id -> time earned.
    pub medals: BTreeMap<String, u64>,
    pub gifts_sent: u64,
    pub throws_made: u64,
    /// Recent ledger operation ids (deposits, withdrawals), oldest first.
    pub ops: Vec<u64>,
    /// Players this user has blocked.
    pub blocked: BTreeSet<Principal>,
    pub ban: Option<Ban>,
    pub chat_muted_until: u64,
    pub reports_day: u64,
    pub reports_today: u32,
    pub avatar: u32,
    pub card_back: u32,
    pub inventory: Vec<u32>,
    pub friends: BTreeSet<Principal>,
    pub friend_requests: BTreeSet<Principal>,
    pub stats: Stats,
    pub xp: u64,
    pub gift: Option<ActiveGift>,
    pub created_at: u64,
    pub last_seen: u64,
    pub last_chat_at: u64,
    /// Fish Buffet points and banked spins; created on first earn.
    pub rewards: Option<crate::rewards::RewardsAccount>,
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub enum ItemKind {
    Avatar,
    CardBack,
    Gift,
    Throwable,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct StoreItem {
    pub id: u32,
    pub name: String,
    pub kind: ItemKind,
    /// Price in e8s. Zero means free.
    pub price: u64,
    /// Price in play chips. Zero means it can't be bought with chips.
    pub chip_price: u64,
    /// Emoji or image URL rendered by the frontend.
    pub icon: String,
    pub description: String,
    pub available: bool,
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub enum MessageKind {
    Text,
    System,
    Gift,
    Throw,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub id: u64,
    pub from: Option<Principal>,
    pub username: String,
    pub text: String,
    pub time: u64,
    pub kind: MessageKind,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct ThrowEvent {
    pub id: u64,
    pub from_seat: u8,
    pub to_seat: u8,
    pub icon: String,
    pub time: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct HandPlayer {
    pub seat: u8,
    pub player: Principal,
    pub username: String,
    /// Hole cards. Stored for everyone; views only reveal your own and shown hands.
    pub cards: Vec<u8>,
    pub shown: bool,
    pub committed: u64,
    pub won: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct HandRecord {
    pub id: u64,
    pub table_id: u64,
    pub table_name: String,
    pub currency: Currency,
    pub small_blind: u64,
    pub big_blind: u64,
    pub hand_no: u64,
    pub time: u64,
    pub button: u8,
    pub board: Vec<u8>,
    pub pot: u64,
    pub rake: u64,
    pub players: Vec<HandPlayer>,
    pub log: Vec<LogEntry>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct HandSummary {
    pub id: u64,
    pub table_name: String,
    pub currency: Currency,
    pub time: u64,
    pub board: Vec<u8>,
    pub my_cards: Vec<u8>,
    pub pot: u64,
    /// What the viewer won minus what they put in.
    pub net: i64,
}

#[derive(CandidType, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Channel {
    Lobby,
    Table(u64),
}

/// Heap state that is saved as a snapshot on upgrade. Everything here is
/// bounded in size; anything that grows with the player base belongs in
/// `crate::store`. New fields must be `Option<T>` so older snapshots still
/// decode, or bump `SNAPSHOT_VERSION` and migrate in `State::restore`.
macro_rules! heap_state {
    ($($(#[$doc:meta])* $field:ident : $ty:ty,)*) => {
        pub struct State {
            pub users: Users,
            pub stores: Stores,
            $($(#[$doc])* pub $field: $ty,)*
        }

        #[derive(CandidType, Deserialize)]
        struct HeapSnapshot {
            version: u32,
            $($field: $ty,)*
        }

        impl State {
            fn snapshot(&self) -> HeapSnapshot {
                HeapSnapshot { version: SNAPSHOT_VERSION, $($field: self.$field.clone(),)* }
            }

            fn from_snapshot(users: Users, stores: Stores, snap: HeapSnapshot) -> Self {
                State { users, stores, $($field: snap.$field,)* }
            }
        }
    };
}

pub const SNAPSHOT_VERSION: u32 = 1;

heap_state! {
    ledger: Principal,
    ledger_fee: u64,
    tables: BTreeMap<u64, Table>,
    next_table_id: u64,
    /// Last hand number per table whose result was applied to stats/treasury.
    applied_hand: BTreeMap<u64, u64>,
    items: BTreeMap<u32, StoreItem>,
    next_item_id: u32,
    chats: BTreeMap<Channel, VecDeque<ChatMessage>>,
    next_msg_id: u64,
    /// House revenue (rake and store sales) in e8s, withdrawable by controllers.
    treasury: u64,
    next_hand_id: u64,
    throws: BTreeMap<u64, VecDeque<ThrowEvent>>,
    next_throw_id: u64,
    /// Player-created tables, by table id.
    rooms: BTreeMap<u64, Room>,
    tournaments: BTreeMap<u64, Tournament>,
    next_tournament_id: u64,
    /// Table id -> tournament id for tournament tables.
    table_tournament: BTreeMap<u64, u64>,
    /// Recurring tournaments; a fresh instance opens whenever the previous one starts.
    templates: Vec<TournamentTemplate>,
    /// Bad-beat jackpot pool per currency.
    jackpots: BTreeMap<Currency, u64>,
    /// Most recent jackpot hits, newest first.
    jackpot_hits: VecDeque<JackpotHit>,
    /// Latest hit per table, shown while that hand is on screen.
    table_jackpot_hit: BTreeMap<u64, JackpotHit>,
    next_op_id: u64,
    /// Ledger ops whose outcome is unknown (retried or awaiting an admin).
    unresolved: BTreeSet<u64>,
    pause: PauseFlags,
    last_solvency: Option<SolvencyReport>,
    /// Sum of all ICP wallet balances, kept up to date so solvency checks never scan users.
    wallet_total: u64,
    /// Top players by ICP net winnings, kept up to date as hands finish.
    leaders: Vec<(Principal, i64)>,
    moderators: BTreeSet<Principal>,
    banned_words: Vec<String>,
    /// Privileged actions, newest first.
    audit_log: VecDeque<AuditEntry>,
    open_reports: BTreeSet<u64>,
    next_report_id: u64,
    counters: Counters,
    /// Sign-ups this hour. Past `signups_per_hour` the sign-up proof-of-work
    /// gets harder (see `crate::signup`); 0 closes sign-ups.
    signup_hour: u64,
    signup_count: u32,
    signups_per_hour: u32,
    /// Base proof-of-work difficulty (leading zero bits) for a new account.
    signup_pow_bits: u8,
    /// (table, player) -> (chips taken, when) for recent cash-game departures:
    /// coming back within `RETURN_WINDOW_NS` needs at least that many chips.
    cashouts: BTreeMap<(u64, Principal), (u64, u64)>,
    /// Secret randomness from `raw_rand`, refreshed regularly and on every
    /// deal, for decisions made outside a deal (tournament seating).
    entropy: Vec<u8>,
    /// One-shot results of auto-sit (seated, or dropped and why), read by the
    /// player's next `auto_sit_ping`.
    auto_sit_outcomes: Option<BTreeMap<Principal, AutoSitStatus>>,
    /// Reward pools (see `crate::rewards`).
    rewards: Option<crate::rewards::RewardsState>,
}

/// Monotonic counters exported as metrics.
#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct Counters {
    pub hands_dealt: u64,
    pub ledger_ops_failed: u64,
    pub ledger_ops_unknown: u64,
    pub reports_filed: u64,
    pub bans: u64,
    pub idle_stand_ups: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Room {
    pub owner: Principal,
    /// SHA-256 of (table id, password) for password-protected rooms.
    pub password_hash: Option<Vec<u8>>,
    pub created_at: u64,
    pub last_active: u64,
    /// Players who entered the password: they may watch and chat.
    pub guests: BTreeSet<Principal>,
}

/// Most players who can unlock one password-protected room.
const MAX_ROOM_GUESTS: usize = 500;

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct RoomArgs {
    pub name: String,
    pub currency: Currency,
    pub big_blind: u64,
    pub max_seats: u8,
    pub password: Option<String>,
    pub action_timeout_secs: u64,
}

// ------------------------------------------------------------------ views

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Profile {
    pub id: Principal,
    pub username: String,
    pub status: String,
    pub balance: u64,
    pub chips: u64,
    pub settings: Settings,
    pub daily_streak: u32,
    /// When the next daily chip bonus can be claimed (ns).
    pub next_daily_at: u64,
    /// Size of the next daily bonus.
    pub next_daily_amount: u64,
    pub avatar: u32,
    pub card_back: u32,
    pub inventory: Vec<u32>,
    pub friends: Vec<Principal>,
    pub friend_requests: Vec<Principal>,
    pub stats: Stats,
    pub xp: u64,
    pub level: u32,
    pub created_at: u64,
    pub medals: Vec<EarnedMedal>,
    pub blocked: Vec<Principal>,
    pub ban: Option<Ban>,
    pub chat_muted_until: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct EarnedMedal {
    pub id: String,
    pub earned_at: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct PublicProfile {
    pub id: Principal,
    pub username: String,
    pub status: String,
    pub avatar: String,
    pub stats: Stats,
    pub level: u32,
    pub online: bool,
    pub tables: Vec<u64>,
    pub medals: Vec<String>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct SeatView {
    pub seat: u8,
    pub player: Principal,
    pub username: String,
    pub avatar: String,
    pub card_back: String,
    pub level: u32,
    pub stack: u64,
    pub bet: u64,
    pub in_hand: bool,
    pub folded: bool,
    pub all_in: bool,
    pub sitting_out: bool,
    pub has_cards: bool,
    pub cards: Vec<u8>,
    pub last_action: Option<String>,
    pub gift: Option<String>,
    /// Who sent the current gift (their username).
    pub gift_from: Option<String>,
    /// When the current gift was sent; a repeat of the same gift gets a new time.
    pub gift_sent_at: Option<u64>,
    /// The viewer's own note about this player, if any.
    pub note: Option<PlayerNote>,
    /// Seconds left in this player's time bank.
    pub time_bank_secs: u32,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TableView {
    pub id: u64,
    pub config: TableConfig,
    /// The game this table belongs to: the original table's id (its own id for originals and rooms).
    pub game: u64,
    /// 1 for the original, 2.. for copies opened when the game filled up.
    pub number: u8,
    /// The viewer's place in this table's auto-sit line, if they're in it.
    pub my_auto_sit: Option<AutoSitView>,
    /// How many watchers are waiting to be seated here automatically.
    pub auto_sit_waiting: u32,
    pub phase: Phase,
    pub hand_no: u64,
    pub seats: Vec<Option<SeatView>>,
    pub board: Vec<u8>,
    pub pot: u64,
    pub current_bet: u64,
    pub button: u8,
    pub to_act: Option<u8>,
    pub action_deadline: u64,
    /// The player to act is on their time bank (`action_deadline` includes it).
    pub time_bank_active: bool,
    pub now: u64,
    pub last_result: Option<HandResult>,
    pub my_seat: Option<u8>,
    pub to_call: u64,
    pub min_raise_to: u64,
    pub max_raise_to: u64,
    /// Items thrown in the last few seconds, for clients to animate.
    pub throws: Vec<ThrowEvent>,
    pub tournament: Option<TournamentBadge>,
    /// The player who created this room, if it is a player-created table.
    pub room_owner: Option<Principal>,
    /// Joining needs a password.
    pub locked: bool,
    /// A private room the viewer hasn't unlocked: game details are hidden.
    pub restricted: bool,
    /// Smallest buy-in the viewer may sit down with (they left recently with
    /// this many chips); 0 means the table's normal minimum.
    pub my_min_buy_in: u64,
    /// Bad-beat jackpot for this table's currency (0 at tournament tables).
    pub jackpot: u64,
    /// A jackpot hit in the hand on screen, for the celebration.
    pub jackpot_hit: Option<JackpotHit>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TableSummary {
    pub id: u64,
    /// The game this table belongs to: the original table's id (its own id for originals and rooms).
    pub game: u64,
    /// 1 for the original, 2.. for copies opened when the game filled up.
    pub number: u8,
    /// No more copies of this game can open right now (always true for rooms).
    pub game_at_cap: bool,
    pub name: String,
    pub currency: Currency,
    pub small_blind: u64,
    pub big_blind: u64,
    pub min_buy_in: u64,
    pub max_buy_in: u64,
    pub players: u8,
    pub max_seats: u8,
    pub phase: Phase,
    pub avg_pot: u64,
    /// Username of the player who created this room, for player-created tables.
    pub room_owner: Option<String>,
    pub locked: bool,
}

/// Where a player stands with auto-sit at a table.
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub enum AutoSitStatus {
    NotQueued,
    Queued { place: u32, waiting: u32, buy_in: u64 },
    /// Seated automatically at this table (reported once).
    Seated { table: u64 },
    /// Taken out of the line because seating failed (reported once).
    Dropped { reason: String },
}

/// The viewer's own place in a table's auto-sit line.
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct AutoSitView {
    pub place: u32,
    pub buy_in: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct FriendInfo {
    pub id: Principal,
    pub username: String,
    pub avatar: String,
    pub online: bool,
    pub tables: Vec<u64>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct LeaderboardEntry {
    pub id: Principal,
    pub username: String,
    pub avatar: String,
    pub level: u32,
    pub stats: Stats,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct HouseStats {
    pub users: u64,
    pub wallet_balances: u64,
    pub chips_on_tables: u64,
    pub treasury: u64,
    /// ICP bad-beat jackpot.
    pub jackpot: u64,
    /// What the canister's main ledger account must hold to cover everything.
    pub liabilities_total: u64,
    /// Fish and leaderboard prize pools, owed to players.
    pub rewards_pools: u64,
}

pub fn level_for_xp(xp: u64) -> u32 {
    // Level n requires 50 * n^2 xp.
    ((xp as f64 / 50.0).sqrt()) as u32 + 1
}

fn validate_username(name: &str) -> R<()> {
    let ok = (3..=16).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(())
    } else {
        Err("Usernames are 3-16 letters, digits or underscores".into())
    }
}

fn dm_key(a: Principal, b: Principal) -> (Principal, Principal) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// (kind, name, ICP price in e8s, chip price, icon, description)
type ItemSpec = (ItemKind, &'static str, u64, u64, &'static str, &'static str);

fn default_items() -> Vec<ItemSpec> {
    use ItemKind::*;
    vec![
        (Avatar, "Rookie", 0, 0, "🙂", "Everyone starts somewhere"),
        (Avatar, "Cool Cat", 0, 0, "😎", "Too cool for the rail"),
        (Avatar, "Robot", 0, 0, "🤖", "Calculating pot odds"),
        (Avatar, "Cowboy", E8S / 20, 100_000, "🤠", "Yeehaw, all in"),
        (Avatar, "Alien", E8S / 20, 100_000, "👽", "Plays from another galaxy"),
        (Avatar, "Ninja", E8S / 10, 250_000, "🥷", "Silent and deadly"),
        (Avatar, "Shark", E8S / 10, 250_000, "🦈", "Smells fish at the table"),
        (Avatar, "Dragon", E8S / 4, 0, "🐉", "Hoards every pot"),
        (Avatar, "Unicorn", E8S / 4, 0, "🦄", "Rare as a royal flush"),
        (Avatar, "Crown", E8S, 0, "👑", "Table royalty"),
        (CardBack, "Classic Red", 0, 0, "#b91c1c", "The house deck"),
        (CardBack, "Midnight", E8S / 50, 50_000, "#1e3a8a", "Deep blue backs"),
        (CardBack, "Emerald", E8S / 50, 50_000, "#047857", "Felt-green backs"),
        (CardBack, "Gold Foil", E8S / 5, 0, "#ca8a04", "Shine on every deal"),
        (Gift, "Beer", E8S / 1000, 500, "🍺", "Buy the table a round"),
        (Gift, "Coffee", E8S / 1000, 500, "☕", "For the long session"),
        (Gift, "Rose", E8S / 500, 1_000, "🌹", "Show some love"),
        (Gift, "Cocktail", E8S / 500, 1_000, "🍸", "Shaken, not stirred"),
        (Gift, "Pizza", E8S / 200, 2_500, "🍕", "Fuel for the grind"),
        (Gift, "Cigar", E8S / 200, 2_500, "🚬", "Celebrate the big win"),
        (Gift, "Champagne", E8S / 100, 5_000, "🍾", "Pop it after a monster pot"),
        (Gift, "Trophy", E8S / 20, 0, "🏆", "Crown the champ"),
        (Throwable, "Tomato", E8S / 2000, 200, "🍅", "Splat! For a bad call"),
        (Throwable, "Pie", E8S / 2000, 200, "🥧", "Pie to the face"),
        (Throwable, "Snowball", E8S / 2000, 200, "❄️", "Chill out"),
        (Throwable, "Fish", E8S / 1000, 500, "🐟", "You know what you did"),
        (Throwable, "Kiss", E8S / 1000, 500, "💋", "Mwah"),
        (Throwable, "Money", E8S / 500, 1_000, "💸", "Make it rain"),
        (Throwable, "Bomb", E8S / 500, 1_000, "💣", "Boom"),
    ]
}

pub fn default_tables() -> Vec<TableConfig> {
    let t = |name: &str, currency: Currency, sb: u64, seats: u8| TableConfig {
        name: name.into(),
        currency,
        small_blind: sb,
        big_blind: sb * 2,
        min_buy_in: sb * 2 * 20,
        max_buy_in: sb * 2 * 100,
        max_seats: seats,
        // Play-chip tables are rake free; the house only earns on real ICP.
        rake_bps: if currency == Currency::Icp { 250 } else { 0 },
        rake_cap: if currency == Currency::Icp { sb * 2 * 3 } else { 0 },
        jackpot_bps: JACKPOT_DROP_BPS,
        jackpot_cap: sb * 2,
        action_timeout_secs: 25,
    };
    use Currency::*;
    vec![
        t("Free Roll Fun", Chips, 5, 9),
        t("Chip Chasers", Chips, 25, 6),
        t("Heads Up Hangout", Chips, 25, 2),
        t("Big Stack Saloon", Chips, 250, 9),
        t("Penny Lane", Icp, 10_000, 9),
        t("Micro Madness", Icp, 10_000, 6),
        t("Low Rollers", Icp, 100_000, 9),
        t("Heads Up Arena", Icp, 100_000, 2),
        t("Mid Stakes Saloon", Icp, 1_000_000, 6),
        t("High Roller Lounge", Icp, 10_000_000, 6),
    ]
}

/// Daily chip bonus for a given streak day (1-based): grows each day up to a week.
pub fn daily_amount(streak: u32) -> u64 {
    DAILY_CHIPS * streak.clamp(1, MAX_STREAK) as u64
}

impl State {
    /// Writes the heap snapshot to stable memory (call from `pre_upgrade`).
    pub fn save(&self) {
        let bytes = candid::encode_one(self.snapshot()).expect("encode heap snapshot");
        save_blob(&self.stores.heap_mem, &bytes);
    }

    /// Reopens state from stable memory after an upgrade.
    pub fn restore(memory: DefaultMemoryImpl) -> Self {
        let (stores, users) = Stores::init(memory);
        let bytes = load_blob(&stores.heap_mem).expect("heap snapshot missing");
        let snap: HeapSnapshot = candid::decode_one(&bytes).expect("decode heap snapshot");
        assert!(snap.version <= SNAPSHOT_VERSION, "snapshot from a newer version");
        State::from_snapshot(users, stores, snap)
    }

    pub fn new(ledger: Principal, ledger_fee: u64) -> Self {
        Self::new_in(DefaultMemoryImpl::default(), ledger, ledger_fee)
    }

    /// Fresh state on top of `memory` (the canister's stable memory, or a test buffer).
    pub fn new_in(memory: DefaultMemoryImpl, ledger: Principal, ledger_fee: u64) -> Self {
        let (stores, users) = Stores::init(memory);
        let snap = HeapSnapshot {
            version: SNAPSHOT_VERSION,
            ledger,
            ledger_fee,
            tables: BTreeMap::new(),
            next_table_id: 1,
            applied_hand: BTreeMap::new(),
            items: BTreeMap::new(),
            next_item_id: 1,
            chats: BTreeMap::new(),
            next_msg_id: 1,
            treasury: 0,
            next_hand_id: 1,
            throws: BTreeMap::new(),
            next_throw_id: 1,
            rooms: BTreeMap::new(),
            tournaments: BTreeMap::new(),
            next_tournament_id: 1,
            table_tournament: BTreeMap::new(),
            templates: crate::tournament::default_templates(),
            jackpots: BTreeMap::from([(Currency::Chips, crate::jackpot::CHIP_JACKPOT_SEED)]),
            jackpot_hits: VecDeque::new(),
            table_jackpot_hit: BTreeMap::new(),
            next_op_id: 1,
            unresolved: BTreeSet::new(),
            pause: PauseFlags::default(),
            last_solvency: None,
            wallet_total: 0,
            leaders: vec![],
            moderators: BTreeSet::new(),
            banned_words: crate::moderation::default_banned_words(),
            audit_log: VecDeque::new(),
            open_reports: BTreeSet::new(),
            next_report_id: 1,
            counters: Counters::default(),
            signup_hour: 0,
            signup_count: 0,
            signups_per_hour: DEFAULT_SIGNUPS_PER_HOUR,
            signup_pow_bits: crate::signup::DEFAULT_POW_BITS,
            cashouts: BTreeMap::new(),
            auto_sit_outcomes: None,
            rewards: None,
            entropy: vec![],
        };
        let mut s = State::from_snapshot(users, stores, snap);
        for (kind, name, price, chip_price, icon, description) in default_items() {
            s.add_item(StoreItem {
                id: 0,
                name: name.into(),
                kind,
                price,
                chip_price,
                icon: icon.into(),
                description: description.into(),
                available: true,
            });
        }
        for cfg in default_tables() {
            s.create_table(cfg).unwrap();
        }
        s
    }

    // ------------------------------------------------------------ profiles

    /// A copy of a player's record.
    pub(crate) fn user(&self, p: &Principal) -> R<User> {
        self.users.get(p).ok_or_else(|| "Create a profile first".to_string())
    }

    /// A player's record for editing; changes are written back when the guard drops.
    pub(crate) fn user_mut(&mut self, p: &Principal) -> R<UserMut<'_>> {
        self.users.get_mut(p).ok_or_else(|| "Create a profile first".to_string())
    }

    pub fn touch(&mut self, p: &Principal, now: u64) {
        if let Some(mut u) = self.users.get_mut(p) {
            u.last_seen = now;
        }
    }

    fn first_free(&self, kind: ItemKind) -> u32 {
        self.items
            .values()
            .find(|i| i.kind == kind && i.price == 0)
            .map(|i| i.id)
            .unwrap_or(0)
    }

    pub(crate) fn icon(&self, id: u32) -> String {
        self.items.get(&id).map(|i| i.icon.clone()).unwrap_or_default()
    }

    pub fn register(&mut self, caller: Principal, username: String, now: u64) -> R<Profile> {
        if caller == Principal::anonymous() {
            return Err("Please sign in first".into());
        }
        if self.users.contains_key(&caller) {
            return Err("Profile already exists".into());
        }
        validate_username(&username)?;
        if crate::moderation::name_is_offensive(&username, &self.banned_words) {
            return Err("Please pick a different username".into());
        }
        let hour = now / (60 * 60 * 1_000_000_000);
        if self.signup_hour != hour {
            self.signup_hour = hour;
            self.signup_count = 0;
        }
        if self.signups_per_hour == 0 {
            return Err("Sign-ups are closed right now. Please try again later.".into());
        }
        let key = username.to_lowercase();
        if self.stores.usernames.contains_key(&key) {
            return Err("Username is taken".into());
        }
        let avatar = self.first_free(ItemKind::Avatar);
        let card_back = self.first_free(ItemKind::CardBack);
        self.stores.usernames.insert(key, caller);
        self.signup_count += 1;
        self.users.insert(
            caller,
            User {
                username,
                status: String::new(),
                balance: 0,
                chips: SIGNUP_CHIPS,
                settings: Settings::default(),
                history: VecDeque::new(),
                daily_streak: 0,
                last_daily_at: 0,
                last_throw_at: 0,
                medals: BTreeMap::new(),
                gifts_sent: 0,
                throws_made: 0,
                ops: vec![],
                blocked: BTreeSet::new(),
                ban: None,
                chat_muted_until: 0,
                reports_day: 0,
                reports_today: 0,
                avatar,
                card_back,
                inventory: vec![],
                friends: BTreeSet::new(),
                friend_requests: BTreeSet::new(),
                stats: Stats::default(),
                xp: 0,
                gift: None,
                created_at: now,
                last_seen: now,
                last_chat_at: 0,
                rewards: None,
            },
        );
        Ok(self.profile(&caller).unwrap())
    }

    pub fn profile(&self, p: &Principal) -> Option<Profile> {
        let u = self.users.get(p)?;
        Some(Profile {
            id: *p,
            username: u.username.clone(),
            status: u.status.clone(),
            balance: u.balance,
            chips: u.chips,
            settings: u.settings.clone(),
            daily_streak: u.daily_streak,
            next_daily_at: if u.last_daily_at == 0 { 0 } else { u.last_daily_at + DAY_NS },
            next_daily_amount: daily_amount(u.daily_streak + 1),
            avatar: u.avatar,
            card_back: u.card_back,
            inventory: u.inventory.clone(),
            friends: u.friends.iter().copied().collect(),
            friend_requests: u.friend_requests.iter().copied().collect(),
            stats: u.stats.clone(),
            xp: u.xp,
            level: level_for_xp(u.xp),
            created_at: u.created_at,
            medals: u.medals.iter().map(|(id, t)| EarnedMedal { id: id.clone(), earned_at: *t }).collect(),
            blocked: u.blocked.iter().copied().collect(),
            ban: u.ban.clone(),
            chat_muted_until: u.chat_muted_until,
        })
    }

    fn tables_of(&self, p: &Principal) -> Vec<u64> {
        self.tables
            .values()
            .filter(|t| t.seat_of(p).is_some())
            .map(|t| t.id)
            .collect()
    }

    fn is_online(&self, p: &Principal, u: &User, now: u64) -> bool {
        now.saturating_sub(u.last_seen) < ONLINE_WINDOW_NS || !self.tables_of(p).is_empty()
    }

    pub fn public_profile(&self, p: &Principal, now: u64) -> Option<PublicProfile> {
        let u = self.users.get(p)?;
        Some(PublicProfile {
            id: *p,
            username: u.username.clone(),
            status: u.status.clone(),
            avatar: self.icon(u.avatar),
            stats: u.stats.clone(),
            level: level_for_xp(u.xp),
            online: self.is_online(p, &u, now),
            tables: self.tables_of(p),
            medals: u.medals.keys().cloned().collect(),
        })
    }

    pub fn find_user(&self, username: &str) -> Option<Principal> {
        self.stores.usernames.get(&username.to_lowercase())
    }

    pub fn set_status(&mut self, caller: Principal, status: String) -> R<()> {
        if status.chars().count() > 80 {
            return Err("Status is limited to 80 characters".into());
        }
        // Shown publicly on profiles: same rules as chat.
        let status = self.filter_text(&status);
        self.user_mut(&caller)?.status = status;
        Ok(())
    }

    // --------------------------------------------------------------- store

    pub fn add_item(&mut self, mut item: StoreItem) -> u32 {
        item.id = self.next_item_id;
        self.next_item_id += 1;
        let id = item.id;
        self.items.insert(id, item);
        id
    }

    fn owns(&self, u: &User, item: &StoreItem) -> bool {
        (item.price == 0 && item.chip_price == 0) || u.inventory.contains(&item.id)
    }

    /// Charges the caller for an item in the chosen currency.
    /// ICP goes to the house treasury; spent chips simply leave circulation.
    fn pay_for(&mut self, caller: &Principal, item: &StoreItem, currency: Currency) -> R<()> {
        let treasury_cut = match currency {
            Currency::Icp => item.price,
            Currency::Chips => {
                if item.chip_price == 0 {
                    return Err(format!("{} can only be bought with ICP", item.name));
                }
                0
            }
        };
        let price = match currency {
            Currency::Icp => item.price,
            Currency::Chips => item.chip_price,
        };
        self.debit_in(caller, currency, price)?;
        self.treasury += treasury_cut;
        Ok(())
    }

    pub fn buy_item(&mut self, caller: Principal, id: u32, currency: Currency) -> R<()> {
        let item = self.items.get(&id).cloned().ok_or("No such item")?;
        if !item.available {
            return Err("Item is not for sale".into());
        }
        if matches!(item.kind, ItemKind::Gift | ItemKind::Throwable) {
            return Err("Gifts and throwables are used on players at a table".into());
        }
        let owned = self.owns(&self.user(&caller)?, &item);
        if owned {
            return Err("You already own this item".into());
        }
        self.pay_for(&caller, &item, currency)?;
        let mut u = self.user_mut(&caller)?;
        u.inventory.push(id);
        u.xp += 10;
        Ok(())
    }

    pub fn equip(&mut self, caller: Principal, id: u32) -> R<()> {
        let item = self.items.get(&id).cloned().ok_or("No such item")?;
        if !self.owns(&self.user(&caller)?, &item) {
            return Err("You don't own this item".into());
        }
        let mut u = self.user_mut(&caller)?;
        match item.kind {
            ItemKind::Avatar => u.avatar = id,
            ItemKind::CardBack => u.card_back = id,
            ItemKind::Gift | ItemKind::Throwable => return Err("That item can't be equipped".into()),
        }
        Ok(())
    }

    /// Looks up an item of `kind` and the names/seats of sender and recipient at a table.
    fn table_item_target(
        &self,
        caller: &Principal,
        table_id: u64,
        to: &Principal,
        item_id: u32,
        kind: ItemKind,
    ) -> R<(StoreItem, String, String, Option<usize>, usize)> {
        let item = self.items.get(&item_id).cloned().ok_or("No such item")?;
        if item.kind != kind || !item.available {
            return Err("That item can't be used here".into());
        }
        if self.either_blocked(caller, to) {
            return Err("You can't send that to this player".into());
        }
        if !self.can_watch(table_id, Some(*caller)) {
            return Err("This table is private".into());
        }
        let t = self.tables.get(&table_id).ok_or("No such table")?;
        let to_seat = t.seat_of(to).ok_or("That player is not at this table")?;
        let from_seat = t.seat_of(caller);
        let from_name = self.user(caller)?.username.clone();
        let to_name = self.user(to)?.username.clone();
        Ok((item, from_name, to_name, from_seat, to_seat))
    }

    pub fn send_gift(&mut self, caller: Principal, table_id: u64, to: Principal, item_id: u32, currency: Currency, now: u64) -> R<()> {
        let (item, from_name, to_name, _, _) = self.table_item_target(&caller, table_id, &to, item_id, ItemKind::Gift)?;
        self.pay_for(&caller, &item, currency)?;
        let gifts_sent = {
            let mut u = self.user_mut(&caller)?;
            u.xp += 5;
            u.gifts_sent += 1;
            u.gifts_sent
        };
        if gifts_sent >= 10 {
            self.award(&caller, "generous", Some(Channel::Table(table_id)), now);
        }
        self.user_mut(&to)?.gift = Some(ActiveGift {
            item: item_id,
            from: from_name.clone(),
            expires_at: now + GIFT_DURATION_NS,
        });
        self.post(
            Channel::Table(table_id),
            Some(caller),
            from_name.clone(),
            format!("{} sent {} a {} {}", from_name, to_name, item.name, item.icon),
            MessageKind::Gift,
            now,
        );
        Ok(())
    }

    /// Throws an item from the caller's seat at another player (animated on every client).
    pub fn throw_item(&mut self, caller: Principal, table_id: u64, to: Principal, item_id: u32, currency: Currency, now: u64) -> R<()> {
        let (item, from_name, to_name, from_seat, to_seat) =
            self.table_item_target(&caller, table_id, &to, item_id, ItemKind::Throwable)?;
        let from_seat = from_seat.ok_or("Sit down to throw things")?;
        if caller == to {
            return Err("You can't throw things at yourself".into());
        }
        if now.saturating_sub(self.user(&caller)?.last_throw_at) < THROW_COOLDOWN_NS {
            return Err("Wait a moment before throwing again".into());
        }
        self.pay_for(&caller, &item, currency)?;
        let throws_made = {
            let mut u = self.user_mut(&caller)?;
            u.last_throw_at = now;
            u.throws_made += 1;
            u.throws_made
        };
        if throws_made >= 25 {
            self.award(&caller, "food_fight", Some(Channel::Table(table_id)), now);
        }
        let id = self.next_throw_id;
        self.next_throw_id += 1;
        let q = self.throws.entry(table_id).or_default();
        q.push_back(ThrowEvent {
            id,
            from_seat: from_seat as u8,
            to_seat: to_seat as u8,
            icon: item.icon.clone(),
            time: now,
        });
        while q.front().is_some_and(|e| e.time + THROW_VISIBLE_NS < now) || q.len() > 20 {
            q.pop_front();
        }
        self.post(
            Channel::Table(table_id),
            Some(caller),
            from_name.clone(),
            format!("{} threw a {} {} at {}", from_name, item.name, item.icon, to_name),
            MessageKind::Throw,
            now,
        );
        Ok(())
    }

    // -------------------------------------------------------------- wallet

    pub fn credit(&mut self, p: &Principal, amount: u64) -> R<()> {
        self.credit_in(p, Currency::Icp, amount)
    }

    pub fn debit(&mut self, p: &Principal, amount: u64) -> R<()> {
        self.debit_in(p, Currency::Icp, amount)
    }

    fn purse(u: &mut User, currency: Currency) -> &mut u64 {
        match currency {
            Currency::Icp => &mut u.balance,
            Currency::Chips => &mut u.chips,
        }
    }

    /// Every change to a player's ICP or chip balance goes through these two,
    /// which also keep `wallet_total` (the sum of ICP balances) exact.
    pub fn credit_in(&mut self, p: &Principal, currency: Currency, amount: u64) -> R<()> {
        {
            let mut u = self.user_mut(p)?;
            let bal = Self::purse(&mut u, currency);
            *bal = bal.checked_add(amount).ok_or("Overflow")?;
        }
        if currency == Currency::Icp {
            self.wallet_total += amount;
        }
        Ok(())
    }

    pub fn debit_in(&mut self, p: &Principal, currency: Currency, amount: u64) -> R<()> {
        {
            let mut u = self.user_mut(p)?;
            let bal = Self::purse(&mut u, currency);
            if *bal < amount {
                return Err(match currency {
                    Currency::Icp => "Insufficient ICP balance".into(),
                    Currency::Chips => "Not enough chips".into(),
                });
            }
            *bal -= amount;
        }
        if currency == Currency::Icp {
            self.wallet_total -= amount;
        }
        Ok(())
    }

    /// Claims the daily free-chip bonus. Claiming on consecutive days grows the bonus.
    pub fn claim_daily(&mut self, caller: Principal, now: u64) -> R<u64> {
        let (amount, streak) = {
            let mut u = self.user_mut(&caller)?;
            if u.last_daily_at != 0 && now < u.last_daily_at + DAY_NS {
                let hours = (u.last_daily_at + DAY_NS - now).div_ceil(DAY_NS / 24);
                return Err(format!("Your next bonus is ready in {hours}h"));
            }
            u.daily_streak = if u.last_daily_at != 0 && now < u.last_daily_at + 2 * DAY_NS {
                u.daily_streak + 1
            } else {
                1
            };
            u.last_daily_at = now;
            let amount = daily_amount(u.daily_streak);
            u.chips += amount;
            (amount, u.daily_streak)
        };
        if streak >= MAX_STREAK {
            self.award(&caller, "dedicated", None, now);
        }
        Ok(amount)
    }

    /// Saves (or, when both text and color are empty, deletes) a private note about a player.
    pub fn set_note(&mut self, caller: Principal, player: Principal, text: String, color: Option<NoteColor>, now: u64) -> R<()> {
        if caller == player {
            return Err("You can't keep notes on yourself".into());
        }
        self.user(&player).map_err(|_| "No such player".to_string())?;
        let text = crate::moderation::clean_text(&text, true);
        if text.chars().count() > MAX_NOTE_LEN {
            return Err(format!("Notes are limited to {MAX_NOTE_LEN} characters"));
        }
        self.user(&caller)?;
        let key = (caller, player);
        if text.is_empty() && color.is_none() {
            self.stores.notes.remove(&key);
            return Ok(());
        }
        if !self.stores.notes.contains_key(&key) && self.note_count(caller) >= MAX_NOTES {
            return Err(format!("You can keep up to {MAX_NOTES} notes"));
        }
        self.stores.notes.insert(key, PlayerNote { text, color, updated_at: now });
        Ok(())
    }

    /// The caller's notes, by subject (a range scan over keys starting with the caller).
    fn notes_of(&self, caller: Principal) -> impl Iterator<Item = (Principal, PlayerNote)> + '_ {
        self.stores
            .notes
            .range((caller, Principal::management_canister())..)
            .map(|e| e.into_pair())
            .take_while(move |((author, _), _)| *author == caller)
            .map(|((_, subject), n)| (subject, n))
    }

    fn note_count(&self, caller: Principal) -> usize {
        self.notes_of(caller).count()
    }

    pub fn notes(&self, caller: Principal) -> Vec<NoteView> {
        let mut v: Vec<NoteView> = self
            .notes_of(caller)
            .map(|(p, n)| {
                let u = self.users.get(&p);
                NoteView {
                    player: p,
                    username: u.as_ref().map(|u| u.username.clone()).unwrap_or_default(),
                    avatar: u.as_ref().map(|u| self.icon(u.avatar)).unwrap_or_default(),
                    text: n.text.clone(),
                    color: n.color,
                    updated_at: n.updated_at,
                }
            })
            .collect();
        v.sort_by_key(|n| std::cmp::Reverse(n.updated_at));
        v
    }

    pub fn update_settings(&mut self, caller: Principal, settings: Settings) -> R<()> {
        self.user_mut(&caller)?.settings = settings;
        Ok(())
    }

    /// Full scan of every user's ICP balance (admin audit of `wallet_total`).
    pub fn recount_wallet_total(&self) -> u64 {
        self.users.iter().map(|(_, u)| u.balance).sum()
    }

    pub fn house_stats(&self) -> HouseStats {
        let wallet_balances = self.wallet_total;
        // Play-chip tables are not backed by ICP.
        let chips_on_tables: u64 = self
            .tables
            .values()
            .filter(|t| t.config.currency == Currency::Icp)
            .map(|t| t.chips_in_play() + t.rake_accrued + t.jackpot_accrued)
            .sum();
        let rewards_pools = self.rewards.as_ref().map_or(0, |r| r.pools_total());
        HouseStats {
            users: self.users.len() as u64,
            wallet_balances,
            chips_on_tables,
            treasury: self.treasury,
            jackpot: self.jackpot(Currency::Icp),
            liabilities_total: wallet_balances
                + chips_on_tables
                + self.treasury
                + self.icp_in_tournaments()
                + self.jackpot(Currency::Icp)
                + rewards_pools,
            rewards_pools,
        }
    }

    // ---------------------------------------------------------------- chat

    pub(crate) fn post(&mut self, ch: Channel, from: Option<Principal>, username: String, text: String, kind: MessageKind, now: u64) {
        let id = self.next_msg_id;
        self.next_msg_id += 1;
        let q = self.chats.entry(ch).or_default();
        q.push_back(ChatMessage {
            id,
            from,
            username,
            text,
            time: now,
            kind,
        });
        while q.len() > CHAT_HISTORY {
            q.pop_front();
        }
    }

    fn check_chat(&mut self, caller: &Principal, text: &str, now: u64) -> R<String> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > MAX_CHAT_LEN {
            return Err(format!("Messages are 1-{MAX_CHAT_LEN} characters"));
        }
        let mut u = self.user_mut(caller)?;
        if now.saturating_sub(u.last_chat_at) < CHAT_COOLDOWN_NS {
            return Err("Slow down a little".into());
        }
        u.last_chat_at = now;
        Ok(u.username.clone())
    }

    pub fn send_chat(&mut self, caller: Principal, ch: Channel, text: String, now: u64) -> R<()> {
        if let Channel::Table(id) = ch {
            if !self.tables.contains_key(&id) {
                return Err("No such table".into());
            }
            if !self.can_watch(id, Some(caller)) {
                return Err("This table is private".into());
            }
        }
        self.check_can_chat(&caller, now)?;
        let name = self.check_chat(&caller, &text, now)?;
        let text = self.filter_text(text.trim());
        self.post(ch, Some(caller), name, text, MessageKind::Text, now);
        Ok(())
    }

    /// Chat as `viewer` sees it: messages from players they've blocked are hidden.
    pub fn chat_for(&self, viewer: Principal, ch: Channel, since: u64) -> Vec<ChatMessage> {
        if let Channel::Table(id) = ch {
            if !self.can_watch(id, Some(viewer)) {
                return vec![];
            }
        }
        let blocked = self.users.get(&viewer).map(|u| u.blocked).unwrap_or_default();
        let mut msgs = self.chat_since(ch, since);
        if !blocked.is_empty() {
            msgs.retain(|m| m.from.is_none_or(|f| !blocked.contains(&f)));
        }
        msgs
    }

    pub fn chat_since(&self, ch: Channel, since: u64) -> Vec<ChatMessage> {
        self.chats
            .get(&ch)
            .map(|q| q.iter().filter(|m| m.id > since).cloned().collect())
            .unwrap_or_default()
    }

    pub fn send_dm(&mut self, caller: Principal, to: Principal, text: String, now: u64) -> R<()> {
        if !self.user(&caller)?.friends.contains(&to) || self.either_blocked(&caller, &to) {
            return Err("You can only message friends".into());
        }
        self.check_can_chat(&caller, now)?;
        let name = self.check_chat(&caller, &text, now)?;
        let text = self.filter_text(&text);
        let id = self.next_msg_id;
        self.next_msg_id += 1;
        let key = dm_key(caller, to);
        let mut thread = self.stores.dms.get(&key).unwrap_or_default();
        thread.0.push(ChatMessage {
            id,
            from: Some(caller),
            username: name,
            text: text.trim().to_string(),
            time: now,
            kind: MessageKind::Text,
        });
        let excess = thread.0.len().saturating_sub(DM_HISTORY);
        thread.0.drain(..excess);
        self.stores.dms.insert(key, thread);
        Ok(())
    }

    pub fn dms_with(&self, caller: Principal, other: Principal, since: u64) -> Vec<ChatMessage> {
        self.stores
            .dms
            .get(&dm_key(caller, other))
            .map(|t: DmThread| t.0.into_iter().filter(|m| m.id > since).collect())
            .unwrap_or_default()
    }

    // ------------------------------------------------------------- friends

    pub fn friend_request(&mut self, caller: Principal, to: Principal) -> R<()> {
        if caller == to {
            return Err("You can't friend yourself".into());
        }
        self.user(&caller)?;
        let theirs = self.user(&to).map_err(|_| "No such player".to_string())?;
        if theirs.friends.contains(&caller) {
            return Err("Already friends".into());
        }
        if self.either_blocked(&caller, &to) {
            return Err("You can't send a friend request to this player".into());
        }
        if self.user(&caller)?.friends.len() >= MAX_FRIENDS || theirs.friends.len() >= MAX_FRIENDS {
            return Err(format!("Friend lists are limited to {MAX_FRIENDS} players"));
        }
        // If they already asked us, accept straight away.
        if self.user(&caller)?.friend_requests.contains(&to) {
            return self.respond_friend(caller, to, true);
        }
        if theirs.friend_requests.len() >= MAX_FRIEND_REQUESTS {
            return Err("That player has too many pending friend requests".into());
        }
        self.user_mut(&to)?.friend_requests.insert(caller);
        Ok(())
    }

    pub fn respond_friend(&mut self, caller: Principal, from: Principal, accept: bool) -> R<()> {
        if !self.user_mut(&caller)?.friend_requests.remove(&from) {
            return Err("No such friend request".into());
        }
        if accept {
            self.user_mut(&caller)?.friends.insert(from);
            self.user_mut(&from)?.friends.insert(caller);
            for p in [caller, from] {
                if self.users.get(&p).is_some_and(|u| u.friends.len() >= 10) {
                    self.award(&p, "popular", None, 0);
                }
            }
        }
        Ok(())
    }

    pub fn remove_friend(&mut self, caller: Principal, other: Principal) -> R<()> {
        self.user_mut(&caller)?.friends.remove(&other);
        if let Ok(mut u) = self.user_mut(&other) {
            u.friends.remove(&caller);
        }
        Ok(())
    }

    pub fn friends(&self, caller: Principal, now: u64) -> Vec<FriendInfo> {
        let Ok(me) = self.user(&caller) else {
            return vec![];
        };
        me.friends
            .iter()
            .filter_map(|f| {
                let u = self.users.get(f)?;
                Some(FriendInfo {
                    id: *f,
                    username: u.username.clone(),
                    avatar: self.icon(u.avatar),
                    online: self.is_online(f, &u, now),
                    tables: self.tables_of(f),
                })
            })
            .collect()
    }

    pub fn leaderboard(&self, limit: usize) -> Vec<LeaderboardEntry> {
        self.leaders
            .iter()
            .take(limit)
            .filter_map(|(p, _)| {
                let u = self.users.get(p)?;
                Some(LeaderboardEntry {
                    id: *p,
                    username: u.username.clone(),
                    avatar: self.icon(u.avatar),
                    level: level_for_xp(u.xp),
                    stats: u.stats.clone(),
                })
            })
            .collect()
    }

    /// Keeps the top-`LEADERS` list current as a player's net winnings change.
    fn update_leader(&mut self, p: Principal, net: i64) {
        self.leaders.retain(|(q, _)| *q != p);
        self.leaders.push((p, net));
        self.leaders.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        self.leaders.truncate(LEADERS);
    }

    // -------------------------------------------------------------- tables

    pub fn create_table(&mut self, cfg: TableConfig) -> R<u64> {
        cfg.validate()?;
        let id = self.next_table_id;
        self.next_table_id += 1;
        self.tables.insert(id, Table::new(id, cfg));
        Ok(id)
    }

    /// The game a table belongs to: its original's id, or its own id.
    pub fn game_of(&self, table: u64) -> u64 {
        self.tables.get(&table).and_then(|t| t.game).unwrap_or(table)
    }

    /// Tables of a game (original first, then copies by number), never tournament tables.
    pub fn game_tables(&self, game: u64) -> Vec<u64> {
        let mut ids: Vec<(u8, u64)> = self
            .tables
            .values()
            .filter(|t| !self.table_tournament.contains_key(&t.id) && t.game.unwrap_or(t.id) == game)
            .map(|t| (t.copy_number.unwrap_or(1), t.id))
            .collect();
        ids.sort();
        ids.into_iter().map(|(_, id)| id).collect()
    }

    /// An original cash table that may have copies: not a room, a copy or a tournament table.
    fn is_copyable_game(&self, game: u64) -> bool {
        self.tables.get(&game).is_some_and(|t| t.game.is_none())
            && !self.rooms.contains_key(&game)
            && !self.table_tournament.contains_key(&game)
    }

    fn can_open_copy(&self, game: u64) -> bool {
        if !self.is_copyable_game(game) {
            return false;
        }
        let currency = self.tables[&game].config.currency;
        let copies = self.tables.values().filter(|t| t.game.is_some());
        let (mut of_game, mut of_currency) = (0, 0);
        for t in copies {
            of_game += usize::from(t.game == Some(game));
            of_currency += usize::from(t.config.currency == currency);
        }
        let budget = match currency {
            Currency::Chips => MAX_CHIP_COPIES,
            Currency::Icp => MAX_ICP_COPIES,
        };
        of_game < MAX_COPIES_PER_GAME && of_currency < budget
    }

    /// Opens another table of a full game, with the same settings.
    pub fn open_copy(&mut self, game: u64, now: u64) -> R<u64> {
        if !self.is_copyable_game(game) {
            return Err("This table can't have copies".into());
        }
        let cfg = self.tables[&game].config.clone();
        if !self.can_open_copy(game) {
            return Err(format!("{} is full right now. Watch a table and turn on auto-sit to get the next seat.", cfg.name));
        }
        let used: BTreeSet<u8> = self.tables.values().filter(|t| t.game == Some(game)).filter_map(|t| t.copy_number).collect();
        let number = (2..=u8::MAX).find(|n| !used.contains(n)).ok_or("No free table number")?;
        let id = self.create_table(cfg)?;
        let t = self.tables.get_mut(&id).unwrap();
        t.game = Some(game);
        t.copy_number = Some(number);
        t.empty_since = Some(now);
        Ok(id)
    }

    /// Closes copies that have been empty for `COPY_IDLE_CLOSE_NS`.
    fn close_empty_copies(&mut self, now: u64) {
        let mut done = vec![];
        for t in self.tables.values_mut().filter(|t| t.game.is_some()) {
            // Seats of players who left mid-hand still hold chips until the hand ends.
            let empty = t.seats.iter().all(Option::is_none) && !t.phase.is_betting();
            if !empty {
                t.empty_since = None;
            } else if now.saturating_sub(*t.empty_since.get_or_insert(now)) >= COPY_IDLE_CLOSE_NS {
                done.push(t.id);
            }
        }
        for id in done {
            self.remove_table(id);
        }
    }

    /// Creates a player-hosted table. ICP rooms pay the host a share of the rake.
    pub fn create_room(&mut self, caller: Principal, args: RoomArgs, now: u64) -> R<u64> {
        if self.pause.joins {
            return Err(self.paused_msg("New tables"));
        }
        self.check_not_banned(&caller, now)?;
        // Room names are listed in the lobby.
        let clean = crate::moderation::clean_text(&args.name, false);
        if crate::moderation::mask_words(&clean, &self.banned_words) != clean {
            return Err("Please pick a different table name".into());
        }
        if level_for_xp(self.user(&caller)?.xp) < HOST_MIN_LEVEL {
            return Err(format!("Reach level {HOST_MIN_LEVEL} to host your own table"));
        }
        if self.rooms.len() >= MAX_ROOMS {
            return Err("All hosted tables are in use. Please try again later.".into());
        }
        let owned = self.rooms.values().filter(|r| r.owner == caller).count();
        if owned >= MAX_ROOMS_PER_USER {
            return Err(format!("You can have at most {MAX_ROOMS_PER_USER} open tables"));
        }
        let bb = args.big_blind;
        let (min_bb, max_bb) = match args.currency {
            Currency::Chips => (2, 10_000),
            Currency::Icp => (2_000, 10_000_000),
        };
        if !bb.is_multiple_of(2) || bb < min_bb || bb > max_bb {
            return Err(format!("Big blind must be an even amount between {} and {}", fmt_amount(args.currency, min_bb), fmt_amount(args.currency, max_bb)));
        }
        if let Some(pw) = &args.password {
            if pw.is_empty() || pw.len() > 32 {
                return Err("Passwords are 1-32 characters".into());
            }
        }
        let icp = args.currency == Currency::Icp;
        let cfg = TableConfig {
            name: crate::moderation::clean_text(&args.name, false),
            currency: args.currency,
            small_blind: bb / 2,
            big_blind: bb,
            min_buy_in: bb * 20,
            max_buy_in: bb * 100,
            max_seats: args.max_seats,
            rake_bps: if icp { 250 } else { 0 },
            rake_cap: if icp { bb * 3 } else { 0 },
            jackpot_bps: JACKPOT_DROP_BPS,
            jackpot_cap: bb,
            action_timeout_secs: args.action_timeout_secs,
        };
        let id = self.create_table(cfg)?;
        self.rooms.insert(
            id,
            Room {
                owner: caller,
                password_hash: args.password.map(|pw| room_password_hash(id, &pw)),
                created_at: now,
                last_active: now,
                guests: BTreeSet::new(),
            },
        );
        self.award(&caller, "host", None, now);
        Ok(id)
    }

    /// Closes the caller's room once nobody is seated.
    pub fn close_room(&mut self, caller: Principal, id: u64) -> R<()> {
        let room = self.rooms.get(&id).ok_or("Not a player-created table")?;
        if room.owner != caller {
            return Err("Only the host can close this table".into());
        }
        if self.tables.get(&id).is_some_and(|t| t.player_count() > 0) {
            return Err("Wait until everyone has left".into());
        }
        self.remove_table(id);
        Ok(())
    }

    fn remove_table(&mut self, id: u64) {
        self.rooms.remove(&id);
        self.table_jackpot_hit.remove(&id);
        self.tables.remove(&id);
        self.applied_hand.remove(&id);
        self.throws.remove(&id);
        self.chats.remove(&Channel::Table(id));
    }

    fn close_idle_rooms(&mut self, now: u64) {
        let idle: Vec<u64> = self
            .rooms
            .iter()
            .filter(|(id, r)| {
                now.saturating_sub(r.last_active) > ROOM_IDLE_CLOSE_NS && self.tables.get(id).is_none_or(|t| t.player_count() == 0)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in idle {
            self.remove_table(id);
        }
    }

    pub(crate) fn table_mut(&mut self, id: u64) -> R<&mut Table> {
        self.tables.get_mut(&id).ok_or_else(|| "No such table".to_string())
    }

    /// Currency of a cash table. Tournament tables can't be joined or left directly.
    fn table_currency(&self, id: u64) -> R<Currency> {
        if self.table_tournament.contains_key(&id) {
            return Err("This is a tournament table. Register from the Tournaments page; sit out if you need a break".into());
        }
        Ok(self.tables.get(&id).ok_or("No such table")?.config.currency)
    }

    pub fn join_table(&mut self, caller: Principal, id: u64, seat: u8, buy_in: u64, password: Option<String>, now: u64) -> R<()> {
        if self.pause.joins {
            return Err(self.paused_msg("New seats"));
        }
        self.check_not_banned(&caller, now)?;
        let cash_seats = self
            .tables
            .values()
            .filter(|t| !self.table_tournament.contains_key(&t.id) && t.seat_of(&caller).is_some())
            .count();
        if cash_seats >= MAX_CASH_SEATS {
            return Err(format!("You can play at most {MAX_CASH_SEATS} cash tables at once"));
        }
        let name = self.user(&caller)?.username.clone();
        let currency = self.table_currency(id)?;
        if !self.can_watch(id, Some(caller)) {
            self.unlock_room(caller, id, password.unwrap_or_default())?;
        }
        // Every refusal comes before the debit, so a refused join never takes money.
        let required = self.min_return_buy_in(id, &caller, now);
        if buy_in < required {
            return Err(format!(
                "You left this table less than 30 minutes ago with {}: buy back in for at least that much",
                fmt_amount(currency, required)
            ));
        }
        // Check funds before seating so a failed debit can't leave chips on the table.
        self.debit_in(&caller, currency, buy_in)?;
        if let Err(e) = self.table_mut(id)?.sit_with_minimum(caller, seat, buy_in, required) {
            self.credit_in(&caller, currency, buy_in)?;
            return Err(e);
        }
        self.post(Channel::Table(id), None, String::new(), format!("{name} sat down"), MessageKind::System, now);
        // Sitting down yourself takes you out of this table's line.
        self.leave_auto_sit_at(&caller, id);
        Ok(())
    }

    fn leave_auto_sit_at(&mut self, p: &Principal, table: u64) {
        if let Some(q) = self.tables.get_mut(&table).and_then(|t| t.auto_sit.as_mut()) {
            q.retain(|e| e.player != *p);
        }
    }

    fn queue_status(&self, p: &Principal, table: u64) -> AutoSitStatus {
        let Some(q) = self.tables.get(&table).and_then(|t| t.auto_sit.as_ref()) else {
            return AutoSitStatus::NotQueued;
        };
        match q.iter().position(|e| e.player == *p) {
            Some(i) => AutoSitStatus::Queued { place: i as u32 + 1, waiting: q.len() as u32, buy_in: q[i].buy_in },
            None => AutoSitStatus::NotQueued,
        }
    }

    /// Joins (with a buy-in) or leaves (`None`) a table's auto-sit line. Joining
    /// moves you out of any other line.
    pub fn auto_sit(&mut self, caller: Principal, table: u64, buy_in: Option<u64>, now: u64) -> R<AutoSitStatus> {
        let Some(buy_in) = buy_in else {
            for t in self.tables.values_mut() {
                if let Some(q) = t.auto_sit.as_mut() {
                    q.retain(|e| e.player != caller);
                }
            }
            return Ok(AutoSitStatus::NotQueued);
        };
        if self.pause.joins {
            return Err(self.paused_msg("New seats"));
        }
        let currency = self.table_currency(table)?;
        self.check_not_banned(&caller, now)?;
        if !self.can_watch(table, Some(caller)) {
            return Err("Enter the table password first".into());
        }
        let t = &self.tables[&table];
        if t.seat_of(&caller).is_some() {
            return Err("You're already at this table".into());
        }
        let (min, max) = (t.config.min_buy_in, t.config.max_buy_in);
        if buy_in < min || buy_in > max {
            return Err(format!("Buy in between {} and {}", fmt_amount(currency, min), fmt_amount(currency, max)));
        }
        let required = self.min_return_buy_in(table, &caller, now);
        if buy_in < required {
            return Err(format!(
                "You left this game less than 30 minutes ago with {}: buy back in for at least that much",
                fmt_amount(currency, required)
            ));
        }
        let u = self.user(&caller)?;
        let have = match currency {
            Currency::Chips => u.chips,
            Currency::Icp => u.balance,
        };
        if have < buy_in {
            return Err("Not enough funds for that buy-in".into());
        }
        // A result from an earlier line is old news now.
        if let Some(m) = self.auto_sit_outcomes.as_mut() {
            m.remove(&caller);
        }
        // Out of every other line; keep this line's place if already in it.
        for t in self.tables.values_mut().filter(|t| t.id != table) {
            if let Some(q) = t.auto_sit.as_mut() {
                q.retain(|e| e.player != caller);
            }
        }
        let q = self.tables.get_mut(&table).unwrap().auto_sit.get_or_insert_with(VecDeque::new);
        match q.iter_mut().find(|e| e.player == caller) {
            Some(e) => {
                e.buy_in = buy_in;
                e.last_seen = now;
            }
            None => {
                if q.len() >= AUTO_SIT_MAX_QUEUE {
                    return Err("The line for this table is full".into());
                }
                q.push_back(AutoSitEntry { player: caller, buy_in, last_seen: now });
            }
        }
        Ok(self.queue_status(&caller, table))
    }

    /// Keeps the caller in a table's line while they watch, and reports what
    /// happened since the last check: seated, dropped, or their place.
    pub fn auto_sit_ping(&mut self, caller: Principal, table: u64, now: u64) -> AutoSitStatus {
        if let Some(outcome) = self.auto_sit_outcomes.as_mut().and_then(|m| m.remove(&caller)) {
            return outcome;
        }
        let entry = self
            .tables
            .get_mut(&table)
            .and_then(|t| t.auto_sit.as_mut())
            .and_then(|q| q.iter_mut().find(|e| e.player == caller));
        if let Some(e) = entry {
            e.last_seen = now;
        }
        self.queue_status(&caller, table)
    }

    /// Seats waiting players at free seats, first in line first. Players whose
    /// browser hasn't checked in recently are skipped, then dropped.
    fn seat_from_queues(&mut self, now: u64) {
        let ids: Vec<u64> = self
            .tables
            .values()
            .filter(|t| t.auto_sit.as_ref().is_some_and(|q| !q.is_empty()))
            .map(|t| t.id)
            .collect();
        for id in ids {
            if let Some(q) = self.tables.get_mut(&id).and_then(|t| t.auto_sit.as_mut()) {
                q.retain(|e| now.saturating_sub(e.last_seen) <= AUTO_SIT_EXPIRE_NS);
            }
            loop {
                let Some(t) = self.tables.get_mut(&id) else { break };
                let Some(seat) = t.first_free_seat() else { break };
                let Some(q) = t.auto_sit.as_mut() else { break };
                let Some(i) = q.iter().position(|e| now.saturating_sub(e.last_seen) <= AUTO_SIT_LIVE_NS) else {
                    break;
                };
                let e = q.remove(i).unwrap();
                let outcome = match self.join_table(e.player, id, seat, e.buy_in, None, now) {
                    Ok(()) => AutoSitStatus::Seated { table: id },
                    Err(reason) => AutoSitStatus::Dropped { reason },
                };
                self.auto_sit_outcomes.get_or_insert_with(BTreeMap::new).insert(e.player, outcome);
            }
        }
    }

    /// Tables of a game the caller could sit at now: a free seat, not already
    /// seated there. Fullest first, then by table number, so tables fill up
    /// before new ones open.
    pub(crate) fn join_candidates(&self, caller: &Principal, game: u64) -> Vec<u64> {
        let mut c: Vec<(std::cmp::Reverse<usize>, u8, u64)> = self
            .game_tables(game)
            .into_iter()
            .map(|id| &self.tables[&id])
            // A table with a line keeps its free seats for the line.
            .filter(|t| t.first_free_seat().is_some() && t.seat_of(caller).is_none() && t.auto_sit.as_ref().is_none_or(|q| q.is_empty()))
            .map(|t| (std::cmp::Reverse(t.player_count()), t.copy_number.unwrap_or(1), t.id))
            .collect();
        c.sort();
        c.into_iter().map(|(_, _, id)| id).collect()
    }

    /// Joins a game rather than a table: a free seat at its fullest table, or a
    /// new copy when all of them are full. Returns the table the caller sat at.
    pub fn join_game(&mut self, caller: Principal, game: u64, buy_in: u64, now: u64) -> R<u64> {
        if !self.is_copyable_game(game) {
            return Err("Join this table directly".into());
        }
        // Cheap checks first, so a caller who can't sit can't open copies either.
        if self.pause.joins {
            return Err(self.paused_msg("New seats"));
        }
        self.check_not_banned(&caller, now)?;
        let cash_seats = self
            .tables
            .values()
            .filter(|t| !self.table_tournament.contains_key(&t.id) && t.seat_of(&caller).is_some())
            .count();
        if cash_seats >= MAX_CASH_SEATS {
            return Err(format!("You can play at most {MAX_CASH_SEATS} cash tables at once"));
        }
        let cfg = &self.tables[&game].config;
        if buy_in < cfg.min_buy_in || buy_in > cfg.max_buy_in {
            return Err(format!(
                "Buy in between {} and {}",
                fmt_amount(cfg.currency, cfg.min_buy_in),
                fmt_amount(cfg.currency, cfg.max_buy_in)
            ));
        }
        let currency = cfg.currency;
        let required = self.min_return_buy_in(game, &caller, now);
        if buy_in < required {
            return Err(format!(
                "You left this game less than 30 minutes ago with {}: buy back in for at least that much",
                fmt_amount(currency, required)
            ));
        }
        let u = self.user(&caller)?;
        let have = match currency {
            Currency::Chips => u.chips,
            Currency::Icp => u.balance,
        };
        if have < buy_in {
            return Err("Not enough funds for that buy-in".into());
        }
        let id = match self.join_candidates(&caller, game).first() {
            Some(id) => *id,
            None => self.open_copy(game, now)?,
        };
        let seat = self.tables[&id].first_free_seat().ok_or("Seat is taken")?;
        self.join_table(caller, id, seat, buy_in, None, now)?;
        Ok(id)
    }

    pub fn leave_table(&mut self, caller: Principal, id: u64, now: u64) -> R<u64> {
        let currency = self.table_currency(id)?;
        let chips = self.table_mut(id)?.stand(&caller, now)?;
        self.credit_in(&caller, currency, chips)?;
        if !self.table_tournament.contains_key(&id) {
            // Per game: leaving another table of it can't lower what you must bring back.
            let key = (self.game_of(id), caller);
            let kept = self.cashouts.get(&key).filter(|(_, at)| now < at + RETURN_WINDOW_NS).map_or(0, |(c, _)| *c);
            self.cashouts.insert(key, (chips.max(kept), now));
        }
        let name = self.user(&caller)?.username.clone();
        self.post(Channel::Table(id), None, String::new(), format!("{name} left the table"), MessageKind::System, now);
        self.after_table_change(id, now);
        Ok(chips)
    }

    pub fn top_up(&mut self, caller: Principal, id: u64, amount: u64) -> R<()> {
        let currency = self.table_currency(id)?;
        self.debit_in(&caller, currency, amount)?;
        if let Err(e) = self.table_mut(id)?.top_up(&caller, amount) {
            self.credit_in(&caller, currency, amount)?;
            return Err(e);
        }
        Ok(())
    }

    /// Mixes fresh randomness (from `raw_rand`) into the state's secret entropy.
    pub fn mix_entropy(&mut self, fresh: &[u8]) {
        let mut h = Sha256::new();
        h.update(&self.entropy);
        h.update(fresh);
        self.entropy = h.finalize().to_vec();
    }

    pub(crate) fn entropy(&self) -> &[u8] {
        &self.entropy
    }

    /// The smallest buy-in `p` may sit down with at table `id` right now: what
    /// they left with if they left within the last 30 minutes, else 0.
    pub fn min_return_buy_in(&self, id: u64, p: &Principal, now: u64) -> u64 {
        self.cashouts.get(&(self.game_of(id), *p)).filter(|(_, at)| now < at + RETURN_WINDOW_NS).map_or(0, |(chips, _)| *chips)
    }

    pub fn sit_out(&mut self, caller: Principal, id: u64, out: bool, now: u64) -> R<()> {
        if !out {
            // Suspended players are sat out of tournaments and must stay out.
            self.check_not_banned(&caller, now)?;
        }
        self.table_mut(id)?.set_sitting_out(&caller, out, now)
    }

    /// Shows the caller's cards after winning a hand without a showdown, at the
    /// table and in the hand history of everyone dealt in.
    pub fn show_cards(&mut self, caller: Principal, id: u64, _now: u64) -> R<()> {
        let t = self.table_mut(id)?;
        let seat = t.show_cards(&caller)? as u8;
        let hand_no = t.last_result.as_ref().map_or(0, |r| r.hand_no);
        let shows = t.log.last().cloned();
        // The hand was already recorded when it ended: mark it shown there too.
        let latest = self.users.get(&caller).and_then(|u| u.history.back().copied());
        if let Some(hid) = latest {
            if let Some(mut rec) = self.stores.hands.get(&hid).filter(|r| r.table_id == id && r.hand_no == hand_no) {
                if let Some(pl) = rec.players.iter_mut().find(|x| x.seat == seat) {
                    pl.shown = true;
                }
                rec.log.extend(shows);
                self.stores.hands.insert(hid, rec);
            }
        }
        Ok(())
    }

    pub fn act(&mut self, caller: Principal, id: u64, action: Action, now: u64) -> R<()> {
        self.check_not_banned(&caller, now)?;
        self.table_mut(id)?.act(&caller, action, now)?;
        self.after_table_change(id, now);
        Ok(())
    }

    /// After a hand ends: applies auto-muck, records the hand history, updates
    /// stats, sweeps rake and runs auto top-ups. Idempotent per hand.
    pub fn after_table_change(&mut self, id: u64, now: u64) {
        let Some(t) = self.tables.get_mut(&id) else {
            return;
        };
        let currency = t.config.currency;
        let rake = std::mem::take(&mut t.rake_accrued);
        let drop = std::mem::take(&mut t.jackpot_accrued);
        *self.jackpots.entry(currency).or_default() += drop;
        if currency == Currency::Icp && rake > 0 {
            // Room hosts earn a share of the rake their table generates.
            let host = self.rooms.get(&id).map(|r| r.owner);
            let host_cut = if host.is_some() { rake * HOST_RAKE_SHARE_BPS / 10_000 } else { 0 };
            if let Some(h) = host {
                if self.credit(&h, host_cut).is_err() {
                    self.treasury += host_cut;
                }
            }
            self.fund_rewards(rake - host_cut);
        }
        if let Some(r) = self.rooms.get_mut(&id) {
            r.last_active = now;
        }
        let in_tournament = self.table_tournament.contains_key(&id);
        let Some(t) = self.tables.get(&id) else {
            return;
        };
        if t.phase != Phase::HandOver {
            return;
        }
        let Some(result) = t.last_result.clone() else {
            return;
        };
        if self.applied_hand.get(&id) == Some(&result.hand_no) {
            return;
        }
        self.applied_hand.insert(id, result.hand_no);

        // Auto-muck: losing hands of players who opted in are not shown.
        let won_by = |p: &Principal| result.players.iter().find(|r| r.player == *p).map_or(0, |r| r.won);
        let muckers: Vec<Principal> = result
            .shown
            .iter()
            .filter(|sh| won_by(&sh.player) == 0)
            .filter(|sh| self.users.get(&sh.player).is_some_and(|u| u.settings.auto_muck))
            .map(|sh| sh.player)
            .collect();
        let t = self.tables.get_mut(&id).unwrap();
        if let Some(r) = t.last_result.as_mut() {
            r.shown.retain(|sh| !muckers.contains(&sh.player));
        }
        let shown: Vec<u8> = t.last_result.as_ref().map_or(vec![], |r| r.shown.iter().map(|sh| sh.seat).collect());

        self.record_hand(id, &result, &shown, now);

        let mut leader_updates = vec![];
        for pr in &result.players {
            if let Some(mut u) = self.users.get_mut(&pr.player) {
                u.stats.hands_played += 1;
                u.xp += 1;
                let net = pr.won as i64 - pr.committed as i64;
                match currency {
                    // Tournament chips have no value outside the event.
                    _ if in_tournament => {}
                    Currency::Icp => {
                        u.stats.net_winnings += net;
                        if pr.won > 0 {
                            u.stats.biggest_pot_won = u.stats.biggest_pot_won.max(pr.won);
                        }
                    }
                    Currency::Chips => u.stats.chip_net += net,
                }
                if pr.won > 0 {
                    u.stats.hands_won += 1;
                    u.xp += 2;
                }
                leader_updates.push((pr.player, u.stats.net_winnings));
            }
        }
        for (p, net) in leader_updates {
            self.update_leader(p, net);
        }
        self.apply_hand_rewards(id, currency, in_tournament, &result, now);
        for w in &result.winners {
            let name = self.users.get(&w.player).map(|u| u.username.clone()).unwrap_or_default();
            let text = if in_tournament {
                format!("{} wins {} chips ({})", name, w.amount, w.hand)
            } else {
                format!("{} wins {} ({})", name, fmt_amount(currency, w.amount), w.hand)
            };
            self.post(Channel::Table(id), None, String::new(), text, MessageKind::System, now);
        }
        // Medals only see hands that were actually shown, so a mucked hand's
        // strength never leaks through a "Bad Beat" announcement.
        let mut shown_result = result.clone();
        shown_result.shown.retain(|sh| !muckers.contains(&sh.player));
        self.award_hand_medals(id, (!in_tournament).then_some(currency), &shown_result, now);
        if !in_tournament {
            // The jackpot judges the real hands: a qualifying hand is always revealed (see the rules).
            self.check_bad_beat(id, &result, now);
        }
        if in_tournament {
            self.after_tournament_hand(id, now);
        } else {
            self.auto_top_up(id);
        }
    }

    /// Fish Buffet points and leaderboard counters for a finished hand.
    /// - ICP cash: the rake is attributed by contribution and earns points;
    ///   spins are staked with the house's part (after a room host's cut).
    /// - ICP tournaments: the hand counts toward qualifying, with no net.
    /// - Chip tables and chip tournaments: nothing.
    fn apply_hand_rewards(&mut self, id: u64, currency: Currency, in_tournament: bool, result: &HandResult, now: u64) {
        if in_tournament {
            let icp_event = self
                .table_tournament
                .get(&id)
                .and_then(|tid| self.tournaments.get(tid))
                .is_some_and(|t| t.template.currency == Currency::Icp);
            if icp_event {
                for pr in &result.players {
                    self.record_icp_hand(&pr.player, 0, now);
                }
            }
            return;
        }
        if currency != Currency::Icp {
            return;
        }
        let hosted = self.rooms.contains_key(&id);
        let committed: Vec<(Principal, u64)> = result.players.iter().map(|r| (r.player, r.committed)).collect();
        let shares = crate::rewards::attribute(result.rake, &committed);
        for (pr, (who, share)) in result.players.iter().zip(shares) {
            if share > 0 {
                let house = if hosted { share - share * HOST_RAKE_SHARE_BPS / 10_000 } else { share };
                self.earn_rake(&who, share, house, now);
            }
            self.record_icp_hand(&who, pr.won as i64 - pr.committed as i64, now);
        }
    }

    fn record_hand(&mut self, table_id: u64, result: &HandResult, shown: &[u8], now: u64) {
        let t = &self.tables[&table_id];
        let players: Vec<HandPlayer> = t
            .dealt_hands()
            .into_iter()
            .map(|(seat, player, cards)| {
                let r = result.players.iter().find(|r| r.player == player);
                HandPlayer {
                    seat,
                    player,
                    username: self.users.get(&player).map(|u| u.username.clone()).unwrap_or_default(),
                    cards,
                    shown: shown.contains(&seat),
                    committed: r.map_or(0, |r| r.committed),
                    won: r.map_or(0, |r| r.won),
                }
            })
            .collect();
        let id = self.next_hand_id;
        self.next_hand_id += 1;
        let record = HandRecord {
            id,
            table_id,
            table_name: t.config.name.clone(),
            currency: t.config.currency,
            small_blind: t.config.small_blind,
            big_blind: t.config.big_blind,
            hand_no: result.hand_no,
            time: now,
            button: t.button as u8,
            board: result.board.clone(),
            pot: result.pot,
            rake: result.rake,
            players,
            // Drop the showdown line of anyone who mucked.
            log: t
                .log
                .iter()
                .filter(|e| !(e.text.starts_with("Shows") && e.seat.is_some_and(|seat| !shown.contains(&seat))))
                .cloned()
                .collect(),
        };
        for p in &record.players {
            if let Some(mut u) = self.users.get_mut(&p.player) {
                u.history.push_back(id);
                while u.history.len() > HISTORY_PER_USER {
                    u.history.pop_front();
                }
            }
        }
        self.stores.hands.insert(id, record);
        while self.stores.hands.len() > HISTORY_MAX as u64 {
            self.stores.hands.pop_first();
        }
    }

    /// Refills stacks of players with auto top-up on when they drop below half the max buy-in.
    fn auto_top_up(&mut self, id: u64) {
        let t = &self.tables[&id];
        let currency = t.config.currency;
        let max = t.config.max_buy_in;
        let wants: Vec<(Principal, u64)> = t
            .seats
            .iter()
            .flatten()
            .filter(|s| !s.left && s.stack < max / 2)
            .filter(|s| self.users.get(&s.player).is_some_and(|u| u.settings.auto_top_up))
            .map(|s| (s.player, max - s.stack))
            .collect();
        for (p, want) in wants {
            let available = self.users.get(&p).map_or(0, |u| match currency {
                Currency::Icp => u.balance,
                Currency::Chips => u.chips,
            });
            let amount = want.min(available);
            if amount > 0 && self.debit_in(&p, currency, amount).is_ok() {
                let t = self.tables.get_mut(&id).unwrap();
                if t.top_up(&p, amount).is_err() {
                    let _ = self.credit_in(&p, currency, amount);
                }
            }
        }
    }

    pub fn hand_history(&self, caller: Principal, limit: usize) -> Vec<HandSummary> {
        let Some(u) = self.users.get(&caller) else {
            return vec![];
        };
        u.history
            .iter()
            .rev()
            .filter_map(|id| self.stores.hands.get(id))
            .take(limit)
            .map(|h| {
                let me = h.players.iter().find(|p| p.player == caller);
                HandSummary {
                    id: h.id,
                    table_name: h.table_name.clone(),
                    currency: h.currency,
                    time: h.time,
                    board: h.board.clone(),
                    my_cards: me.map(|p| p.cards.clone()).unwrap_or_default(),
                    pot: h.pot,
                    net: me.map_or(0, |p| p.won as i64 - p.committed as i64),
                }
            })
            .collect()
    }

    /// A hand as seen by `caller`: only their own and shown hole cards are revealed.
    /// Only players who were dealt in can view it.
    pub fn hand(&self, caller: Principal, id: u64) -> Option<HandRecord> {
        let mut h = self.stores.hands.get(&id)?;
        if !h.players.iter().any(|p| p.player == caller) {
            return None;
        }
        for p in h.players.iter_mut() {
            if p.player != caller && !p.shown {
                p.cards.clear();
            }
        }
        Some(h)
    }

    /// Times out slow players and reports tables that need a new hand dealt.
    pub fn tick(&mut self, now: u64) -> Vec<u64> {
        self.close_periods(now);
        self.tournament_tick(now);
        self.close_idle_rooms(now);
        self.close_empty_copies(now);
        self.seat_from_queues(now);
        self.stand_up_idle(now);
        self.cashouts.retain(|_, (_, at)| now < *at + RETURN_WINDOW_NS);
        let ids: Vec<u64> = self.tables.keys().copied().collect();
        let mut ready = vec![];
        for id in ids {
            let t = self.tables.get_mut(&id).unwrap();
            if let Some(p) = t.handle_timeout(now) {
                let name = self.users.get(&p).map(|u| u.username.clone()).unwrap_or_default();
                self.post(Channel::Table(id), None, String::new(), format!("{name} timed out and is sitting out"), MessageKind::System, now);
                self.after_table_change(id, now);
            }
            let t = self.tables.get_mut(&id).unwrap();
            if t.ready_for_next_hand(now) {
                if t.eligible_count() >= 2 {
                    // While dealing is paused, hands in progress finish but no new ones start.
                    if !self.pause.dealing {
                        ready.push(id);
                    }
                } else if t.phase == Phase::HandOver {
                    t.settle_after_hand();
                }
            }
        }
        ready
    }

    /// Stands up players who have been sitting out at a cash table for
    /// `IDLE_STAND_NS`, returning their chips to their wallet.
    fn stand_up_idle(&mut self, now: u64) {
        let cutoff = now.saturating_sub(IDLE_STAND_NS);
        let idle: Vec<(u64, Principal)> = self
            .tables
            .values()
            .filter(|t| !self.table_tournament.contains_key(&t.id))
            .flat_map(|t| t.idle_players(cutoff).into_iter().map(move |p| (t.id, p)))
            .collect();
        for (id, p) in idle {
            if self.leave_table(p, id, now).is_ok() {
                self.counters.idle_stand_ups += 1;
            }
        }
    }

    pub fn start_hand(&mut self, id: u64, seed: [u8; 32], now: u64) {
        if let Some(t) = self.tables.get_mut(&id) {
            if t.ready_for_next_hand(now) && t.start_hand(seed, now).is_ok() {
                self.counters.hands_dealt += 1;
                // Blinds can put players all-in and end the hand immediately.
                self.after_table_change(id, now);
            }
        }
    }

    pub fn list_tables(&self) -> Vec<TableSummary> {
        self.tables
            .values()
            .filter(|t| !self.table_tournament.contains_key(&t.id))
            .map(|t| TableSummary {
                id: t.id,
                game: t.game.unwrap_or(t.id),
                number: t.copy_number.unwrap_or(1),
                game_at_cap: !self.can_open_copy(t.game.unwrap_or(t.id)),
                name: t.config.name.clone(),
                currency: t.config.currency,
                small_blind: t.config.small_blind,
                big_blind: t.config.big_blind,
                min_buy_in: t.config.min_buy_in,
                max_buy_in: t.config.max_buy_in,
                players: t.player_count() as u8,
                max_seats: t.config.max_seats,
                phase: t.phase,
                avg_pot: t.last_result.as_ref().map_or(0, |r| r.pot),
                room_owner: self
                    .rooms
                    .get(&t.id)
                    .map(|r| self.users.get(&r.owner).map(|u| u.username.clone()).unwrap_or_default()),
                locked: self.rooms.get(&t.id).is_some_and(|r| r.password_hash.is_some()),
            })
            .collect()
    }

    pub fn table_view(&self, id: u64, viewer: Option<Principal>, now: u64) -> Option<TableView> {
        let view = self.full_table_view(id, viewer, now)?;
        if self.can_watch(id, viewer) {
            return Some(view);
        }
        // A private room seen from outside: who sits where (to pick a seat), nothing about the game.
        let seats = view
            .seats
            .into_iter()
            .map(|s| {
                s.map(|s| SeatView { stack: 0, bet: 0, cards: vec![], has_cards: false, last_action: None, gift: None, gift_from: None, gift_sent_at: None, in_hand: false, folded: false, all_in: false, ..s })
            })
            .collect();
        Some(TableView {
            seats,
            board: vec![],
            pot: 0,
            current_bet: 0,
            to_act: None,
            last_result: None,
            throws: vec![],
            jackpot_hit: None,
            restricted: true,
            ..view
        })
    }

    fn full_table_view(&self, id: u64, viewer: Option<Principal>, now: u64) -> Option<TableView> {
        let t = self.tables.get(&id)?;
        let my_seat = viewer.and_then(|v| t.seat_of(&v));
        let seats = t
            .seats
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let s = s.as_ref().filter(|s| !s.left)?;
                let u = self.users.get(&s.player);
                let gift = u.as_ref().and_then(|u| u.gift.as_ref()).filter(|g| g.expires_at > now);
                Some(SeatView {
                    seat: i as u8,
                    player: s.player,
                    username: u.as_ref().map(|u| u.username.clone()).unwrap_or_default(),
                    avatar: u.as_ref().map(|u| self.icon(u.avatar)).unwrap_or_default(),
                    card_back: u.as_ref().map(|u| self.icon(u.card_back)).unwrap_or_default(),
                    level: u.as_ref().map_or(1, |u| level_for_xp(u.xp)),
                    stack: s.stack,
                    bet: s.bet,
                    in_hand: s.in_hand,
                    folded: s.folded,
                    all_in: s.all_in,
                    sitting_out: s.sitting_out,
                    has_cards: s.in_hand && !s.folded && !s.hole.is_empty(),
                    cards: t.visible_cards(i, viewer.as_ref()),
                    last_action: s.last_action.clone(),
                    gift: gift.map(|g| self.icon(g.item)),
                    gift_from: gift.map(|g| g.from.clone()),
                    gift_sent_at: gift.map(|g| g.expires_at.saturating_sub(GIFT_DURATION_NS)),
                    note: viewer.and_then(|v| self.stores.notes.get(&(v, s.player))),
                    time_bank_secs: if t.time_bank_active && t.to_act == Some(i) {
                        t.action_deadline.saturating_sub(now) / 1_000_000_000
                    } else {
                        s.time_bank / 1_000_000_000
                    } as u32,
                })
            })
            .collect();
        let (to_call, min_raise_to, max_raise_to) = match (my_seat, t.to_act) {
            (Some(m), Some(a)) if m == a => t.betting_options(m),
            _ => (0, 0, 0),
        };
        let pot = if t.phase.is_betting() {
            t.pot()
        } else {
            t.last_result.as_ref().map_or(0, |r| r.pot)
        };
        Some(TableView {
            id,
            config: t.config.clone(),
            game: t.game.unwrap_or(id),
            number: t.copy_number.unwrap_or(1),
            my_auto_sit: viewer.and_then(|v| {
                let q = t.auto_sit.as_ref()?;
                let i = q.iter().position(|e| e.player == v)?;
                Some(AutoSitView { place: i as u32 + 1, buy_in: q[i].buy_in })
            }),
            auto_sit_waiting: t.auto_sit.as_ref().map_or(0, |q| q.len() as u32),
            phase: t.phase,
            hand_no: t.hand_no,
            seats,
            board: t.board.clone(),
            pot,
            current_bet: t.current_bet,
            button: t.button as u8,
            to_act: t.to_act.map(|i| i as u8),
            action_deadline: t.action_deadline,
            time_bank_active: t.time_bank_active,
            now,
            last_result: t.last_result.clone(),
            my_seat: my_seat.map(|i| i as u8),
            to_call,
            min_raise_to,
            max_raise_to,
            throws: self
                .throws
                .get(&id)
                .map(|q| q.iter().filter(|e| e.time + THROW_VISIBLE_NS > now).cloned().collect())
                .unwrap_or_default(),
            tournament: self.table_tournament.get(&id).and_then(|tid| self.tournament_badge(*tid, viewer)),
            room_owner: self.rooms.get(&id).map(|r| r.owner),
            locked: self.rooms.get(&id).is_some_and(|r| r.password_hash.is_some()),
            jackpot: if t.config.jackpot_bps > 0 { self.jackpot(t.config.currency) } else { 0 },
            jackpot_hit: self.current_hit(id),
            restricted: false,
            my_min_buy_in: viewer.map_or(0, |v| self.min_return_buy_in(id, &v, now)),
        })
    }
}

impl State {
    /// Password-protected rooms are private: only the host, players seated
    /// there and players who entered the password can watch or chat.
    pub fn can_watch(&self, id: u64, viewer: Option<Principal>) -> bool {
        let Some(room) = self.rooms.get(&id).filter(|r| r.password_hash.is_some()) else {
            return true;
        };
        viewer.is_some_and(|v| room.owner == v || room.guests.contains(&v) || self.tables.get(&id).is_some_and(|t| t.seat_of(&v).is_some()))
    }

    /// Enters a room's password, to watch it (or sit down later).
    pub fn unlock_room(&mut self, caller: Principal, id: u64, password: String) -> R<()> {
        let room = self.rooms.get_mut(&id).ok_or("No such table")?;
        let Some(hash) = &room.password_hash else { return Ok(()) };
        if password.len() > 32 || room_password_hash(id, &password) != *hash {
            return Err("Wrong table password".into());
        }
        if room.guests.len() >= MAX_ROOM_GUESTS && !room.guests.contains(&caller) {
            return Err("This table has too many guests".into());
        }
        room.guests.insert(caller);
        Ok(())
    }
}

fn room_password_hash(table_id: u64, password: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(table_id.to_le_bytes());
    h.update(password.as_bytes());
    h.finalize().to_vec()
}

pub fn fmt_amount(currency: Currency, amount: u64) -> String {
    match currency {
        Currency::Icp => format!("{} ICP", fmt_icp(amount)),
        Currency::Chips => format!("{amount} chips"),
    }
}

pub fn fmt_icp(e8s: u64) -> String {
    let s = format!("{}.{:08}", e8s / E8S, e8s % E8S);
    let s = s.trim_end_matches('0');
    s.trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEC: u64 = 1_000_000_000;

    fn p(n: u8) -> Principal {
        Principal::from_slice(&[n; 10])
    }

    fn setup() -> State {
        let mut s = State::new(Principal::management_canister(), 10_000);
        for k in 1..=3 {
            s.register(p(k), format!("player{k}"), 0).unwrap();
            s.credit(&p(k), 10 * E8S).unwrap();
        }
        s
    }

    fn total_value(s: &State) -> u64 {
        s.house_stats().liabilities_total
    }

    /// Everything the house holds as its own or earmarked: treasury plus reward pools.
    fn house_take(s: &State) -> u64 {
        s.treasury + s.house_stats().rewards_pools
    }

    #[test]
    fn registration_rules() {
        let mut s = setup();
        assert!(s.register(p(9), "player1".into(), 0).is_err()); // taken
        assert!(s.register(p(9), "PLAYER1".into(), 0).is_err()); // case-insensitive
        assert!(s.register(p(9), "x".into(), 0).is_err());
        assert!(s.register(Principal::anonymous(), "anon".into(), 0).is_err());
        assert!(s.register(p(9), "new_guy".into(), 0).is_ok());
        assert_eq!(s.find_user("NEW_GUY"), Some(p(9)));
    }

    #[test]
    fn store_and_gifts() {
        let mut s = setup();
        let ninja = s.items.values().find(|i| i.name == "Ninja").unwrap().id;
        assert!(s.equip(p(1), ninja).is_err());
        s.buy_item(p(1), ninja, Currency::Icp).unwrap();
        assert!(s.buy_item(p(1), ninja, Currency::Icp).is_err());
        s.equip(p(1), ninja).unwrap();
        assert_eq!(s.users.get(&p(1)).unwrap().balance, 10 * E8S - E8S / 10);
        assert_eq!(s.treasury, E8S / 10);

        let beer = s.items.values().find(|i| i.name == "Beer").unwrap().id;
        let icp_table = 5; // Penny Lane
        assert!(s.send_gift(p(1), icp_table, p(2), beer, Currency::Icp, 0).is_err()); // p2 not seated
        s.join_table(p(2), icp_table, 0, 1_000_000, None, 0).unwrap();
        s.send_gift(p(1), icp_table, p(2), beer, Currency::Icp, 0).unwrap();
        let v = s.table_view(icp_table, None, 1).unwrap();
        assert_eq!(v.seats[0].as_ref().unwrap().gift.as_deref(), Some("🍺"));
        assert_eq!(v.seats[0].as_ref().unwrap().gift_from.as_deref(), Some("player1"));
        assert_eq!(v.seats[0].as_ref().unwrap().gift_sent_at, Some(0));
        // A repeat of the same gift is told apart by its sent time.
        s.send_gift(p(1), icp_table, p(2), beer, Currency::Icp, 5 * SEC).unwrap();
        let v = s.table_view(icp_table, None, 6 * SEC).unwrap();
        assert_eq!(v.seats[0].as_ref().unwrap().gift.as_deref(), Some("🍺"));
        assert_eq!(v.seats[0].as_ref().unwrap().gift_sent_at, Some(5 * SEC));
        // Gifts expire lazily: they simply stop being shown.
        let v = s.table_view(icp_table, None, 5 * SEC + GIFT_DURATION_NS + 1).unwrap();
        let seat = v.seats[0].as_ref().unwrap();
        assert!(seat.gift.is_none() && seat.gift_from.is_none() && seat.gift_sent_at.is_none());
    }

    #[test]
    fn friends_and_dms() {
        let mut s = setup();
        assert!(s.send_dm(p(1), p(2), "hi".into(), 0).is_err());
        s.friend_request(p(1), p(2)).unwrap();
        s.respond_friend(p(2), p(1), true).unwrap();
        assert_eq!(s.friends(p(1), 0).len(), 1);
        s.send_dm(p(1), p(2), "hi".into(), 10 * SEC).unwrap();
        assert_eq!(s.dms_with(p(2), p(1), 0).len(), 1);
        s.remove_friend(p(2), p(1)).unwrap();
        assert!(s.friends(p(1), 0).is_empty());
    }

    #[test]
    fn chat_rate_limit_and_history() {
        let mut s = setup();
        s.send_chat(p(1), Channel::Lobby, "gl all".into(), 10 * SEC).unwrap();
        assert!(s.send_chat(p(1), Channel::Lobby, "spam".into(), 10 * SEC + 1).is_err());
        assert!(s.send_chat(p(1), Channel::Lobby, "  ".into(), 20 * SEC).is_err());
        let msgs = s.chat_since(Channel::Lobby, 0);
        assert_eq!(msgs.len(), 1);
        assert!(s.chat_since(Channel::Lobby, msgs[0].id).is_empty());
    }

    #[test]
    fn full_game_flow_conserves_value() {
        let mut s = setup();
        let before = total_value(&s);
        let table = 7; // Low Rollers: 0.001/0.002 ICP
        let buy = s.tables[&table].config.max_buy_in;
        s.join_table(p(1), table, 0, buy, None, 0).unwrap();
        s.join_table(p(2), table, 3, buy, None, 0).unwrap();
        s.join_table(p(3), table, 5, buy, None, 0).unwrap();
        assert!(s.join_table(p(1), table, 1, buy, None, 0).is_err());
        assert_eq!(total_value(&s), before);

        let mut now = 1;
        for hand in 0..20u8 {
            let ready = s.tick(now);
            assert_eq!(ready, vec![table], "hand {hand}");
            s.start_hand(table, [hand; 32], now);
            while s.tables[&table].phase.is_betting() {
                let v = s.tables[&table].clone();
                let actor = v.seats[v.to_act.unwrap()].as_ref().unwrap().player;
                let action = if hand % 3 == 0 { Action::Call } else { Action::Check };
                if s.act(actor, table, action, now).is_err() {
                    s.act(actor, table, Action::Call, now).unwrap();
                }
            }
            assert_eq!(total_value(&s), before);
            now += 6_000_000_000;
            // Busted players get topped back up.
            for k in 1..=3 {
                if s.tables[&table].seat_of(&p(k)).is_none() {
                    let seat = (0..9).find(|&i| s.tables[&table].seats[i].is_none()).unwrap();
                    s.join_table(p(k), table, seat as u8, buy, None, now).unwrap();
                }
            }
        }
        let played: u64 = (1..=3).map(|k| s.users.get(&p(k)).unwrap().stats.hands_played).sum();
        assert_eq!(played, 60);
        let net: i64 = (1..=3).map(|k| s.users.get(&p(k)).unwrap().stats.net_winnings).sum();
        // Players' combined losses are exactly the rake plus the jackpot drop.
        assert_eq!(net, -((house_take(&s) + s.jackpot(Currency::Icp)) as i64));
        assert!(s.jackpot(Currency::Icp) > 0, "cash pots feed the jackpot");

        // Hole cards are private until showdown.
        let v = s.table_view(table, Some(p(1)), now).unwrap();
        for seat in v.seats.iter().flatten() {
            if seat.player != p(1) && v.phase.is_betting() {
                assert!(seat.cards.is_empty());
            }
        }

        for k in 1..=3 {
            s.leave_table(p(k), table, now).unwrap();
        }
        assert_eq!(s.tables[&table].player_count(), 0);
        assert_eq!(total_value(&s), before);
        let wallets: u64 = (1..=3).map(|k| s.users.get(&p(k)).unwrap().balance).sum();
        assert_eq!(wallets + house_take(&s) + s.jackpot(Currency::Icp), 30 * E8S);
    }

    fn play_hands(s: &mut State, table: u64, hands: u8, start: u64) -> u64 {
        let mut now = start;
        for hand in 0..hands {
            s.tick(now);
            s.start_hand(table, [hand.wrapping_add(start as u8); 32], now);
            while s.tables[&table].phase.is_betting() {
                let t = &s.tables[&table];
                let actor = t.seats[t.to_act.unwrap()].as_ref().unwrap().player;
                if s.act(actor, table, Action::Check, now).is_err() {
                    s.act(actor, table, Action::Call, now).unwrap();
                }
            }
            now += 6 * SEC;
        }
        now
    }

    #[test]
    fn play_chips_are_separate_from_icp() {
        let mut s = setup();
        assert_eq!(s.users.get(&p(1)).unwrap().chips, SIGNUP_CHIPS);
        let chip_table = 1; // Free Roll Fun, 5/10 chips
        assert_eq!(s.tables[&chip_table].config.currency, Currency::Chips);
        let before = s.house_stats();
        s.join_table(p(1), chip_table, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), chip_table, 1, 1_000, None, 0).unwrap();
        // ICP is untouched and chip tables don't count as ICP liabilities.
        assert_eq!(s.users.get(&p(1)).unwrap().balance, 10 * E8S);
        assert_eq!(s.users.get(&p(1)).unwrap().chips, SIGNUP_CHIPS - 1_000);
        assert_eq!(s.house_stats().liabilities_total, before.liabilities_total);
        let now = play_hands(&mut s, chip_table, 5, SEC);
        assert_eq!(s.treasury, 0, "chip tables are rake free");
        s.leave_table(p(1), chip_table, now).unwrap();
        s.leave_table(p(2), chip_table, now).unwrap();
        let chips: u64 = (1..=2).map(|k| s.users.get(&p(k)).unwrap().chips).sum();
        assert_eq!(chips, 2 * SIGNUP_CHIPS);
        assert_eq!(s.users.get(&p(1)).unwrap().stats.chip_net + s.users.get(&p(2)).unwrap().stats.chip_net, 0);
        assert_eq!(s.users.get(&p(1)).unwrap().stats.net_winnings, 0);
        // Not enough chips to sit at the 250/500 table (min buy-in 10,000 is fine, 50,000 is not).
        assert!(s.join_table(p(3), 4, 0, 50_000, None, now).is_err());
        assert_eq!(s.users.get(&p(3)).unwrap().chips, SIGNUP_CHIPS);
    }

    #[test]
    fn daily_bonus_streak() {
        let mut s = setup();
        let day = DAY_NS;
        assert_eq!(s.claim_daily(p(1), day).unwrap(), DAILY_CHIPS);
        assert!(s.claim_daily(p(1), day + day / 2).is_err());
        assert_eq!(s.claim_daily(p(1), 2 * day + 1).unwrap(), 2 * DAILY_CHIPS);
        assert_eq!(s.profile(&p(1)).unwrap().next_daily_amount, 3 * DAILY_CHIPS);
        // Missing a day resets the streak.
        assert_eq!(s.claim_daily(p(1), 5 * day).unwrap(), DAILY_CHIPS);
        assert_eq!(s.users.get(&p(1)).unwrap().chips, SIGNUP_CHIPS + 4 * DAILY_CHIPS);
    }

    #[test]
    fn buying_with_chips() {
        let mut s = setup();
        let cowboy = s.items.values().find(|i| i.name == "Cowboy").unwrap().id;
        let crown = s.items.values().find(|i| i.name == "Crown").unwrap().id;
        assert!(s.buy_item(p(1), cowboy, Currency::Chips).is_err()); // 100k chips needed
        s.users.get_mut(&p(1)).unwrap().chips = 200_000;
        s.buy_item(p(1), cowboy, Currency::Chips).unwrap();
        assert_eq!(s.users.get(&p(1)).unwrap().chips, 100_000);
        assert!(s.buy_item(p(1), crown, Currency::Chips).is_err()); // ICP only
        assert_eq!(s.treasury, 0);
    }

    #[test]
    fn throwables() {
        let mut s = setup();
        let tomato = s.items.values().find(|i| i.name == "Tomato").unwrap().id;
        let table = 1;
        s.join_table(p(1), table, 0, 1_000, None, 0).unwrap();
        assert!(s.throw_item(p(1), table, p(2), tomato, Currency::Chips, SEC).is_err()); // target not seated
        s.join_table(p(2), table, 4, 1_000, None, 0).unwrap();
        assert!(s.throw_item(p(3), table, p(2), tomato, Currency::Chips, SEC).is_err()); // thrower not seated
        assert!(s.throw_item(p(1), table, p(1), tomato, Currency::Chips, SEC).is_err());
        s.throw_item(p(1), table, p(2), tomato, Currency::Chips, 10 * SEC).unwrap();
        assert!(s.throw_item(p(1), table, p(2), tomato, Currency::Chips, 11 * SEC).is_err()); // cooldown
        let v = s.table_view(table, None, 12 * SEC).unwrap();
        assert_eq!(v.throws.len(), 1);
        assert_eq!((v.throws[0].from_seat, v.throws[0].to_seat), (0, 4));
        assert!(s.table_view(table, None, 30 * SEC).unwrap().throws.is_empty());
        assert_eq!(s.users.get(&p(1)).unwrap().chips, SIGNUP_CHIPS - 1_000 - 200);
    }

    #[test]
    fn hand_history_is_private_and_respects_muck() {
        let mut s = setup();
        let table = 1;
        s.join_table(p(1), table, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), table, 1, 1_000, None, 0).unwrap();
        // p1 shows losing hands (the default), p2 mucks them.
        let muck = Settings { auto_muck: true, ..Settings::default() };
        s.update_settings(p(2), muck).unwrap();
        play_hands(&mut s, table, 6, SEC);

        let hist = s.hand_history(p(1), 50);
        assert_eq!(hist.len(), 6);
        assert!(s.hand_history(p(3), 50).is_empty());
        for h in &hist {
            assert_eq!(h.my_cards.len(), 2);
            assert!(s.hand(p(3), h.id).is_none(), "outsiders can't see hands");
            let rec = s.hand(p(1), h.id).unwrap();
            assert!(!rec.log.is_empty());
            let opp = rec.players.iter().find(|x| x.player == p(2)).unwrap();
            if opp.won == 0 {
                assert!(opp.cards.is_empty(), "losing opponent mucked");
                assert!(!rec.log.iter().any(|e| e.seat == Some(1) && e.text.starts_with("Shows")));
            }
            let me = rec.players.iter().find(|x| x.player == p(1)).unwrap();
            assert_eq!(me.cards.len(), 2);
        }
        // A hand p1 lost at showdown: p2 (the winner) still sees p1's cards.
        let lost = hist.iter().find(|h| h.net < 0 && h.board.len() == 5).expect("p1 lost a showdown");
        let rec = s.hand(p(2), lost.id).unwrap();
        assert_eq!(rec.players.iter().find(|x| x.player == p(1)).unwrap().cards.len(), 2);
    }

    #[test]
    fn losing_hands_are_shown_at_showdown_by_default() {
        let mut s = setup();
        let table = 1;
        s.join_table(p(1), table, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), table, 1, 1_000, None, 0).unwrap();
        let mut now = SEC;
        // Check down hands until one has a loser (not a split pot).
        let (result, loser) = loop {
            now = play_hands(&mut s, table, 1, now);
            let r = s.table_view(table, Some(p(3)), now).unwrap().last_result.unwrap();
            if let Some(l) = r.players.iter().find(|x| x.won == 0) {
                break (r.clone(), l.player);
            }
        };
        assert!(result.shown.iter().any(|sh| sh.player == loser), "the losing hand is revealed");
    }

    /// Plays one hand at `table` where the first player to act folds; returns (winner, folder).
    fn fold_hand(s: &mut State, table: u64, now: u64) -> (Principal, Principal) {
        s.tick(now);
        s.start_hand(table, [7; 32], now);
        let t = &s.tables[&table];
        let folder = t.seats[t.to_act.unwrap()].as_ref().unwrap().player;
        s.act(folder, table, Action::Fold, now).unwrap();
        let winner = s.tables[&table].last_result.as_ref().unwrap().winners[0].player;
        (winner, folder)
    }

    #[test]
    fn uncontested_winner_can_show_cards() {
        let mut s = setup();
        let table = 1;
        s.join_table(p(1), table, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), table, 1, 1_000, None, 0).unwrap();
        let (winner, folder) = fold_hand(&mut s, table, SEC);
        let seat = s.tables[&table].seat_of(&winner).unwrap();
        let hand_id = s.hand_history(folder, 1)[0].id;
        let cards_seen_by_folder = |s: &State| {
            s.hand(folder, hand_id).unwrap().players.iter().find(|x| x.player == winner).unwrap().cards.len()
        };
        assert_eq!(s.table_view(table, Some(p(3)), SEC).unwrap().seats[seat].as_ref().unwrap().cards.len(), 0);
        assert_eq!(cards_seen_by_folder(&s), 0);

        assert!(s.show_cards(folder, table, SEC).is_err(), "only the winner can show");
        s.show_cards(winner, table, SEC).unwrap();

        let view = s.table_view(table, Some(p(3)), SEC).unwrap();
        assert_eq!(view.seats[seat].as_ref().unwrap().cards.len(), 2, "watchers see the shown cards");
        assert!(view.last_result.unwrap().shown.iter().any(|sh| sh.player == winner));
        assert_eq!(cards_seen_by_folder(&s), 2, "the hand history reveals them too");
        let rec = s.hand(folder, hand_id).unwrap();
        assert!(rec.log.iter().any(|e| e.seat == Some(seat as u8) && e.text.starts_with("Shows")));
        assert!(s.show_cards(winner, table, SEC).is_err(), "cards can only be shown once");

        // Once the next hand is dealt, the moment has passed.
        let (winner, _) = fold_hand(&mut s, table, 10 * SEC);
        s.tick(20 * SEC);
        s.start_hand(table, [8; 32], 20 * SEC);
        assert!(s.show_cards(winner, table, 20 * SEC).is_err());
    }

    #[test]
    fn showdown_hands_cannot_be_shown_again() {
        let mut s = setup();
        let table = 1;
        s.join_table(p(1), table, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), table, 1, 1_000, None, 0).unwrap();
        play_hands(&mut s, table, 1, SEC);
        let winner = s.tables[&table].last_result.as_ref().unwrap().winners[0].player;
        assert!(s.show_cards(winner, table, 2 * SEC).is_err(), "only for hands won without a showdown");
    }

    #[test]
    fn auto_top_up_refills_stack() {
        let mut s = setup();
        let table = 1; // max buy-in 1,000 chips
        let st = Settings { auto_top_up: true, ..Settings::default() };
        s.update_settings(p(1), st).unwrap();
        s.join_table(p(1), table, 0, 200, None, 0).unwrap();
        s.join_table(p(2), table, 1, 1_000, None, 0).unwrap();
        play_hands(&mut s, table, 1, SEC);
        let stack = s.tables[&table].seats[0].as_ref().unwrap().stack;
        assert_eq!(stack, 1_000);
        let total = s.users.get(&p(1)).unwrap().chips + stack;
        assert_eq!(total as i64, SIGNUP_CHIPS as i64 + s.users.get(&p(1)).unwrap().stats.chip_net);
    }

    /// Drives every table: deals ready tables and plays aggressively until `done`.
    fn run_until(s: &mut State, mut now: u64, max_rounds: u32, done: impl Fn(&State) -> bool) -> u64 {
        let mut k: u64 = 7;
        for round in 0..max_rounds {
            if done(s) {
                return now;
            }
            for id in s.tick(now) {
                s.start_hand(id, [(round % 251) as u8; 32], now);
            }
            let ids: Vec<u64> = s.tables.keys().copied().collect();
            for id in ids {
                while s.tables.get(&id).is_some_and(|t| t.phase.is_betting()) {
                    let t = &s.tables[&id];
                    let i = t.to_act.unwrap();
                    let actor = t.seats[i].as_ref().unwrap().player;
                    k = k.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    let action = match (k >> 33) % 4 {
                        0 => Action::AllIn,
                        1 => Action::Fold,
                        _ => Action::Call,
                    };
                    if s.act(actor, id, action, now).is_err() {
                        s.act(actor, id, Action::Call, now).unwrap();
                    }
                }
            }
            now += 6 * SEC;
        }
        panic!("did not finish in {max_rounds} rounds");
    }

    fn open_tournament(s: &mut State, name: &str) -> u64 {
        s.ensure_tournaments(0);
        s.tournaments
            .values()
            .find(|t| t.template.name == name && t.status == crate::tournament::TournamentStatus::Registering)
            .unwrap()
            .id
    }

    fn many_players(s: &mut State, n: u8) {
        for k in 10..10 + n {
            s.register(p(k), format!("bot{k}"), 0).unwrap();
            s.credit(&p(k), 10 * E8S).unwrap();
        }
    }

    #[test]
    fn chip_sit_and_go_runs_to_completion() {
        use crate::tournament::TournamentStatus::*;
        let mut s = setup();
        many_players(&mut s, 6);
        let id = open_tournament(&mut s, "Chip Sit & Go · 6 max");
        let chips_before: u64 = s.users.iter().map(|(_, u)| u.chips).sum();
        for k in 10..15 {
            s.register_tournament(p(k), id, SEC).unwrap();
        }
        assert!(s.register_tournament(p(10), id, SEC).is_err(), "no double registration");
        s.unregister_tournament(p(14), id).unwrap();
        s.register_tournament(p(14), id, SEC).unwrap();
        assert_eq!(s.tournaments[&id].status, Registering);
        s.register_tournament(p(15), id, 2 * SEC).unwrap(); // sixth player starts it
        assert_eq!(s.tournaments[&id].status, Running);
        let table = s.tournaments[&id].tables[0];
        assert!(s.leave_table(p(10), table, 2 * SEC).is_err(), "can't cash out tournament chips");
        assert!(s.list_tables().iter().all(|t| t.id != table));
        // A new instance opened for the next six players.
        s.ensure_tournaments(3 * SEC);
        assert_eq!(s.list_tournaments(None).iter().filter(|t| t.name == "Chip Sit & Go · 6 max").count(), 2);

        let now = run_until(&mut s, 3 * SEC, 5_000, |s| s.tournaments[&id].status == Finished);
        let t = &s.tournaments[&id];
        let mut places: Vec<u32> = t.entrants.iter().map(|e| e.place.unwrap()).collect();
        places.sort();
        assert_eq!(places, (1..=6).collect::<Vec<_>>());
        assert_eq!(t.paid, 6_000);
        let winner = t.entrants.iter().find(|e| e.place == Some(1)).unwrap();
        assert_eq!(winner.prize, 3_900);
        assert!(s.users.get(&winner.player).unwrap().medals.contains_key("sng_champ"));
        let chips_after: u64 = s.users.iter().map(|(_, u)| u.chips).sum();
        assert_eq!(chips_after, chips_before, "buy-ins all paid back out as prizes");
        // Tables close a minute after the finish.
        s.tick(now + 61 * SEC);
        assert!(!s.tables.contains_key(&table));
    }

    #[test]
    fn icp_tournament_accounting() {
        use crate::tournament::TournamentStatus::*;
        let mut s = setup();
        many_players(&mut s, 2);
        let before = s.house_stats().liabilities_total;
        let id = open_tournament(&mut s, "ICP Heads-Up Duel");
        s.register_tournament(p(10), id, SEC).unwrap();
        assert_eq!(s.house_stats().liabilities_total, before);
        s.register_tournament(p(11), id, SEC).unwrap();
        assert_eq!(s.tournaments[&id].status, Running);
        assert_eq!(s.house_stats().liabilities_total, before);
        run_until(&mut s, 2 * SEC, 5_000, |s| s.tournaments[&id].status == Finished);
        assert_eq!(s.house_stats().liabilities_total, before);
        let fee = E8S / 400;
        assert_eq!(house_take(&s), 2 * fee);
        let balances = s.users.get(&p(10)).unwrap().balance + s.users.get(&p(11)).unwrap().balance;
        assert_eq!(balances, 20 * E8S - 2 * fee);
    }

    #[test]
    fn scheduled_multi_table_tournament_balances_tables() {
        use crate::tournament::TournamentStatus::*;
        let mut s = setup();
        many_players(&mut s, 20);
        let id = open_tournament(&mut s, "Hourly Freeroll");
        let start_at = s.tournaments[&id].start_at;
        for k in 10..30 {
            s.register_tournament(p(k), id, SEC).unwrap();
        }
        s.tick(start_at - 1);
        assert_eq!(s.tournaments[&id].status, Registering);
        s.tick(start_at);
        assert_eq!(s.tournaments[&id].status, Running);
        assert_eq!(s.tournaments[&id].tables.len(), 3);
        let pool = s.tournaments[&id].prize_pool;
        assert_eq!(pool, 50_000, "freeroll guarantee");
        // Blinds rise every level (5 minutes), on every table.
        s.tick(start_at + 300 * SEC);
        assert_eq!(s.tournaments[&id].level, 1);
        for tid in &s.tournaments[&id].tables {
            assert_eq!(s.tables[tid].config.big_blind, crate::tournament::blinds_at(1, 3_000).1);
        }

        let check = |s: &State| {
            let t = &s.tournaments[&id];
            for tid in &t.tables {
                if let Some(tb) = s.tables.get(tid) {
                    assert!(tb.seats.iter().flatten().count() <= 9);
                }
            }
            // Every remaining player is seated exactly once.
            for e in t.entrants.iter().filter(|e| e.active()) {
                let seats = t.tables.iter().filter(|tid| s.tables.get(tid).is_some_and(|tb| tb.seat_of(&e.player).is_some())).count();
                assert_eq!(seats, 1);
            }
            t.status == Finished
        };
        run_until(&mut s, start_at + 300 * SEC, 20_000, check);
        let t = &s.tournaments[&id];
        let mut places: Vec<u32> = t.entrants.iter().map(|e| e.place.unwrap()).collect();
        places.sort();
        assert_eq!(places, (1..=20).collect::<Vec<_>>());
        assert_eq!(t.paid, pool);
        assert_eq!(t.entrants.iter().filter(|e| e.prize > 0).count(), 5);
    }

    #[test]
    fn scheduled_tournament_cancels_and_refunds() {
        use crate::tournament::TournamentStatus::*;
        let mut s = setup();
        many_players(&mut s, 2);
        let id = open_tournament(&mut s, "ICP Nightly");
        s.register_tournament(p(10), id, SEC).unwrap();
        s.register_tournament(p(11), id, SEC).unwrap(); // needs 3
        assert!(s.users.get(&p(10)).unwrap().balance < 10 * E8S);
        s.tick(s.tournaments[&id].start_at);
        assert_eq!(s.tournaments[&id].status, Cancelled);
        assert_eq!(s.users.get(&p(10)).unwrap().balance, 10 * E8S);
        assert_eq!(s.treasury, 0);
    }

    #[test]
    fn hosted_room_pools_come_from_the_house_share() {
        let mut s = setup();
        s.user_mut(&p(1)).unwrap().xp = 50;
        let id = s
            .create_room(
                p(1),
                RoomArgs { name: "Pools".into(), currency: Currency::Icp, big_blind: 200_000, max_seats: 6, password: None, action_timeout_secs: 30 },
                0,
            )
            .unwrap();
        s.join_table(p(2), id, 1, 10_000_000, None, 0).unwrap();
        s.join_table(p(1), id, 0, 10_000_000, None, 0).unwrap();
        play_hands(&mut s, id, 10, SEC);
        let rake: u64 = s.stores.hands.iter().map(|e| e.value()).collect::<Vec<_>>().into_iter().filter(|h| h.table_id == id).map(|h| h.rake).sum();
        assert!(rake > 0);
        // The host's cut is taken per hand before the house share is split.
        let host_cut: u64 = s.stores.hands.iter().map(|e| e.value()).collect::<Vec<_>>().into_iter().filter(|h| h.table_id == id).map(|h| h.rake * HOST_RAKE_SHARE_BPS / 10_000).sum();
        let r = s.rewards();
        assert!(r.fish_pool > 0);
        assert_eq!(s.treasury + r.fish_pool + r.board_pools.iter().sum::<u64>(), rake - host_cut);
        assert!(r.fish_pool * 10 / 3 <= rake - host_cut);
    }

    #[test]
    fn player_rooms() {
        let mut s = setup();
        let args = |pw: Option<&str>| RoomArgs {
            name: "Mark's Game".into(),
            currency: Currency::Icp,
            big_blind: 200_000,
            max_seats: 6,
            password: pw.map(String::from),
            action_timeout_secs: 30,
        };
        assert!(s.create_room(p(1), args(None), 0).unwrap_err().contains("level"), "new players can't host");
        s.user_mut(&p(1)).unwrap().xp = 50; // level 2
        let id = s.create_room(p(1), args(Some("aces")), 0).unwrap();
        assert!(s.users.get(&p(1)).unwrap().medals.contains_key("host"));
        let summary = s.list_tables().into_iter().find(|t| t.id == id).unwrap();
        assert!(summary.locked);
        assert_eq!(summary.room_owner.as_deref(), Some("player1"));
        assert!(s.join_table(p(2), id, 1, 10_000_000, None, 0).is_err());
        assert!(s.join_table(p(2), id, 1, 10_000_000, Some("kings".into()), 0).is_err());
        s.join_table(p(2), id, 1, 10_000_000, Some("aces".into()), 0).unwrap();
        s.join_table(p(1), id, 0, 10_000_000, None, 0).unwrap(); // host needs no password
        let before = s.house_stats().liabilities_total;
        let now = play_hands(&mut s, id, 10, SEC);
        assert_eq!(s.house_stats().liabilities_total, before);
        // Each hand's rake is split 80/20 between the house and the host.
        let rakes: Vec<u64> = s.stores.hands.iter().map(|e| e.value()).collect::<Vec<_>>().into_iter().filter(|h| h.table_id == id).map(|h| h.rake).collect();
        assert!(rakes.iter().sum::<u64>() > 0);
        let host_cut: u64 = rakes.iter().map(|r| r * HOST_RAKE_SHARE_BPS / 10_000).sum();
        assert_eq!(house_take(&s), rakes.iter().sum::<u64>() - host_cut);
        let net_play: i64 = (1..=2).map(|k| s.users.get(&p(k)).unwrap().stats.net_winnings).sum();
        // This room is the only ICP table in play, so it fed the whole ICP jackpot.
        let drops = s.jackpot(Currency::Icp);
        assert_eq!(net_play, -((rakes.iter().sum::<u64>() + drops) as i64));
        assert!(s.close_room(p(1), id).is_err(), "players still seated");
        assert!(s.close_room(p(2), id).is_err());
        s.leave_table(p(1), id, now).unwrap();
        s.leave_table(p(2), id, now).unwrap();
        s.create_room(p(1), args(None), now).unwrap();
        assert!(s.create_room(p(1), args(None), now).is_err(), "room limit");
        s.close_room(p(1), id).unwrap();
        assert!(!s.tables.contains_key(&id));
    }

    #[test]
    fn no_ratholing_or_blind_dodging() {
        let mut s = setup();
        let table = 1;
        let (min, max) = (s.tables[&table].config.min_buy_in, s.tables[&table].config.max_buy_in);
        s.join_table(p(1), table, 0, max, None, 0).unwrap();
        s.join_table(p(2), table, 1, max, None, 0).unwrap();
        // p1 wins big, then leaves.
        let i = s.tables[&table].seat_of(&p(1)).unwrap();
        s.tables.get_mut(&table).unwrap().seats[i].as_mut().unwrap().stack = max * 3;
        assert_eq!(s.leave_table(p(1), table, SEC).unwrap(), max * 3);
        // Coming back soon means bringing the stack back, even above the table max.
        assert!(s.join_table(p(1), table, 0, min, None, 2 * SEC).unwrap_err().contains("buy back in"));
        assert_eq!(s.table_view(table, Some(p(1)), 2 * SEC).unwrap().my_min_buy_in, max * 3);
        s.join_table(p(1), table, 0, max * 3, None, 2 * SEC).unwrap();
        s.leave_table(p(1), table, 3 * SEC).unwrap();
        // After 30 minutes the normal limits apply again (and the record is pruned).
        let later = 3 * SEC + RETURN_WINDOW_NS;
        s.tick(later);
        assert_eq!(s.min_return_buy_in(table, &p(1), later), 0);
        s.join_table(p(1), table, 0, min, None, later).unwrap();

        // Sitting out through hands means posting a big blind on return.
        s.join_table(p(3), table, 2, max, None, later).unwrap();
        let now = later + SEC;
        let t = s.tables.get_mut(&table).unwrap();
        t.start_hand([1; 32], now).unwrap();
        while t.phase.is_betting() {
            let pl = t.seats[t.to_act.unwrap()].as_ref().unwrap().player;
            t.act(&pl, Action::Fold, now).unwrap();
        }
        t.set_sitting_out(&p(3), true, now).unwrap();
        t.start_hand([2; 32], now + 10 * SEC).unwrap(); // p3 misses this one
        while t.phase.is_betting() {
            let pl = t.seats[t.to_act.unwrap()].as_ref().unwrap().player;
            t.act(&pl, Action::Fold, now).unwrap();
        }
        t.set_sitting_out(&p(3), false, now).unwrap();
        let i3 = t.seat_of(&p(3)).unwrap();
        assert!(t.seats[i3].as_ref().unwrap().owes_bb);
        t.start_hand([3; 32], now + 20 * SEC).unwrap();
        let s3 = t.seats[i3].as_ref().unwrap();
        assert!(!s3.owes_bb);
        assert_eq!(s3.bet, t.config.big_blind, "posted a big blind to come back in");
    }

    #[test]
    fn password_rooms_are_private() {
        let mut s = setup();
        s.user_mut(&p(1)).unwrap().xp = 50;
        let args = RoomArgs { name: "Home Game".into(), currency: Currency::Chips, big_blind: 100, max_seats: 6, password: Some("aces".into()), action_timeout_secs: 30 };
        let id = s.create_room(p(1), args, 0).unwrap();
        s.join_table(p(1), id, 0, 2_000, None, 0).unwrap();
        s.send_chat(p(1), Channel::Table(id), "welcome".into(), 10 * SEC).unwrap();

        // Outsiders (and anonymous viewers) see who sits where, nothing else.
        for viewer in [Some(p(3)), None] {
            let v = s.table_view(id, viewer, 1).unwrap();
            assert!(v.restricted);
            assert_eq!(v.seats[0].as_ref().unwrap().stack, 0);
        }
        assert!(s.chat_for(p(3), Channel::Table(id), 0).is_empty());
        assert!(s.send_chat(p(3), Channel::Table(id), "hi".into(), 20 * SEC).unwrap_err().contains("private"));

        // The password unlocks watching and chat.
        assert!(s.unlock_room(p(3), id, "kings".into()).is_err());
        s.unlock_room(p(3), id, "aces".into()).unwrap();
        assert!(!s.table_view(id, Some(p(3)), 1).unwrap().restricted);
        assert!(s.chat_for(p(3), Channel::Table(id), 0).iter().any(|m| m.text == "welcome"));
        s.send_chat(p(3), Channel::Table(id), "hi".into(), 20 * SEC).unwrap();
        // And they can sit without typing it again.
        s.join_table(p(3), id, 1, 2_000, None, 0).unwrap();
        // Public tables stay open to everyone.
        assert!(!s.table_view(1, None, 1).unwrap().restricted);
    }

    #[test]
    fn medals_for_hands_and_social() {
        let mut s = setup();
        let table = 1;
        s.join_table(p(1), table, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), table, 1, 1_000, None, 0).unwrap();
        play_hands(&mut s, table, 3, SEC);
        assert!(s.users.get(&p(1)).unwrap().medals.contains_key("first_hand"));
        let winner = if s.users.get(&p(1)).unwrap().stats.hands_won > 0 { p(1) } else { p(2) };
        assert!(s.users.get(&winner).unwrap().medals.contains_key("first_pot"));
        let xp = s.users.get(&p(1)).unwrap().xp;
        s.award(&p(1), "first_hand", None, 0);
        assert_eq!(s.users.get(&p(1)).unwrap().xp, xp, "medals are awarded once");

        // A royal flush at showdown.
        let card = |r: u8, su: u8| r * 4 + su;
        let result = HandResult {
            hand_no: 99,
            board: vec![card(12, 3), card(11, 3), card(10, 3), card(0, 0), card(1, 1)],
            winners: vec![crate::table::Winner { seat: 0, player: p(3), amount: 100, hand: "Straight Flush".into() }],
            shown: vec![
                crate::table::ShownHand { seat: 0, player: p(3), cards: vec![card(9, 3), card(8, 3)], hand: String::new() },
                crate::table::ShownHand { seat: 1, player: p(2), cards: vec![card(12, 0), card(12, 1)], hand: String::new() },
            ],
            players: vec![],
            pot: 100,
            rake: 0,
            jackpot_drop: 0,
        };
        s.award_hand_medals(table, Some(Currency::Chips), &result, 0);
        for m in ["royal_flush", "straight_flush", "quads", "full_house"] {
            assert!(s.users.get(&p(3)).unwrap().medals.contains_key(m), "{m}");
        }
        // p2 lost holding trip aces: not strong enough for a bad beat.
        assert!(!s.users.get(&p(2)).unwrap().medals.contains_key("bad_beat"));
    }

    #[test]
    fn one_player_at_several_tables() {
        let mut s = setup();
        s.join_table(p(1), 1, 0, 1_000, None, 0).unwrap();
        s.join_table(p(1), 2, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), 1, 1, 1_000, None, 0).unwrap();
        s.join_table(p(3), 2, 1, 1_000, None, 0).unwrap();
        assert_eq!(s.public_profile(&p(1), 0).unwrap().tables, vec![1, 2]);
        let now = run_until(&mut s, SEC, 1_000, |s| s.users.get(&p(1)).unwrap().stats.hands_played >= 10);
        let tables: BTreeSet<String> = s.hand_history(p(1), 50).into_iter().map(|h| h.table_name).collect();
        assert_eq!(tables.len(), 2, "history covers both tables");
        for t in [1, 2] {
            if s.tables[&t].seat_of(&p(1)).is_some() {
                s.leave_table(p(1), t, now).unwrap();
            }
        }
        for (k, t) in [(2, 1), (3, 2)] {
            if s.tables[&t].seat_of(&p(k)).is_some() {
                s.leave_table(p(k), t, now).unwrap();
            }
        }
        let chips: u64 = (1..=3).map(|k| s.users.get(&p(k)).unwrap().chips).sum();
        let dropped = s.jackpot(Currency::Chips) - crate::jackpot::CHIP_JACKPOT_SEED;
        assert_eq!(chips + dropped, 3 * SIGNUP_CHIPS);
    }

    fn start_scheduled(s: &mut State, name: &str, players: std::ops::Range<u8>) -> (u64, u64) {
        let id = open_tournament(s, name);
        for k in players {
            s.register_tournament(p(k), id, SEC).unwrap();
        }
        let start_at = s.tournaments[&id].start_at;
        s.tick(start_at);
        assert_eq!(s.tournaments[&id].status, crate::tournament::TournamentStatus::Running);
        (id, start_at)
    }

    fn stack_of(s: &State, id: u64, who: Principal) -> Option<u64> {
        let t = &s.tournaments[&id];
        t.tables.iter().find_map(|tid| {
            let tb = s.tables.get(tid)?;
            tb.seat_of(&who).and_then(|i| tb.seats[i].as_ref()).map(|x| x.stack)
        })
    }

    #[test]
    fn late_registration() {
        let mut s = setup();
        many_players(&mut s, 4);
        let (id, start_at) = start_scheduled(&mut s, "Chip Rebuy Madness", 10..13);
        assert_eq!(s.tournaments[&id].prize_pool, 25_000, "guarantee");
        let summary = s.list_tournaments(Some(p(13))).into_iter().find(|t| t.id == id).unwrap();
        assert!(summary.can_register);
        s.register_tournament(p(13), id, start_at + SEC).unwrap();
        assert_eq!(stack_of(&s, id, p(13)), Some(3_000), "late entrant gets a seat and a starting stack");
        assert_eq!(s.tournaments[&id].collected, 8_000);
        assert!(s.register_tournament(p(13), id, start_at + SEC).is_err());
        // Late registration closes after level 4.
        s.tick(start_at + 4 * 300 * SEC);
        assert_eq!(s.tournaments[&id].level, 4);
        assert!(s.register_tournament(p(1), id, start_at + 4 * 300 * SEC).is_err());
        assert!(!s.list_tournaments(Some(p(1))).into_iter().find(|t| t.id == id).unwrap().can_register);
    }

    #[test]
    fn rebuy_window_and_knockout() {
        use crate::tournament::REBUY_WINDOW_NS;
        let mut s = setup();
        many_players(&mut s, 3);
        let (id, start_at) = start_scheduled(&mut s, "Chip Rebuy Madness", 10..13);
        let table = s.tournaments[&id].tables[0];
        let now = start_at + SEC;

        // Above the starting stack you can't rebuy.
        s.tables.get_mut(&table).unwrap().seats.iter_mut().flatten().find(|x| x.player == p(10)).unwrap().stack = 3_001;
        assert!(s.rebuy(p(10), id, now).is_err());
        // At or below it you can, between hands: a fresh stack is added.
        s.tables.get_mut(&table).unwrap().seats.iter_mut().flatten().find(|x| x.player == p(10)).unwrap().stack = 500;
        let chips = s.users.get(&p(10)).unwrap().chips;
        s.rebuy(p(10), id, now).unwrap();
        assert_eq!(stack_of(&s, id, p(10)), Some(3_500));
        assert_eq!(s.users.get(&p(10)).unwrap().chips, chips - 2_000);
        assert_eq!(s.tournaments[&id].collected, 8_000);

        // Play until somebody busts: they get a rebuy window, not a knockout.
        let mut now = run_until(&mut s, now, 5_000, |s| s.tournaments[&id].entrants.iter().any(|e| e.rebuy_deadline != 0));
        let busted = s.tournaments[&id].entrants.iter().find(|e| e.rebuy_deadline != 0).unwrap().player;
        assert!(s.tournaments[&id].entrants.iter().all(|e| e.place.is_none()));
        let badge = s.tournament_badge(id, Some(busted)).unwrap();
        assert!(badge.can_rebuy && badge.rebuy_deadline > 0);
        s.rebuy(busted, id, now).unwrap();
        assert_eq!(stack_of(&s, id, busted), Some(3_000));

        // Next bust: let the window expire and they're knocked out.
        now = run_until(&mut s, now, 5_000, |s| s.tournaments[&id].entrants.iter().any(|e| e.rebuy_deadline != 0));
        let busted = s.tournaments[&id].entrants.iter().find(|e| e.rebuy_deadline != 0).unwrap().player;
        s.tick(now + REBUY_WINDOW_NS);
        let e = s.tournaments[&id].entrants.iter().find(|e| e.player == busted).unwrap();
        assert!(e.place.is_none() && e.out_seq.is_some(), "out, but placed once late registration closes");
        assert!(!s.tournament_badge(id, Some(busted)).unwrap().can_rebuy);
        s.tick(start_at + 4 * 300 * SEC);
        let e = s.tournaments[&id].entrants.iter().find(|e| e.player == busted).unwrap();
        assert_eq!(e.place, Some(3));
    }

    #[test]
    fn late_entry_fee_funds_the_pools() {
        let mut s = setup();
        many_players(&mut s, 4);
        let (id, start_at) = start_scheduled(&mut s, "ICP Nightly", 10..13);
        let (treasury, fish, total) = (s.treasury, s.rewards().fish_pool, s.treasury + s.rewards().pools_total());
        let fee = E8S / 40;
        s.register_tournament(p(13), id, start_at + SEC).unwrap();
        assert_eq!(s.rewards().fish_pool, fish + fee * 3_000 / 10_000);
        assert_eq!(s.treasury, treasury + fee * 6_000 / 10_000);
        assert_eq!(s.treasury + s.rewards().pools_total(), total + fee);
    }

    #[test]
    fn late_registration_places_are_fair() {
        let mut s = setup();
        many_players(&mut s, 7);
        let (id, start_at) = start_scheduled(&mut s, "Chip Rebuy Madness", 10..16);
        let now = start_at + SEC;
        // Four of six are knocked out while late registration is open...
        for k in 10..14 {
            s.knock_out(id, &p(k), None, now);
        }
        // ...then a seventh player joins late, and one more busts.
        s.register_tournament(p(16), id, now).unwrap();
        s.knock_out(id, &p(14), None, now);
        assert!(s.tournaments[&id].entrants.iter().all(|e| e.place.is_none()), "no places yet");
        // Late registration closes: seven entries, two still playing.
        s.tick(start_at + 4 * 300 * SEC);
        let t = &s.tournaments[&id];
        let place = |k: u8| t.entrants.iter().find(|e| e.player == p(k)).unwrap().place;
        assert_eq!(place(14), Some(3), "last out finishes just below the players left");
        assert_eq!(place(13), Some(4));
        assert_eq!(place(10), Some(7), "first out finishes last");
        let mut places: Vec<u32> = t.entrants.iter().filter_map(|e| e.place).collect();
        places.sort();
        assert_eq!(places, (3..=7).collect::<Vec<_>>(), "no duplicate places");
        assert!(t.paid <= t.prize_pool);
    }

    /// Plays a rebuy event to the end with every busted player rebuying while they can.
    fn play_rebuy_event(s: &mut State, id: u64, start: u64) {
        use crate::tournament::TournamentStatus::Finished;
        let mut now = start;
        for _ in 0..200 {
            now = run_until(s, now, 20_000, |s| {
                s.tournaments[&id].status == Finished || s.tournaments[&id].entrants.iter().any(|e| e.rebuy_deadline != 0)
            });
            if s.tournaments[&id].status == Finished {
                return;
            }
            let everyone: Vec<Principal> = s.tournaments[&id].entrants.iter().map(|e| e.player).collect();
            for p in everyone {
                let _ = s.addon(p, id, now);
            }
            let waiting: Vec<Principal> =
                s.tournaments[&id].entrants.iter().filter(|e| e.rebuy_deadline != 0).map(|e| e.player).collect();
            for w in waiting {
                if s.rebuy(w, id, now).is_err() {
                    s.tick(now + 31 * SEC);
                }
            }
            now += 31 * SEC;
        }
        panic!("tournament didn't finish");
    }

    #[test]
    fn rebuy_tournament_pays_out_exactly() {
        let mut s = setup();
        many_players(&mut s, 12);
        let chips_before: u64 = s.users.iter().map(|(_, u)| u.chips).sum();
        let (id, start_at) = start_scheduled(&mut s, "Chip Rebuy Madness", 10..22);
        play_rebuy_event(&mut s, id, start_at + SEC);
        let t = &s.tournaments[&id];
        assert!(t.entries() > 12, "some players rebought");
        assert!(t.entrants.iter().all(|e| e.rebuys <= 3));
        let mut places: Vec<u32> = t.entrants.iter().map(|e| e.place.unwrap()).collect();
        places.sort();
        assert_eq!(places, (1..=12).collect::<Vec<_>>());
        assert_eq!(t.paid, t.prize_pool);
        let addons = t.entrants.iter().filter(|e| e.addon).count() as u64;
        assert_eq!(t.collected, 2_000 * t.entries() as u64 + 2_000 * addons);
        let chips_after: u64 = s.users.iter().map(|(_, u)| u.chips).sum();
        assert_eq!(chips_after, chips_before - t.collected + t.prize_pool);
    }

    #[test]
    fn icp_rebuy_tournament_accounting() {
        let mut s = setup();
        many_players(&mut s, 4);
        let before = s.house_stats().liabilities_total;
        let (id, start_at) = start_scheduled(&mut s, "ICP Nightly", 10..14);
        assert_eq!(s.house_stats().liabilities_total, before);
        play_rebuy_event(&mut s, id, start_at + SEC);
        let t = &s.tournaments[&id];
        assert_eq!(s.house_stats().liabilities_total, before);
        // The fee is charged once per player: rebuys and add-ons go entirely into the pool.
        assert_eq!(house_take(&s), 4 * (E8S / 40));
        let addons = t.entrants.iter().filter(|e| e.addon).count() as u64;
        assert_eq!(t.collected, t.entries() as u64 * (E8S / 2) + addons * (E8S / 2));
        assert_eq!(t.paid, t.prize_pool);
    }

    #[test]
    fn entry_fees_are_held_until_the_start() {
        let mut s = setup();
        many_players(&mut s, 3);
        let before = s.house_stats().liabilities_total;
        let id = open_tournament(&mut s, "ICP Nightly");
        // Two register: their fees are held by the event, not the treasury.
        for k in 10..12 {
            s.register_tournament(p(k), id, SEC).unwrap();
        }
        assert_eq!(s.treasury, 0);
        assert_eq!(s.tournaments[&id].fees_held, 2 * (E8S / 40));
        assert_eq!(s.house_stats().liabilities_total, before);
        // One unregisters and gets everything back.
        s.unregister_tournament(p(11), id).unwrap();
        assert_eq!(s.users.get(&p(11)).unwrap().balance, 10 * E8S);
        // Too few players at the start: cancelled and refunded, treasury untouched (it used to underflow here).
        let start_at = s.tournaments[&id].start_at;
        s.tick(start_at);
        assert_eq!(s.tournaments[&id].status, crate::tournament::TournamentStatus::Cancelled);
        assert_eq!(s.users.get(&p(10)).unwrap().balance, 10 * E8S);
        assert_eq!(s.treasury, 0);
        assert_eq!(s.house_stats().liabilities_total, before);

        // When an event does start, the fees move to the treasury.
        let (id, _) = start_scheduled(&mut s, "ICP Nightly", 10..13);
        assert_eq!(s.tournaments[&id].fees_held, 0);
        assert_eq!(house_take(&s), 3 * (E8S / 40));
        assert_eq!(s.house_stats().liabilities_total, before);
    }

    #[test]
    fn tournament_fees_fund_the_pools_at_start() {
        let mut s = setup();
        many_players(&mut s, 3);
        let id = open_tournament(&mut s, "ICP Nightly");
        for k in 10..12 {
            s.register_tournament(p(k), id, SEC).unwrap();
        }
        assert_eq!(s.rewards().pools_total(), 0, "held fees are not funded yet");
        s.unregister_tournament(p(11), id).unwrap();
        s.tick(s.tournaments[&id].start_at);
        assert_eq!(s.tournaments[&id].status, crate::tournament::TournamentStatus::Cancelled);
        assert_eq!(s.rewards().pools_total(), 0, "a cancelled event funds nothing");

        let (_, _) = start_scheduled(&mut s, "ICP Nightly", 10..13);
        let fees = 3 * (E8S / 40);
        let r = s.rewards();
        assert_eq!(r.fish_pool, fees * 30 / 100);
        assert_eq!(r.board_pools, [fees / 40; 4]);
        assert_eq!(s.treasury, fees * 60 / 100);
    }

    #[test]
    fn liabilities_include_reward_pools() {
        let mut s = setup();
        let before = s.house_stats();
        assert_eq!(before.rewards_pools, 0);
        s.fund_rewards(10_000);
        let after = s.house_stats();
        assert_eq!(after.rewards_pools, 4_000);
        assert_eq!(after.treasury, before.treasury + 6_000);
        assert_eq!(after.liabilities_total, before.liabilities_total + 10_000);
    }

    #[test]
    fn rebuys_charge_no_fee() {
        let mut s = setup();
        many_players(&mut s, 3);
        let (id, start_at) = start_scheduled(&mut s, "ICP Nightly", 10..13);
        let treasury = s.treasury;
        let table = s.tournaments[&id].tables[0];
        s.tables.get_mut(&table).unwrap().seats.iter_mut().flatten().find(|x| x.player == p(10)).unwrap().stack = 1_000;
        let before = s.users.get(&p(10)).unwrap().balance;
        s.rebuy(p(10), id, start_at + SEC).unwrap();
        assert_eq!(s.users.get(&p(10)).unwrap().balance, before - E8S / 2, "buy-in only");
        assert_eq!(s.treasury, treasury);
        assert_eq!(s.tournament_badge(id, Some(p(10))).unwrap().rebuy_cost, E8S / 2);
    }

    #[test]
    fn icp_tournament_finish_counts_net() {
        let mut s = setup();
        many_players(&mut s, 3);
        let (id, start_at) = start_scheduled(&mut s, "ICP Nightly", 10..13);
        let table = s.tournaments[&id].tables[0];
        s.tables.get_mut(&table).unwrap().seats.iter_mut().flatten().find(|x| x.player == p(10)).unwrap().stack = 1_000;
        s.rebuy(p(10), id, start_at + SEC).unwrap();
        play_rebuy_event(&mut s, id, start_at + 2 * SEC);
        let t = &s.tournaments[&id];
        let (buy_in, fee, addon_cost) = (t.template.buy_in, t.template.fee, t.template.addon_cost);
        assert!(t.entrants.iter().find(|e| e.player == p(10)).unwrap().rebuys >= 1);
        let mut winner = None;
        for e in &t.entrants {
            // Entry is buy-in + fee; each rebuy costs the buy-in; the add-on its price.
            let cost = buy_in + fee + e.rebuys as u64 * buy_in + if e.addon { addon_cost } else { 0 };
            let net = e.prize as i64 - cost as i64;
            let a = account(&s, e.player);
            assert_eq!((a.period.w_net, a.period.m_net), (net, net), "entrant {:?}", e.player);
            if e.place == Some(1) {
                winner = Some((e.player, net));
            }
        }
        let (w, net) = winner.unwrap();
        assert!(net > 0);
        let v = &s.leaderboards(None, start_at)[1];
        assert_eq!(v.rows[0].player, w);
        assert_eq!(v.rows[0].value, net);
        assert!(v.rows.iter().all(|r| r.value > 0), "losers are not listed");
    }

    #[test]
    fn addon_window_and_limits() {
        let mut s = setup();
        many_players(&mut s, 3);
        let (id, start_at) = start_scheduled(&mut s, "Chip Rebuy Madness", 10..13);
        let level_at = |l: u64| start_at + l * 300 * SEC;
        // Not yet: the add-on opens on the last rebuy level (level 4 of 4).
        assert!(s.addon(p(10), id, level_at(0) + SEC).is_err());
        assert!(!s.tournament_badge(id, Some(p(10))).unwrap().can_addon);
        s.tick(level_at(3));
        assert_eq!(s.tournaments[&id].level, 3);
        let badge = s.tournament_badge(id, Some(p(10))).unwrap();
        assert!(badge.can_addon && badge.addon_chips == 4_500 && badge.addon_cost == 2_000);

        let stack = stack_of(&s, id, p(10)).unwrap();
        let chips = s.users.get(&p(10)).unwrap().chips;
        let collected = s.tournaments[&id].collected;
        s.addon(p(10), id, level_at(3) + SEC).unwrap();
        assert_eq!(stack_of(&s, id, p(10)), Some(stack + 4_500));
        assert_eq!(s.users.get(&p(10)).unwrap().chips, chips - 2_000);
        assert_eq!(s.tournaments[&id].collected, collected + 2_000);
        assert!(s.addon(p(10), id, level_at(3) + 2 * SEC).is_err(), "once per player");
        assert!(!s.tournament_badge(id, Some(p(10))).unwrap().can_addon);

        // Still open the level after rebuys close; gone the level after that.
        s.tick(level_at(4));
        assert!(s.rebuy(p(11), id, level_at(4) + SEC).is_err(), "rebuys closed");
        s.addon(p(11), id, level_at(4) + SEC).unwrap();
        s.tick(level_at(5));
        assert!(s.addon(p(12), id, level_at(5) + SEC).is_err());
    }

    #[test]
    fn addon_refused_mid_hand_and_when_busted() {
        let mut s = setup();
        many_players(&mut s, 2);
        let (id, start_at) = start_scheduled(&mut s, "Chip Rebuy Madness", 10..12);
        let now = start_at + 3 * 300 * SEC;
        s.tick(now);
        let table = s.tournaments[&id].tables[0];
        s.start_hand(table, [1; 32], now + 6 * SEC);
        assert!(s.tables[&table].phase.is_betting());
        let chips = s.users.get(&p(10)).unwrap().chips;
        assert!(s.addon(p(10), id, now + 7 * SEC).is_err(), "not during a hand");
        assert_eq!(s.users.get(&p(10)).unwrap().chips, chips, "nothing charged");
        // A busted player has to rebuy before they can add on.
        s.tournaments.get_mut(&id).unwrap().entrants.iter_mut().find(|e| e.player == p(11)).unwrap().rebuy_deadline = now + 30 * SEC;
        assert!(s.addon(p(11), id, now + 7 * SEC).is_err());
    }

    fn card(s: &str) -> u8 {
        let b = s.as_bytes();
        let r = b"23456789TJQKA".iter().position(|&x| x == b[0]).unwrap() as u8;
        let su = b"cdhs".iter().position(|&x| x == b[1]).unwrap() as u8;
        r * 4 + su
    }

    fn cards(s: &str) -> Vec<u8> {
        s.split_whitespace().map(card).collect()
    }

    /// Seats players 1-3 plus `extra` at Penny Lane and fabricates a finished hand.
    fn bad_beat_hand(s: &mut State, n_players: u8, loser_hole: &str, board: &str) -> (u64, HandResult) {
        let table = 5; // Penny Lane (ICP)
        for k in 1..=n_players {
            if !s.users.contains_key(&p(k)) {
                s.register(p(k), format!("player{k}"), 0).unwrap();
                s.credit(&p(k), 10 * E8S).unwrap();
            }
            s.join_table(p(k), table, k - 1, 1_000_000, None, 0).unwrap();
        }
        let t = s.tables.get_mut(&table).unwrap();
        t.hand_no = 7;
        let result = HandResult {
            hand_no: 7,
            board: cards(board),
            winners: vec![crate::table::Winner { seat: 1, player: p(2), amount: 500_000, hand: "Straight Flush".into() }],
            shown: vec![
                crate::table::ShownHand { seat: 0, player: p(1), cards: cards(loser_hole), hand: String::new() },
                crate::table::ShownHand { seat: 1, player: p(2), cards: cards("9h Th"), hand: String::new() },
            ],
            players: (1..=n_players).map(|k| crate::table::PlayerResult { player: p(k), committed: 100_000, won: 0 }).collect(),
            pot: 500_000,
            rake: 0,
            jackpot_drop: 0,
        };
        (table, result)
    }

    #[test]
    fn mucked_hands_earn_no_public_medals() {
        for muck in [true, false] {
            let mut s = setup();
            let (table, result) = bad_beat_hand(&mut s, 3, "7c 7d", "7h 7s Jh Qh Kh");
            s.user_mut(&p(1)).unwrap().settings.auto_muck = muck;
            let t = s.tables.get_mut(&table).unwrap();
            t.phase = Phase::HandOver;
            t.last_result = Some(result);
            s.after_table_change(table, 3);
            let medal = s.users.get(&p(1)).unwrap().medals.contains_key("bad_beat");
            assert_eq!(medal, !muck, "a mucked losing hand must not reveal itself through a medal (muck={muck})");
            let shown = s.table_view(table, Some(p(3)), 4).unwrap().last_result.unwrap().shown;
            assert_eq!(shown.iter().any(|sh| sh.player == p(1)), !muck);
        }
    }

    #[test]
    fn rake_is_split_between_pools_and_treasury() {
        let mut s = setup();
        s.fund_rewards(10_000);
        let r = s.rewards();
        assert_eq!(r.fish_pool, 3_000);
        assert_eq!(r.board_pools, [250; 4]);
        assert_eq!(s.treasury, 6_000);
        // Rounding goes to the treasury and nothing is lost.
        let mut s = setup();
        s.fund_rewards(7);
        let r = s.rewards();
        assert_eq!(r.fish_pool, 2);
        assert_eq!(r.board_pools, [0; 4]);
        assert_eq!(s.treasury, 5);
        assert_eq!(s.treasury + r.pools_total(), 7);
    }

    #[test]
    fn icp_hand_rake_funds_the_pools() {
        let mut s = setup();
        let table = 7; // Low Rollers
        let buy = s.tables[&table].config.max_buy_in;
        s.join_table(p(1), table, 0, buy, None, 0).unwrap();
        s.join_table(p(2), table, 3, buy, None, 0).unwrap();
        play_hands(&mut s, table, 20, SEC);
        let rake: u64 = s.stores.hands.iter().map(|e| e.value()).collect::<Vec<_>>().into_iter().filter(|h| h.table_id == table).map(|h| h.rake).sum();
        assert!(rake > 0);
        let r = s.rewards();
        assert_eq!(s.treasury + r.pools_total(), rake);
        // Each hand rounds separately, so the pool is within a few e8s of 30%.
        assert!((r.fish_pool * 10 / 3).abs_diff(rake) <= 40, "fish {} of rake {}", r.fish_pool, rake);
        assert!(r.board_pools.iter().all(|&b| b == r.board_pools[0] && b > 0));
    }

    #[test]
    fn bad_beat_jackpot_pays_the_table() {
        let mut s = setup();
        s.jackpots.insert(Currency::Icp, E8S);
        // Pocket sevens make quads; the board also lets 9h-Th make a straight flush.
        let (table, result) = bad_beat_hand(&mut s, 4, "7c 7d", "7h 7s Jh Qh Kh");
        let before = s.house_stats().liabilities_total;
        let balances: Vec<u64> = (1..=4).map(|k| s.users.get(&p(k)).unwrap().balance).collect();
        s.check_bad_beat(table, &result, 3);

        let paid = E8S * 9 / 10;
        assert_eq!(s.jackpot(Currency::Icp), E8S - paid, "10% seeds the next jackpot");
        assert_eq!(s.users.get(&p(1)).unwrap().balance - balances[0], paid / 2, "loser gets half");
        assert_eq!(s.users.get(&p(2)).unwrap().balance - balances[1], paid / 4, "winner a quarter");
        assert_eq!(s.users.get(&p(3)).unwrap().balance - balances[2], paid / 8, "table splits the rest");
        assert_eq!(s.users.get(&p(4)).unwrap().balance - balances[3], paid / 8);
        assert_eq!(s.house_stats().liabilities_total, before);

        let hit = s.table_view(table, Some(p(3)), 4).unwrap().jackpot_hit.unwrap();
        assert_eq!((hit.loser, hit.winner), (p(1), p(2)));
        assert_eq!(hit.loser_hand, "Four of a Kind");
        assert_eq!(hit.winner_hand, "Straight Flush");
        assert!(s.users.get(&p(1)).unwrap().medals.contains_key("jackpot"));
        assert_eq!(s.jackpot_info().recent.len(), 1);
        assert!(s.chat_since(Channel::Lobby, 0).iter().any(|m| m.text.contains("BAD BEAT JACKPOT")));
        // Once the next hand starts, the celebration is gone.
        s.tables.get_mut(&table).unwrap().hand_no = 8;
        assert!(s.table_view(table, None, 5).unwrap().jackpot_hit.is_none());
    }

    #[test]
    fn bad_beat_jackpot_rules() {
        // Fewer than four players dealt in.
        let mut s = setup();
        s.jackpots.insert(Currency::Icp, E8S);
        let (table, result) = bad_beat_hand(&mut s, 3, "7c 7d", "7h 7s Jh Qh Kh");
        s.check_bad_beat(table, &result, 3);
        assert_eq!(s.jackpot(Currency::Icp), E8S);

        // Quads on the board: the loser's hole cards don't both play.
        let mut s = setup();
        s.jackpots.insert(Currency::Icp, E8S);
        let (table, result) = bad_beat_hand(&mut s, 4, "Ac 2d", "7c 7d 7h 7s Kh");
        s.check_bad_beat(table, &result, 3);
        assert_eq!(s.jackpot(Currency::Icp), E8S);

        // A losing full house isn't bad enough.
        let mut s = setup();
        s.jackpots.insert(Currency::Icp, E8S);
        let (table, result) = bad_beat_hand(&mut s, 4, "Jc Jd", "7h 7s Jh Qh Kh");
        s.check_bad_beat(table, &result, 3);
        assert_eq!(s.jackpot(Currency::Icp), E8S);
    }

    #[test]
    fn jackpot_is_fed_by_cash_tables_only() {
        let mut s = setup();
        assert_eq!(s.jackpot(Currency::Chips), crate::jackpot::CHIP_JACKPOT_SEED);
        for k in 1..=3 {
            s.join_table(p(k), 1, k - 1, 1_000, None, 0).unwrap();
        }
        let now = run_until(&mut s, SEC, 1_000, |s| s.jackpot(Currency::Chips) > crate::jackpot::CHIP_JACKPOT_SEED);
        assert!(s.jackpot(Currency::Chips) > crate::jackpot::CHIP_JACKPOT_SEED);
        let chips: u64 = (1..=3).map(|k| s.users.get(&p(k)).unwrap().chips + s.tables[&1].seats.iter().flatten().find(|x| x.player == p(k)).map_or(0, |x| x.stack)).sum();
        assert_eq!(chips + s.jackpot(Currency::Chips) - crate::jackpot::CHIP_JACKPOT_SEED, 3 * SIGNUP_CHIPS, "every dropped chip is in the jackpot");
        // Tournament tables never feed it (stop the cash game first).
        for k in 1..=3 {
            if s.tables[&1].seat_of(&p(k)).is_some() {
                s.leave_table(p(k), 1, now).unwrap();
            }
        }
        many_players(&mut s, 2);
        let chips_jackpot = s.jackpot(Currency::Chips);
        let id = open_tournament(&mut s, "Chip Heads-Up Duel");
        s.register_tournament(p(10), id, now).unwrap();
        s.register_tournament(p(11), id, now).unwrap();
        let tid = s.tournaments[&id].tables[0];
        assert_eq!(s.tables[&tid].config.jackpot_bps, 0);
        run_until(&mut s, now + SEC, 5_000, |s| s.tournaments[&id].status == crate::tournament::TournamentStatus::Finished);
        assert_eq!(s.jackpot(Currency::Chips), chips_jackpot);
        assert_eq!(s.table_view(tid, None, now).map_or(0, |v| v.jackpot), 0);
    }

    #[test]
    fn state_survives_an_upgrade() {
        use ic_stable_structures::VectorMemory;
        let mem = VectorMemory::default();
        let mut s = State::new_in(mem.clone(), Principal::management_canister(), 10_000);
        for k in 1..=3 {
            s.register(p(k), format!("player{k}"), 0).unwrap();
            s.credit(&p(k), 10 * E8S).unwrap();
        }
        s.set_note(p(1), p(2), "tight".into(), Some(NoteColor::Blue), 1).unwrap();
        s.friend_request(p(1), p(2)).unwrap();
        s.respond_friend(p(2), p(1), true).unwrap();
        s.send_dm(p(1), p(2), "hi".into(), 10 * SEC).unwrap();
        let table = 7;
        for k in 1..=3 {
            s.join_table(p(k), table, k - 1, 10_000_000, None, 0).unwrap();
        }
        let now = play_hands(&mut s, table, 5, 20 * SEC);
        s.set_pause(crate::wallet::PauseFlags { withdrawals: true, reason: Some("test".into()), ..Default::default() });
        let before = s.house_stats();
        let history = s.hand_history(p(1), 50).len();
        s.save();
        drop(s);

        // A new canister instance over the same stable memory.
        let r = State::restore(mem);
        assert_eq!(r.house_stats().liabilities_total, before.liabilities_total);
        assert_eq!(r.wallet_total, before.wallet_balances);
        assert_eq!(r.users.len(), 3);
        assert_eq!(r.find_user("PLAYER2"), Some(p(2)));
        assert_eq!(r.notes(p(1))[0].text, "tight");
        assert_eq!(r.dms_with(p(2), p(1), 0).len(), 1);
        assert_eq!(r.hand_history(p(1), 50).len(), history);
        assert_eq!(r.tables[&table].hand_no, 5);
        assert!(r.pause.withdrawals);
        assert_eq!(r.users.get(&p(1)).unwrap().friends.len(), 1);
        let _ = now;
    }

    #[test]
    fn wallet_total_matches_a_full_recount() {
        let mut s = setup();
        let table = 7;
        for k in 1..=3 {
            s.join_table(p(k), table, k - 1, 10_000_000, None, 0).unwrap();
        }
        let now = run_until(&mut s, SEC, 2_000, |s| s.tables[&table].hand_no >= 15 || s.tables[&table].eligible_count() < 2);
        let ninja = s.items.values().find(|i| i.name == "Ninja").unwrap().id;
        let _ = s.buy_item(p(1), ninja, Currency::Icp);
        for k in 1..=3 {
            if s.tables[&table].seat_of(&p(k)).is_some() {
                s.leave_table(p(k), table, now).unwrap();
            }
        }
        assert_eq!(s.wallet_total, s.recount_wallet_total());
    }

    #[test]
    fn pausing_dealing_and_joins() {
        let mut s = setup();
        s.join_table(p(1), 1, 0, 1_000, None, 0).unwrap();
        s.set_pause(crate::wallet::PauseFlags { joins: true, dealing: true, reason: Some("maintenance".into()), ..Default::default() });
        assert!(s.join_table(p(2), 1, 1, 1_000, None, 0).unwrap_err().contains("maintenance"));
        s.set_pause(crate::wallet::PauseFlags { dealing: true, ..Default::default() });
        s.join_table(p(2), 1, 1, 1_000, None, 0).unwrap();
        assert!(s.tick(SEC).is_empty(), "no new hands while dealing is paused");
        s.set_pause(crate::wallet::PauseFlags::default());
        assert_eq!(s.tick(2 * SEC), vec![1]);
    }

    #[test]
    fn player_notes_are_private() {
        let mut s = setup();
        s.set_note(p(1), p(2), "calls everything".into(), Some(NoteColor::Red), 5).unwrap();
        assert!(s.set_note(p(1), p(1), "me".into(), None, 5).is_err());
        assert!(s.set_note(p(1), p(9), "ghost".into(), None, 5).is_err());
        assert!(s.set_note(p(1), p(3), "x".repeat(501), None, 5).is_err());
        let notes = s.notes(p(1));
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].username, "player2");
        assert!(s.notes(p(2)).is_empty());

        s.join_table(p(2), 1, 0, 1_000, None, 0).unwrap();
        let mine = s.table_view(1, Some(p(1)), 0).unwrap();
        let seat = mine.seats[0].as_ref().unwrap();
        assert_eq!(seat.note.as_ref().unwrap().color, Some(NoteColor::Red));
        let theirs = s.table_view(1, Some(p(3)), 0).unwrap();
        assert!(theirs.seats[0].as_ref().unwrap().note.is_none(), "other viewers don't see it");

        // Editing replaces; clearing text and color deletes.
        s.set_note(p(1), p(2), "".into(), Some(NoteColor::Green), 9).unwrap();
        assert_eq!(s.notes(p(1))[0].text, "");
        s.set_note(p(1), p(2), "  ".into(), None, 10).unwrap();
        assert!(s.notes(p(1)).is_empty());
    }

    #[test]
    fn blocking_hides_chat_and_cuts_contact() {
        let mut s = setup();
        s.friend_request(p(1), p(2)).unwrap();
        s.respond_friend(p(2), p(1), true).unwrap();
        s.send_chat(p(2), Channel::Lobby, "hello".into(), 10 * SEC).unwrap();
        s.block(p(1), p(2), true).unwrap();
        assert!(s.block(p(1), p(1), true).is_err());
        assert!(s.chat_for(p(1), Channel::Lobby, 0).is_empty(), "blocked player's chat hidden");
        assert_eq!(s.chat_for(p(3), Channel::Lobby, 0).len(), 1, "others still see it");
        assert!(s.friends(p(1), 0).is_empty() && s.friends(p(2), 0).is_empty(), "friendship ended");
        assert!(s.friend_request(p(2), p(1)).is_err());
        assert!(s.send_dm(p(2), p(1), "hi".into(), 20 * SEC).is_err());
        assert_eq!(s.profile(&p(1)).unwrap().blocked, vec![p(2)]);
        s.block(p(1), p(2), false).unwrap();
        assert_eq!(s.chat_for(p(1), Channel::Lobby, 0).len(), 1);
        s.friend_request(p(2), p(1)).unwrap();
    }

    #[test]
    fn reports_and_moderation() {
        use crate::moderation::{ModAction, ReportReason, ReportStatus};
        let mut s = setup();
        let admin = p(200);
        for k in 20..30 {
            s.register(p(k), format!("other{k}"), 0).unwrap();
        }
        s.send_chat(p(2), Channel::Lobby, "you all suck".into(), 10 * SEC).unwrap();
        s.send_chat(p(3), Channel::Lobby, "gl".into(), 10 * SEC).unwrap();
        let id = s.report(p(1), p(2), ReportReason::AbusiveChat, "rude".into(), Some(Channel::Lobby), 11 * SEC).unwrap();
        assert!(s.report(p(1), p(1), ReportReason::Spam, String::new(), None, 0).is_err());
        let open = s.open_reports(None, 50).reports;
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].evidence.len(), 1, "only the target's messages are captured");
        assert_eq!(open[0].evidence[0].text, "you all suck");
        // One open report per reporter and target.
        s.report(p(3), p(2), ReportReason::Spam, String::new(), None, 12 * SEC).unwrap();
        assert!(s.report(p(3), p(2), ReportReason::Spam, String::new(), None, 12 * SEC).unwrap_err().contains("already reported"));
        // Ten a day.
        for k in 20..29 {
            s.report(p(3), p(k), ReportReason::Spam, String::new(), None, 12 * SEC).unwrap();
        }
        assert!(s.report(p(3), p(29), ReportReason::Spam, String::new(), None, 12 * SEC).is_err(), "daily cap");

        // A chat mute from a report.
        s.resolve_report(admin, id, ModAction::MuteChat(1), "cool off".into(), 20 * SEC).unwrap();
        assert!(s.resolve_report(admin, id, ModAction::Dismiss, String::new(), 20 * SEC).is_err());
        assert!(s.send_chat(p(2), Channel::Lobby, "still here".into(), 30 * SEC).unwrap_err().contains("muted"));
        s.send_chat(p(2), Channel::Lobby, "sorry".into(), 20 * SEC + 3600 * SEC).unwrap();
        assert_eq!(s.audit_log.len(), 2, "mute and resolution are audited");
        assert!(s.resolve_report(admin, 2, ModAction::Dismiss, "x".repeat(301), 0).is_err(), "note length is capped");

        // The queue pages, oldest first.
        let page = s.open_reports(None, 4);
        assert_eq!((page.reports.len(), page.open_total), (4, 10));
        let next = s.open_reports(Some(page.reports[3].id), 50);
        assert_eq!(next.reports.len(), 6);
        assert!(next.reports[0].id > page.reports[3].id);

        let view = s.mod_player_view(&p(2)).unwrap();
        assert_eq!(view.reports.len(), 2, "found through the index");
        assert!(matches!(view.reports.last().unwrap().status, ReportStatus::Resolved { .. }), "oldest is the resolved one");
        assert!(view.chat_muted_until > 0);
        assert_eq!(s.recent_reports(3).len(), 3);
        assert_eq!(s.recent_reports(3)[0].id, 11, "newest first");
    }

    #[test]
    fn bans_stand_players_up_but_allow_withdrawals() {
        let mut s = setup();
        let table = 7;
        let buy = s.tables[&table].config.max_buy_in;
        s.join_table(p(1), table, 0, buy, None, 0).unwrap();
        s.join_table(p(2), table, 1, buy, None, 0).unwrap();
        let before = s.users.get(&p(1)).unwrap().balance;
        s.ban(p(200), p(1), 24, "collusion", SEC).unwrap();
        assert!(s.tables[&table].seat_of(&p(1)).is_none(), "stood up");
        assert_eq!(s.users.get(&p(1)).unwrap().balance, before + buy, "chips returned");
        assert!(s.join_table(p(1), table, 0, buy, None, 2 * SEC).unwrap_err().contains("suspended"));
        assert!(s.send_chat(p(1), Channel::Lobby, "hey".into(), 20 * SEC).is_err());
        let to = icrc_ledger_types::icrc1::account::Account { owner: p(1), subaccount: None };
        s.begin_withdraw(p(1), to, E8S, 3 * SEC).unwrap();
        // Temporary bans expire; unban lifts early.
        s.join_table(p(1), table, 0, buy, None, 25 * 3600 * SEC).unwrap();
        s.ban(p(200), p(3), 0, "", 0).unwrap();
        assert!(s.join_table(p(3), 1, 0, 1_000, None, u64::MAX / 2).is_err(), "permanent");
        s.unban(p(200), p(3), 0).unwrap();
        s.join_table(p(3), 1, 0, 1_000, None, 0).unwrap();
    }

    #[test]
    fn suspended_players_stay_out_of_tournaments() {
        let mut s = setup();
        many_players(&mut s, 3);
        let (id, start_at) = start_scheduled(&mut s, "Chip Rebuy Madness", 10..13);
        let tid = s.tournaments[&id].tables[0];
        s.ban(p(200), p(10), 24, "collusion", start_at).unwrap();
        let seat = s.tables[&tid].seat_of(&p(10)).unwrap();
        assert!(s.tables[&tid].seats[seat].as_ref().unwrap().sitting_out);
        assert!(s.sit_out(p(10), tid, false, start_at + 1).unwrap_err().contains("suspended"));
        assert!(s.rebuy(p(10), id, start_at + 1).unwrap_err().contains("suspended"));
        assert!(s.addon(p(10), id, start_at + 1).unwrap_err().contains("suspended"));
        assert!(s.act(p(10), tid, Action::Fold, start_at + 1).unwrap_err().contains("suspended"));
        s.sit_out(p(10), tid, true, start_at + 1).unwrap(); // staying out is always allowed
    }

    #[test]
    fn moderators_cannot_act_on_staff_or_themselves() {
        let mut s = setup();
        s.moderators.insert(p(1));
        s.moderators.insert(p(2));
        assert!(s.check_mod_target(&p(1), false, &p(1), false).is_err(), "self");
        assert!(s.check_mod_target(&p(1), false, &p(2), false).is_err(), "another moderator");
        assert!(s.check_mod_target(&p(1), false, &p(3), true).is_err(), "an admin");
        s.check_mod_target(&p(1), false, &p(3), false).unwrap();
        s.check_mod_target(&p(200), true, &p(2), false).unwrap();
        assert!(s.check_mod_target(&p(200), true, &p(200), true).is_err());
    }

    #[test]
    fn word_filter_and_offensive_names() {
        let mut s = setup();
        s.send_chat(p(1), Channel::Lobby, "what the fuck".into(), 10 * SEC).unwrap();
        assert_eq!(s.chat_since(Channel::Lobby, 0)[0].text, "what the ****");
        assert!(s.register(p(9), "shithead".into(), 0).is_err());
        s.set_status(p(1), "shit happens \u{202E}".into()).unwrap();
        assert_eq!(s.users.get(&p(1)).unwrap().status, "**** happens");
        s.user_mut(&p(1)).unwrap().xp = 50;
        let room = |name: &str| RoomArgs { name: name.into(), currency: Currency::Chips, big_blind: 100, max_seats: 6, password: None, action_timeout_secs: 30 };
        assert!(s.create_room(p(1), room("Shit Game"), 0).is_err());
        s.create_room(p(1), room("Scunthorpe Social"), 0).unwrap();
    }

    #[test]
    fn signup_throttle() {
        let mut s = setup();
        let base = s.signup_difficulty(0);
        s.signups_per_hour = 4; // setup() already used 3
        s.register(p(10), "fourth".into(), 0).unwrap();
        // Past the hourly target sign-ups stay open but cost more work, so a
        // flood of bot accounts can't lock real players out.
        let busy = s.signup_difficulty(0);
        assert!(busy > base);
        s.register(p(11), "fifth".into(), 0).unwrap();
        assert!(s.signup_difficulty(0) >= busy);
        assert!(s.signup_difficulty(0) <= crate::signup::MAX_POW_BITS);
        assert_eq!(s.signup_difficulty(3600 * SEC), base, "a new hour starts easy again");
        s.signups_per_hour = 0;
        assert!(s.register(p(12), "sixth".into(), 3600 * SEC).unwrap_err().contains("closed"));
    }

    #[test]
    fn caps_on_cash_seats_and_friends() {
        let mut s = setup();
        s.credit(&p(1), 100 * E8S).unwrap();
        let chip_tables: Vec<u64> = s.tables.keys().copied().collect();
        assert!(chip_tables.len() > MAX_CASH_SEATS);
        for &t in &chip_tables[..MAX_CASH_SEATS] {
            s.join_table(p(1), t, 0, s.tables[&t].config.min_buy_in, None, 0).unwrap();
        }
        let t = chip_tables[MAX_CASH_SEATS];
        assert!(s.join_table(p(1), t, 0, s.tables[&t].config.min_buy_in, None, 0).unwrap_err().contains("at most"));

        s.user_mut(&p(1)).unwrap().friends = (0..MAX_FRIENDS as u32).map(|i| Principal::from_slice(&i.to_le_bytes())).collect();
        assert!(s.friend_request(p(2), p(1)).is_err(), "their friend list is full");
    }

    #[test]
    fn idle_players_are_stood_up() {
        let mut s = setup();
        let table = 1;
        s.join_table(p(1), table, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), table, 1, 1_000, None, 0).unwrap();
        s.sit_out(p(1), table, true, SEC).unwrap();
        s.tick(SEC + IDLE_STAND_NS - 1);
        assert!(s.tables[&table].seat_of(&p(1)).is_some());
        s.tick(SEC + IDLE_STAND_NS);
        assert!(s.tables[&table].seat_of(&p(1)).is_none(), "stood up after 10 minutes");
        assert_eq!(s.counters.idle_stand_ups, 1);
        assert!(s.tables[&table].seat_of(&p(2)).is_some());
    }

    #[test]
    fn icp_formatting() {
        assert_eq!(fmt_icp(E8S), "1");
        assert_eq!(fmt_icp(150_000_000), "1.5");
        assert_eq!(fmt_icp(10_000), "0.0001");
    }

    // ---------------------------------------------------------- table copies

    const HU: u64 = 3; // Heads Up Hangout, a 2-seat chip game (buy-in 1,000-5,000)

    #[test]
    fn copies_share_config_and_number_up() {
        let mut s = setup();
        let c2 = s.open_copy(HU, 0).unwrap();
        let c3 = s.open_copy(HU, 0).unwrap();
        assert_eq!(s.tables[&c2].copy_number, Some(2));
        assert_eq!(s.tables[&c3].copy_number, Some(3));
        for c in [c2, c3] {
            assert_eq!(format!("{:?}", s.tables[&c].config), format!("{:?}", s.tables[&HU].config));
            assert_eq!(s.game_of(c), HU);
        }
        assert_eq!(s.game_of(HU), HU);
        assert_eq!(s.game_tables(HU), vec![HU, c2, c3]);
    }

    #[test]
    fn copy_numbers_reuse_the_lowest_free() {
        let mut s = setup();
        let c2 = s.open_copy(HU, 0).unwrap();
        let c3 = s.open_copy(HU, 0).unwrap();
        s.tick(COPY_IDLE_CLOSE_NS); // both empty since 0: both close
        assert!(!s.tables.contains_key(&c2) && !s.tables.contains_key(&c3));
        let again = s.open_copy(HU, COPY_IDLE_CLOSE_NS).unwrap();
        assert_eq!(s.tables[&again].copy_number, Some(2));
    }

    #[test]
    fn copy_caps() {
        let mut s = setup();
        for _ in 0..MAX_COPIES_PER_GAME {
            s.open_copy(HU, 0).unwrap();
        }
        assert!(s.open_copy(HU, 0).unwrap_err().contains("is full right now"), "per-game cap");
        // Fill the play-chip budget across the other chip games plus one extra admin game.
        let extra = s.create_table(s.tables[&HU].config.clone()).unwrap();
        let chip_games: Vec<u64> = s
            .tables
            .values()
            .filter(|t| t.game.is_none() && t.config.currency == Currency::Chips && t.id != HU && t.id != extra)
            .map(|t| t.id)
            .collect();
        let mut opened = MAX_COPIES_PER_GAME;
        'fill: for g in chip_games {
            for _ in 0..MAX_COPIES_PER_GAME {
                if opened == MAX_CHIP_COPIES {
                    break 'fill;
                }
                s.open_copy(g, 0).unwrap();
                opened += 1;
            }
        }
        assert_eq!(opened, MAX_CHIP_COPIES);
        assert!(s.open_copy(extra, 0).is_err(), "play-chip budget used up");
        let icp_game = s.tables.values().find(|t| t.config.currency == Currency::Icp && !s.rooms.contains_key(&t.id)).unwrap().id;
        assert!(s.open_copy(icp_game, 0).is_ok(), "ICP games have their own budget");
    }

    #[test]
    fn rooms_and_copies_are_not_copyable() {
        let mut s = setup();
        s.user_mut(&p(1)).unwrap().xp = 50; // level 2: may host
        let room = s
            .create_room(
                p(1),
                RoomArgs { name: "Mark's Game".into(), currency: Currency::Chips, big_blind: 50, max_seats: 2, password: None, action_timeout_secs: 30 },
                0,
            )
            .unwrap();
        assert!(s.open_copy(room, 0).is_err());
        let c = s.open_copy(HU, 0).unwrap();
        assert!(s.open_copy(c, 0).is_err(), "copies come from the original");
        assert!(s.list_tables().iter().find(|t| t.id == room).unwrap().game_at_cap);
    }

    #[test]
    fn empty_copy_closes_after_a_minute() {
        let mut s = setup();
        let c = s.open_copy(HU, 0).unwrap();
        s.tick(COPY_IDLE_CLOSE_NS - SEC);
        assert!(s.tables.contains_key(&c));
        s.tick(COPY_IDLE_CLOSE_NS);
        assert!(!s.tables.contains_key(&c));
        s.tick(100 * COPY_IDLE_CLOSE_NS);
        assert!(s.tables.contains_key(&HU), "originals never close");
    }

    #[test]
    fn occupied_copy_never_closes() {
        let mut s = setup();
        let c = s.open_copy(HU, 0).unwrap();
        s.join_table(p(1), c, 0, 1_000, None, 0).unwrap();
        s.tick(5 * COPY_IDLE_CLOSE_NS);
        assert!(s.tables.contains_key(&c));
        assert_eq!(s.tables[&c].empty_since, None);
    }

    #[test]
    fn buy_back_minimum_covers_the_whole_game() {
        let mut s = setup();
        s.join_table(p(1), HU, 0, 3_000, None, 0).unwrap();
        s.leave_table(p(1), HU, SEC).unwrap();
        let c = s.open_copy(HU, SEC).unwrap();
        let err = s.join_table(p(1), c, 0, 1_000, None, 2 * SEC).unwrap_err();
        assert!(err.contains("less than 30 minutes ago"), "{err}");
        s.join_table(p(1), c, 0, 3_000, None, 2 * SEC).unwrap();
    }

    #[test]
    fn joining_a_closed_copy_fails_cleanly() {
        let mut s = setup();
        let c = s.open_copy(HU, 0).unwrap();
        s.tick(COPY_IDLE_CLOSE_NS);
        let before = s.users.get(&p(1)).unwrap().chips;
        assert_eq!(s.join_table(p(1), c, 0, 1_000, None, COPY_IDLE_CLOSE_NS).unwrap_err(), "No such table");
        assert_eq!(s.users.get(&p(1)).unwrap().chips, before);
    }

    #[test]
    fn summaries_and_views_carry_game_and_number() {
        let mut s = setup();
        let c = s.open_copy(HU, 0).unwrap();
        let rows = s.list_tables();
        let copy = rows.iter().find(|t| t.id == c).unwrap();
        assert_eq!((copy.game, copy.number, copy.game_at_cap), (HU, 2, false));
        let orig = rows.iter().find(|t| t.id == HU).unwrap();
        assert_eq!((orig.game, orig.number), (HU, 1));
        let v = s.table_view(c, None, 0).unwrap();
        assert_eq!((v.game, v.number), (HU, 2));
    }

    #[test]
    fn old_snapshots_decode_new_options_as_none() {
        #[derive(CandidType)]
        struct Old {
            a: u64,
        }
        #[derive(CandidType, Deserialize)]
        struct New {
            a: u64,
            b: Option<u64>,
        }
        let bytes = candid::encode_one(Old { a: 1 }).unwrap();
        let new: New = candid::decode_one(&bytes).unwrap();
        assert_eq!((new.a, new.b), (1, None));
    }

    // ---------------------------------------------------------- join_game

    #[test]
    fn join_game_fills_the_fullest_table_first() {
        let mut s = setup();
        let g = 1; // Free Roll Fun, 9 seats
        let c = s.open_copy(g, 0).unwrap();
        s.join_table(p(1), c, 0, 200, None, 0).unwrap();
        s.join_table(p(2), c, 1, 200, None, 0).unwrap();
        assert_eq!(s.join_game(p(3), g, 200, 0).unwrap(), c);
    }

    #[test]
    fn join_game_opens_a_copy_when_full() {
        let mut s = setup();
        s.register(p(4), "player4".into(), 0).unwrap();
        s.join_table(p(1), HU, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), HU, 1, 1_000, None, 0).unwrap();
        let c = s.join_game(p(3), HU, 1_000, 0).unwrap();
        assert_ne!(c, HU);
        assert_eq!(s.game_of(c), HU);
        assert_eq!(s.join_game(p(4), HU, 1_000, 0).unwrap(), c, "fills the half-empty copy before opening another");
        assert_eq!(s.game_tables(HU).len(), 2);
    }

    #[test]
    fn join_game_skips_tables_you_already_sit_at() {
        let mut s = setup();
        s.join_table(p(1), HU, 0, 1_000, None, 0).unwrap();
        let c = s.join_game(p(1), HU, 1_000, 0).unwrap();
        assert_ne!(c, HU);
        let seated = s.game_tables(HU).iter().filter(|id| s.tables[id].seat_of(&p(1)).is_some()).count();
        assert_eq!(seated, 2);
    }

    #[test]
    fn join_game_rejects_rooms_and_tournament_tables() {
        let mut s = setup();
        s.user_mut(&p(1)).unwrap().xp = 50;
        let room = s
            .create_room(
                p(1),
                RoomArgs { name: "Mark's Game".into(), currency: Currency::Chips, big_blind: 50, max_seats: 2, password: None, action_timeout_secs: 30 },
                0,
            )
            .unwrap();
        let tables = s.tables.len();
        assert!(s.join_game(p(2), room, 1_000, 0).is_err());
        let tourney_table = s.create_table(s.tables[&HU].config.clone()).unwrap();
        s.table_tournament.insert(tourney_table, 99);
        assert!(s.join_game(p(2), tourney_table, 1_000, 0).is_err());
        assert_eq!(s.tables.len(), tables + 1, "no copies were opened");
    }

    #[test]
    fn join_game_checks_money_before_opening_a_copy() {
        let mut s = setup();
        s.join_table(p(1), HU, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), HU, 1, 1_000, None, 0).unwrap();
        s.user_mut(&p(3)).unwrap().chips = 500;
        assert!(s.join_game(p(3), HU, 1_000, 0).is_err());
        assert!(s.join_game(p(3), HU, 100, 0).is_err(), "below the table minimum");
        assert_eq!(s.game_tables(HU).len(), 1, "no copy was opened");
    }

    #[test]
    fn join_game_at_cap_says_full() {
        let mut s = setup();
        s.join_table(p(1), HU, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), HU, 1, 1_000, None, 0).unwrap();
        for _ in 0..MAX_COPIES_PER_GAME {
            let c = s.open_copy(HU, 0).unwrap();
            // Fill each copy with two fresh players.
            for k in 0..2u8 {
                let who = Principal::from_slice(&[100 + (c as u8), k, 7, 7, 7, 7, 7, 7, 7, 7]);
                s.register(who, format!("f{c}x{k}"), 0).unwrap();
                s.join_table(who, c, k, 1_000, None, 0).unwrap();
            }
        }
        assert!(s.join_game(p(3), HU, 1_000, 0).unwrap_err().contains("is full right now"));
    }

    // ---------------------------------------------------------- auto-sit

    /// Heads Up Hangout full with player1 and player2; player3 and player4 registered too.
    fn full_hu() -> State {
        let mut s = setup();
        s.register(p(4), "player4".into(), 0).unwrap();
        s.join_table(p(1), HU, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), HU, 1, 1_000, None, 0).unwrap();
        s
    }

    fn queued(place: u32) -> impl Fn(&AutoSitStatus) -> bool {
        move |st| matches!(st, AutoSitStatus::Queued { place: p, .. } if *p == place)
    }

    #[test]
    fn auto_sit_seats_the_first_live_player_when_a_seat_opens() {
        let mut s = full_hu();
        assert!(queued(1)(&s.auto_sit(p(3), HU, Some(1_000), 0).unwrap()));
        assert!(queued(2)(&s.auto_sit(p(4), HU, Some(1_000), 0).unwrap()));
        s.leave_table(p(2), HU, SEC).unwrap();
        s.tick(2 * SEC);
        assert!(s.tables[&HU].seat_of(&p(3)).is_some());
        assert_eq!(s.auto_sit_ping(p(3), HU, 3 * SEC), AutoSitStatus::Seated { table: HU });
        assert!(queued(1)(&s.auto_sit_ping(p(4), HU, 3 * SEC)));
    }

    #[test]
    fn auto_sit_skips_players_not_seen_recently() {
        let mut s = full_hu();
        s.auto_sit(p(3), HU, Some(1_000), 0).unwrap();
        s.auto_sit(p(4), HU, Some(1_000), 0).unwrap();
        s.auto_sit_ping(p(4), HU, 30 * SEC);
        s.leave_table(p(2), HU, 31 * SEC).unwrap();
        s.tick(31 * SEC);
        assert!(s.tables[&HU].seat_of(&p(4)).is_some(), "the live player gets the seat");
        assert!(s.tables[&HU].seat_of(&p(3)).is_none());
        assert!(queued(1)(&s.auto_sit_ping(p(3), HU, 31 * SEC)), "skipped but still in line until it expires");
        // Not seen since 31s: dropped at 92s.
        s.tick(92 * SEC);
        assert_eq!(s.auto_sit_ping(p(3), HU, 92 * SEC), AutoSitStatus::NotQueued);
    }

    #[test]
    fn auto_sit_failure_is_reported_once() {
        let mut s = full_hu();
        s.auto_sit(p(3), HU, Some(1_000), 0).unwrap();
        s.user_mut(&p(3)).unwrap().chips = 10;
        s.leave_table(p(2), HU, SEC).unwrap();
        s.tick(SEC);
        assert!(s.tables[&HU].seat_of(&p(3)).is_none());
        match s.auto_sit_ping(p(3), HU, 2 * SEC) {
            AutoSitStatus::Dropped { reason } => assert!(!reason.is_empty()),
            other => panic!("expected Dropped, got {other:?}"),
        }
        assert_eq!(s.auto_sit_ping(p(3), HU, 3 * SEC), AutoSitStatus::NotQueued);
    }

    #[test]
    fn one_queue_per_player() {
        let mut s = full_hu();
        let c = s.open_copy(HU, 0).unwrap();
        s.auto_sit(p(3), HU, Some(1_000), 0).unwrap();
        s.auto_sit(p(3), c, Some(1_000), 0).unwrap();
        assert_eq!(s.auto_sit_ping(p(3), HU, SEC), AutoSitStatus::NotQueued);
        assert!(queued(1)(&s.auto_sit_ping(p(3), c, SEC)));
        assert_eq!(s.auto_sit(p(3), c, None, SEC).unwrap(), AutoSitStatus::NotQueued);
        assert_eq!(s.auto_sit_ping(p(3), c, SEC), AutoSitStatus::NotQueued);
    }

    #[test]
    fn manual_seat_click_beats_the_queue_and_leaves_it() {
        let mut s = full_hu();
        s.auto_sit(p(3), HU, Some(1_000), 0).unwrap();
        s.leave_table(p(2), HU, SEC).unwrap();
        s.join_table(p(4), HU, 1, 1_000, None, SEC).unwrap(); // first click wins
        assert!(queued(1)(&s.auto_sit_ping(p(3), HU, SEC)));
        s.leave_table(p(4), HU, 2 * SEC).unwrap();
        s.join_table(p(3), HU, 1, 1_000, None, 2 * SEC).unwrap();
        assert_eq!(s.tables[&HU].auto_sit.as_ref().map_or(0, |q| q.len()), 0, "sitting manually leaves the line");
    }

    #[test]
    fn join_game_skips_tables_with_a_queue() {
        let mut s = full_hu();
        let c = s.open_copy(HU, 0).unwrap();
        s.join_table(p(4), c, 0, 1_000, None, 0).unwrap();
        s.leave_table(p(2), HU, SEC).unwrap(); // HU has a free seat now...
        s.auto_sit(p(2), HU, Some(1_000), SEC).unwrap(); // ...but a line for it
        assert_eq!(s.join_game(p(3), HU, 1_000, SEC).unwrap(), c);
    }

    #[test]
    fn auto_sit_validates_buy_in() {
        let mut s = full_hu();
        assert!(s.auto_sit(p(3), HU, Some(100), 0).is_err(), "below the minimum");
        assert!(s.auto_sit(p(3), HU, Some(100_000), 0).is_err(), "above the maximum");
        s.user_mut(&p(3)).unwrap().chips = 500;
        assert!(s.auto_sit(p(3), HU, Some(1_000), 0).is_err(), "more than the balance");
        assert!(s.auto_sit(p(1), HU, Some(1_000), 0).is_err(), "already seated here");
    }

    #[test]
    fn table_view_shows_queue_state() {
        let mut s = full_hu();
        s.auto_sit(p(3), HU, Some(1_000), 0).unwrap();
        s.auto_sit(p(4), HU, Some(2_000), 0).unwrap();
        let v = s.table_view(HU, Some(p(4)), 0).unwrap();
        assert_eq!(v.auto_sit_waiting, 2);
        let mine = v.my_auto_sit.unwrap();
        assert_eq!((mine.place, mine.buy_in), (2, 2_000));
        assert!(s.table_view(HU, Some(p(1)), 0).unwrap().my_auto_sit.is_none());
    }

    #[test]
    fn upgrade_round_trip_keeps_queues() {
        use ic_stable_structures::VectorMemory;
        let mem = VectorMemory::default();
        let mut s = State::new_in(mem.clone(), Principal::management_canister(), 10_000);
        for k in 1..=4 {
            s.register(p(k), format!("player{k}"), 0).unwrap();
        }
        s.join_table(p(1), HU, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), HU, 1, 1_000, None, 0).unwrap();
        s.auto_sit(p(3), HU, Some(1_000), 0).unwrap();
        s.auto_sit(p(4), HU, Some(1_000), 0).unwrap();
        s.user_mut(&p(4)).unwrap().chips = 0;
        s.tables.get_mut(&HU).unwrap().auto_sit.as_mut().unwrap().swap(0, 1); // p4 first: fails
        s.leave_table(p(2), HU, SEC).unwrap();
        s.tick(SEC);
        s.save();
        drop(s);
        let mut r = State::restore(mem);
        assert!(matches!(r.auto_sit_ping(p(4), HU, 2 * SEC), AutoSitStatus::Dropped { .. }), "outcome survives");
        assert!(r.tables[&HU].seat_of(&p(3)).is_some());
    }

    // ---------------------------------------------------------- review fixes

    #[test]
    fn a_refused_buy_back_never_takes_the_buy_in() {
        let mut s = setup();
        s.join_table(p(1), HU, 0, 3_000, None, 0).unwrap();
        s.leave_table(p(1), HU, SEC).unwrap();
        let before = s.users.get(&p(1)).unwrap().chips;
        let c = s.open_copy(HU, SEC).unwrap();
        assert!(s.join_table(p(1), c, 0, 1_000, None, 2 * SEC).is_err());
        assert_eq!(s.users.get(&p(1)).unwrap().chips, before, "nothing debited");
    }

    #[test]
    fn auto_sit_refused_by_the_buy_back_rule_keeps_the_money() {
        let mut s = full_hu(); // p1, p2 at HU
        let c = s.open_copy(HU, 0).unwrap();
        s.join_table(p(3), c, 0, 1_000, None, 0).unwrap();
        s.join_table(p(4), c, 1, 1_000, None, 0).unwrap();
        // p1 waits for the copy with a small buy-in, then leaves HU with more.
        s.auto_sit(p(1), c, Some(1_000), 0).unwrap();
        s.users.get(&p(1)).unwrap();
        s.tables.get_mut(&HU).unwrap().seats[0].as_mut().unwrap().stack = 4_000;
        s.leave_table(p(1), HU, SEC).unwrap();
        let before = s.users.get(&p(1)).unwrap().chips;
        s.leave_table(p(4), c, SEC).unwrap();
        s.tick(2 * SEC);
        assert!(matches!(s.auto_sit_ping(p(1), c, 2 * SEC), AutoSitStatus::Dropped { .. }));
        assert_eq!(s.users.get(&p(1)).unwrap().chips, before, "nothing debited");
    }

    #[test]
    fn leaving_copies_in_any_order_keeps_the_biggest_stack_for_buy_back() {
        let mut s = setup();
        s.join_table(p(1), HU, 0, 5_000, None, 0).unwrap();
        let c = s.open_copy(HU, 0).unwrap();
        s.join_table(p(1), c, 0, 1_000, None, 0).unwrap();
        s.leave_table(p(1), HU, SEC).unwrap(); // 5,000
        s.leave_table(p(1), c, 2 * SEC).unwrap(); // 1,000 must not lower the minimum
        assert!(s.join_game(p(1), HU, 1_000, 3 * SEC).unwrap_err().contains("30 minutes"));
        assert_eq!(s.min_return_buy_in(HU, &p(1), 3 * SEC), 5_000);
    }

    #[test]
    fn joining_a_new_line_forgets_an_old_line_result() {
        let mut s = full_hu();
        let c = s.open_copy(HU, 0).unwrap();
        s.auto_sit(p(3), HU, Some(1_000), 0).unwrap();
        s.user_mut(&p(3)).unwrap().chips = 10;
        s.leave_table(p(2), HU, SEC).unwrap();
        s.tick(SEC); // p3 dropped at HU (can't afford it)
        s.user_mut(&p(3)).unwrap().chips = 5_000;
        s.join_table(p(4), c, 0, 1_000, None, SEC).unwrap();
        s.join_table(p(2), c, 1, 1_000, None, SEC).unwrap();
        s.auto_sit(p(3), c, Some(1_000), 2 * SEC).unwrap();
        assert!(queued(1)(&s.auto_sit_ping(p(3), c, 3 * SEC)), "no stale Dropped from the old line");
    }

    #[test]
    fn auto_sit_respects_locked_rooms_and_the_joins_pause() {
        let mut s = setup();
        s.user_mut(&p(1)).unwrap().xp = 50;
        let room = s
            .create_room(
                p(1),
                RoomArgs { name: "Locked".into(), currency: Currency::Chips, big_blind: 50, max_seats: 2, password: Some("aces".into()), action_timeout_secs: 30 },
                0,
            )
            .unwrap();
        assert!(s.auto_sit(p(2), room, Some(1_000), 0).is_err(), "not a guest of the locked room");
        s.pause.joins = true;
        assert!(s.auto_sit(p(2), HU, Some(1_000), 0).is_err(), "joins are paused");
    }

    #[test]
    fn a_copy_with_a_live_pot_never_closes() {
        let mut s = setup();
        let c = s.open_copy(HU, 0).unwrap();
        s.join_table(p(1), c, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), c, 1, 1_000, None, 0).unwrap();
        s.start_hand(c, [9; 32], 0);
        // Both marked as gone while the hand is still running (e.g. all-in, then left).
        for seat in s.tables.get_mut(&c).unwrap().seats.iter_mut().flatten() {
            seat.left = true;
        }
        assert_eq!(s.tables[&c].player_count(), 0);
        s.tick(5 * COPY_IDLE_CLOSE_NS);
        assert!(s.tables.contains_key(&c), "seats still hold chips in a live hand");
    }

    // ---- Rewards: points, tiers, decay and the spin meter ----

    fn account(s: &State, who: Principal) -> crate::rewards::RewardsAccount {
        s.users.get(&who).unwrap().rewards.unwrap_or_default()
    }

    fn table_hands(s: &State, table: u64) -> Vec<HandRecord> {
        s.stores.hands.iter().map(|e| e.value()).filter(|h| h.table_id == table).collect()
    }

    /// Each player's attributed rake summed over the given hands, in full or
    /// as the house's part (after a room host's cut).
    fn attributed(hands: &[HandRecord], house_only: bool) -> BTreeMap<Principal, u64> {
        let mut out = BTreeMap::new();
        for h in hands {
            let committed: Vec<(Principal, u64)> = h.players.iter().map(|x| (x.player, x.committed)).collect();
            for (who, share) in crate::rewards::attribute(h.rake, &committed) {
                let part = if house_only { share - share * HOST_RAKE_SHARE_BPS / 10_000 } else { share };
                *out.entry(who).or_default() += part;
            }
        }
        out
    }

    #[test]
    fn icp_hand_earns_points_by_contribution() {
        let mut s = setup();
        let table = 7; // Low Rollers
        let buy = s.tables[&table].config.max_buy_in;
        for (k, seat) in [(1, 0), (2, 3), (3, 5)] {
            s.join_table(p(k), table, seat, buy, None, 0).unwrap();
        }
        play_hands(&mut s, table, 6, SEC);
        let hands = table_hands(&s, table);
        let rake: u64 = hands.iter().map(|h| h.rake).sum();
        assert!(rake > 0);
        let shares = attributed(&hands, false);
        let mut earned = 0;
        for k in 1..=3 {
            let a = account(&s, p(k));
            let u = s.users.get(&p(k)).unwrap();
            assert_eq!(a.lifetime_points * 1_000 + a.carry_e8s, shares[&p(k)]);
            assert_eq!(a.tier_points, a.lifetime_points);
            assert_eq!(a.period.w_points, a.lifetime_points);
            assert_eq!(a.period.m_points, a.lifetime_points);
            assert_eq!(a.period.w_rake, shares[&p(k)]);
            assert_eq!(a.period.w_hands as u64, u.stats.hands_played);
            assert_eq!(a.period.m_hands as u64, u.stats.hands_played);
            assert_eq!(a.period.w_net, u.stats.net_winnings);
            assert_eq!(a.period.m_net, u.stats.net_winnings);
            earned += a.lifetime_points * 1_000 + a.carry_e8s;
        }
        // Only the attribution's rounding remainder goes to no one.
        assert!(earned <= rake && rake - earned < 3 * hands.len() as u64, "earned {earned} of {rake}");
    }

    #[test]
    fn chip_and_tournament_hands_earn_no_points() {
        use crate::tournament::TournamentStatus::*;
        let mut s = setup();
        let chip_table = 1;
        s.join_table(p(1), chip_table, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), chip_table, 1, 1_000, None, 0).unwrap();
        play_hands(&mut s, chip_table, 5, SEC);
        assert!(s.users.get(&p(1)).unwrap().stats.hands_played > 0);
        assert!(s.users.get(&p(1)).unwrap().rewards.is_none());
        assert!(s.users.get(&p(2)).unwrap().rewards.is_none());

        // A chip tournament: nothing at all.
        many_players(&mut s, 8);
        let id = open_tournament(&mut s, "Chip Sit & Go · 6 max");
        for k in 10..16 {
            s.register_tournament(p(k), id, SEC).unwrap();
        }
        run_until(&mut s, 2 * SEC, 5_000, |s| s.tournaments[&id].status == Finished);
        for k in 10..16 {
            assert!(s.users.get(&p(k)).unwrap().rewards.is_none());
        }

        // An ICP tournament: the fee earns points, its hands only count as hands.
        let id = open_tournament(&mut s, "ICP Heads-Up Duel");
        s.register_tournament(p(16), id, SEC).unwrap();
        s.register_tournament(p(17), id, SEC).unwrap();
        let fee = E8S / 400;
        run_until(&mut s, 2 * SEC, 5_000, |s| s.tournaments[&id].status == Finished);
        for k in 16..18 {
            let a = account(&s, p(k));
            assert_eq!(a.lifetime_points, fee / 1_000);
            assert_eq!(a.period.w_rake, fee);
            let hands = s.users.get(&p(k)).unwrap().stats.hands_played;
            assert!(hands > 0);
            assert_eq!(a.period.w_hands as u64, hands);
            // Tournament chips are not ICP won or lost; the event's prize minus its costs is.
            let prize = s.tournaments[&id].entrants.iter().find(|e| e.player == p(k)).unwrap().prize;
            assert_eq!(a.period.w_net, prize as i64 - (E8S / 20 + fee) as i64);
        }
    }

    #[test]
    fn crossing_a_step_banks_a_spin_at_the_current_tier() {
        use crate::rewards::BankedSpin;
        let mut s = setup();
        s.earn_rake(&p(1), 99_000, 99_000, SEC);
        let a = account(&s, p(1));
        assert_eq!((a.lifetime_points, a.meter, a.meter_stake), (99, 99, 99_000));
        assert!(a.spins.is_empty());
        let mut s = setup();
        s.earn_rake(&p(1), 100_000, 100_000, SEC);
        let a = account(&s, p(1));
        assert_eq!(a.lifetime_points, 100);
        assert_eq!(a.tier_points, 100);
        assert_eq!(a.spins, vec![BankedSpin { stake: 100_000, tier: 0 }]);
        assert_eq!((a.meter, a.meter_stake), (0, 0));
        assert_eq!(a.last_earned_at, SEC);
        // Leftover e8s carry forward.
        s.earn_rake(&p(1), 1_500, 1_500, 2 * SEC);
        s.earn_rake(&p(1), 600, 600, 3 * SEC);
        let a = account(&s, p(1));
        assert_eq!((a.lifetime_points, a.carry_e8s, a.meter), (102, 100, 2));
        // Unregistered players are ignored.
        s.earn_rake(&p(99), 100_000, 100_000, SEC);
        assert!(s.users.get(&p(99)).is_none());
    }

    #[test]
    fn one_big_pot_banks_several_spins_and_caps_at_fifty() {
        let mut s = setup();
        let house = 1_000 * E8S;
        s.earn_rake(&p(1), house, house, SEC);
        let a = account(&s, p(1));
        assert_eq!(a.lifetime_points, house / 1_000);
        assert_eq!(a.spins.len(), crate::rewards::MAX_SPINS);
        assert_eq!(a.spins.len(), 50);
        let stakes: u64 = a.spins.iter().map(|x| x.stake).sum();
        assert!(stakes > 0 && stakes <= house, "stakes {stakes}");
        assert!(a.spins.iter().all(|x| x.stake == a.spins[0].stake && x.tier == 6));
        // A full bank stays full; the meter keeps filling below a step.
        s.earn_rake(&p(1), house, house, 2 * SEC);
        let b = account(&s, p(1));
        assert_eq!(b.spins, a.spins);
        assert!(b.meter < crate::rewards::TIERS[6].step);
        assert_eq!(b.lifetime_points, 2 * house / 1_000);
        // Several spins in one call, below the cap: the stake is split evenly.
        let mut s = setup();
        s.earn_rake(&p(2), 300_000, 300_000, SEC);
        let a = account(&s, p(2));
        assert_eq!(a.spins.iter().map(|x| x.stake).collect::<Vec<_>>(), vec![100_000; 3]);
    }

    #[test]
    fn hosted_room_spin_stake_is_the_house_share() {
        let mut s = setup();
        s.user_mut(&p(1)).unwrap().xp = 50;
        let id = s
            .create_room(
                p(1),
                RoomArgs { name: "Spins".into(), currency: Currency::Icp, big_blind: 2_000_000, max_seats: 6, password: None, action_timeout_secs: 30 },
                0,
            )
            .unwrap();
        s.join_table(p(2), id, 1, 100_000_000, None, 0).unwrap();
        s.join_table(p(1), id, 0, 100_000_000, None, 0).unwrap();
        play_hands(&mut s, id, 10, SEC);
        let hands = table_hands(&s, id);
        let shares = attributed(&hands, false);
        let house_of = attributed(&hands, true);
        let mut banked = 0;
        for k in 1..=2 {
            let a = account(&s, p(k));
            // Points count the full rake...
            assert_eq!(a.lifetime_points * 1_000 + a.carry_e8s, shares[&p(k)]);
            // ...spins are staked with the house's 80%.
            let stakes: u64 = a.spins.iter().map(|x| x.stake).sum();
            assert_eq!(stakes + a.meter_stake, house_of[&p(k)]);
            assert!(house_of[&p(k)] < shares[&p(k)]);
            banked += a.spins.len();
        }
        assert!(banked > 0, "the test should bank at least one spin");
    }

    #[test]
    fn decay_lowers_the_tier_but_keeps_spins() {
        use crate::rewards::{tier_index, BankedSpin};
        let mut s = setup();
        s.earn_rake(&p(1), 500_000, 500_000, 0);
        let a = account(&s, p(1));
        assert_eq!(tier_index(a.tier_points), 1);
        assert_eq!(a.spins, vec![BankedSpin { stake: 500_000, tier: 1 }]);
        assert_eq!(a.meter, 100, "Shrimp steps are 400 points");
        // Sub-point rake neither earns points nor restarts the decay clock.
        s.earn_rake(&p(1), 500, 500, 20 * DAY_NS);
        assert_eq!(account(&s, p(1)).last_earned_at, 0);
        s.earn_rake(&p(1), 500, 500, 30 * DAY_NS);
        let a = account(&s, p(1));
        assert_eq!(a.tier_points, crate::rewards::decayed(500, 0, 30 * DAY_NS) + 1);
        assert_eq!(tier_index(a.tier_points), 0, "back to Plankton");
        assert_eq!(a.lifetime_points, 501);
        assert_eq!(a.last_earned_at, 30 * DAY_NS);
        assert_eq!(a.spins[0], BankedSpin { stake: 500_000, tier: 1 }, "banked spins are kept");
        // The meter follows lifetime points: 100 + 1 crosses a Plankton step.
        assert_eq!(a.spins.len(), 2);
        assert_eq!(a.spins[1].tier, 0);
    }

    #[test]
    fn tournament_fee_earns_points_at_start_only() {
        use crate::tournament::TournamentStatus::*;
        let mut s = setup();
        many_players(&mut s, 4);
        let fee = E8S / 40;
        let id = open_tournament(&mut s, "ICP Nightly");
        for k in 10..12 {
            s.register_tournament(p(k), id, SEC).unwrap();
        }
        assert!(s.users.get(&p(10)).unwrap().rewards.is_none(), "registering earns nothing");
        s.tick(s.tournaments[&id].start_at);
        assert_eq!(s.tournaments[&id].status, Cancelled);
        assert!(s.users.get(&p(10)).unwrap().rewards.is_none(), "a cancelled event earns nothing");

        let (id, start_at) = start_scheduled(&mut s, "ICP Nightly", 10..13);
        for k in 10..13 {
            let a = account(&s, p(k));
            assert_eq!(a.lifetime_points, fee / 1_000);
            assert_eq!(a.carry_e8s, 0);
            assert_eq!(a.period.w_rake, fee);
        }
        // A late entry's fee is final at once, so it earns at registration.
        s.register_tournament(p(13), id, start_at + SEC).unwrap();
        assert_eq!(account(&s, p(13)).lifetime_points, fee / 1_000);
    }

    // ---- Rewards: the wheel ----

    fn bank_spin(s: &mut State, who: Principal, stake: u64, tier: u8) {
        let mut u = s.users.get_mut(&who).unwrap();
        let a = u.rewards.get_or_insert_with(Default::default);
        a.spins.push(crate::rewards::BankedSpin { stake, tier });
    }

    #[test]
    fn wheel_expected_value_is_exactly_one() {
        use crate::rewards::WHEEL;
        let ev: u64 = WHEEL.iter().map(|(m, w)| m * w).sum();
        let weights: u64 = WHEEL.iter().map(|(_, w)| w).sum();
        assert_eq!(ev, 10 * weights);
        assert_eq!(WHEEL.map(|(m, _)| m), [5, 10, 15, 20, 50, 100]);
        let min = WHEEL.iter().map(|(_, w)| *w).min().unwrap();
        assert_eq!(WHEEL[5].1, min, "the 10x segment is the rarest");
    }

    #[test]
    fn spin_pays_stake_times_rate_times_multiplier() {
        let mut s = setup();
        bank_spin(&mut s, p(1), 1_000_000, 1);
        s.rewards_mut().fish_pool = 10 * E8S;
        let (bal, pool) = (s.users.get(&p(1)).unwrap().balance, s.rewards().fish_pool);
        let r = s.spin_wheel(p(1), 5 * SEC).unwrap();
        let mult = crate::rewards::WHEEL[r.segment as usize].0;
        assert_eq!(r.multiplier_x10, mult);
        assert_eq!(r.prize, 1_000_000 * 1100 / 10_000 * mult / 10);
        assert_eq!((r.paid, r.capped, r.stake, r.time), (r.prize, false, 1_000_000, 5 * SEC));
        assert_eq!(r.tier, "Shrimp");
        assert_eq!(s.users.get(&p(1)).unwrap().balance, bal + r.paid);
        assert_eq!(s.rewards().fish_pool, pool - r.paid);
        assert!(account(&s, p(1)).spins.is_empty());
    }

    #[test]
    fn spin_is_capped_by_the_pool() {
        let mut s = setup();
        bank_spin(&mut s, p(1), 1_000_000, 1);
        s.rewards_mut().fish_pool = 3;
        let bal = s.users.get(&p(1)).unwrap().balance;
        let r = s.spin_wheel(p(1), SEC).unwrap();
        assert!(r.prize > 3);
        assert_eq!((r.paid, r.capped), (3, true));
        assert_eq!(s.rewards().fish_pool, 0);
        assert_eq!(s.users.get(&p(1)).unwrap().balance, bal + 3);
    }

    #[test]
    fn no_spins_is_an_error() {
        let mut s = setup();
        assert_eq!(s.spin_wheel(p(1), SEC).unwrap_err(), "No spins yet");
        assert!(s.spin_wheel(p(99), SEC).is_err());
    }

    #[test]
    fn spin_history_keeps_twenty() {
        let mut s = setup();
        s.rewards_mut().fish_pool = 100 * E8S;
        for k in 0..25 {
            bank_spin(&mut s, p(1), 1_000 + k, 0);
        }
        for k in 0..25 {
            s.spin_wheel(p(1), k).unwrap();
        }
        let a = account(&s, p(1));
        assert_eq!(a.history.len(), 20);
        assert_eq!(a.history.front().unwrap().stake, 1_005, "oldest five dropped");
        assert_eq!(a.history.back().unwrap().stake, 1_024);
        // The draw depends on how many were taken, so spins differ.
        let segs: std::collections::BTreeSet<u8> = a.history.iter().map(|r| r.segment).collect();
        assert!(segs.len() > 1);
    }

    #[test]
    fn spins_conserve_money() {
        let mut s = setup();
        s.rewards_mut().fish_pool = 5 * E8S;
        for _ in 0..10 {
            bank_spin(&mut s, p(2), 400_000_000, 3);
        }
        let before = (total_value(&s), house_take(&s), s.wallet_total);
        let pool_before = s.rewards().fish_pool;
        for k in 0..10 {
            s.spin_wheel(p(2), k).unwrap();
        }
        // Wallets gained exactly what the pool lost; total liabilities are unchanged.
        let paid = pool_before - s.rewards().fish_pool;
        assert!(paid > 0);
        assert_eq!(total_value(&s), before.0);
        assert_eq!(house_take(&s), before.1 - paid);
        assert_eq!(s.wallet_total, before.2 + paid);
    }

    #[test]
    fn rewards_view_decays_and_zero_view_for_strangers() {
        let mut s = setup();
        let z = s.rewards_view(&p(77), 99 * DAY_NS);
        assert_eq!((z.tier, z.tier_points, z.spins_banked, z.next_tier_at, z.decay_starts_in_days), (0, 0, 0, Some(500), None));
        assert_eq!((z.tier_name.as_str(), z.step, z.tiers.len(), z.days_since_earned), ("Plankton", 100, 7, 0));
        s.earn_rake(&p(1), 500_000, 500_000, 0);
        let v = s.rewards_view(&p(1), 3 * DAY_NS);
        assert_eq!((v.tier, v.tier_points, v.decay_starts_in_days, v.days_since_earned), (1, 500, Some(4), 3));
        assert_eq!((v.spins_banked, v.meter, v.next_tier_at, v.step), (1, 100, Some(2_500), 400));
        let v = s.rewards_view(&p(1), 30 * DAY_NS);
        assert_eq!(v.tier_points, crate::rewards::decayed(500, 0, 30 * DAY_NS));
        assert_eq!((v.tier, v.decay_starts_in_days), (0, Some(0)));
        assert_eq!(v.lifetime_points, 500);
    }
}
