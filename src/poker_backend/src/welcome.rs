//! Welcome bonus: a new player's first successful ICP deposit earns a locked
//! matching bonus (up to 100 ICP). It is not money: it never touches balances,
//! `wallet_total` or the house stats. Later it is released as rake is paid.

use crate::state::{State, E8S};
use crate::wallet::{OpKind, OpStatus};
use candid::{CandidType, Deserialize, Principal};

/// The most a welcome bonus can be (100 ICP).
pub const WELCOME_MAX: u64 = 100 * E8S;
/// A bonus still locked this long after the grant expires (90 days).
pub const WELCOME_EXPIRY_NS: u64 = 90 * 24 * 60 * 60 * 1_000_000_000;

/// The share of the house's cut of each rake payment that releases the bonus (30%).
pub const WELCOME_RELEASE_BPS: u64 = 3_000;

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub enum WelcomeBonus {
    /// Not eligible (deposited before the bonus existed, or nothing to match).
    Ineligible,
    Active {
        granted_at: u64,
        /// The full bonus granted (e8s). Locked = `amount - released`, or 0 once expired.
        amount: u64,
        released: u64,
        /// Fraction of a release unit carried to the next one (basis points).
        carry_bps: u64,
    },
}

/// The part of the bonus still locked at `now`: zero once released or expired.
pub fn welcome_locked(b: &WelcomeBonus, now: u64) -> u64 {
    match b {
        WelcomeBonus::Ineligible => 0,
        WelcomeBonus::Active { granted_at, amount, released, .. } => {
            if now.saturating_sub(*granted_at) >= WELCOME_EXPIRY_NS {
                0
            } else {
                amount.saturating_sub(*released)
            }
        }
    }
}

/// What the caller sees of their welcome bonus.
#[derive(CandidType, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct WelcomeView {
    /// No bonus yet and still entitled to one on the first deposit.
    pub eligible: bool,
    /// Granted, not fully released, not expired.
    pub active: bool,
    pub amount: u64,
    pub released: u64,
    /// Still locked: zero once expired or fully released.
    pub locked: u64,
    pub expires_at: u64,
    pub expired: bool,
}

impl State {
    /// The caller's bonus: strangers and ineligible players get all zeros.
    pub fn welcome_view(&self, p: &Principal, now: u64) -> WelcomeView {
        let Some(u) = self.users.get(p) else { return WelcomeView::default() };
        match u.welcome {
            None => WelcomeView { eligible: true, ..Default::default() },
            Some(WelcomeBonus::Ineligible) => WelcomeView::default(),
            Some(b @ WelcomeBonus::Active { granted_at, amount, released, .. }) => {
                let locked = welcome_locked(&b, now);
                let expires_at = granted_at.saturating_add(WELCOME_EXPIRY_NS);
                WelcomeView { eligible: false, active: locked > 0, amount, released, locked, expires_at, expired: now >= expires_at }
            }
        }
    }

    /// Grants the bonus on a player's first successful deposit; a no-op for
    /// anyone who already has a verdict.
    pub(crate) fn grant_welcome(&mut self, p: &Principal, deposit: u64, now: u64) {
        let Some(mut u) = self.users.get_mut(p) else { return };
        if u.welcome.is_some() {
            return;
        }
        let amount = deposit.min(WELCOME_MAX);
        u.welcome = Some(if amount == 0 {
            WelcomeBonus::Ineligible
        } else {
            WelcomeBonus::Active { granted_at: now, amount, released: 0, carry_bps: 0 }
        });
    }

