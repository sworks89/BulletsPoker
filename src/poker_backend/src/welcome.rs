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

impl State {
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
