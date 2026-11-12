//! Limits players set for themselves: a break from ICP play, a daily cap on
//! deposits, and keeping their tables off their public profile.
//!
//! A limit that protects the player takes effect at once; one that loosens it
//! (a higher cap, no cap) waits a day, and a break can't be ended early, so a
//! decision made in a calm moment holds. Nothing here ever stops a player
//! taking money out: withdrawals and claiming ICP already sent to the deposit
//! address always work.

use crate::state::{State, R};
use crate::table::Currency;
use crate::tournament::TournamentStatus;
use candid::{CandidType, Deserialize, Principal};

const DAY_NS: u64 = 24 * 60 * 60 * 1_000_000_000;
/// How long a loosened limit waits before it applies.
pub const COOLING_OFF_NS: u64 = DAY_NS;
/// Longest break that can be taken in one go.
pub const MAX_BREAK_DAYS: u32 = 365;

/// A loosened deposit cap waiting for its cooling-off day.
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PendingCap {
    /// The new cap in e8s; `None` removes the cap.
    pub cap: Option<u64>,
    /// When it applies (ns).
    pub from: u64,
}

/// Stored on the player (`User::limits`), created the first time they set one.
/// New fields must be `Option<T>` so stored records still decode.
#[derive(CandidType, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Limits {
    /// No ICP games or deposits before this time (ns); 0 = no break.
    pub excluded_until: u64,
    /// Most ICP deposited per UTC day, in e8s.
    pub deposit_cap: Option<u64>,
    pub pending_cap: Option<PendingCap>,
    /// The UTC day `deposited_today` counts.
    pub deposit_day: u64,
    /// ICP deposited on `deposit_day` (e8s), wallet and address deposits alike.
    pub deposited_today: u64,
    /// Leave the player's tables (and the "at a table" part of being online)
    /// off their public profile. Friends still see them.
    pub hide_tables: bool,
}

impl Limits {
    /// Applies a pending cap that is due and starts a new day's count.
    fn settle(&mut self, now: u64) {
        if self.pending_cap.as_ref().is_some_and(|p| now >= p.from) {
            self.deposit_cap = self.pending_cap.take().unwrap().cap;
        }
        if self.deposit_day != now / DAY_NS {
            self.deposit_day = now / DAY_NS;
            self.deposited_today = 0;
        }
    }
}

impl State {
    /// The caller's limits as they stand at `now`.
    pub fn limits(&self, p: &Principal, now: u64) -> Limits {
        let mut l = self.users.get(p).and_then(|u| u.limits).unwrap_or_default();
        l.settle(now);
        l
    }

    fn update_limits(&mut self, p: &Principal, now: u64, f: impl FnOnce(&mut Limits) -> R<()>) -> R<Limits> {
        let mut u = self.user_mut(p)?;
        let mut l = u.limits.clone().unwrap_or_default();
        l.settle(now);
        f(&mut l)?;
        u.limits = Some(l.clone());
        Ok(l)
    }

    /// Takes a break from ICP games and deposits for `days`. A break already
    /// running is only ever extended. It also takes the player out of ICP play
    /// they're in: they stand up from ICP cash tables (after the hand, if one
    /// is on) and are refunded from ICP events that haven't started. Events
    /// already under way carry on: leaving one would forfeit the entry.
    pub fn take_break(&mut self, p: Principal, days: u32, now: u64) -> R<Limits> {
        if days == 0 || days > MAX_BREAK_DAYS {
            return Err(format!("Choose between 1 and {MAX_BREAK_DAYS} days"));
        }
        let l = self.update_limits(&p, now, |l| {
            l.excluded_until = l.excluded_until.max(now + days as u64 * DAY_NS);
            Ok(())
        })?;
        let cash: Vec<u64> = self
            .tables
            .values()
            .filter(|t| t.config.currency == Currency::Icp && !self.table_tournament.contains_key(&t.id) && t.seat_of(&p).is_some())
            .map(|t| t.id)
            .collect();
        for id in cash {
            let _ = self.leave_table(p, id, now);
        }
        let events: Vec<u64> = self
            .tournaments
            .values()
            .filter(|t| t.template.currency == Currency::Icp && t.status == TournamentStatus::Registering && t.entrants.iter().any(|e| e.player == p))
            .map(|t| t.id)
            .collect();
        for id in events {
            let _ = self.unregister_tournament(p, id);
        }
        Ok(l)
    }

