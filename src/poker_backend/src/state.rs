//! Canister state and the business logic around it: profiles, wallets, the
//! item store, gifts, chat, friends and tables. Everything here is pure (the
//! caller and the current time are passed in) so it can be unit tested
//! natively; `lib.rs` wraps it in canister endpoints.

use crate::table::{Action, HandResult, Phase, Table, TableConfig};
use candid::{CandidType, Deserialize, Principal};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const E8S: u64 = 100_000_000;
const CHAT_HISTORY: usize = 100;
const DM_HISTORY: usize = 200;
const MAX_CHAT_LEN: usize = 280;
const CHAT_COOLDOWN_NS: u64 = 700_000_000;
const GIFT_DURATION_NS: u64 = 10 * 60 * 1_000_000_000;
const ONLINE_WINDOW_NS: u64 = 5 * 60 * 1_000_000_000;

pub type R<T> = Result<T, String>;

#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct Stats {
    pub hands_played: u64,
    pub hands_won: u64,
    pub biggest_pot_won: u64,
    pub net_winnings: i64,
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
    /// Wallet balance in e8s (chips not sitting on a table).
    pub balance: u64,
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
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct StoreItem {
    pub id: u32,
    pub name: String,
    pub kind: ItemKind,
    /// Price in e8s. Zero means free.
    pub price: u64,
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
}

