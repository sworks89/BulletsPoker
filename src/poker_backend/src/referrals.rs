//! Referrals: a player can attach one referrer, by username, in their first 24 hours.
//!
//! The referrer later earns a cut of the friend's rake (see the referral design).
//! This module holds the data types, the stable friend-list key and `set_referrer`.

use crate::leaderboards::DAY_NS;
use crate::state::{State, R};
use candid::{CandidType, Deserialize, Principal};
use ic_stable_structures::storable::Bound;
use ic_stable_structures::Storable;
use std::borrow::Cow;

/// A new account can add a referral code for this long after it was created.
pub const ATTACH_WINDOW_NS: u64 = DAY_NS;

/// Where a player's referrer was recorded, kept on the friend's `User`.
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub struct ReferralLink {
    pub referrer: Principal,
    /// When the referrer was attached (ns). The 12-month window runs from here.
    pub attached_at: u64,
}

/// A referrer's running totals, kept on the referrer's `User`.
#[derive(CandidType, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ReferrerStats {
    pub friends: u32,
    /// Everything paid out to this referrer so far (e8s).
    pub paid_total: u64,
    /// Remainder below 1 e8s, in units of e8s x 10_000.
    pub carry_bps: u64,
}

/// One referred friend, as listed for the referrer.
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub struct FriendEntry {
    pub attached_at: u64,
    pub earned: u64,
    pub last_earned_at: u64,
}

/// Longest principal, in bytes.
const PRINCIPAL_MAX: usize = 29;
/// A principal as stored in a key: a length byte, then the bytes zero-padded.
const PRINCIPAL_SLOT: usize = 1 + PRINCIPAL_MAX;
const KEY_LEN: usize = PRINCIPAL_SLOT + 8 + PRINCIPAL_SLOT;

fn principal_slot(p: &Principal) -> [u8; PRINCIPAL_SLOT] {
    let mut out = [0u8; PRINCIPAL_SLOT];
    let b = p.as_slice();
    out[0] = b.len() as u8;
    out[1..1 + b.len()].copy_from_slice(b);
    out
}

/// Key of the stable friends map: `(referrer, attached_at, friend)` in a fixed
/// 68-byte layout. All of one referrer's friends are adjacent and ordered by
/// attach time (then friend), so a reverse range scan lists them newest first.
///
/// Layout: `[len][referrer padded to 29]` `[attached_at big-endian u64]`
/// `[len][friend padded to 29]`. Ordering is the byte ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FriendKey([u8; KEY_LEN]);

impl FriendKey {
    pub fn new(referrer: &Principal, attached_at: u64, friend: &Principal) -> Self {
        let mut k = [0u8; KEY_LEN];
        k[..PRINCIPAL_SLOT].copy_from_slice(&principal_slot(referrer));
        k[PRINCIPAL_SLOT..PRINCIPAL_SLOT + 8].copy_from_slice(&attached_at.to_be_bytes());
        k[PRINCIPAL_SLOT + 8..].copy_from_slice(&principal_slot(friend));
        FriendKey(k)
    }

    /// Inclusive bounds covering every friend of `referrer`.
    pub fn range(referrer: &Principal) -> (FriendKey, FriendKey) {
        let mut lo = [0u8; KEY_LEN];
        lo[..PRINCIPAL_SLOT].copy_from_slice(&principal_slot(referrer));
        let mut hi = [0xFFu8; KEY_LEN];
        hi[..PRINCIPAL_SLOT].copy_from_slice(&principal_slot(referrer));
        (FriendKey(lo), FriendKey(hi))
    }

    fn slot_principal(slot: &[u8]) -> Principal {
        Principal::from_slice(&slot[1..1 + slot[0] as usize])
    }

    pub fn referrer(&self) -> Principal {
        Self::slot_principal(&self.0[..PRINCIPAL_SLOT])
    }

    pub fn attached_at(&self) -> u64 {
        u64::from_be_bytes(self.0[PRINCIPAL_SLOT..PRINCIPAL_SLOT + 8].try_into().unwrap())
    }

    pub fn friend(&self) -> Principal {
        Self::slot_principal(&self.0[PRINCIPAL_SLOT + 8..])
    }
}

impl Storable for FriendKey {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(&self.0)
    }
    fn into_bytes(self) -> Vec<u8> {
        self.0.to_vec()
    }
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        FriendKey(bytes.as_ref().try_into().expect("friend key length"))
    }
    const BOUND: Bound = Bound::Bounded { max_size: KEY_LEN as u32, is_fixed_size: true };
}