    /// Releases the locked bonus as rake is paid: `WELCOME_RELEASE_BPS` of the
    /// house's share (`house_e8s`) moves from the treasury into the player's
    /// balance, until the bonus is used up or expires.
    ///
    /// Called at the end of `earn_rake`, after `fund_rewards` has put that rake
    /// into the treasury. The amount is capped by the treasury, so it can
    /// never underflow.
    pub(crate) fn release_welcome(&mut self, p: &Principal, house_e8s: u64, now: u64) {
        if house_e8s == 0 {
            return;
        }
        // A release capped by the treasury or the locked amount is forfeited, not owed.
        let Some(b) = self.users.get(p).and_then(|u| u.welcome) else { return };
        let WelcomeBonus::Active { carry_bps, .. } = b else { return };
        let locked = welcome_locked(&b, now);
        if locked == 0 || self.is_banned(p, now) {
            return;
        }
        let total = house_e8s as u128 * WELCOME_RELEASE_BPS as u128 + carry_bps as u128;
        let want = (total / 10_000).min(u64::MAX as u128) as u64;
        let amount = want.min(locked).min(self.treasury);
        let paid = if amount > 0 {
            self.treasury -= amount;
            if self.credit(p, amount).is_ok() {
                amount
            } else {
                self.treasury += amount;
                0
            }
        } else {
            0
        };
        if let Some(mut u) = self.users.get_mut(p) {
            if let Some(WelcomeBonus::Active { released, carry_bps, .. }) = u.welcome.as_mut() {
                *carry_bps = (total % 10_000) as u64;
                *released += paid;
            }
        }
    }

