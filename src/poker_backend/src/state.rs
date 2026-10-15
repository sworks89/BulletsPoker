//! Canister state and the business logic around it: profiles, wallets, the
//! item store, gifts, chat, friends and tables. Everything here is pure (the
//! caller and the current time are passed in) so it can be unit tested
//! natively; `lib.rs` wraps it in canister endpoints.

use crate::table::{Action, Currency, HandResult, LogEntry, Phase, Table, TableConfig};
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
const MAX_ROOMS_PER_USER: usize = 2;
const MAX_NOTE_LEN: usize = 500;
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
            auto_muck: true,
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
    /// Private notes about other players.
    pub notes: BTreeMap<Principal, PlayerNote>,
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

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct State {
    pub ledger: Principal,
    pub ledger_fee: u64,
    pub users: BTreeMap<Principal, User>,
    pub usernames: BTreeMap<String, Principal>,
    pub tables: BTreeMap<u64, Table>,
    pub next_table_id: u64,
    /// Last hand number per table whose result was applied to stats/treasury.
    pub applied_hand: BTreeMap<u64, u64>,
    pub items: BTreeMap<u32, StoreItem>,
    pub next_item_id: u32,
    pub chats: BTreeMap<Channel, VecDeque<ChatMessage>>,
    pub dms: BTreeMap<(Principal, Principal), VecDeque<ChatMessage>>,
    pub next_msg_id: u64,
    /// House revenue (rake and store sales) in e8s, withdrawable by controllers.
    pub treasury: u64,
    pub hands: BTreeMap<u64, HandRecord>,
    pub next_hand_id: u64,
    pub throws: BTreeMap<u64, VecDeque<ThrowEvent>>,
    pub next_throw_id: u64,
    /// Player-created tables, by table id.
    pub rooms: BTreeMap<u64, Room>,
    pub tournaments: BTreeMap<u64, Tournament>,
    pub next_tournament_id: u64,
    /// Table id -> tournament id for tournament tables.
    pub table_tournament: BTreeMap<u64, u64>,
    /// Recurring tournaments; a fresh instance opens whenever the previous one starts.
    pub templates: Vec<TournamentTemplate>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Room {
    pub owner: Principal,
    /// SHA-256 of (table id, password) for password-protected rooms.
    pub password_hash: Option<Vec<u8>>,
    pub created_at: u64,
    pub last_active: u64,
}

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
    /// The viewer's own note about this player, if any.
    pub note: Option<PlayerNote>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TableView {
    pub id: u64,
    pub config: TableConfig,
    pub phase: Phase,
    pub hand_no: u64,
    pub seats: Vec<Option<SeatView>>,
    pub board: Vec<u8>,
    pub pot: u64,
    pub current_bet: u64,
    pub button: u8,
    pub to_act: Option<u8>,
    pub action_deadline: u64,
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
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TableSummary {
    pub id: u64,
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
    /// What the canister's main ledger account must hold to cover everything.
    pub liabilities_total: u64,
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
    pub fn new(ledger: Principal, ledger_fee: u64) -> Self {
        let mut s = State {
            ledger,
            ledger_fee,
            users: BTreeMap::new(),
            usernames: BTreeMap::new(),
            tables: BTreeMap::new(),
            next_table_id: 1,
            applied_hand: BTreeMap::new(),
            items: BTreeMap::new(),
            next_item_id: 1,
            chats: BTreeMap::new(),
            dms: BTreeMap::new(),
            next_msg_id: 1,
            treasury: 0,
            hands: BTreeMap::new(),
            next_hand_id: 1,
            throws: BTreeMap::new(),
            next_throw_id: 1,
            rooms: BTreeMap::new(),
            tournaments: BTreeMap::new(),
            next_tournament_id: 1,
            table_tournament: BTreeMap::new(),
            templates: crate::tournament::default_templates(),
        };
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

    pub(crate) fn user(&self, p: &Principal) -> R<&User> {
        self.users.get(p).ok_or_else(|| "Create a profile first".to_string())
    }

    pub(crate) fn user_mut(&mut self, p: &Principal) -> R<&mut User> {
        self.users.get_mut(p).ok_or_else(|| "Create a profile first".to_string())
    }

    pub fn touch(&mut self, p: &Principal, now: u64) {
        if let Some(u) = self.users.get_mut(p) {
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
        let key = username.to_lowercase();
        if self.usernames.contains_key(&key) {
            return Err("Username is taken".into());
        }
        let avatar = self.first_free(ItemKind::Avatar);
        let card_back = self.first_free(ItemKind::CardBack);
        self.usernames.insert(key, caller);
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
                notes: BTreeMap::new(),
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
            online: self.is_online(p, u, now),
            tables: self.tables_of(p),
            medals: u.medals.keys().cloned().collect(),
        })
    }

    pub fn find_user(&self, username: &str) -> Option<Principal> {
        self.usernames.get(&username.to_lowercase()).copied()
    }

    pub fn set_status(&mut self, caller: Principal, status: String) -> R<()> {
        if status.chars().count() > 80 {
            return Err("Status is limited to 80 characters".into());
        }
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
        let owned = self.owns(self.user(&caller)?, &item);
        if owned {
            return Err("You already own this item".into());
        }
        self.pay_for(&caller, &item, currency)?;
        let u = self.user_mut(&caller)?;
        u.inventory.push(id);
        u.xp += 10;
        Ok(())
    }

    pub fn equip(&mut self, caller: Principal, id: u32) -> R<()> {
        let item = self.items.get(&id).cloned().ok_or("No such item")?;
        if !self.owns(self.user(&caller)?, &item) {
            return Err("You don't own this item".into());
        }
        let u = self.user_mut(&caller)?;
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
        let u = self.user_mut(&caller)?;
        u.xp += 5;
        u.gifts_sent += 1;
        if u.gifts_sent >= 10 {
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
        let u = self.user_mut(&caller)?;
        u.last_throw_at = now;
        u.throws_made += 1;
        if u.throws_made >= 25 {
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

    pub fn credit_in(&mut self, p: &Principal, currency: Currency, amount: u64) -> R<()> {
        let bal = Self::purse(self.user_mut(p)?, currency);
        *bal = bal.checked_add(amount).ok_or("Overflow")?;
        Ok(())
    }

    pub fn debit_in(&mut self, p: &Principal, currency: Currency, amount: u64) -> R<()> {
        let bal = Self::purse(self.user_mut(p)?, currency);
        if *bal < amount {
            return Err(match currency {
                Currency::Icp => "Insufficient ICP balance".into(),
                Currency::Chips => "Not enough chips".into(),
            });
        }
        *bal -= amount;
        Ok(())
    }

    /// Claims the daily free-chip bonus. Claiming on consecutive days grows the bonus.
    pub fn claim_daily(&mut self, caller: Principal, now: u64) -> R<u64> {
        let u = self.user_mut(&caller)?;
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
        if u.daily_streak >= MAX_STREAK {
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
        let text = text.trim().to_string();
        if text.chars().count() > MAX_NOTE_LEN {
            return Err(format!("Notes are limited to {MAX_NOTE_LEN} characters"));
        }
        let notes = &mut self.user_mut(&caller)?.notes;
        if text.is_empty() && color.is_none() {
            notes.remove(&player);
            return Ok(());
        }
        if !notes.contains_key(&player) && notes.len() >= MAX_NOTES {
            return Err(format!("You can keep up to {MAX_NOTES} notes"));
        }
        notes.insert(player, PlayerNote { text, color, updated_at: now });
        Ok(())
    }

    pub fn notes(&self, caller: Principal) -> Vec<NoteView> {
        let Some(me) = self.users.get(&caller) else { return vec![] };
        let mut v: Vec<NoteView> = me
            .notes
            .iter()
            .map(|(p, n)| {
                let u = self.users.get(p);
                NoteView {
                    player: *p,
                    username: u.map(|u| u.username.clone()).unwrap_or_default(),
                    avatar: u.map(|u| self.icon(u.avatar)).unwrap_or_default(),
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

    pub fn house_stats(&self) -> HouseStats {
        let wallet_balances: u64 = self.users.values().map(|u| u.balance).sum();
        // Play-chip tables are not backed by ICP.
        let chips_on_tables: u64 = self
            .tables
            .values()
            .filter(|t| t.config.currency == Currency::Icp)
            .map(|t| t.chips_in_play() + t.rake_accrued)
            .sum();
        HouseStats {
            users: self.users.len() as u64,
            wallet_balances,
            chips_on_tables,
            treasury: self.treasury,
            liabilities_total: wallet_balances + chips_on_tables + self.treasury + self.icp_in_tournaments(),
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
        let u = self.user_mut(caller)?;
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
        }
        let name = self.check_chat(&caller, &text, now)?;
        self.post(ch, Some(caller), name, text.trim().to_string(), MessageKind::Text, now);
        Ok(())
    }

    pub fn chat_since(&self, ch: Channel, since: u64) -> Vec<ChatMessage> {
        self.chats
            .get(&ch)
            .map(|q| q.iter().filter(|m| m.id > since).cloned().collect())
            .unwrap_or_default()
    }

    pub fn send_dm(&mut self, caller: Principal, to: Principal, text: String, now: u64) -> R<()> {
        if !self.user(&caller)?.friends.contains(&to) {
            return Err("You can only message friends".into());
        }
        let name = self.check_chat(&caller, &text, now)?;
        let id = self.next_msg_id;
        self.next_msg_id += 1;
        let q = self.dms.entry(dm_key(caller, to)).or_default();
        q.push_back(ChatMessage {
            id,
            from: Some(caller),
            username: name,
            text: text.trim().to_string(),
            time: now,
            kind: MessageKind::Text,
        });
        while q.len() > DM_HISTORY {
            q.pop_front();
        }
        Ok(())
    }

    pub fn dms_with(&self, caller: Principal, other: Principal, since: u64) -> Vec<ChatMessage> {
        self.dms
            .get(&dm_key(caller, other))
            .map(|q| q.iter().filter(|m| m.id > since).cloned().collect())
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
        // If they already asked us, accept straight away.
        if self.user(&caller)?.friend_requests.contains(&to) {
            return self.respond_friend(caller, to, true);
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
                if self.users[&p].friends.len() >= 10 {
                    self.award(&p, "popular", None, 0);
                }
            }
        }
        Ok(())
    }

    pub fn remove_friend(&mut self, caller: Principal, other: Principal) -> R<()> {
        self.user_mut(&caller)?.friends.remove(&other);
        if let Ok(u) = self.user_mut(&other) {
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
                    online: self.is_online(f, u, now),
                    tables: self.tables_of(f),
                })
            })
            .collect()
    }

    pub fn leaderboard(&self, limit: usize) -> Vec<LeaderboardEntry> {
        let mut v: Vec<LeaderboardEntry> = self
            .users
            .iter()
            .filter(|(_, u)| u.stats.hands_played > 0)
            .map(|(p, u)| LeaderboardEntry {
                id: *p,
                username: u.username.clone(),
                avatar: self.icon(u.avatar),
                level: level_for_xp(u.xp),
                stats: u.stats.clone(),
            })
            .collect();
        v.sort_by_key(|e| std::cmp::Reverse(e.stats.net_winnings));
        v.truncate(limit);
        v
    }

    // -------------------------------------------------------------- tables

    pub fn create_table(&mut self, cfg: TableConfig) -> R<u64> {
        cfg.validate()?;
        let id = self.next_table_id;
        self.next_table_id += 1;
        self.tables.insert(id, Table::new(id, cfg));
        Ok(id)
    }

    /// Creates a player-hosted table. ICP rooms pay the host a share of the rake.
    pub fn create_room(&mut self, caller: Principal, args: RoomArgs, now: u64) -> R<u64> {
        self.user(&caller)?;
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
            name: args.name.trim().to_string(),
            currency: args.currency,
            small_blind: bb / 2,
            big_blind: bb,
            min_buy_in: bb * 20,
            max_buy_in: bb * 100,
            max_seats: args.max_seats,
            rake_bps: if icp { 250 } else { 0 },
            rake_cap: if icp { bb * 3 } else { 0 },
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
        let name = self.user(&caller)?.username.clone();
        let currency = self.table_currency(id)?;
        if let Some(room) = self.rooms.get(&id) {
            if let Some(hash) = &room.password_hash {
                let ok = room.owner == caller || password.is_some_and(|pw| room_password_hash(id, &pw) == *hash);
                if !ok {
                    return Err("Wrong table password".into());
                }
            }
        }
        // Check funds before seating so a failed debit can't leave chips on the table.
        self.debit_in(&caller, currency, buy_in)?;
        if let Err(e) = self.table_mut(id)?.sit(caller, seat, buy_in) {
            self.credit_in(&caller, currency, buy_in)?;
            return Err(e);
        }
        self.post(Channel::Table(id), None, String::new(), format!("{name} sat down"), MessageKind::System, now);
        Ok(())
    }

    pub fn leave_table(&mut self, caller: Principal, id: u64, now: u64) -> R<u64> {
        let currency = self.table_currency(id)?;
        let chips = self.table_mut(id)?.stand(&caller, now)?;
        self.credit_in(&caller, currency, chips)?;
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

    pub fn sit_out(&mut self, caller: Principal, id: u64, out: bool) -> R<()> {
        self.table_mut(id)?.set_sitting_out(&caller, out)
    }

    pub fn act(&mut self, caller: Principal, id: u64, action: Action, now: u64) -> R<()> {
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
        if currency == Currency::Icp && rake > 0 {
            // Room hosts earn a share of the rake their table generates.
            let host = self.rooms.get(&id).map(|r| r.owner);
            let host_cut = if host.is_some() { rake * HOST_RAKE_SHARE_BPS / 10_000 } else { 0 };
            if let Some(h) = host {
                if self.credit(&h, host_cut).is_err() {
                    self.treasury += host_cut;
                }
            }
            self.treasury += rake - host_cut;
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

        for pr in &result.players {
            if let Some(u) = self.users.get_mut(&pr.player) {
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
            }
        }
        for w in &result.winners {
            let name = self.users.get(&w.player).map(|u| u.username.clone()).unwrap_or_default();
            let text = if in_tournament {
                format!("{} wins {} chips ({})", name, w.amount, w.hand)
            } else {
                format!("{} wins {} ({})", name, fmt_amount(currency, w.amount), w.hand)
            };
            self.post(Channel::Table(id), None, String::new(), text, MessageKind::System, now);
        }
        self.award_hand_medals(id, (!in_tournament).then_some(currency), &result, now);
        if in_tournament {
            self.after_tournament_hand(id, now);
        } else {
            self.auto_top_up(id);
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
            if let Some(u) = self.users.get_mut(&p.player) {
                u.history.push_back(id);
                while u.history.len() > HISTORY_PER_USER {
                    u.history.pop_front();
                }
            }
        }
        self.hands.insert(id, record);
        while self.hands.len() > HISTORY_MAX {
            self.hands.pop_first();
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
            let available = self.users.get_mut(&p).map_or(0, |u| *Self::purse(u, currency));
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
            .filter_map(|id| self.hands.get(id))
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
        let mut h = self.hands.get(&id)?.clone();
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
        self.tournament_tick(now);
        self.close_idle_rooms(now);
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
                    ready.push(id);
                } else if t.phase == Phase::HandOver {
                    t.settle_after_hand();
                }
            }
        }
        ready
    }

    pub fn start_hand(&mut self, id: u64, seed: [u8; 32], now: u64) {
        if let Some(t) = self.tables.get_mut(&id) {
            if t.ready_for_next_hand(now) && t.start_hand(seed, now).is_ok() {
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
        let t = self.tables.get(&id)?;
        let my_seat = viewer.and_then(|v| t.seat_of(&v));
        let seats = t
            .seats
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let s = s.as_ref().filter(|s| !s.left)?;
                let u = self.users.get(&s.player);
                Some(SeatView {
                    seat: i as u8,
                    player: s.player,
                    username: u.map(|u| u.username.clone()).unwrap_or_default(),
                    avatar: u.map(|u| self.icon(u.avatar)).unwrap_or_default(),
                    card_back: u.map(|u| self.icon(u.card_back)).unwrap_or_default(),
                    level: u.map_or(1, |u| level_for_xp(u.xp)),
                    stack: s.stack,
                    bet: s.bet,
                    in_hand: s.in_hand,
                    folded: s.folded,
                    all_in: s.all_in,
                    sitting_out: s.sitting_out,
                    has_cards: s.in_hand && !s.folded && !s.hole.is_empty(),
                    cards: t.visible_cards(i, viewer.as_ref()),
                    last_action: s.last_action.clone(),
                    gift: u
                        .and_then(|u| u.gift.as_ref())
                        .filter(|g| g.expires_at > now)
                        .map(|g| self.icon(g.item)),
                    note: viewer
                        .and_then(|v| self.users.get(&v))
                        .and_then(|me| me.notes.get(&s.player))
                        .cloned(),
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
            phase: t.phase,
            hand_no: t.hand_no,
            seats,
            board: t.board.clone(),
            pot,
            current_bet: t.current_bet,
            button: t.button as u8,
            to_act: t.to_act.map(|i| i as u8),
            action_deadline: t.action_deadline,
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
        })
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
        assert_eq!(s.users[&p(1)].balance, 10 * E8S - E8S / 10);
        assert_eq!(s.treasury, E8S / 10);

        let beer = s.items.values().find(|i| i.name == "Beer").unwrap().id;
        let icp_table = 5; // Penny Lane
        assert!(s.send_gift(p(1), icp_table, p(2), beer, Currency::Icp, 0).is_err()); // p2 not seated
        s.join_table(p(2), icp_table, 0, 1_000_000, None, 0).unwrap();
        s.send_gift(p(1), icp_table, p(2), beer, Currency::Icp, 0).unwrap();
        let v = s.table_view(icp_table, None, 1).unwrap();
        assert_eq!(v.seats[0].as_ref().unwrap().gift.as_deref(), Some("🍺"));
        // Gifts expire lazily: they simply stop being shown.
        let v = s.table_view(icp_table, None, GIFT_DURATION_NS + 1).unwrap();
        assert!(v.seats[0].as_ref().unwrap().gift.is_none());
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
        let played: u64 = (1..=3).map(|k| s.users[&p(k)].stats.hands_played).sum();
        assert_eq!(played, 60);
        let net: i64 = (1..=3).map(|k| s.users[&p(k)].stats.net_winnings).sum();
        assert_eq!(net, -(s.treasury as i64));

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
        let wallets: u64 = (1..=3).map(|k| s.users[&p(k)].balance).sum();
        assert_eq!(wallets + s.treasury, 30 * E8S);
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
        assert_eq!(s.users[&p(1)].chips, SIGNUP_CHIPS);
        let chip_table = 1; // Free Roll Fun, 5/10 chips
        assert_eq!(s.tables[&chip_table].config.currency, Currency::Chips);
        let before = s.house_stats();
        s.join_table(p(1), chip_table, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), chip_table, 1, 1_000, None, 0).unwrap();
        // ICP is untouched and chip tables don't count as ICP liabilities.
        assert_eq!(s.users[&p(1)].balance, 10 * E8S);
        assert_eq!(s.users[&p(1)].chips, SIGNUP_CHIPS - 1_000);
        assert_eq!(s.house_stats().liabilities_total, before.liabilities_total);
        let now = play_hands(&mut s, chip_table, 5, SEC);
        assert_eq!(s.treasury, 0, "chip tables are rake free");
        s.leave_table(p(1), chip_table, now).unwrap();
        s.leave_table(p(2), chip_table, now).unwrap();
        let chips: u64 = (1..=2).map(|k| s.users[&p(k)].chips).sum();
        assert_eq!(chips, 2 * SIGNUP_CHIPS);
        assert_eq!(s.users[&p(1)].stats.chip_net + s.users[&p(2)].stats.chip_net, 0);
        assert_eq!(s.users[&p(1)].stats.net_winnings, 0);
        // Not enough chips to sit at the 250/500 table (min buy-in 10,000 is fine, 50,000 is not).
        assert!(s.join_table(p(3), 4, 0, 50_000, None, now).is_err());
        assert_eq!(s.users[&p(3)].chips, SIGNUP_CHIPS);
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
        assert_eq!(s.users[&p(1)].chips, SIGNUP_CHIPS + 4 * DAILY_CHIPS);
    }

    #[test]
    fn buying_with_chips() {
        let mut s = setup();
        let cowboy = s.items.values().find(|i| i.name == "Cowboy").unwrap().id;
        let crown = s.items.values().find(|i| i.name == "Crown").unwrap().id;
        assert!(s.buy_item(p(1), cowboy, Currency::Chips).is_err()); // 100k chips needed
        s.users.get_mut(&p(1)).unwrap().chips = 200_000;
        s.buy_item(p(1), cowboy, Currency::Chips).unwrap();
        assert_eq!(s.users[&p(1)].chips, 100_000);
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
        assert_eq!(s.users[&p(1)].chips, SIGNUP_CHIPS - 1_000 - 200);
    }

    #[test]
    fn hand_history_is_private_and_respects_muck() {
        let mut s = setup();
        let table = 1;
        s.join_table(p(1), table, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), table, 1, 1_000, None, 0).unwrap();
        // p1 shows losing hands, p2 mucks them (the default).
        let open = Settings { auto_muck: false, ..Settings::default() };
        s.update_settings(p(1), open).unwrap();
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
        let total = s.users[&p(1)].chips + stack;
        assert_eq!(total as i64, SIGNUP_CHIPS as i64 + s.users[&p(1)].stats.chip_net);
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
        let chips_before: u64 = s.users.values().map(|u| u.chips).sum();
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
        assert!(s.users[&winner.player].medals.contains_key("sng_champ"));
        let chips_after: u64 = s.users.values().map(|u| u.chips).sum();
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
        assert_eq!(s.treasury, 2 * fee);
        let balances = s.users[&p(10)].balance + s.users[&p(11)].balance;
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
            for e in t.entrants.iter().filter(|e| e.place.is_none()) {
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
        assert!(s.users[&p(10)].balance < 10 * E8S);
        s.tick(s.tournaments[&id].start_at);
        assert_eq!(s.tournaments[&id].status, Cancelled);
        assert_eq!(s.users[&p(10)].balance, 10 * E8S);
        assert_eq!(s.treasury, 0);
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
        let id = s.create_room(p(1), args(Some("aces")), 0).unwrap();
        assert!(s.users[&p(1)].medals.contains_key("host"));
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
        let rakes: Vec<u64> = s.hands.values().filter(|h| h.table_id == id).map(|h| h.rake).collect();
        assert!(rakes.iter().sum::<u64>() > 0);
        let host_cut: u64 = rakes.iter().map(|r| r * HOST_RAKE_SHARE_BPS / 10_000).sum();
        assert_eq!(s.treasury, rakes.iter().sum::<u64>() - host_cut);
        let net_play: i64 = (1..=2).map(|k| s.users[&p(k)].stats.net_winnings).sum();
        assert_eq!(net_play, -(rakes.iter().sum::<u64>() as i64));
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
    fn medals_for_hands_and_social() {
        let mut s = setup();
        let table = 1;
        s.join_table(p(1), table, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), table, 1, 1_000, None, 0).unwrap();
        play_hands(&mut s, table, 3, SEC);
        assert!(s.users[&p(1)].medals.contains_key("first_hand"));
        let winner = if s.users[&p(1)].stats.hands_won > 0 { p(1) } else { p(2) };
        assert!(s.users[&winner].medals.contains_key("first_pot"));
        let xp = s.users[&p(1)].xp;
        s.award(&p(1), "first_hand", None, 0);
        assert_eq!(s.users[&p(1)].xp, xp, "medals are awarded once");

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
        };
        s.award_hand_medals(table, Some(Currency::Chips), &result, 0);
        for m in ["royal_flush", "straight_flush", "quads", "full_house"] {
            assert!(s.users[&p(3)].medals.contains_key(m), "{m}");
        }
        // p2 lost holding trip aces: not strong enough for a bad beat.
        assert!(!s.users[&p(2)].medals.contains_key("bad_beat"));
    }

    #[test]
    fn one_player_at_several_tables() {
        let mut s = setup();
        s.join_table(p(1), 1, 0, 1_000, None, 0).unwrap();
        s.join_table(p(1), 2, 0, 1_000, None, 0).unwrap();
        s.join_table(p(2), 1, 1, 1_000, None, 0).unwrap();
        s.join_table(p(3), 2, 1, 1_000, None, 0).unwrap();
        assert_eq!(s.public_profile(&p(1), 0).unwrap().tables, vec![1, 2]);
        let now = run_until(&mut s, SEC, 1_000, |s| s.users[&p(1)].stats.hands_played >= 10);
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
        let chips: u64 = (1..=3).map(|k| s.users[&p(k)].chips).sum();
        assert_eq!(chips, 3 * SIGNUP_CHIPS);
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
        let chips = s.users[&p(10)].chips;
        s.rebuy(p(10), id, now).unwrap();
        assert_eq!(stack_of(&s, id, p(10)), Some(3_500));
        assert_eq!(s.users[&p(10)].chips, chips - 2_000);
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
        assert_eq!(e.place, Some(3));
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
        let chips_before: u64 = s.users.values().map(|u| u.chips).sum();
        let (id, start_at) = start_scheduled(&mut s, "Chip Rebuy Madness", 10..22);
        play_rebuy_event(&mut s, id, start_at + SEC);
        let t = &s.tournaments[&id];
        assert!(t.entries() > 12, "some players rebought");
        assert!(t.entrants.iter().all(|e| e.rebuys <= 3));
        let mut places: Vec<u32> = t.entrants.iter().map(|e| e.place.unwrap()).collect();
        places.sort();
        assert_eq!(places, (1..=12).collect::<Vec<_>>());
        assert_eq!(t.paid, t.prize_pool);
        assert_eq!(t.collected, 2_000 * t.entries() as u64);
        let chips_after: u64 = s.users.values().map(|u| u.chips).sum();
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
        assert_eq!(s.treasury, t.entries() as u64 * (E8S / 40));
        assert_eq!(t.paid, t.prize_pool);
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
    fn icp_formatting() {
        assert_eq!(fmt_icp(E8S), "1");
        assert_eq!(fmt_icp(150_000_000), "1.5");
        assert_eq!(fmt_icp(10_000), "0.0001");
    }
}