    /// Sets (`Some`) or removes (`None`) the daily deposit cap. A lower cap
    /// applies now; a higher one, or none, after the cooling-off day.
    pub fn set_deposit_cap(&mut self, p: Principal, cap: Option<u64>, now: u64) -> R<Limits> {
        self.update_limits(&p, now, |l| {
            let tighter = match (cap, l.deposit_cap) {
                (Some(new), Some(old)) => new <= old,
                (Some(_), None) => true,
                (None, None) => true,
                (None, Some(_)) => false,
            };
            if tighter {
                l.deposit_cap = cap;
                l.pending_cap = None;
            } else {
                l.pending_cap = Some(PendingCap { cap, from: now + COOLING_OFF_NS });
            }
            Ok(())
        })
    }

    pub fn set_hide_tables(&mut self, p: Principal, hide: bool, now: u64) -> R<Limits> {
        self.update_limits(&p, now, |l| {
            l.hide_tables = hide;
            Ok(())
        })
    }

    /// Refuses putting ICP into play during a break. Play chips aren't money,
    /// so they stay open.
    pub fn check_can_play(&self, p: &Principal, currency: Currency, now: u64) -> R<()> {
        if currency != Currency::Icp {
            return Ok(());
        }
        let until = self.limits(p, now).excluded_until;
        if until > now {
            let days = (until - now).div_ceil(DAY_NS);
            return Err(format!("You're taking a break from ICP games ({days} day{} left)", if days == 1 { "" } else { "s" }));
        }
        Ok(())
    }

    /// For deposits from the player's wallet: the break, and today's cap.
    pub fn check_deposit_allowed(&self, p: &Principal, amount: u64, now: u64) -> R<()> {
        self.check_can_play(p, Currency::Icp, now).map_err(|_| "You're taking a break: deposits are closed until it ends. Withdrawals still work.".to_string())?;
        let l = self.limits(p, now);
        if let Some(cap) = l.deposit_cap {
            let left = cap.saturating_sub(l.deposited_today);
            if amount > left {
                return Err(format!("That's over your daily deposit limit ({} ICP left today)", crate::state::fmt_icp(left)));
            }
        }
        Ok(())
    }

    /// Counts a completed deposit toward today's cap. Players who never set a
    /// limit aren't given a record just for this.
    pub fn note_deposited(&mut self, p: &Principal, amount: u64, now: u64) {
        if self.users.get(p).is_some_and(|u| u.limits.is_some()) {
            let _ = self.update_limits(p, now, |l| {
                l.deposited_today = l.deposited_today.saturating_add(amount);
                Ok(())
            });
        }
    }

    pub fn hides_tables(&self, p: &Principal) -> bool {
        self.users.get(p).and_then(|u| u.limits).is_some_and(|l| l.hide_tables)
    }
}

#[cfg(test)]
mod tests {
    use crate::state::tests::{p, setup, SEC};
    use crate::state::E8S;
    use crate::table::Currency;
    use super::*;

    const DAY: u64 = DAY_NS;

    #[test]
    fn a_break_closes_icp_play_and_deposits_but_not_chips_or_withdrawals() {
        let mut s = setup();
        let now = 10 * DAY;
        s.take_break(p(1), 7, now).unwrap();
        assert!(s.check_can_play(&p(1), Currency::Icp, now).unwrap_err().contains("7 days left"));
        s.check_can_play(&p(1), Currency::Chips, now).unwrap();
        assert!(s.check_deposit_allowed(&p(1), E8S, now).unwrap_err().contains("Withdrawals still work"));
        let to = icrc_ledger_types::icrc1::account::Account { owner: p(9), subaccount: None };
        assert!(s.begin_withdraw(p(1), to, E8S, now).is_ok());
        // ICP tables refuse a seat.
        let icp = s.tables.values().find(|t| t.config.currency == Currency::Icp).unwrap();
        let (id, buy) = (icp.id, icp.config.min_buy_in);
        assert!(s.join_table(p(1), id, 0, buy, None, now).unwrap_err().contains("break"));
        // It can't be shortened, only extended, and it ends on time.
        s.take_break(p(1), 1, now).unwrap();
        assert_eq!(s.limits(&p(1), now).excluded_until, now + 7 * DAY);
        s.check_can_play(&p(1), Currency::Icp, now + 7 * DAY).unwrap();
        assert!(s.take_break(p(1), 0, now).is_err());
        assert!(s.take_break(p(1), MAX_BREAK_DAYS + 1, now).is_err());
    }