    /// One-time pass after the upgrade that ships the bonus: everyone who had
    /// already deposited is ineligible. Returns how many players were marked;
    /// running it again does nothing.
    pub fn mark_existing_depositors(&mut self) -> usize {
        if self.welcome_marked == Some(true) {
            return 0;
        }
        let depositors: std::collections::BTreeSet<Principal> = self
            .stores
            .ledger_ops
            .iter()
            .map(|e| e.value())
            .filter(|op| matches!(op.kind, OpKind::Deposit | OpKind::Sweep) && matches!(op.status, OpStatus::Completed { .. }))
            .map(|op| op.user)
            .collect();
        let mut marked = 0;
        for p in depositors {
            if let Some(mut u) = self.users.get_mut(&p) {
                if u.welcome.is_none() {
                    u.welcome = Some(WelcomeBonus::Ineligible);
                    marked += 1;
                }
            }
        }
        self.welcome_marked = Some(true);
        marked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests::{p, setup, total_value};
    use crate::moderation::Ban;
    use crate::state::{ActiveGift, Settings, Stats, User, E8S};
    use crate::wallet::Outcome;
    use candid::{CandidType, Deserialize, Principal};
    use icrc_ledger_types::icrc1::account::Account;
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

    fn me() -> Account {
        Account { owner: Principal::anonymous(), subaccount: None }
    }

    fn deposit(s: &mut crate::state::State, who: u8, amount: u64, ok: bool, now: u64) -> u64 {
        let from = Account { owner: p(who), subaccount: None };
        let op = s.begin_deposit(p(who), from, me(), amount, now).unwrap();
        let out = if ok { Outcome::Ok(1) } else { Outcome::Failed("no funds".into()) };
        s.finish_op(op.id, out, now + 1).unwrap();
        op.id
    }

    fn welcome(s: &crate::state::State, who: u8) -> Option<WelcomeBonus> {
        s.users.get(&p(who)).unwrap().welcome
    }

    #[test]
    fn first_deposit_grants_a_matching_bonus_capped_at_100() {
        let mut s = setup();
        let (wt, liab) = (s.wallet_total, total_value(&s));
        deposit(&mut s, 1, 3 * E8S, true, 5);
        assert_eq!(welcome(&s, 1), Some(WelcomeBonus::Active { granted_at: 6, amount: 3 * E8S, released: 0, carry_bps: 0 }));
        assert_eq!(s.users.get(&p(1)).unwrap().balance, 13 * E8S, "balance rises by the deposit only");
        assert_eq!(s.wallet_total, wt + 3 * E8S);
        assert_eq!(total_value(&s), liab + 3 * E8S, "the bonus adds no liability");

        deposit(&mut s, 2, 250 * E8S, true, 5);
        match welcome(&s, 2) {
            Some(WelcomeBonus::Active { amount, .. }) => assert_eq!(amount, 100 * E8S),
            other => panic!("{other:?}"),
        }
        assert_eq!(s.users.get(&p(2)).unwrap().balance, 260 * E8S);
        assert_eq!(s.wallet_total, wt + 253 * E8S);
        assert_eq!(total_value(&s), liab + 253 * E8S);
    }

    #[test]
    fn second_deposit_and_failed_deposit_grant_nothing() {
        let mut s = setup();
        deposit(&mut s, 1, E8S, false, 5);
        assert_eq!(welcome(&s, 1), None, "a failed deposit grants nothing and keeps eligibility");
        deposit(&mut s, 1, E8S, true, 10);
        let first = welcome(&s, 1);
        assert!(matches!(first, Some(WelcomeBonus::Active { amount, .. }) if amount == E8S));
        deposit(&mut s, 1, 5 * E8S, true, 20);
        assert_eq!(welcome(&s, 1), first, "the second deposit changes nothing");
    }

    #[test]
    fn finishing_the_same_op_twice_grants_once() {
        let mut s = setup();
        let id = deposit(&mut s, 1, 2 * E8S, true, 5);
        let first = welcome(&s, 1);
        s.finish_op(id, Outcome::Ok(1), 50).unwrap();
        assert_eq!(welcome(&s, 1), first);
        assert_eq!(s.users.get(&p(1)).unwrap().balance, 12 * E8S);
    }

    #[test]
    fn sweep_counts_as_a_first_deposit() {
        let mut s = setup();
        let op = s.begin_sweep(p(1), [1; 32], me(), 4 * E8S, 5).unwrap();
        s.finish_op(op.id, Outcome::Ok(1), 6).unwrap();
        assert!(matches!(welcome(&s, 1), Some(WelcomeBonus::Active { amount, .. }) if amount == 4 * E8S));
    }

    #[test]
    fn zero_amount_marks_ineligible() {
        let mut s = setup();
        s.grant_welcome(&p(1), 0, 5);
        assert_eq!(welcome(&s, 1), Some(WelcomeBonus::Ineligible));
    }

    #[test]
    fn welcome_view_states() {
        let mut s = setup();
        let v = s.welcome_view(&p(1), 5);
        assert!(v.eligible && !v.active && !v.expired);
        assert_eq!((v.amount, v.locked, v.expires_at), (0, 0, 0));

        s.users.get_mut(&p(1)).unwrap().welcome = Some(WelcomeBonus::Active { granted_at: 10, amount: 8, released: 3, carry_bps: 0 });
        let v = s.welcome_view(&p(1), 11);
        assert_eq!(v, WelcomeView { eligible: false, active: true, amount: 8, released: 3, locked: 5, expires_at: 10 + WELCOME_EXPIRY_NS, expired: false });

        s.users.get_mut(&p(1)).unwrap().welcome = Some(WelcomeBonus::Active { granted_at: 10, amount: 8, released: 8, carry_bps: 0 });
        let v = s.welcome_view(&p(1), 11);
        assert!(!v.active && !v.expired && !v.eligible);
        assert_eq!((v.amount, v.released, v.locked), (8, 8, 0));

        s.users.get_mut(&p(1)).unwrap().welcome = Some(WelcomeBonus::Active { granted_at: 10, amount: 8, released: 3, carry_bps: 0 });
        let v = s.welcome_view(&p(1), 10 + WELCOME_EXPIRY_NS);
        assert!(v.expired && !v.active && !v.eligible);
        assert_eq!((v.amount, v.released, v.locked), (8, 3, 0));

        s.users.get_mut(&p(1)).unwrap().welcome = Some(WelcomeBonus::Ineligible);
        assert_eq!(s.welcome_view(&p(1), 11), WelcomeView::default());
    }

    #[test]
    fn welcome_view_zero_for_strangers() {
        let s = setup();
        assert_eq!(s.welcome_view(&Principal::anonymous(), 5), WelcomeView::default());
        assert_eq!(s.welcome_view(&p(200), 5), WelcomeView::default());
    }

    #[test]
    fn locked_amount_after_release_and_expiry() {
        let b = WelcomeBonus::Active { granted_at: 10, amount: 8, released: 3, carry_bps: 0 };
        assert_eq!(welcome_locked(&b, 10), 5);
        assert_eq!(welcome_locked(&WelcomeBonus::Ineligible, 10), 0);
        assert_eq!(welcome_locked(&b, 10 + WELCOME_EXPIRY_NS), 0);
    }

    #[test]
    fn existing_depositors_are_marked_ineligible_once() {
        let mut s = setup();
        deposit(&mut s, 1, 3 * E8S, true, 5);
        deposit(&mut s, 2, 3 * E8S, false, 5);
        // Simulate the state before this feature existed.
        s.users.get_mut(&p(1)).unwrap().welcome = None;
        assert_eq!(s.mark_existing_depositors(), 1);
        assert_eq!(welcome(&s, 1), Some(WelcomeBonus::Ineligible));
        assert_eq!(welcome(&s, 2), None, "failed deposits do not count");
        assert_eq!(welcome(&s, 3), None, "never deposited");
        assert_eq!(s.mark_existing_depositors(), 0, "second pass does nothing");
        deposit(&mut s, 1, 3 * E8S, true, 50);
        assert_eq!(welcome(&s, 1), Some(WelcomeBonus::Ineligible), "a later deposit grants nothing");
    }

    #[test]
    fn old_user_records_decode_without_welcome() {
        #[derive(CandidType, Deserialize)]
        struct OldUser {
            username: String,
            status: String,
            balance: u64,
            chips: u64,
            settings: Settings,
            history: VecDeque<u64>,
            daily_streak: u32,
            last_daily_at: u64,
            last_throw_at: u64,
            medals: BTreeMap<String, u64>,
            gifts_sent: u64,
            throws_made: u64,
            ops: Vec<u64>,
            blocked: BTreeSet<Principal>,
            ban: Option<Ban>,
            chat_muted_until: u64,
            reports_day: u64,
            reports_today: u32,
            avatar: u32,
            card_back: u32,
            inventory: Vec<u32>,
            friends: BTreeSet<Principal>,
            friend_requests: BTreeSet<Principal>,
            stats: Stats,
            xp: u64,
            gift: Option<ActiveGift>,
            created_at: u64,
            last_seen: u64,
            last_chat_at: u64,
            rewards: Option<crate::rewards::RewardsAccount>,
            referral: Option<crate::referrals::ReferralLink>,
            referrer_stats: Option<crate::referrals::ReferrerStats>,
        }
        let s = setup();
        let u = s.users.get(&p(1)).unwrap();
        let old = OldUser {
            username: u.username.clone(),
            status: u.status.clone(),
            balance: u.balance,
            chips: u.chips,
            settings: u.settings.clone(),
            history: u.history.clone(),
            daily_streak: u.daily_streak,
            last_daily_at: u.last_daily_at,
            last_throw_at: u.last_throw_at,
            medals: u.medals.clone(),
            gifts_sent: u.gifts_sent,
            throws_made: u.throws_made,
            ops: u.ops.clone(),
            blocked: u.blocked.clone(),
            ban: u.ban.clone(),
            chat_muted_until: u.chat_muted_until,
            reports_day: u.reports_day,
            reports_today: u.reports_today,
            avatar: u.avatar,
            card_back: u.card_back,
            inventory: u.inventory.clone(),
            friends: u.friends.clone(),
            friend_requests: u.friend_requests.clone(),
            stats: u.stats.clone(),
            xp: u.xp,
            gift: u.gift.clone(),
            created_at: u.created_at,
            last_seen: u.last_seen,
            last_chat_at: u.last_chat_at,
            rewards: None,
            referral: None,
            referrer_stats: None,
        };
        let back: User = candid::decode_one(&candid::encode_one(&old).unwrap()).unwrap();
        assert_eq!((back.username.as_str(), back.balance), ("player1", 10 * E8S));
        assert!(back.welcome.is_none());
    }
}

/// Release of the locked bonus, run against the full game state.
#[cfg(test)]
mod release_tests {
    use super::*;
    use crate::moderation::Ban;
    use crate::referrals::{FriendEntry, FriendKey, ReferralLink, ReferrerStats};
    use crate::state::tests::{many_players, p, run_until, setup, start_scheduled, total_value, SEC};
    use crate::state::{RoomArgs, E8S};
    use crate::table::Currency;

