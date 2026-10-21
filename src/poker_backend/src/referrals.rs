//! Referrals: a player can attach one referrer, by username, in their first 24 hours.
//!
//! The referrer later earns a cut of the friend's rake (see the referral design).
//! This module holds the data types, the stable friend-list key and `set_referrer`.

use crate::leaderboards::{period_end, week_of, DAY_NS};
use crate::state::{State, R};
use candid::{CandidType, Deserialize, Principal};
use ic_stable_structures::storable::Bound;
use ic_stable_structures::Storable;
use std::borrow::Cow;
use std::collections::BTreeMap;

/// A new account can add a referral code for this long after it was created.
pub const ATTACH_WINDOW_NS: u64 = DAY_NS;
/// The referrer's cut of the house's share of a friend's rake (20%).
pub const REFERRAL_CUT_BPS: u64 = 2_000;
/// How long a referrer earns from a friend, from when they were attached.
pub const REFERRAL_DURATION_NS: u64 = 365 * DAY_NS;

/// Cuts owed to referrers, paid at the weekly close.
#[derive(CandidType, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ReferralState {
    /// Owed per referrer (e8s). Bounded by the referrers who earned this week.
    pub owed: BTreeMap<Principal, u64>,
    /// Sum of `owed`, a liability in the house stats.
    pub owed_total: u64,
    /// The week whose cuts `owed` holds.
    pub week: u64,
}

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

/// Friends listed per page of the referrals view.
pub const FRIENDS_PAGE: usize = 50;
/// A friend counts as active if they earned the referrer something this recently.
pub const ACTIVE_WINDOW_NS: u64 = 7 * DAY_NS;
/// `active_7d` is counted over the newest this-many friends only, so a query
/// never decodes an unbounded list.
pub const ACTIVE_SCAN_MAX: usize = 2_000;

/// One friend in the referrer's list.
#[derive(CandidType, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct FriendRow {
    pub username: String,
    pub joined_at: u64,
    /// When the 12-month earning window for this friend ends (ns).
    pub ends_at: u64,
    /// Cuts earned from this friend (e8s). Earned, not paid: a cut forfeited
    /// because the referrer was suspended at payout still counts here.
    pub earned: u64,
    /// Earned the referrer something in the last 7 days.
    pub active: bool,
}