// ------------------------------------------------------------------ views

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Profile {
    pub id: Principal,
    pub username: String,
    pub status: String,
    pub balance: u64,
    pub avatar: u32,
    pub card_back: u32,
    pub inventory: Vec<u32>,
    pub friends: Vec<Principal>,
    pub friend_requests: Vec<Principal>,
    pub stats: Stats,
    pub xp: u64,
    pub level: u32,
    pub created_at: u64,
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
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TableSummary {
    pub id: u64,
    pub name: String,
    pub small_blind: u64,
    pub big_blind: u64,
    pub min_buy_in: u64,
    pub max_buy_in: u64,
    pub players: u8,
    pub max_seats: u8,
    pub phase: Phase,
    pub avg_pot: u64,
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

fn default_items() -> Vec<(ItemKind, &'static str, u64, &'static str, &'static str)> {
    use ItemKind::*;
    vec![
        (Avatar, "Rookie", 0, "🙂", "Everyone starts somewhere"),
        (Avatar, "Cool Cat", 0, "😎", "Too cool for the rail"),
        (Avatar, "Robot", 0, "🤖", "Calculating pot odds"),
        (Avatar, "Cowboy", E8S / 20, "🤠", "Yeehaw, all in"),
        (Avatar, "Alien", E8S / 20, "👽", "Plays from another galaxy"),
        (Avatar, "Ninja", E8S / 10, "🥷", "Silent and deadly"),
        (Avatar, "Shark", E8S / 10, "🦈", "Smells fish at the table"),
        (Avatar, "Dragon", E8S / 4, "🐉", "Hoards every pot"),
        (Avatar, "Unicorn", E8S / 4, "🦄", "Rare as a royal flush"),
        (Avatar, "Crown", E8S, "👑", "Table royalty"),
        (CardBack, "Classic Red", 0, "#b91c1c", "The house deck"),
        (CardBack, "Midnight", E8S / 50, "#1e3a8a", "Deep blue backs"),
        (CardBack, "Emerald", E8S / 50, "#047857", "Felt-green backs"),
        (CardBack, "Gold Foil", E8S / 5, "#ca8a04", "Shine on every deal"),
        (Gift, "Beer", E8S / 1000, "🍺", "Buy the table a round"),
        (Gift, "Coffee", E8S / 1000, "☕", "For the long session"),
        (Gift, "Rose", E8S / 500, "🌹", "Show some love"),
        (Gift, "Cocktail", E8S / 500, "🍸", "Shaken, not stirred"),
        (Gift, "Pizza", E8S / 200, "🍕", "Fuel for the grind"),
        (Gift, "Cigar", E8S / 200, "🚬", "Celebrate the big win"),
        (Gift, "Champagne", E8S / 100, "🍾", "Pop it after a monster pot"),
        (Gift, "Trophy", E8S / 20, "🏆", "Crown the champ"),
    ]
}

pub fn default_tables() -> Vec<TableConfig> {
    let t = |name: &str, sb: u64, seats: u8| TableConfig {
        name: name.into(),
        small_blind: sb,
        big_blind: sb * 2,
        min_buy_in: sb * 2 * 20,
        max_buy_in: sb * 2 * 100,
        max_seats: seats,
        rake_bps: 250,
        rake_cap: sb * 2 * 3,
        action_timeout_secs: 25,
    };
    vec![
        t("Penny Lane", 10_000, 9),
        t("Micro Madness", 10_000, 6),
        t("Low Rollers", 100_000, 9),
        t("Heads Up Arena", 100_000, 2),
        t("Mid Stakes Saloon", 1_000_000, 6),
        t("High Roller Lounge", 10_000_000, 6),
    ]
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
        };
        for (kind, name, price, icon, description) in default_items() {
            s.add_item(StoreItem {
                id: 0,
                name: name.into(),
                kind,
                price,
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

    fn user(&self, p: &Principal) -> R<&User> {
        self.users.get(p).ok_or_else(|| "Create a profile first".to_string())
    }

    fn user_mut(&mut self, p: &Principal) -> R<&mut User> {
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

    fn icon(&self, id: u32) -> String {
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
            avatar: u.avatar,
            card_back: u.card_back,
            inventory: u.inventory.clone(),
            friends: u.friends.iter().copied().collect(),
            friend_requests: u.friend_requests.iter().copied().collect(),
            stats: u.stats.clone(),
            xp: u.xp,
            level: level_for_xp(u.xp),
            created_at: u.created_at,
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
        item.price == 0 || u.inventory.contains(&item.id)
    }

    pub fn buy_item(&mut self, caller: Principal, id: u32) -> R<()> {
        let item = self.items.get(&id).cloned().ok_or("No such item")?;
        if !item.available {
            return Err("Item is not for sale".into());
        }
        if item.kind == ItemKind::Gift {
            return Err("Gifts are sent to players at a table".into());
        }
        let owned = self.owns(self.user(&caller)?, &item);
        if owned {
            return Err("You already own this item".into());
        }
        let u = self.user_mut(&caller)?;
        if u.balance < item.price {
            return Err("Insufficient balance".into());
        }
        u.balance -= item.price;
        u.inventory.push(id);
        u.xp += 10;
        self.treasury += item.price;
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
            ItemKind::Gift => return Err("Gifts can't be equipped".into()),
        }
        Ok(())
    }

    pub fn send_gift(&mut self, caller: Principal, table_id: u64, to: Principal, item_id: u32, now: u64) -> R<()> {
        let item = self.items.get(&item_id).cloned().ok_or("No such item")?;
        if item.kind != ItemKind::Gift || !item.available {
            return Err("That is not a gift".into());
        }
        let t = self.tables.get(&table_id).ok_or("No such table")?;
        if t.seat_of(&to).is_none() {
            return Err("That player is not at this table".into());
        }
        let from_name = self.user(&caller)?.username.clone();
        let to_name = self.user(&to)?.username.clone();
        let u = self.user_mut(&caller)?;
        if u.balance < item.price {
            return Err("Insufficient balance".into());
        }
        u.balance -= item.price;
        u.xp += 5;
        self.treasury += item.price;
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

    // -------------------------------------------------------------- wallet

    pub fn credit(&mut self, p: &Principal, amount: u64) -> R<()> {
        let u = self.user_mut(p)?;
        u.balance = u.balance.checked_add(amount).ok_or("Overflow")?;
        Ok(())
    }

    pub fn debit(&mut self, p: &Principal, amount: u64) -> R<()> {
        let u = self.user_mut(p)?;
        if u.balance < amount {
            return Err("Insufficient balance".into());
        }
        u.balance -= amount;
        Ok(())
    }

    pub fn house_stats(&self) -> HouseStats {
        let wallet_balances: u64 = self.users.values().map(|u| u.balance).sum();
        let chips_on_tables: u64 = self
            .tables
            .values()
            .map(|t| t.chips_in_play() + t.rake_accrued)
            .sum();
        HouseStats {
            users: self.users.len() as u64,
            wallet_balances,
            chips_on_tables,
            treasury: self.treasury,
            liabilities_total: wallet_balances + chips_on_tables + self.treasury,
        }
    }

    // ---------------------------------------------------------------- chat

    fn post(&mut self, ch: Channel, from: Option<Principal>, username: String, text: String, kind: MessageKind, now: u64) {
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

    fn table_mut(&mut self, id: u64) -> R<&mut Table> {
        self.tables.get_mut(&id).ok_or_else(|| "No such table".to_string())
    }

    pub fn join_table(&mut self, caller: Principal, id: u64, seat: u8, buy_in: u64, now: u64) -> R<()> {
        let name = self.user(&caller)?.username.clone();
        if self.user(&caller)?.balance < buy_in {
            return Err("Insufficient balance for that buy-in".into());
        }
        self.table_mut(id)?.sit(caller, seat, buy_in)?;
        self.debit(&caller, buy_in)?;
        self.post(Channel::Table(id), None, String::new(), format!("{name} sat down"), MessageKind::System, now);
        Ok(())
    }

    pub fn leave_table(&mut self, caller: Principal, id: u64, now: u64) -> R<u64> {
        let chips = self.table_mut(id)?.stand(&caller, now)?;
        self.credit(&caller, chips)?;
        let name = self.user(&caller)?.username.clone();
        self.post(Channel::Table(id), None, String::new(), format!("{name} left the table"), MessageKind::System, now);
        self.after_table_change(id, now);
        Ok(chips)
    }

    pub fn top_up(&mut self, caller: Principal, id: u64, amount: u64) -> R<()> {
        if self.user(&caller)?.balance < amount {
            return Err("Insufficient balance".into());
        }
        self.table_mut(id)?.top_up(&caller, amount)?;
        self.debit(&caller, amount)
    }

    pub fn sit_out(&mut self, caller: Principal, id: u64, out: bool) -> R<()> {
        self.table_mut(id)?.set_sitting_out(&caller, out)
    }

    pub fn act(&mut self, caller: Principal, id: u64, action: Action, now: u64) -> R<()> {
        self.table_mut(id)?.act(&caller, action, now)?;
        self.after_table_change(id, now);
        Ok(())
    }

    /// Applies a finished hand to player stats and sweeps rake to the treasury.
    pub fn after_table_change(&mut self, id: u64, now: u64) {
        let Some(t) = self.tables.get_mut(&id) else {
            return;
        };
        self.treasury += std::mem::take(&mut t.rake_accrued);
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
        for pr in &result.players {
            if let Some(u) = self.users.get_mut(&pr.player) {
                u.stats.hands_played += 1;
                u.xp += 1;
                u.stats.net_winnings += pr.won as i64 - pr.committed as i64;
                if pr.won > 0 {
                    u.stats.hands_won += 1;
                    u.xp += 2;
                    u.stats.biggest_pot_won = u.stats.biggest_pot_won.max(pr.won);
                }
            }
        }
        for w in &result.winners {
            let name = self.users.get(&w.player).map(|u| u.username.clone()).unwrap_or_default();
            let text = format!("{} wins {} ICP ({})", name, fmt_icp(w.amount), w.hand);
            self.post(Channel::Table(id), None, String::new(), text, MessageKind::System, now);
        }
    }

    /// Times out slow players and reports tables that need a new hand dealt.
    pub fn tick(&mut self, now: u64) -> Vec<u64> {
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
            .map(|t| TableSummary {
                id: t.id,
                name: t.config.name.clone(),
                small_blind: t.config.small_blind,
                big_blind: t.config.big_blind,
                min_buy_in: t.config.min_buy_in,
                max_buy_in: t.config.max_buy_in,
                players: t.player_count() as u8,
                max_seats: t.config.max_seats,
                phase: t.phase,
                avg_pot: t.last_result.as_ref().map_or(0, |r| r.pot),
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
        })
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
        s.buy_item(p(1), ninja).unwrap();
        assert!(s.buy_item(p(1), ninja).is_err());
        s.equip(p(1), ninja).unwrap();
        assert_eq!(s.users[&p(1)].balance, 10 * E8S - E8S / 10);
        assert_eq!(s.treasury, E8S / 10);

        let beer = s.items.values().find(|i| i.name == "Beer").unwrap().id;
        assert!(s.send_gift(p(1), 1, p(2), beer, 0).is_err()); // p2 not seated
        s.join_table(p(2), 1, 0, 1_000_000, 0).unwrap();
        s.send_gift(p(1), 1, p(2), beer, 0).unwrap();
        let v = s.table_view(1, None, 1).unwrap();
        assert_eq!(v.seats[0].as_ref().unwrap().gift.as_deref(), Some("🍺"));
        // Gifts expire lazily: they simply stop being shown.
        let v = s.table_view(1, None, GIFT_DURATION_NS + 1).unwrap();
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
        let table = 3; // Low Rollers: 0.001/0.002 ICP
        let buy = s.tables[&table].config.max_buy_in;
        s.join_table(p(1), table, 0, buy, 0).unwrap();
        s.join_table(p(2), table, 3, buy, 0).unwrap();
        s.join_table(p(3), table, 5, buy, 0).unwrap();
        assert!(s.join_table(p(1), table, 1, buy, 0).is_err());
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
                    s.join_table(p(k), table, seat as u8, buy, now).unwrap();
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

    #[test]
    fn icp_formatting() {
        assert_eq!(fmt_icp(E8S), "1");
        assert_eq!(fmt_icp(150_000_000), "1.5");
        assert_eq!(fmt_icp(10_000), "0.0001");
    }
}