    fn give(s: &mut State, who: u8, amount: u64, granted_at: u64) {
        s.users.get_mut(&p(who)).unwrap().welcome = Some(WelcomeBonus::Active { granted_at, amount, released: 0, carry_bps: 0 });
    }

    fn released(s: &State, who: u8) -> u64 {
        match s.users.get(&p(who)).unwrap().welcome {
            Some(WelcomeBonus::Active { released, .. }) => released,
            other => panic!("{other:?}"),
        }
    }

    fn carry(s: &State, who: u8) -> u64 {
        match s.users.get(&p(who)).unwrap().welcome {
            Some(WelcomeBonus::Active { carry_bps, .. }) => carry_bps,
            other => panic!("{other:?}"),
        }
    }

    fn bal(s: &State, who: u8) -> u64 {
        s.users.get(&p(who)).unwrap().balance
    }

    /// Pays rake the way the table does: the house share funds the rewards first.
    fn rake(s: &mut State, who: u8, house: u64, now: u64) {
        s.fund_rewards(house);
        s.earn_rake(&p(who), house, house, now);
    }

    fn house_rake(s: &State, who: u8) -> u64 {
        s.user(&p(who)).unwrap().rewards.unwrap().period.w_house_rake.unwrap_or(0)
    }

    fn play_cash(s: &mut State) -> u64 {
        let table = 7;
        for k in 1..=3 {
            s.join_table(p(k), table, k - 1, 200_000_000, None, 0).unwrap();
        }
        run_until(s, SEC, 2_000, |s| s.tables[&table].hand_no >= 15 || s.tables[&table].eligible_count() < 2)
    }