impl State {
    /// Attaches `code` (a username, any case) as `friend`'s referrer.
    pub fn set_referrer(&mut self, friend: Principal, code: &str, now: u64) -> R<()> {
        let me = self.user(&friend)?;
        let referrer = self
            .stores
            .usernames
            .get(&code.trim().to_lowercase())
            .ok_or_else(|| "No player with that code".to_string())?;
        if referrer == friend {
            return Err("You can't refer yourself".into());
        }
        if now >= me.created_at.saturating_add(ATTACH_WINDOW_NS) {
            return Err("Referral codes can only be added in your first 24 hours".into());
        }
        if me.referral.is_some() {
            return Err("You already have a referrer".into());
        }
        let them = self.user(&referrer)?;
        if them.created_at >= me.created_at {
            return Err("That player joined after you".into());
        }
        if them.referral.as_ref().is_some_and(|l| l.referrer == friend) {
            return Err("That player was referred by you".into());
        }
        self.user_mut(&friend)?.referral = Some(ReferralLink { referrer, attached_at: now });
        self.stores
            .referrals
            .insert(FriendKey::new(&referrer, now, &friend), FriendEntry { attached_at: now, earned: 0, last_earned_at: 0 });
        let mut r = self.user_mut(&referrer)?;
        let stats = r.referrer_stats.get_or_insert_with(ReferrerStats::default);
        stats.friends += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: u64 = 3_600 * 1_000_000_000;

    fn p(n: u8) -> Principal {
        Principal::from_slice(&[n; 10])
    }

    /// Alice joined at t=1h, Bob at t=2h, Cara at t=3h.
    fn setup() -> State {
        let mut s = State::new(Principal::management_canister(), 10_000);
        s.register(p(1), "Alice".into(), HOUR).unwrap();
        s.register(p(2), "Bob".into(), 2 * HOUR).unwrap();
        s.register(p(3), "Cara".into(), 3 * HOUR).unwrap();
        s
    }

    #[test]
    fn attaches_a_referrer_by_username_ignoring_case() {
        let mut s = setup();
        s.set_referrer(p(2), "aLiCe", 4 * HOUR).unwrap();
        let link = s.user(&p(2)).unwrap().referral.unwrap();
        assert_eq!(link, ReferralLink { referrer: p(1), attached_at: 4 * HOUR });
        assert_eq!(s.user(&p(1)).unwrap().referrer_stats.unwrap().friends, 1);
        let (lo, hi) = FriendKey::range(&p(1));
        let rows: Vec<_> = s.stores.referrals.range(lo..=hi).collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key().friend(), p(2));
        assert_eq!(rows[0].key().attached_at(), 4 * HOUR);
        assert_eq!(rows[0].value(), FriendEntry { attached_at: 4 * HOUR, earned: 0, last_earned_at: 0 });
        assert!(s.user(&p(3)).unwrap().referral.is_none());
    }

    #[test]
    fn refuses_self_unknown_late_second_newer_and_loops() {
        let mut s = setup();
        assert_eq!(s.set_referrer(p(2), "nobody", 4 * HOUR), Err("No player with that code".to_string()));
        assert_eq!(s.set_referrer(p(2), "bob", 4 * HOUR), Err("You can't refer yourself".to_string()));
        assert_eq!(s.set_referrer(p(1), "bob", 4 * HOUR), Err("That player joined after you".to_string()));
        // Bob is past his first 24 hours.
        assert_eq!(
            s.set_referrer(p(2), "alice", 2 * HOUR + 24 * HOUR),
            Err("Referral codes can only be added in your first 24 hours".to_string())
        );
        s.set_referrer(p(3), "bob", 4 * HOUR).unwrap();
        assert_eq!(s.set_referrer(p(3), "alice", 5 * HOUR), Err("You already have a referrer".to_string()));
        // Force a loop: Alice (older) was referred by Bob, then Bob names Alice.
        s.user_mut(&p(1)).unwrap().referral = Some(ReferralLink { referrer: p(2), attached_at: HOUR });
        assert_eq!(s.set_referrer(p(2), "alice", 5 * HOUR), Err("That player was referred by you".to_string()));
        assert_eq!(s.user(&p(1)).unwrap().referrer_stats, None);
    }

    #[test]
    fn anonymous_caller_is_refused() {
        let mut s = setup();
        assert_eq!(s.set_referrer(Principal::anonymous(), "alice", 4 * HOUR), Err("Create a profile first".to_string()));
        assert_eq!(s.set_referrer(p(9), "alice", 4 * HOUR), Err("Create a profile first".to_string()));
    }

    #[test]
    fn friend_keys_group_by_referrer_and_order_by_attach_time() {
        let long = Principal::from_slice(&[7u8; 29]);
        let short = Principal::from_slice(&[7u8; 3]);
        let a = FriendKey::new(&short, 5, &p(1));
        let b = FriendKey::new(&short, 6, &p(1));
        let other = FriendKey::new(&long, 0, &p(1));
        assert!(a < b);
        let (lo, hi) = FriendKey::range(&short);
        assert!(lo <= a && b <= hi);
        assert!(!(lo <= other && other <= hi));
        let max = FriendKey::new(&short, u64::MAX, &Principal::from_slice(&[255u8; 29]));
        assert!(max <= hi);
        let k = FriendKey::new(&long, 42, &short);
        assert_eq!((k.referrer(), k.attached_at(), k.friend()), (long, 42, short));
        assert_eq!(FriendKey::from_bytes(k.to_bytes()), k);
    }
}