/// What the referrals page shows. The zero view (`Default`) is for an
/// anonymous caller or a player with no profile.
#[derive(CandidType, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ReferralView {
    /// The caller's username, which is their referral code.
    pub code: String,
    pub friends: u32,
    /// Friends who earned something in the last 7 days. Counted over the
    /// newest `ACTIVE_SCAN_MAX` (2,000) friends only, so with more friends than
    /// that it can undercount.
    pub active_7d: u32,
    /// Cuts waiting for the next Monday payout (e8s).
    pub owed_this_week: u64,
    pub paid_total: u64,
    /// The next Monday 00:00 UTC (ns), when `owed_this_week` is paid.
    pub pays_at: u64,
    /// At most `FRIENDS_PAGE` friends, newest first, from `offset`.
    pub rows: Vec<FriendRow>,
    /// Within the first 24 hours and no referrer yet.
    pub can_enter_code: bool,
    /// Username of the caller's referrer.
    pub my_referrer: Option<String>,
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

    /// The referrals page for `who`, with friends from `offset` (newest first).
    ///
    /// `owed_this_week` is the referrer's balance in `ReferralState.owed`
    /// whatever its week: if the week has rolled but no tick has closed it
    /// yet, that balance is about to be paid, so it still shows as owed.
    pub fn referral_view(&self, who: &Principal, offset: u32, now: u64) -> ReferralView {
        let Some(me) = self.users.get(who) else { return ReferralView::default() };
        let name_of = |p: &Principal| self.users.get(p).map(|u| u.username);
        let stats = me.referrer_stats.clone().unwrap_or_default();
        let (lo, hi) = FriendKey::range(who);
        let offset = offset as usize;
        let scan = ACTIVE_SCAN_MAX.max(offset.saturating_add(FRIENDS_PAGE));
        let mut active = 0u32;
        let mut rows = Vec::new();
        for (i, item) in self.stores.referrals.range(lo..=hi).rev().take(scan).enumerate() {
            let e = item.value();
            let is_active = e.last_earned_at > 0 && now.saturating_sub(e.last_earned_at) < ACTIVE_WINDOW_NS;
            if i < ACTIVE_SCAN_MAX && is_active {
                active += 1;
            }
            if i >= offset && i - offset < FRIENDS_PAGE {
                let key = item.key();
                rows.push(FriendRow {
                    username: name_of(&key.friend()).unwrap_or_default(),
                    joined_at: key.attached_at(),
                    ends_at: key.attached_at().saturating_add(REFERRAL_DURATION_NS),
                    earned: e.earned,
                    active: is_active,
                });
            }
        }
        ReferralView {
            code: me.username.clone(),
            friends: stats.friends,
            active_7d: active,
            owed_this_week: self.referrals.as_ref().and_then(|r| r.owed.get(who).copied()).unwrap_or(0),
            paid_total: stats.paid_total,
            pays_at: period_end(0, now),
            rows,
            can_enter_code: me.referral.is_none() && now < me.created_at.saturating_add(ATTACH_WINDOW_NS),
            my_referrer: me.referral.as_ref().and_then(|l| name_of(&l.referrer)),
        }
    }

    /// Moves the referrer's cut of `house_e8s` (the house's share of the
    /// friend's rake or fee) from the treasury into what they are owed.
    ///
    /// Called at the end of `earn_rake`, after `fund_rewards` has put that same
    /// rake into the treasury, so the treasury always covers the cut.
    pub(crate) fn referral_cut(&mut self, friend: &Principal, house_e8s: u64, now: u64) {
        if house_e8s == 0 {
            return;
        }
        let Some(link) = self.users.get(friend).and_then(|u| u.referral) else { return };
        if now >= link.attached_at.saturating_add(REFERRAL_DURATION_NS) {
            return;
        }
        self.close_referral_week(now);
        let cut = {
            let Ok(mut r) = self.user_mut(&link.referrer) else { return };
            let stats = r.referrer_stats.get_or_insert_with(ReferrerStats::default);
            let total = house_e8s as u128 * REFERRAL_CUT_BPS as u128 + stats.carry_bps as u128;
            stats.carry_bps = (total % 10_000) as u64;
            (total / 10_000).min(u64::MAX as u128) as u64
        };
        let cut = cut.min(self.treasury);
        self.treasury -= cut;
        let state = self.referrals.get_or_insert_with(ReferralState::default);
        *state.owed.entry(link.referrer).or_default() += cut;
        state.owed_total += cut;
        let key = FriendKey::new(&link.referrer, link.attached_at, friend);
        if cut == 0 {
            return;
        }
        if let Some(mut e) = self.stores.referrals.get(&key) {
            e.earned = e.earned.saturating_add(cut);
            e.last_earned_at = now;
            self.stores.referrals.insert(key, e);
        }
    }

    /// Pays what referrers are owed once their week has ended: into their ICP
    /// balance, or back to the treasury for a suspended referrer.
    ///
    /// Idempotent: the week is advanced before anything is paid, so a repeated
    /// call finds nothing to pay.
    pub(crate) fn close_referral_week(&mut self, now: u64) {
        let current = week_of(now);
        let owed = {
            let state = self.referrals.get_or_insert_with(ReferralState::default);
            if current <= state.week {
                return;
            }
            state.week = current;
            std::mem::take(&mut state.owed)
        };
        for (referrer, amount) in owed {
            if let Some(state) = self.referrals.as_mut() {
                state.owed_total -= amount;
            }
            if amount == 0 {
                continue;
            }
            if self.is_banned(&referrer, now) || self.credit(&referrer, amount).is_err() {
                self.treasury += amount;
                continue;
            }
            if let Ok(mut r) = self.user_mut(&referrer) {
                let stats = r.referrer_stats.get_or_insert_with(ReferrerStats::default);
                stats.paid_total = stats.paid_total.saturating_add(amount);
            }
        }
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

/// The cut and the weekly payout, run against the full game state.
#[cfg(test)]
mod cut_tests {
    use super::*;
    use crate::leaderboards::{week_of, DAY_NS};
    use crate::state::tests::{house_take, many_players, p, run_until, setup, start_scheduled, total_value, SEC};
    use crate::state::E8S;

    /// Links `friend` to `referrer` as `set_referrer` would. The setup players
    /// all joined at 0, so the age rule would refuse `set_referrer` itself.
    fn link(s: &mut State, friend: Principal, referrer: Principal, at: u64) {
        s.user_mut(&friend).unwrap().referral = Some(ReferralLink { referrer, attached_at: at });
        s.stores.referrals.insert(FriendKey::new(&referrer, at, &friend), FriendEntry { attached_at: at, earned: 0, last_earned_at: 0 });
        s.user_mut(&referrer).unwrap().referrer_stats.get_or_insert_with(ReferrerStats::default).friends += 1;
    }

    fn owed(s: &State, r: Principal) -> u64 {
        s.referrals.as_ref().and_then(|x| x.owed.get(&r).copied()).unwrap_or(0)
    }

    fn owed_total(s: &State) -> u64 {
        s.referrals.as_ref().map_or(0, |x| x.owed_total)
    }

    fn stats(s: &State, r: Principal) -> ReferrerStats {
        s.user(&r).unwrap().referrer_stats.unwrap_or_default()
    }

    fn entry(s: &State, friend: Principal) -> FriendEntry {
        let link = s.user(&friend).unwrap().referral.unwrap();
        s.stores.referrals.get(&FriendKey::new(&link.referrer, link.attached_at, &friend)).unwrap()
    }

    fn balance(s: &State, who: Principal) -> u64 {
        s.user(&who).unwrap().balance
    }

    /// The house's share of `who`'s rake recorded this week.
    fn house_rake(s: &State, who: Principal) -> u64 {
        s.user(&who).unwrap().rewards.unwrap().period.w_house_rake.unwrap_or(0)
    }

    /// ICP cash hands at Low Rollers (0.05/0.1 ICP, not hosted) for players 1-3.
    fn play_cash(s: &mut State) -> u64 {
        let table = 7;
        for k in 1..=3 {
            s.join_table(p(k), table, k - 1, 200_000_000, None, 0).unwrap();
        }
        run_until(s, SEC, 2_000, |s| s.tables[&table].hand_no >= 15 || s.tables[&table].eligible_count() < 2)
    }

    /// The same cash session twice: Bob (2) referred by Alice (1), and no referrals.
    fn twin_sessions() -> (State, State) {
        let mut s = setup();
        link(&mut s, p(2), p(1), 0);
        let mut twin = setup();
        assert_eq!(s.treasury, 0, "the cut must come from this session's own rake");
        play_cash(&mut s);
        play_cash(&mut twin);
        (s, twin)
    }

    #[test]
    fn friends_icp_rake_owes_the_referrer_a_fifth_of_the_house_share() {
        let (s, twin) = twin_sessions();
        let house = house_rake(&s, p(2));
        assert!(house > 0);
        assert_eq!(REFERRAL_CUT_BPS, 2_000);
        let cut = house * REFERRAL_CUT_BPS / 10_000;
        assert_eq!(owed(&s, p(1)), cut);
        assert_eq!(stats(&s, p(1)).carry_bps, house * REFERRAL_CUT_BPS % 10_000);
        assert_eq!(owed_total(&s), cut);
        assert_eq!(twin.treasury - s.treasury, cut);
        let e = entry(&s, p(2));
        assert_eq!(e.earned, cut);
        assert!(e.last_earned_at > 0);
        assert_eq!(s.referrals.as_ref().unwrap().owed.len(), 1, "only Alice is owed");
        assert_eq!(s.referrals.as_ref().unwrap().week, week_of(SEC));
    }

    #[test]
    fn cut_comes_only_from_the_treasury() {
        let (s, twin) = twin_sessions();
        let (r, t) = (s.rewards(), twin.rewards());
        assert_eq!(r.fish_pool, t.fish_pool);
        assert_eq!(r.board_pools, t.board_pools);
        assert_eq!(house_take(&twin) - house_take(&s), owed(&s, p(1)));
        assert_eq!(total_value(&s), total_value(&twin));
    }

    #[test]
    fn tournament_fee_cut_at_start_and_late_entry() {
        let mut s = setup();
        many_players(&mut s, 4);
        link(&mut s, p(10), p(1), 0);
        link(&mut s, p(13), p(1), 0);
        assert_eq!(s.treasury, 0);
        let fee = E8S / 10;
        let (id, start_at) = start_scheduled(&mut s, "ICP Nightly", 10..13);
        assert_eq!(owed(&s, p(1)), fee / 5, "the fee of bot 10, at the start");
        assert_eq!(s.treasury, 3 * fee * 6_000 / 10_000 - fee / 5);
        // The late fee alone must cover its own cut.
        s.treasury = 0;
        s.register_tournament(p(13), id, start_at + SEC).unwrap();
        assert_eq!(owed(&s, p(1)), 2 * fee / 5);
        assert_eq!(s.treasury, fee * 6_000 / 10_000 - fee / 5);
        assert_eq!(entry(&s, p(13)).earned, fee / 5);
        assert_eq!(entry(&s, p(13)).last_earned_at, start_at + SEC);
    }

    #[test]
    fn a_zero_cut_does_not_mark_the_friend_active() {
        let mut s = setup();
        link(&mut s, p(2), p(1), 5);
        // 1 e8s of house share is 0.2 of an e8s: only carry, nothing taken.
        s.treasury = 10 * E8S;
        s.referral_cut(&p(2), 1, 100);
        assert_eq!(owed(&s, p(1)), 0);
        assert_eq!(entry(&s, p(2)).last_earned_at, 0);
        assert_eq!(entry(&s, p(2)).earned, 0);
        // A cut capped to nothing by an empty treasury likewise.
        s.treasury = 0;
        s.referral_cut(&p(2), 10_000, 200);
        assert_eq!(owed(&s, p(1)), 0);
        assert_eq!(entry(&s, p(2)).last_earned_at, 0);
        // A real cut still marks it.
        s.treasury = 10 * E8S;
        s.referral_cut(&p(2), 10_000, 300);
        assert_eq!(entry(&s, p(2)).earned, 2_000);
        assert_eq!(entry(&s, p(2)).last_earned_at, 300);
    }

    #[test]
    fn no_cut_after_twelve_months() {
        assert_eq!(REFERRAL_DURATION_NS, 365 * DAY_NS);
        let mut s = setup();
        link(&mut s, p(2), p(1), 5);
        s.treasury = 10 * E8S;
        let end = 5 + REFERRAL_DURATION_NS;
        s.earn_rake(&p(2), 10_000, 10_000, end - 1);
        assert_eq!(owed(&s, p(1)), 2_000);
        s.earn_rake(&p(2), 10_000, 10_000, end);
        assert_eq!(owed(&s, p(1)), 2_000, "the window has ended");
        assert_eq!(s.treasury, 10 * E8S - 2_000);
        s.earn_rake(&p(3), 10_000, 10_000, end - 1);
        assert_eq!(owed_total(&s), 2_000, "an unreferred player gives no cut");
    }

    #[test]
    fn chip_play_gives_no_cut() {
        let mut s = setup();
        many_players(&mut s, 3);
        for k in [2, 3, 10] {
            link(&mut s, p(k), p(1), 0);
        }
        let table = 1;
        assert_eq!(s.tables[&table].config.currency, crate::table::Currency::Chips);
        let buy = s.tables[&table].config.max_buy_in;
        for k in 2..=3 {
            s.join_table(p(k), table, k - 1, buy, None, 0).unwrap();
        }
        run_until(&mut s, SEC, 2_000, |s| s.tables[&table].hand_no >= 15 || s.tables[&table].eligible_count() < 2);
        start_scheduled(&mut s, "Chip Rebuy Madness", 10..13);
        assert_eq!(owed_total(&s), 0);
        assert_eq!(stats(&s, p(1)).carry_bps, 0);
        assert_eq!(entry(&s, p(10)).earned, 0);
    }

    #[test]
    fn weekly_close_pays_once_and_is_idempotent() {
        let mut s = setup();
        link(&mut s, p(2), p(1), 0);
        s.treasury = 10 * E8S;
        let monday = 4 * DAY_NS;
        assert_eq!((week_of(monday - 1), week_of(monday)), (0, 1));
        let bal = balance(&s, p(1));

        s.earn_rake(&p(2), 50_000, 50_000, SEC);
        assert_eq!(owed(&s, p(1)), 10_000);
        s.tick(monday - 1);
        assert_eq!(balance(&s, p(1)), bal, "not before the week ends");
        s.tick(monday);
        s.tick(monday);
        assert_eq!(balance(&s, p(1)), bal + 10_000);
        assert_eq!((owed(&s, p(1)), owed_total(&s), stats(&s, p(1)).paid_total), (0, 0, 10_000));

        s.earn_rake(&p(2), 50_000, 50_000, monday + SEC);
        let later = monday + 5 * 7 * DAY_NS;
        s.tick(later);
        s.tick(later);
        assert_eq!(balance(&s, p(1)), bal + 20_000);
        assert_eq!(stats(&s, p(1)).paid_total, 20_000);

        // A cut in a new week, before any tick, closes the old week first.
        s.earn_rake(&p(2), 50_000, 50_000, later + SEC);
        let next = later + 7 * DAY_NS;
        s.earn_rake(&p(2), 25_000, 25_000, next);
        assert_eq!(balance(&s, p(1)), bal + 30_000);
        assert_eq!((owed(&s, p(1)), owed_total(&s)), (5_000, 5_000));
        assert_eq!(s.referrals.as_ref().unwrap().week, week_of(next));
        assert_eq!(s.wallet_total, s.recount_wallet_total());
    }

    #[test]
    fn suspended_referrer_is_not_paid_and_it_returns_to_the_treasury() {
        let mut s = setup();
        link(&mut s, p(2), p(1), 0);
        link(&mut s, p(3), p(2), 0);
        s.treasury = 10 * E8S;
        s.earn_rake(&p(2), 50_000, 50_000, SEC);
        s.earn_rake(&p(3), 50_000, 50_000, SEC);
        assert_eq!(s.treasury, 10 * E8S - 20_000);
        s.user_mut(&p(1)).unwrap().ban = Some(crate::moderation::Ban { until: 0, reason: "collusion".into() });
        let (bal1, bal2) = (balance(&s, p(1)), balance(&s, p(2)));
        s.tick(4 * DAY_NS);
        assert_eq!(balance(&s, p(1)), bal1, "suspended: not paid");
        assert_eq!(stats(&s, p(1)).paid_total, 0);
        assert_eq!(balance(&s, p(2)), bal2 + 10_000);
        assert_eq!(s.treasury, 10 * E8S - 10_000, "the share of Alice went back");
        assert_eq!(owed_total(&s), 0);
    }

    #[test]
    fn money_is_conserved_with_referrals() {
        let mut s = setup();
        link(&mut s, p(2), p(1), 0);
        link(&mut s, p(3), p(1), 0);
        let before = total_value(&s);
        let now = play_cash(&mut s);
        assert!(owed_total(&s) > 0);
        let r = s.referrals.clone().unwrap();
        assert_eq!(r.owed.values().sum::<u64>(), r.owed_total);
        assert_eq!(s.house_stats().referral_owed, r.owed_total);
        assert_eq!(total_value(&s), before);
        for k in 1..=3 {
            if s.tables[&7].seat_of(&p(k)).is_some() {
                s.leave_table(p(k), 7, now).unwrap();
            }
        }
        // Full recount from the fields, while the cuts are still owed: every
        // e8s is in a wallet, the treasury, a pool, the jackpot or owed.
        let wallets = |s: &State| s.users.iter().map(|(_, u)| u.balance).sum::<u64>();
        let recount = |s: &State| {
            wallets(s) + s.treasury + s.rewards().pools_total() + s.jackpot(crate::table::Currency::Icp) + owed_total(s)
        };
        let owed_now = owed_total(&s);
        assert!(owed_now > 0);
        assert_eq!(recount(&s), before);
        let (treasury, pools, wallets_before) = (s.treasury, s.rewards().pools_total(), wallets(&s));

        // Monday: the referral week and the weekly leaderboards both close.
        s.tick(4 * DAY_NS);
        assert_eq!(owed_total(&s), 0);
        assert_eq!(stats(&s, p(1)).paid_total, owed_now);
        assert_eq!(s.treasury, treasury, "the payout moves owed money only");
        let board_prizes: u64 = s.rewards().last_winners.unwrap()[..2].iter().flatten().map(|w| w.prize).sum();
        assert_eq!(pools - s.rewards().pools_total(), board_prizes, "the pools paid only the weekly boards");
        assert_eq!(wallets(&s) - wallets_before, owed_now + board_prizes);
        assert_eq!(recount(&s), before);
        assert_eq!(total_value(&s), before);
        assert_eq!(s.wallet_total, s.recount_wallet_total());
    }

    #[test]
    fn liabilities_include_referral_owed() {
        let mut s = setup();
        link(&mut s, p(2), p(1), 0);
        s.fund_rewards(100_000);
        let before = s.house_stats();
        assert_eq!(before.referral_owed, 0);
        s.earn_rake(&p(2), 100_000, 100_000, SEC);
        let after = s.house_stats();
        assert_eq!(after.referral_owed, 20_000);
        assert_eq!(after.treasury, before.treasury - 20_000);
        assert_eq!(after.liabilities_total, before.liabilities_total);
    }
}

/// The referrals page: totals and the friend list.
#[cfg(test)]
mod view_tests {
    use super::*;
    use crate::leaderboards::{week_of, DAY_NS};

    const HOUR: u64 = 3_600 * 1_000_000_000;

    fn pid(n: u16) -> Principal {
        Principal::from_slice(&[(n >> 8) as u8, n as u8, 9, 9, 9, 9])
    }

    /// Alice (pid 1) joined at 0 with `n` friends (pid 100..) attached at 1h, 2h, ...
    fn with_friends(n: u16) -> State {
        let mut s = State::new(Principal::management_canister(), 10_000);
        s.register(pid(1), "Alice".into(), 0).unwrap();
        for k in 0..n {
            let at = (k as u64 + 1) * HOUR;
            let f = pid(100 + k);
            s.register(f, format!("friend{k}"), at - 1).unwrap();
            s.set_referrer(f, "alice", at).unwrap();
        }
        s
    }

    #[test]
    fn view_lists_friends_newest_first_and_pages() {
        let s = with_friends(120);
        let now = 200 * HOUR;
        let v0 = s.referral_view(&pid(1), 0, now);
        assert_eq!((v0.code.as_str(), v0.friends, v0.rows.len()), ("Alice", 120, 50));
        assert_eq!(v0.rows[0].username, "friend119");
        assert_eq!(v0.rows[0].joined_at, 120 * HOUR);
        assert_eq!(v0.rows[0].ends_at, 120 * HOUR + REFERRAL_DURATION_NS);
        assert_eq!(v0.rows[49].username, "friend70");
        let v1 = s.referral_view(&pid(1), 50, now);
        assert_eq!((v1.rows.len(), v1.rows[0].username.as_str()), (50, "friend69"));
        let v2 = s.referral_view(&pid(1), 100, now);
        assert_eq!((v2.rows.len(), v2.rows[19].username.as_str()), (20, "friend0"));
        assert!(s.referral_view(&pid(1), 120, now).rows.is_empty());
        assert!(s.referral_view(&pid(1), u32::MAX, now).rows.is_empty());
    }

    #[test]
    fn view_totals_and_can_enter_code() {
        let mut s = with_friends(2);
        let monday = 4 * DAY_NS;
        // Alice is owed this week and has been paid before.
        s.referrals = Some(ReferralState { owed: [(pid(1), 700)].into(), owed_total: 700, week: week_of(SEC) });
        s.user_mut(&pid(1)).unwrap().referrer_stats.as_mut().unwrap().paid_total = 5_000;
        let v = s.referral_view(&pid(1), 0, SEC * 10);
        assert_eq!((v.owed_this_week, v.paid_total, v.pays_at), (700, 5_000, monday));
        assert_eq!(v.my_referrer, None);
        // Alice has no referrer and her account is more than 24 h old only after a day.
        assert!(s.referral_view(&pid(1), 0, DAY_NS - 1).can_enter_code);
        assert!(!s.referral_view(&pid(1), 0, DAY_NS).can_enter_code);
        // A week that rolled but was not yet closed still shows what is about to be paid.
        assert_eq!(s.referral_view(&pid(1), 0, monday + SEC).owed_this_week, 700);
        // A friend sees the referrer, and cannot enter another code.
        let f = s.referral_view(&pid(100), 0, 2 * HOUR);
        assert_eq!(f.my_referrer.as_deref(), Some("Alice"));
        assert!(!f.can_enter_code);
        assert_eq!((f.friends, f.owed_this_week, f.paid_total), (0, 0, 0));
    }

    #[test]
    fn anonymous_and_unregistered_get_the_zero_view() {
        let s = with_friends(1);
        for who in [Principal::anonymous(), pid(999)] {
            let v = s.referral_view(&who, 0, HOUR);
            assert_eq!(v, ReferralView::default());
            assert!(!v.can_enter_code);
        }
    }

    #[test]
    fn active_means_earned_in_the_last_seven_days() {
        let mut s = with_friends(3);
        let now = 30 * DAY_NS;
        let set = |s: &mut State, k: u16, at: u64| {
            let key = FriendKey::new(&pid(1), (k as u64 + 1) * HOUR, &pid(100 + k));
            let mut e = s.stores.referrals.get(&key).unwrap();
            e.earned = 10;
            e.last_earned_at = at;
            s.stores.referrals.insert(key, e);
        };
        set(&mut s, 0, now - 7 * DAY_NS + 1); // just inside
        set(&mut s, 1, now - 7 * DAY_NS); // just outside
        // friend 2 never earned
        let v = s.referral_view(&pid(1), 0, now);
        assert_eq!(v.active_7d, 1);
        let by_name = |n: &str| v.rows.iter().find(|r| r.username == n).unwrap().clone();
        assert!(by_name("friend0").active);
        assert!(!by_name("friend1").active);
        assert!(!by_name("friend2").active);
        assert_eq!(by_name("friend1").earned, 10);
    }

    const SEC: u64 = 1_000_000_000;
}