    fn link(s: &mut State, friend: u8, referrer: u8) {
        let (f, r) = (p(friend), p(referrer));
        s.user_mut(&f).unwrap().referral = Some(ReferralLink { referrer: r, attached_at: 0 });
        s.stores.referrals.insert(FriendKey::new(&r, 0, &f), FriendEntry { attached_at: 0, earned: 0, last_earned_at: 0 });
        s.user_mut(&r).unwrap().referrer_stats.get_or_insert_with(ReferrerStats::default).friends += 1;
    }

    #[test]
    fn rake_releases_thirty_percent_of_the_house_share() {
        let mut s = setup();
        let mut twin = setup();
        give(&mut s, 1, 100 * E8S, 0);
        play_cash(&mut s);
        play_cash(&mut twin);
        let house = house_rake(&s, 1);
        assert!(house > 0);
        assert_eq!(WELCOME_RELEASE_BPS, 3_000);
        let want = house * 3_000 / 10_000;
        assert!(want > 0);
        assert_eq!(released(&s, 1), want);
        assert_eq!(carry(&s, 1), house * 3_000 % 10_000);
        assert_eq!(bal(&s, 1) - bal(&twin, 1), want, "the balance rose by the release");
        assert_eq!(twin.treasury - s.treasury, want, "and the treasury paid it");
        assert_eq!(s.wallet_total, s.recount_wallet_total());
    }

    #[test]
    fn carry_is_exact_over_many_small_payments() {
        let mut s = setup();
        give(&mut s, 1, 100 * E8S, 0);
        s.treasury = 10 * E8S;
        for _ in 0..7 {
            s.earn_rake(&p(1), 3, 3, SEC);
        }
        // 7 x 3 x 0.3 = 6.3 e8s
        assert_eq!((released(&s, 1), carry(&s, 1)), (6, 3_000));
        assert_eq!(s.treasury, 10 * E8S - 6);
    }

    #[test]
    fn release_is_capped_by_the_treasury() {
        let mut s = setup();
        give(&mut s, 1, 100 * E8S, 0);
        s.treasury = 5;
        let b = bal(&s, 1);
        s.earn_rake(&p(1), 1_000_000, 1_000_000, SEC);
        assert_eq!((s.treasury, released(&s, 1), bal(&s, 1)), (0, 5, b + 5));
        s.earn_rake(&p(1), 1_000_000, 1_000_000, SEC);
        assert_eq!((s.treasury, released(&s, 1), bal(&s, 1)), (0, 5, b + 5));
    }