    #[test]
    fn a_break_takes_the_player_out_of_icp_play_already_under_way() {
        let mut s = setup();
        let now = 10 * DAY;
        s.credit(&p(1), 1_000 * E8S).unwrap();
        let icp = s.tables.values().find(|t| t.config.currency == Currency::Icp && !s.table_tournament.contains_key(&t.id)).unwrap();
        let (table, buy) = (icp.id, icp.config.min_buy_in);
        s.join_table(p(1), table, 0, buy, None, now).unwrap();
        let event = crate::state::tests::open_tournament(&mut s, "ICP Nightly");
        assert_eq!(s.tournaments[&event].template.currency, Currency::Icp);
        s.register_tournament(p(1), event, SEC).unwrap();
        let before = s.users.get(&p(1)).unwrap().balance;

        s.take_break(p(1), 7, now).unwrap();
        assert!(s.tables[&table].seat_of(&p(1)).is_none(), "stood up (no hand was on)");
        assert!(!s.tournaments[&event].entrants.iter().any(|e| e.player == p(1)), "unregistered");
        let t = &s.tournaments[&event].template;
        assert_eq!(s.users.get(&p(1)).unwrap().balance, before + buy + t.buy_in + t.fee, "every e8s came back");
    }

    #[test]
    fn a_lower_cap_applies_now_and_a_higher_one_after_a_day() {
        let mut s = setup();
        let now = 10 * DAY;
        s.set_deposit_cap(p(1), Some(5 * E8S), now).unwrap();
        s.check_deposit_allowed(&p(1), 5 * E8S, now).unwrap();
        assert!(s.check_deposit_allowed(&p(1), 5 * E8S + 1, now).is_err());
        s.note_deposited(&p(1), 3 * E8S, now);
        assert!(s.check_deposit_allowed(&p(1), 3 * E8S, now + SEC).unwrap_err().contains("2 ICP left"));
        // Raising waits; lowering doesn't.
        s.set_deposit_cap(p(1), Some(50 * E8S), now).unwrap();
        assert!(s.check_deposit_allowed(&p(1), 3 * E8S, now + SEC).is_err(), "still the old cap");
        s.set_deposit_cap(p(1), Some(4 * E8S), now + SEC).unwrap();
        assert_eq!(s.limits(&p(1), now + SEC).pending_cap, None, "tightening cancels a pending raise");
        assert!(s.check_deposit_allowed(&p(1), 2 * E8S, now + SEC).is_err(), "4 cap, 3 used");
        // Removing the cap waits too, then applies; a new day starts a new count.
        s.set_deposit_cap(p(1), None, now + SEC).unwrap();
        assert_eq!(s.limits(&p(1), now + SEC).deposit_cap, Some(4 * E8S));
        let later = now + SEC + COOLING_OFF_NS;
        assert_eq!(s.limits(&p(1), later).deposit_cap, None);
        s.check_deposit_allowed(&p(1), 1_000 * E8S, later).unwrap();
        s.set_deposit_cap(p(1), Some(E8S), later).unwrap();
        assert_eq!(s.limits(&p(1), later + DAY).deposited_today, 0);
    }

    #[test]
    fn deposits_are_counted_only_for_players_with_limits() {
        let mut s = setup();
        s.note_deposited(&p(2), E8S, DAY);
        assert!(s.users.get(&p(2)).unwrap().limits.is_none());
    }

    #[test]
    fn hidden_tables_stay_off_the_public_profile() {
        let mut s = setup();
        let id = *s.tables.keys().next().unwrap();
        let buy = s.tables[&id].config.min_buy_in;
        let currency = s.tables[&id].config.currency;
        if currency == Currency::Icp {
            s.credit(&p(1), buy).unwrap();
        }
        s.join_table(p(1), id, 0, buy, None, 0).unwrap();
        assert_eq!(s.public_profile(&p(1), DAY).unwrap().tables, vec![id]);
        s.set_hide_tables(p(1), true, 0).unwrap();
        let v = s.public_profile(&p(1), DAY).unwrap();
        assert!(v.tables.is_empty());
        assert!(!v.online, "being seated doesn't show as online either");
    }
}
