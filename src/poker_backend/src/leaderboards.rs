//! Leaderboards: the weekly and monthly periods (Task 4 adds the boards).
//!
//! A week runs from Monday 00:00 UTC; a month from the 1st at 00:00 UTC.

use crate::state::State;
use candid::Principal;

pub const DAY_NS: u64 = 24 * 60 * 60 * 1_000_000_000;

/// The week containing `now_ns`, counted in Monday-to-Monday weeks since the
/// epoch (1970-01-01 was a Thursday, so week 1 starts on 1970-01-05).
pub fn week_of(now_ns: u64) -> u64 {
    (now_ns / DAY_NS + 3) / 7
}

/// The month containing `now_ns`, as `year * 12 + (month - 1)`.
pub fn month_of(now_ns: u64) -> u64 {
    let (year, month, _) = civil_from_days(now_ns / DAY_NS);
    year * 12 + (month - 1)
}

/// Days since 1970-01-01 to (year, month 1..=12, day 1..=31), after Howard
/// Hinnant's `civil_from_days` (proleptic Gregorian calendar).
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

impl State {
    /// Closes any week or month that has ended. Called before any player's
    /// period counters are written. A no-op until the boards exist.
    pub(crate) fn close_periods(&mut self, _now: u64) {}

    /// A player's period points changed. A no-op until the boards exist.
    pub(crate) fn on_points(&mut self, _p: &Principal, _now: u64) {}

    /// A player's period hands or net changed. A no-op until the boards exist.
    pub(crate) fn on_net(&mut self, _p: &Principal, _now: u64) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEC: u64 = 1_000_000_000;

    #[test]
    fn weeks_start_on_monday_utc() {
        let monday = 1_792_368_000 * SEC; // 2026-10-19 00:00:00 UTC
        assert_eq!(week_of(monday), week_of(monday - SEC) + 1);
        assert_eq!(week_of(monday), week_of(monday + 7 * DAY_NS - SEC));
        assert_eq!(week_of(monday + 7 * DAY_NS), week_of(monday) + 1);
        // 1970-01-01 was a Thursday; the first Monday is 1970-01-05.
        assert_eq!(week_of(0), 0);
        assert_eq!(week_of(4 * DAY_NS - 1), 0);
        assert_eq!(week_of(4 * DAY_NS), 1);
    }

    #[test]
    fn months_start_on_the_first_utc() {
        let last = 1_793_491_199 * SEC; // 2026-10-31 23:59:59 UTC
        let first = 1_793_491_200 * SEC; // 2026-11-01 00:00:00 UTC
        assert_eq!(month_of(last), 2026 * 12 + 9);
        assert_eq!(month_of(first), 2026 * 12 + 10);
        assert_eq!(month_of(0), 1970 * 12);
        // Leap day 2028-02-29 is still February; 2028-03-01 is March.
        let feb29 = 1_835_395_200 * SEC; // 2028-02-29 00:00:00 UTC
        assert_eq!(month_of(feb29), 2028 * 12 + 1);
        assert_eq!(month_of(feb29 + DAY_NS), 2028 * 12 + 2);
    }
}