    #[test]
    fn referred_player_at_the_cap_never_overdraws_the_treasury() {
        let mut s = setup();
        s.user_mut(&p(1)).unwrap().xp = 50;
        let id = s
            .create_room(
                p(1),
                RoomArgs { name: "Cap".into(), currency: Currency::Icp, big_blind: 2_000_000, max_seats: 6, password: None, action_timeout_secs: 30 },
                0,
            )
            .unwrap();
        link(&mut s, 2, 3);
        give(&mut s, 2, 100 * E8S, 0);
        assert_eq!(s.treasury, 0);
        s.join_table(p(2), id, 1, 100_000_000, None, 0).unwrap();
        s.join_table(p(1), id, 0, 100_000_000, None, 0).unwrap();
        let mut now = SEC;
        for n in 1..=30u64 {
            now = run_until(&mut s, now, 2_000, |s| s.tables[&id].hand_no >= n || s.tables[&id].eligible_count() < 2);
            let (h1, h2) = (house_rake(&s, 1), house_rake(&s, 2));
            let cut = s.referrals.as_ref().map_or(0, |r| r.owed_total);
            let tol = 10 * n;
            assert_eq!(released(&s, 2), h2 * 3_000 / 10_000, "hand {n}");
            // p2 alone: cut (20%) + release (30%) stay within half its house share.
            assert!(cut + released(&s, 2) <= h2 * 5_000 / 10_000 + tol, "hand {n}");
            // The host (no bonus, no referrer) leaves its full 60% in the treasury;
            // what is left beyond that is p2's own retention, at least 10% of h2.
            assert!(s.treasury + tol >= h1 * 6_000 / 10_000 + h2 * 1_000 / 10_000, "treasury keeps 10% of p2's share (hand {n})");
            assert!(s.treasury <= h1 + h2, "hand {n}");
        }
        let house: u64 = [1, 2].iter().map(|&k| house_rake(&s, k)).sum();
        assert!(released(&s, 2) > 0 && house > 0);
        assert!(s.referrals.as_ref().unwrap().owed_total > 0);
        assert_eq!(s.wallet_total, s.recount_wallet_total());
    }

    #[test]
    fn tournament_fee_releases_bonus() {
        let mut s = setup();
        many_players(&mut s, 4);
        give(&mut s, 10, 100 * E8S, 0);
        give(&mut s, 13, 100 * E8S, 0);
        let fee = E8S / 10;
        let release = fee * 3_000 / 10_000;
        let (id, start_at) = start_scheduled(&mut s, "ICP Nightly", 10..13);
        assert_eq!(released(&s, 10), release, "released at the start");
        assert_eq!(bal(&s, 10), bal(&s, 11) + release, "same cost as an unbonused entrant, less the release");
        assert_eq!(s.treasury, 3 * fee * 6_000 / 10_000 - release);
        s.treasury = 0;
        s.register_tournament(p(13), id, start_at + SEC).unwrap();
        assert_eq!(released(&s, 13), release, "and on late entry");
        assert_eq!(s.treasury, fee * 6_000 / 10_000 - release);
        assert_eq!(bal(&s, 13), bal(&s, 11) + release);
    }

    #[test]
    fn release_stops_when_fully_released() {
        let mut s = setup();
        give(&mut s, 1, 4_000, 0);
        s.treasury = 10 * E8S;
        let b = bal(&s, 1);
        rake(&mut s, 1, 10_000, SEC);
        assert_eq!(released(&s, 1), 3_000);
        rake(&mut s, 1, 10_000, 2 * SEC);
        assert_eq!(released(&s, 1), 4_000, "capped at what is left");
        let t = s.treasury;
        rake(&mut s, 1, 10_000, 3 * SEC);
        assert_eq!(released(&s, 1), 4_000);
        assert_eq!(bal(&s, 1), b + 4_000);
        assert_eq!(s.treasury, t + 6_000, "only the rake itself was added");
    }

