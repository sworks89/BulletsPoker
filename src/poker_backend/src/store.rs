//! Stable-memory storage.
//!
//! Data that grows with the player base lives directly in stable memory, so
//! upgrades never have to serialize it: users, usernames, notes, direct
//! messages, hand histories and the ledger-operation log. Each value is
//! Candid-encoded, so new fields must be `Option<T>` (Candid decodes a missing
//! optional field as `None`), which keeps old records readable after upgrades.
//!
//! Live game state (tables, tournaments, chats…) stays on the heap and is
//! written as a versioned snapshot in `pre_upgrade`; its size is bounded.

use crate::state::{ChatMessage, HandRecord, PlayerNote, User};
use crate::moderation::Report;
use crate::referrals::{FriendEntry, FriendKey};
use crate::wallet::LedgerOp;
use candid::{CandidType, Deserialize, Principal};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::storable::Bound;
use ic_stable_structures::{DefaultMemoryImpl, Memory, StableBTreeMap, Storable};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

pub type Mem = VirtualMemory<DefaultMemoryImpl>;

const HEAP_SNAPSHOT: MemoryId = MemoryId::new(0);
const USERS: MemoryId = MemoryId::new(1);
const USERNAMES: MemoryId = MemoryId::new(2);
const NOTES: MemoryId = MemoryId::new(3);
const DMS: MemoryId = MemoryId::new(4);
const HANDS: MemoryId = MemoryId::new(5);
const LEDGER_OPS: MemoryId = MemoryId::new(6);
const REPORTS: MemoryId = MemoryId::new(7);
const REPORTS_BY_TARGET: MemoryId = MemoryId::new(8);
const REFERRALS: MemoryId = MemoryId::new(9);
const FAIR_HANDS: MemoryId = MemoryId::new(10);

macro_rules! candid_storable {
    ($($t:ty),*) => {$(
        impl Storable for $t {
            fn to_bytes(&self) -> Cow<'_, [u8]> {
                Cow::Owned(candid::encode_one(self).expect("encode"))
            }
            fn into_bytes(self) -> Vec<u8> {
                candid::encode_one(&self).expect("encode")
            }
            fn from_bytes(bytes: Cow<[u8]>) -> Self {
                candid::decode_one(&bytes).expect("decode")
            }
            const BOUND: Bound = Bound::Unbounded;
        }
    )*};
}

/// A direct-message thread between two players.
#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct DmThread(pub Vec<ChatMessage>);

candid_storable!(User, PlayerNote, DmThread, HandRecord, LedgerOp, Report, FriendEntry);

/// Stable structures other than users, carved out of one stable memory by a memory manager.
pub struct Stores {
    pub heap_mem: Mem,
    pub usernames: StableBTreeMap<String, Principal, Mem>,
    /// (author, subject) -> note.
    pub notes: StableBTreeMap<(Principal, Principal), PlayerNote, Mem>,
    pub dms: StableBTreeMap<(Principal, Principal), DmThread, Mem>,
    pub hands: StableBTreeMap<u64, HandRecord, Mem>,
    pub ledger_ops: StableBTreeMap<u64, LedgerOp, Mem>,
    pub reports: StableBTreeMap<u64, Report, Mem>,
    /// (reported player, report id): finds a player's reports without a scan.
    pub reports_by_target: StableBTreeMap<(Principal, u64), u8, Mem>,
    /// (referrer, attached_at, friend) -> friend entry; one referrer's friends are adjacent, oldest first.
    pub referrals: StableBTreeMap<FriendKey, FriendEntry, Mem>,
    /// (player, hand id) -> reveal_at: the hands with shuffle evidence each
    /// player can still verify, oldest first per player. Kept until
    /// `reveal_at + FAIR_RETENTION_NS`, at most `FAIR_HANDS_PER_USER` per player.
    pub fair_hands: StableBTreeMap<(Principal, u64), u64, Mem>,
}

impl Stores {
    /// Opens (or creates) the structures in `memory`. On a canister this is the
    /// real stable memory, so existing data is picked up after an upgrade.
    pub fn init(memory: DefaultMemoryImpl) -> (Self, Users) {
        let mm = MemoryManager::init(memory);
        let users = Users { map: StableBTreeMap::init(mm.get(USERS)) };
        let stores = Stores {
            heap_mem: mm.get(HEAP_SNAPSHOT),
            usernames: StableBTreeMap::init(mm.get(USERNAMES)),
            notes: StableBTreeMap::init(mm.get(NOTES)),
            dms: StableBTreeMap::init(mm.get(DMS)),
            hands: StableBTreeMap::init(mm.get(HANDS)),
            ledger_ops: StableBTreeMap::init(mm.get(LEDGER_OPS)),
            reports: StableBTreeMap::init(mm.get(REPORTS)),
            reports_by_target: StableBTreeMap::init(mm.get(REPORTS_BY_TARGET)),
            referrals: StableBTreeMap::init(mm.get(REFERRALS)),
            fair_hands: StableBTreeMap::init(mm.get(FAIR_HANDS)),
        };
        (stores, users)
    }
}

/// Writes a length-prefixed blob at the start of `mem`.
pub fn save_blob(mem: &Mem, bytes: &[u8]) {
    let needed = 8 + bytes.len() as u64;
    let pages = needed.div_ceil(65_536);
    if mem.size() < pages && mem.grow(pages - mem.size()) < 0 {
        ic_cdk::trap("out of stable memory saving heap snapshot");
    }
    mem.write(0, &(bytes.len() as u64).to_le_bytes());
    mem.write(8, bytes);
}

pub fn load_blob(mem: &Mem) -> Option<Vec<u8>> {
    if mem.size() == 0 {
        return None;
    }
    let mut len = [0u8; 8];
    mem.read(0, &mut len);
    let len = u64::from_le_bytes(len) as usize;
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u8; len];
    mem.read(8, &mut buf);
    Some(buf)
}

/// Users in stable memory, with a map-like API. `get` returns a copy;
/// `get_mut` returns a guard that writes the user back when dropped (only if
/// it was modified).
pub struct Users {
    map: StableBTreeMap<Principal, User, Mem>,
}

pub struct UserMut<'a> {
    map: &'a mut StableBTreeMap<Principal, User, Mem>,
    key: Principal,
    user: Option<User>,
    dirty: bool,
}

impl Deref for UserMut<'_> {
    type Target = User;
    fn deref(&self) -> &User {
        self.user.as_ref().unwrap()
    }
}

impl DerefMut for UserMut<'_> {
    fn deref_mut(&mut self) -> &mut User {
        self.dirty = true;
        self.user.as_mut().unwrap()
    }
}

impl Drop for UserMut<'_> {
    fn drop(&mut self) {
        if self.dirty {
            if let Some(u) = self.user.take() {
                self.map.insert(self.key, u);
            }
        }
    }
}

impl Users {
    pub fn get(&self, p: &Principal) -> Option<User> {
        self.map.get(p)
    }

    pub fn get_mut(&mut self, p: &Principal) -> Option<UserMut<'_>> {
        let user = self.map.get(p)?;
        Some(UserMut { map: &mut self.map, key: *p, user: Some(user), dirty: false })
    }

    pub fn insert(&mut self, p: Principal, u: User) {
        self.map.insert(p, u);
    }

    pub fn contains_key(&self, p: &Principal) -> bool {
        self.map.contains_key(p)
    }

    pub fn len(&self) -> usize {
        self.map.len() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Full scan: decodes every user. Admin/maintenance use only.
    pub fn iter(&self) -> impl Iterator<Item = (Principal, User)> + '_ {
        self.map.iter().map(|e| e.into_pair())
    }
}