    #[test]
    fn nothing_releases_after_ninety_days() {
        let mut s = setup();
        give(&mut s, 1, 100 * E8S, 7);
        s.treasury = 10 * E8S;
        let end = 7 + WELCOME_EXPIRY_NS;
        s.earn_rake(&p(1), 10_000, 10_000, end - 1);
        assert_eq!(released(&s, 1), 3_000, "the last instant of the window");
        s.earn_rake(&p(1), 10_000, 10_000, end);
        assert_eq!(released(&s, 1), 3_000, "expired");
        assert_eq!(s.treasury, 10 * E8S - 3_000);
    }

    #[test]
    fn suspended_players_release_nothing() {
        let mut s = setup();
        give(&mut s, 1, 100 * E8S, 0);
        s.treasury = 10 * E8S;
        s.user_mut(&p(1)).unwrap().ban = Some(Ban { until: 0, reason: "collusion".into() });
        let b = bal(&s, 1);
        s.earn_rake(&p(1), 10_000, 10_000, SEC);
        assert_eq!((released(&s, 1), carry(&s, 1), bal(&s, 1), s.treasury), (0, 0, b, 10 * E8S));
        // A lapsed suspension no longer blocks it.
        s.user_mut(&p(1)).unwrap().ban = Some(Ban { until: 5 * SEC, reason: "x".into() });
        s.earn_rake(&p(1), 10_000, 10_000, 5 * SEC);
        assert_eq!(released(&s, 1), 3_000);
    }

    #[test]
    fn ineligible_and_unmarked_players_release_nothing() {
        let mut s = setup();
        s.treasury = 10 * E8S;
        s.user_mut(&p(2)).unwrap().welcome = Some(WelcomeBonus::Ineligible);
        s.earn_rake(&p(1), 10_000, 10_000, SEC);
        s.earn_rake(&p(2), 10_000, 10_000, SEC);
        assert_eq!(s.treasury, 10 * E8S);
        assert_eq!((bal(&s, 1), bal(&s, 2)), (10 * E8S, 10 * E8S));
    }

    #[test]
    fn chip_play_releases_nothing() {
        let mut s = setup();
        many_players(&mut s, 3);
        for k in [2, 3, 10] {
            give(&mut s, k, 100 * E8S, 0);
        }
        let table = 1;
        assert_eq!(s.tables[&table].config.currency, Currency::Chips);
        let buy = s.tables[&table].config.max_buy_in;
        for k in 2..=3 {
            s.join_table(p(k), table, k - 1, buy, None, 0).unwrap();
        }
        run_until(&mut s, SEC, 2_000, |s| s.tables[&table].hand_no >= 15 || s.tables[&table].eligible_count() < 2);
        start_scheduled(&mut s, "Chip Rebuy Madness", 10..13);
        for k in [2, 3, 10] {
            assert_eq!(released(&s, k), 0);
            assert_eq!(carry(&s, k), 0);
        }
        assert_eq!(s.treasury, 0);
    }

    #[test]
    fn money_is_conserved_with_welcome_bonus() {
        let mut s = setup();
        for k in 1..=3 {
            give(&mut s, k, 100 * E8S, 0);
        }
        link(&mut s, 2, 1);
        let before = total_value(&s);
        let now = play_cash(&mut s);
        assert!(released(&s, 3) > 0);
        assert_eq!(total_value(&s), before);
        for k in 1..=3 {
            if s.tables[&7].seat_of(&p(k)).is_some() {
                s.leave_table(p(k), 7, now).unwrap();
            }
        }
        let owed = s.referrals.as_ref().map_or(0, |r| r.owed_total);
        let wallets: u64 = s.users.iter().map(|(_, u)| u.balance).sum();
        assert_eq!(wallets + s.treasury + s.rewards().pools_total() + s.jackpot(Currency::Icp) + owed, before);
        assert_eq!(s.wallet_total, s.recount_wallet_total());
        assert_eq!(s.wallet_total, wallets);
    }
}
