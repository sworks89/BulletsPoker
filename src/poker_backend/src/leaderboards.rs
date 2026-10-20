//! Leaderboards: the weekly and monthly periods, the four boards and closing.
//!
//! A week runs from Monday 00:00 UTC; a month from the 1st at 00:00 UTC.
//! Board index order: 0 weekly points, 1 weekly winnings, 2 monthly points,
//! 3 monthly winnings.

use crate::rewards::{PeriodCounters, BOARDS};
use crate::state::{fmt_amount, Channel, MessageKind, State};
use crate::table::Currency;
use candid::{CandidType, Deserialize, Principal};

pub const DAY_NS: u64 = 24 * 60 * 60 * 1_000_000_000;

/// Board ids, as used in medal ids and `BoardView::board`.
pub const BOARD_IDS: [&str; BOARDS] = ["weekly_points", "weekly_winnings", "monthly_points", "monthly_winnings"];
/// Board names, as shown in medals and the lobby.
pub const BOARD_NAMES: [&str; BOARDS] = ["Weekly Points", "Weekly Winnings", "Monthly Points", "Monthly Winnings"];
/// Players kept on each board's live list.
pub const BOARD_SIZE: usize = 200;
/// Prize split of a board's quarter, in percent, for the top 10 qualifiers.
pub const PRIZE_PCT: [u64; 10] = [30, 20, 13, 9, 7, 6, 5, 4, 3, 3];
/// Hands needed in a week (or a month) to qualify for a winnings board.
pub const MIN_HANDS_WEEK: u32 = 200;
pub const MIN_HANDS_MONTH: u32 = 500;
/// Rows shown per board.
pub const SHOWN: usize = 10;

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

/// (year, month 1..=12, day 1..=31) to days since 1970-01-01, the inverse of
/// `civil_from_days` (Hinnant's `days_from_civil`), for dates from 1970 on.
fn days_from_civil(year: u64, month: u64, day: u64) -> u64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The ISO week `YYYY-Www` of week `week` (its Monday's ISO week).
pub fn week_label(week: u64) -> String {
    // The week's Monday is day 7w - 3, so its Thursday is day 7w. An ISO week
    // belongs to the year of its Thursday and is numbered from that year's first.
    let thursday = week * 7;
    let (year, _, _) = civil_from_days(thursday);
    let n = (thursday - days_from_civil(year, 1, 1)) / 7 + 1;
    format!("{year}-W{n:02}")
}

/// `YYYY-MM` of month `m` (`year * 12 + month0`).
pub fn month_label(m: u64) -> String {
    format!("{}-{:02}", m / 12, m % 12 + 1)
}

fn weekly(board: usize) -> bool {
    board < 2
}

/// The board's period (week or month) containing `now`.
pub fn period_of(board: usize, now: u64) -> u64 {
    if weekly(board) {
        week_of(now)
    } else {
        month_of(now)
    }
}

/// The label of a board's period: `2026-W43` or `2026-11`.
pub fn period_label(board: usize, period: u64) -> String {
    if weekly(board) {
        week_label(period)
    } else {
        month_label(period)
    }
}

/// When the board's period `period` ends (ns).
fn period_end_of(board: usize, period: u64) -> u64 {
    if weekly(board) {
        // The next week's Monday: day 7(w + 1) - 3.
        ((period + 1) * 7 - 3) * DAY_NS
    } else {
        let next = period + 1;
        days_from_civil(next / 12, next % 12 + 1, 1) * DAY_NS
    }
}

/// The prize for the `n`th qualifier (0-based) out of a `pool`.
fn share(pool: u64, n: usize) -> u64 {
    PRIZE_PCT.get(n).map_or(0, |pct| (pool as u128 * *pct as u128 / 100) as u64)
}

/// Orders board rows: higher value first, then whoever reached it first, then
/// by principal so the order never depends on insertion history.
fn row_order(a: &(Principal, i64, u64), b: &(Principal, i64, u64)) -> std::cmp::Ordering {
    b.1.cmp(&a.1).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0))
}

/// When the board's current period ends: the ns time of its next reset.
pub fn period_end(board: usize, now: u64) -> u64 {
    period_end_of(board, period_of(board, now))
}

/// A board's live list for one period, best first: (player, value, when the
/// player reached that value). Holds the top `BOARD_SIZE`.
#[derive(CandidType, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct BoardList {
    pub period: u64,
    pub rows: Vec<(Principal, i64, u64)>,
}

/// A row of a board as shown.
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BoardRow {
    /// Position on the live list (1-based), the paid place for past winners, or
    /// 0 for a player outside the list.
    pub rank: u32,
    pub player: Principal,
    pub username: String,
    pub avatar: String,
    /// Points, or net ICP won (e8s).
    pub value: i64,
    /// Hands played in the period.
    pub hands: u32,
    /// Can be paid: in the period, with a value above zero, and (winnings
    /// boards) enough hands.
    pub qualified: bool,
    /// The prize if the period ended now; for past winners, what was paid.
    pub prize: u64,
}

/// One leaderboard as shown.
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BoardView {
    /// "weekly_points", "weekly_winnings", "monthly_points" or "monthly_winnings".
    pub board: String,
    /// The period: "2026-W43" or "2026-11".
    pub label: String,
    /// When the period ends (ns).
    pub ends_at: u64,
    /// The board's prize pool (e8s).
    pub pool: u64,
    /// The top 10.
    pub rows: Vec<BoardRow>,
    /// The caller's row (rank 0 when outside the list).
    pub me: Option<BoardRow>,
    /// The paid winners of the last closed period.
    pub last_winners: Vec<BoardRow>,
}

/// A player's (value, hands, prize cap) on `board` for `period`, or `None`
/// when their counters belong to another period.
///
/// The cap is the chip-dumping guard of the winnings boards: the house's share
/// of the rake the player paid in the period, and for a month less the weekly
/// winnings prizes already paid to them during it. Points boards have no cap.
fn period_counters(c: &PeriodCounters, board: usize, period: u64) -> Option<(i64, u32, u64)> {
    let points = |x: u64| i64::try_from(x).unwrap_or(i64::MAX);
    match board {
        0 => (c.week == period).then(|| (points(c.w_points), c.w_hands, u64::MAX)),
        1 => (c.week == period).then(|| (c.w_net, c.w_hands, c.w_house_rake.unwrap_or(0))),
        2 => (c.month == period).then(|| (points(c.m_points), c.m_hands, u64::MAX)),
        _ => (c.month == period).then(|| (c.m_net, c.m_hands, c.m_house_rake.unwrap_or(0).saturating_sub(c.weekly_prizes_in(period)))),
    }
}

fn min_hands(board: usize) -> u32 {
    match board {
        1 => MIN_HANDS_WEEK,
        3 => MIN_HANDS_MONTH,
        _ => 0,
    }
}

impl State {
    /// Starts the board lists (at the current periods) the first time they
    /// are needed, including after an upgrade from before the boards existed.
    fn ensure_boards(&mut self, now: u64) {
        let r = self.rewards_mut();
        if r.boards.is_none() {
            r.boards = Some(std::array::from_fn(|b| BoardList { period: period_of(b, now), rows: vec![] }));
        }
        if r.last_winners.is_none() {
            r.last_winners = Some(Default::default());
        }
    }

    /// Closes any week or month that has ended: pays its board's prizes,
    /// awards medals, announces the winners and starts the new period's list.
    /// Called before any player's period counters are written, and every tick.
    ///
    /// Idempotent: a list's period is advanced to the current one before
    /// anything is paid, so a repeated call finds nothing to close. Periods
    /// skipped entirely (no tick and no play) had no list activity, since any
    /// activity would have closed the previous period first.
    pub(crate) fn close_periods(&mut self, now: u64) {
        self.ensure_boards(now);
        // Monthly boards close first. When a week and a month end at the same
        // tick (a month that starts on a Monday), paying the week records its
        // prize under the new month, replacing the old month's weekly-prize
        // total, which the old month's winnings cap must read first.
        for b in [2, 3, 0, 1] {
            let current = period_of(b, now);
            let lists = self.rewards_mut().boards.as_mut().expect("boards started");
            if lists[b].period >= current {
                continue;
            }
            let closed = std::mem::replace(&mut lists[b], BoardList { period: current, rows: vec![] });
            self.pay_board(b, &closed, now);
        }
    }

    /// Pays a closed period's top 10 qualifiers from the board's quarter,
    /// awards medals to the top 3, announces them and keeps them as the
    /// board's last winners. Whatever is not paid stays in the quarter.
    fn pay_board(&mut self, b: usize, closed: &BoardList, now: u64) {
        let pool = self.rewards.as_ref().map_or(0, |r| r.board_pools[b]);
        let label = period_label(b, closed.period);
        let winners: Vec<BoardRow> = self
            .standings(b, closed, pool, now)
            .into_iter()
            .filter(|r| r.qualified)
            .take(PRIZE_PCT.len())
            .collect();
        let mut paid_rows = Vec::with_capacity(winners.len());
        for (i, mut w) in winners.into_iter().enumerate() {
            w.rank = i as u32 + 1;
            if w.prize > 0 && self.credit(&w.player, w.prize).is_ok() {
                self.rewards_mut().board_pools[b] -= w.prize;
                if b == 1 {
                    // A weekly winnings prize comes off the cap of the month it is paid in.
                    if let Some(mut u) = self.users.get_mut(&w.player) {
                        u.rewards.get_or_insert_with(Default::default).period.add_weekly_prize(month_of(now), w.prize);
                    }
                }
            } else {
                w.prize = 0;
            }
            if w.rank <= 3 {
                self.award(&w.player, &format!("lb:{}:{}:{label}", BOARD_IDS[b], w.rank), None, now);
            }
            paid_rows.push(w);
        }
        if !paid_rows.is_empty() {
            let medals = ["🥇", "🥈", "🥉"];
            let mut parts: Vec<String> = paid_rows
                .iter()
                .take(3)
                .map(|w| format!("{} {} ({})", medals[w.rank as usize - 1], w.username, fmt_amount(Currency::Icp, w.prize)))
                .collect();
            if paid_rows.len() > 3 {
                parts.push(format!("and {} more", paid_rows.len() - 3));
            }
            let text = format!("🏆 {} · {label} winners: {}", BOARD_NAMES[b], parts.join(", "));
            self.post(Channel::Lobby, None, String::new(), text, MessageKind::System, now);
        }
        self.rewards_mut().last_winners.as_mut().expect("boards started")[b] = paid_rows;
    }

    /// Every row of a board's list as shown: its place on the list, whether it
    /// qualifies (a suspended player never does) and the prize it would get
    /// from `pool` (in qualifier order).
    fn standings(&self, b: usize, list: &BoardList, pool: u64, now: u64) -> Vec<BoardRow> {
        let mut qualifiers = 0;
        list.rows
            .iter()
            .enumerate()
            .map(|(i, &(player, value, _))| {
                let u = self.users.get(&player);
                let c = u.as_ref().and_then(|u| u.rewards.as_ref()).and_then(|a| period_counters(&a.period, b, list.period));
                let qualified = value > 0 && c.is_some_and(|c| c.1 >= min_hands(b)) && !self.is_banned(&player, now);
                let prize = if qualified {
                    let s = share(pool, qualifiers);
                    qualifiers += 1;
                    // The chip-dumping guard (see `period_counters`).
                    s.min(c.map_or(0, |c| c.2))
                } else {
                    0
                };
                BoardRow {
                    rank: i as u32 + 1,
                    player,
                    username: u.as_ref().map(|u| u.username.clone()).unwrap_or_default(),
                    avatar: u.as_ref().map(|u| self.icon(u.avatar)).unwrap_or_default(),
                    value,
                    hands: c.map_or(0, |c| c.1),
                    qualified,
                    prize,
                }
            })
            .collect()
    }

    /// Moves a player to their current value's place on two boards (a points
    /// or a winnings pair). The time a value is reached is kept until it
    /// changes, so ties go to whoever got there first.
    fn rerank(&mut self, boards: [usize; 2], p: &Principal, now: u64) {
        self.ensure_boards(now);
        let counters = self.users.get(p).and_then(|u| u.rewards).map(|a| a.period);
        let Some(counters) = counters else { return };
        for b in boards {
            let period = period_of(b, now);
            let value = period_counters(&counters, b, period).map_or(0, |c| c.0);
            let list = &mut self.rewards_mut().boards.as_mut().expect("boards started")[b];
            // A list of an earlier period is left for `close_periods`.
            if list.period != period {
                continue;
            }
            if let Some(i) = list.rows.iter().position(|r| r.0 == *p) {
                if list.rows[i].1 == value {
                    continue;
                }
                list.rows.remove(i);
            }
            if value <= 0 {
                continue;
            }
            let row = (*p, value, now);
            let at = list.rows.partition_point(|r| row_order(r, &row) == std::cmp::Ordering::Less);
            if at < BOARD_SIZE {
                list.rows.insert(at, row);
                list.rows.truncate(BOARD_SIZE);
            }
        }
    }

    /// A player's period points changed: re-rank them on the points boards.
    pub(crate) fn on_points(&mut self, p: &Principal, now: u64) {
        self.rerank([0, 2], p, now);
    }

    /// A player's period hands or net changed: re-rank them on the winnings boards.
    pub(crate) fn on_net(&mut self, p: &Principal, now: u64) {
        self.rerank([1, 3], p, now);
    }

    /// The four boards, with `caller`'s own row: their place on the list, or
    /// rank 0 with their period counters when outside it.
    pub fn leaderboards(&self, caller: Option<Principal>, now: u64) -> Vec<BoardView> {
        let r = self.rewards.as_ref();
        (0..BOARDS)
            .map(|b| {
                let fresh = BoardList { period: period_of(b, now), rows: vec![] };
                let list = r.and_then(|r| r.boards.as_ref()).map_or(&fresh, |l| &l[b]);
                let pool = r.map_or(0, |r| r.board_pools[b]);
                let mut rows = self.standings(b, list, pool, now);
                let me = caller.and_then(|c| {
                    rows.iter().find(|row| row.player == c).cloned().or_else(|| {
                        let u = self.users.get(&c)?;
                        let pc = u.rewards.as_ref().and_then(|a| period_counters(&a.period, b, list.period));
                        Some(BoardRow {
                            rank: 0,
                            player: c,
                            username: u.username.clone(),
                            avatar: self.icon(u.avatar),
                            value: pc.map_or(0, |x| x.0),
                            hands: pc.map_or(0, |x| x.1),
                            qualified: false,
                            prize: 0,
                        })
                    })
                });
                rows.truncate(SHOWN);
                BoardView {
                    board: BOARD_IDS[b].to_string(),
                    label: period_label(b, list.period),
                    ends_at: period_end_of(b, list.period),
                    pool,
                    rows,
                    me,
                    last_winners: r.and_then(|r| r.last_winners.as_ref()).map(|w| w[b].clone()).unwrap_or_default(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewards::{PeriodCounters, E8S_PER_POINT as PT};
    use crate::state::{Channel, E8S};

    const SEC: u64 = 1_000_000_000;
    /// Monday 2026-06-01 00:00 UTC (ISO week 2026-W23). Four whole weeks of June follow.
    const JUNE: u64 = 1_780_272_000 * SEC;
    const WEEK: u64 = 7 * DAY_NS;

    fn p(n: u8) -> Principal {
        Principal::from_slice(&[n; 10])
    }

    fn setup(n: u8) -> State {
        let mut s = State::new(Principal::management_canister(), 10_000);
        for k in 1..=n {
            s.register(p(k), format!("player{k}"), 0).unwrap();
        }
        s
    }

    /// Writes a player's period counters the way the reward paths do.
    fn set(s: &mut State, who: Principal, now: u64, f: impl FnOnce(&mut PeriodCounters)) {
        s.close_periods(now);
        {
            let mut u = s.users.get_mut(&who).unwrap();
            let a = u.rewards.get_or_insert_with(Default::default);
            a.period.roll(now);
            f(&mut a.period);
        }
        s.on_points(&who, now);
        s.on_net(&who, now);
    }

    fn board(s: &State, b: usize, now: u64) -> BoardView {
        s.leaderboards(None, now).into_iter().nth(b).unwrap()
    }

    fn values(v: &BoardView) -> Vec<(Principal, i64)> {
        v.rows.iter().map(|r| (r.player, r.value)).collect()
    }

    fn balance(s: &State, who: Principal) -> u64 {
        s.users.get(&who).unwrap().balance
    }

    fn house_take(s: &State) -> u64 {
        s.treasury + s.house_stats().rewards_pools
    }

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

    #[test]
    fn week_and_month_boundaries() {
        let monday = 1_792_368_000 * SEC; // 2026-10-19 00:00:00 UTC
        assert_ne!(week_of(monday), week_of(monday - SEC));
        assert_eq!(week_label(week_of(monday)), "2026-W43");
        assert_eq!(week_label(week_of(monday + WEEK - SEC)), "2026-W43");
        assert_eq!(week_label(week_of(JUNE)), "2026-W23");
        assert_eq!(week_label(week_of(0)), "1970-W01");
        // 2026-12-28 (Monday) is in 2026-W53; 2027-01-04 starts 2027-W01;
        // 2024-12-30 (Monday) is already 2025-W01.
        assert_eq!(week_label(week_of(1_798_416_000 * SEC)), "2026-W53");
        assert_eq!(week_label(week_of(1_799_020_800 * SEC)), "2027-W01");
        assert_eq!(week_label(week_of(1_735_516_800 * SEC)), "2025-W01");
        assert_eq!(week_label(week_of(1_736_121_600 * SEC)), "2025-W02");

        let last = 1_793_491_199 * SEC; // 2026-10-31 23:59:59 UTC
        let first = 1_793_491_200 * SEC; // 2026-11-01 00:00:00 UTC
        assert_ne!(month_of(last), month_of(first));
        assert_eq!(month_label(month_of(first)), "2026-11");
        assert_eq!(month_label(month_of(last)), "2026-10");

        // Resets: the next Monday, the next 1st (across a year end too).
        assert_eq!(period_end(0, monday - SEC), monday);
        assert_eq!(period_end(1, monday), monday + WEEK);
        assert_eq!(period_end(2, last), first);
        assert_eq!(period_end(3, first), 1_796_083_200 * SEC); // 2026-12-01
        assert_eq!(period_end(2, 1_796_083_200 * SEC), 1_798_761_600 * SEC); // 2027-01-01
        assert_eq!(period_label(2, month_of(1_798_761_600 * SEC)), "2027-01");
    }

    #[test]
    fn points_board_ranks_and_breaks_ties_by_time() {
        let mut s = setup(4);
        let t = JUNE + DAY_NS;
        s.earn_rake(&p(1), 50 * PT, 0, t);
        s.earn_rake(&p(2), 80 * PT, 0, t + SEC);
        s.earn_rake(&p(3), 50 * PT, 0, t + 2 * SEC);
        for b in [0, 2] {
            assert_eq!(values(&board(&s, b, t + 3 * SEC)), vec![(p(2), 80), (p(1), 50), (p(3), 50)]);
        }
        // Rake worth less than a point, and hands, change nothing: p1 keeps its place.
        s.earn_rake(&p(1), PT / 2, 0, t + 4 * SEC);
        s.record_icp_hand(&p(1), 0, t + 5 * SEC);
        assert_eq!(values(&board(&s, 0, t + 6 * SEC)), vec![(p(2), 80), (p(1), 50), (p(3), 50)]);
        // p3 passes p1; p1 then ties p3 at 60 later, so stays behind.
        s.earn_rake(&p(3), 10 * PT, 0, t + 7 * SEC);
        assert_eq!(values(&board(&s, 0, t + 8 * SEC)), vec![(p(2), 80), (p(3), 60), (p(1), 50)]);
        s.earn_rake(&p(1), 9 * PT + PT / 2, 0, t + 9 * SEC);
        let v = board(&s, 0, t + 10 * SEC);
        assert_eq!(values(&v), vec![(p(2), 80), (p(3), 60), (p(1), 60)]);
        assert_eq!(v.rows.iter().map(|r| r.rank).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert!(v.rows.iter().all(|r| r.qualified));
        assert_eq!((v.label.as_str(), v.ends_at), ("2026-W23", JUNE + WEEK));
        assert_eq!(v.rows[0].username, "player2");
        // Points boards are not winnings boards.
        assert!(board(&s, 1, t + 10 * SEC).rows.is_empty());

        // The list keeps the top 200.
        for k in 0..210u32 {
            let who = Principal::from_slice(&[7, (k >> 8) as u8, k as u8, 9, 9]);
            s.register(who, format!("bot{k}"), 0).unwrap();
            s.earn_rake(&who, (k as u64 + 1) * PT, 0, t + 11 * SEC);
        }
        let r = s.rewards.as_ref().unwrap().boards.as_ref().unwrap()[0].clone();
        assert_eq!(r.rows.len(), BOARD_SIZE);
        assert_eq!(r.rows[0].1, 210);
        let pos = |who: Principal| r.rows.iter().position(|x| x.0 == who).map(|i| i + 1);
        assert_eq!((pos(p(2)), pos(p(3)), pos(p(1))), (Some(131), Some(152), Some(153)), "ties (p2 vs bot at 80, p3 and p1 vs bot at 60) go to who got there first");
        assert_eq!(r.rows.last().unwrap().1, 14, "the 13 smallest bots dropped, none of p1..p3");
        // My row: on the list, or rank 0 outside it.
        let v = &s.leaderboards(Some(p(1)), t + 12 * SEC)[0];
        assert_eq!(v.me.as_ref().map(|m| (m.rank, m.value)), Some((153, 60)));
        assert_eq!(v.rows.len(), SHOWN);
        let small = Principal::from_slice(&[7, 0, 0, 9, 9]);
        let v = &s.leaderboards(Some(small), t + 12 * SEC)[0];
        assert_eq!(v.me.as_ref().map(|m| (m.rank, m.value)), Some((0, 1)));
        assert!(s.leaderboards(Some(p(99)), t)[0].me.is_none());
    }

    #[test]
    fn winnings_board_needs_hands_profit_and_caps_prize_at_rake() {
        let mut s = setup(4);
        let t = JUNE + DAY_NS;
        s.rewards_mut().board_pools[1] = 10 * E8S;
        // p1: a big winner with 199 hands. p2: a loser. p3: a winner whose rake left the house 0.01 ICP (the cap, not the full 2 ICP of rake).
        set(&mut s, p(1), t, |c| (c.w_hands, c.w_net, c.w_rake, c.w_house_rake) = (199, 5 * E8S as i64, E8S, Some(E8S)));
        set(&mut s, p(2), t, |c| (c.w_hands, c.w_net, c.w_rake, c.w_house_rake) = (500, -(E8S as i64), E8S, Some(E8S)));
        set(&mut s, p(3), t, |c| (c.w_hands, c.w_net, c.w_rake, c.w_house_rake) = (200, E8S as i64, 2 * E8S, Some(E8S / 100)));
        let v = board(&s, 1, t);
        assert_eq!(values(&v), vec![(p(1), 5 * E8S as i64), (p(3), E8S as i64)], "the loser is not listed");
        assert_eq!((v.rows[0].qualified, v.rows[0].hands, v.rows[0].prize), (false, 199, 0));
        assert_eq!((v.rows[1].qualified, v.rows[1].prize), (true, E8S / 100), "first qualifier, capped at the house share of the rake");
        assert_eq!(v.pool, 10 * E8S);

        let before = (balance(&s, p(1)), balance(&s, p(2)), balance(&s, p(3)));
        s.tick(JUNE + WEEK);
        assert_eq!(balance(&s, p(1)), before.0, "199 hands get nothing");
        assert_eq!(balance(&s, p(2)), before.1);
        assert_eq!(balance(&s, p(3)), before.2 + E8S / 100, "at most the house share of the rake paid");
        assert_eq!(s.rewards().board_pools[1], 10 * E8S - E8S / 100, "the rest stays in the quarter");
        let v = board(&s, 1, JUNE + WEEK);
        assert_eq!(v.last_winners.len(), 1);
        assert_eq!((v.last_winners[0].player, v.last_winners[0].rank, v.last_winners[0].prize), (p(3), 1, E8S / 100));
        assert!(v.rows.is_empty());
        assert_eq!(v.label, "2026-W24");
    }

    #[test]
    fn monthly_winnings_needs_five_hundred_hands() {
        let mut s = setup(3);
        let t = JUNE + DAY_NS;
        s.rewards_mut().board_pools[3] = E8S;
        set(&mut s, p(1), t, |c| (c.m_hands, c.m_net, c.m_house_rake) = (499, 10, Some(E8S)));
        set(&mut s, p(2), t, |c| (c.m_hands, c.m_net, c.m_house_rake) = (500, 5, Some(E8S)));
        let v = board(&s, 3, t);
        assert_eq!(v.rows.iter().map(|r| r.qualified).collect::<Vec<_>>(), vec![false, true]);
        assert_eq!(v.label, "2026-06");
        let before = balance(&s, p(2));
        s.tick(JUNE + 30 * DAY_NS); // 2026-07-01
        assert_eq!(balance(&s, p(2)), before + 30 * E8S / 100);
    }

    #[test]
    fn period_close_pays_top_ten_and_rolls_the_rest() {
        let mut s = setup(3);
        let t = JUNE + DAY_NS;
        let pool = 1_000_000_007;
        s.rewards_mut().board_pools[0] = pool;
        for (k, pts) in [(1, 300), (2, 200), (3, 100)] {
            s.earn_rake(&p(k), pts * 1_000, 0, t);
        }
        let v = board(&s, 0, t);
        let shares = [pool * 30 / 100, pool * 20 / 100, pool * 13 / 100];
        assert_eq!(v.rows.iter().map(|r| r.prize).collect::<Vec<_>>(), shares);
        let before: Vec<u64> = (1..=3).map(|k| balance(&s, p(k))).collect();
        let (take, wallets) = (house_take(&s), s.wallet_total);
        s.tick(JUNE + WEEK);
        for k in 0..3 {
            assert_eq!(balance(&s, p(k as u8 + 1)), before[k] + shares[k]);
        }
        let paid: u64 = shares.iter().sum();
        assert_eq!(s.rewards().board_pools[0], pool - paid, "37% stays");
        // Conservation: the board pool's loss is exactly the wallets' gain.
        assert_eq!(house_take(&s), take - paid);
        assert_eq!(s.wallet_total, wallets + paid);
        // The monthly board is still open and untouched.
        assert_eq!(board(&s, 2, JUNE + WEEK).rows.len(), 3);
    }

    #[test]
    fn period_close_is_idempotent() {
        let mut s = setup(2);
        let t = JUNE + DAY_NS;
        s.rewards_mut().board_pools[0] = E8S;
        s.earn_rake(&p(1), 100_000, 0, t);
        let before = balance(&s, p(1));
        s.tick(JUNE + WEEK);
        s.tick(JUNE + WEEK);
        assert_eq!(balance(&s, p(1)), before + 30 * E8S / 100);
        // A tick three weeks later (missed ticks) pays nothing more.
        s.tick(JUNE + 4 * WEEK);
        s.tick(JUNE + 4 * WEEK);
        assert_eq!(balance(&s, p(1)), before + 30 * E8S / 100);
        assert_eq!(s.rewards().board_pools[0], E8S - 30 * E8S / 100);
        let v = board(&s, 0, JUNE + 4 * WEEK);
        assert_eq!(v.label, "2026-W27");
        assert!(v.last_winners.is_empty(), "the weeks closed late were empty");
        // A clock that runs backwards closes nothing.
        s.tick(JUNE);
        assert_eq!(board(&s, 0, JUNE + 4 * WEEK).label, "2026-W27");
    }

    #[test]
    fn period_close_pays_nothing_on_an_empty_board_and_survives_a_snapshot() {
        let mut s = setup(2);
        s.rewards_mut().board_pools = [E8S; 4];
        s.tick(JUNE);
        s.earn_rake(&p(1), 100_000, 0, JUNE + SEC);
        s.tick(JUNE + WEEK);
        let bytes = candid::encode_one(s.rewards.clone()).unwrap();
        let back: Option<crate::rewards::RewardsState> = candid::decode_one(&bytes).unwrap();
        let (a, b) = (back.unwrap(), s.rewards.clone().unwrap());
        assert_eq!(a.boards, b.boards);
        assert_eq!(a.last_winners, b.last_winners);
        assert_eq!(a.boards.as_ref().unwrap()[2].rows.len(), 1);
        assert_eq!(b.board_pools[1], E8S, "an empty winnings board pays nothing");
        let lobby: Vec<String> = s.chat_since(Channel::Lobby, 0).into_iter().map(|m| m.text).collect();
        assert_eq!(lobby.iter().filter(|t| t.contains("Weekly Points")).count(), 1, "{lobby:?}");
        assert!(!lobby.iter().any(|t| t.contains("Winnings")), "no line for an empty board");
    }

    #[test]
    fn winners_get_medals_and_a_lobby_announcement() {
        let mut s = setup(3);
        s.rewards_mut().board_pools[0] = E8S;
        s.earn_rake(&p(1), 300_000, 0, JUNE + SEC);
        s.earn_rake(&p(2), 200_000, 0, JUNE + SEC);
        s.tick(JUNE + WEEK);
        s.earn_rake(&p(1), 300_000, 0, JUNE + WEEK + SEC);
        s.tick(JUNE + 2 * WEEK);
        let m = s.users.get(&p(1)).unwrap().medals.clone();
        let lb: Vec<&String> = m.keys().filter(|k| k.starts_with("lb:")).collect();
        assert_eq!(lb, vec!["lb:weekly_points:1:2026-W23", "lb:weekly_points:1:2026-W24"]);
        assert!(s.users.get(&p(2)).unwrap().medals.contains_key("lb:weekly_points:2:2026-W23"));
        let def = crate::medals::medal_def("lb:weekly_points:1:2026-W23").unwrap();
        assert_eq!((def.name.as_str(), def.icon.as_str()), ("Weekly Points 🥇 · 2026-W23", "🥇"));
        assert_eq!(crate::medals::medal_def("lb:monthly_winnings:3:2026-06").unwrap().icon, "🥉");
        assert!(crate::medals::medal_def("lb:weekly_points:4:2026-W23").is_none());
        assert!(crate::medals::medal_def("lb:nope:1:2026-W23").is_none());
        assert_eq!(crate::medals::medal_def("whale").unwrap().icon, "🐋");
        let lobby: Vec<String> = s.chat_since(Channel::Lobby, 0).into_iter().map(|m| m.text).collect();
        let lines: Vec<&String> = lobby.iter().filter(|t| t.contains("Weekly Points")).collect();
        assert_eq!(lines.len(), 2, "{lobby:?}");
        assert!(lines[0].contains("2026-W23") && lines[0].contains("player1") && lines[0].contains("0.3 ICP"), "{}", lines[0]);
        assert!(lines[0].contains("player2") && lines[0].contains("0.2 ICP"), "{}", lines[0]);
    }

    /// A chip-dumping ring: `a` loses `hands` hands to `b`, both paying rake,
    /// through the real counter paths. `hosted`: the table is a room `a` owns,
    /// so 20% of the rake goes back to `a` and only 80% is the house's.
    /// Returns the house share of the rake the ring paid.
    fn dump(s: &mut State, a: Principal, b: Principal, hands: u32, hosted: bool, start: u64) -> u64 {
        let rake = 10_000; // per hand, 5,000 attributed to each
        let house = if hosted { 4_000 } else { 5_000 };
        for h in 0..hands as u64 {
            let now = start + h * SEC;
            s.earn_rake(&a, rake / 2, house, now);
            s.earn_rake(&b, rake / 2, house, now);
            s.record_icp_hand(&a, -100_000, now);
            s.record_icp_hand(&b, 100_000, now);
        }
        2 * house * hands as u64
    }

    #[test]
    fn a_chip_dumping_ring_gets_back_less_than_the_house_rake_it_paid() {
        for hosted in [true, false] {
            let mut s = setup(2);
            let (a, b) = (p(1), p(2));
            // Big winnings pools, so only the guard limits the prizes (points boards empty).
            s.rewards_mut().board_pools = [0, 100 * E8S, 0, 100 * E8S];
            let t = JUNE + DAY_NS;
            let house_paid = dump(&mut s, a, b, 500, hosted, t);
            let b_house = house_paid / 2;
            assert_eq!(s.users.get(&b).unwrap().rewards.unwrap().period.w_house_rake, Some(b_house));
            let start = balance(&s, b);
            s.tick(JUNE + WEEK);
            let weekly = balance(&s, b) - start;
            assert_eq!(weekly, b_house, "weekly capped at the house share (hosted: {hosted})");
            s.tick(JUNE + 30 * DAY_NS); // 2026-07-01
            let total = balance(&s, b) - start;
            assert_eq!(total, b_house, "the monthly cap is used up by the weekly prize (hosted: {hosted})");
            assert!(total <= house_paid);
            assert_eq!(balance(&s, a), 0, "the dumper wins nothing");
        }
    }

    #[test]
    fn monthly_winnings_cap_subtracts_weekly_prizes_paid_that_month() {
        let mut s = setup(2);
        let (a, b) = (p(1), p(2));
        // The weekly pool pays B 0.3 x 1,000,000 = 300,000, below B's house rake.
        s.rewards_mut().board_pools = [0, 1_000_000, 0, 100 * E8S];
        let house_paid = dump(&mut s, a, b, 500, true, JUNE + DAY_NS);
        let b_house = house_paid / 2; // 2,000,000
        let start = balance(&s, b);
        s.tick(JUNE + WEEK);
        assert_eq!(balance(&s, b) - start, 300_000);
        let c = s.users.get(&b).unwrap().rewards.unwrap().period;
        assert_eq!(c.m_weekly_prizes, Some((month_of(JUNE), 300_000)));
        // The month's board shows (and later pays) the remaining 1,700,000.
        let v = board(&s, 3, JUNE + WEEK);
        assert_eq!((v.rows[0].player, v.rows[0].prize), (b, b_house - 300_000));
        s.tick(JUNE + 30 * DAY_NS);
        assert_eq!(balance(&s, b) - start, b_house, "weekly + monthly = the house rake B paid");
    }

    #[test]
    fn a_week_and_a_month_closing_together_both_pay() {
        // 2026-06-01 is a Monday: week 2026-W22 and May close at the same tick.
        let may25 = JUNE - WEEK;
        let mut s = setup(2);
        let (a, b) = (p(1), p(2));
        s.rewards_mut().board_pools = [0, 1_000_000, 0, 100 * E8S];
        let house_paid = dump(&mut s, a, b, 500, false, may25 + DAY_NS);
        let b_house = house_paid / 2; // 2,500,000
        let start = balance(&s, b);
        s.tick(JUNE);
        // Paying the week must not wipe May's counters before May is paid.
        let last = |s: &State, b: usize| board(s, b, JUNE).last_winners.iter().map(|w| (w.player, w.prize)).collect::<Vec<_>>();
        assert_eq!(last(&s, 1), vec![(b, 300_000)]);
        assert_eq!(last(&s, 3), vec![(b, b_house)], "May's cap is May's house rake: the weekly prize was paid in June");
        assert_eq!(balance(&s, b) - start, 300_000 + b_house);
        let c = s.users.get(&b).unwrap().rewards.unwrap().period;
        assert_eq!(c.m_weekly_prizes, Some((month_of(JUNE), 300_000)), "counted against June");
    }


    #[test]
    fn suspended_players_are_not_paid_and_their_share_rolls_over() {
        use crate::moderation::Ban;
        let mut s = setup(3);
        let t = JUNE + DAY_NS;
        s.rewards_mut().board_pools[0] = E8S;
        s.earn_rake(&p(1), 300_000, 0, t);
        s.earn_rake(&p(2), 200_000, 0, t);
        s.earn_rake(&p(3), 100_000, 0, t);
        // p1: suspended until after the close. p3: a ban that ended before it.
        s.users.get_mut(&p(1)).unwrap().ban = Some(Ban { until: JUNE + 2 * WEEK, reason: "chip dumping".into() });
        s.users.get_mut(&p(3)).unwrap().ban = Some(Ban { until: JUNE + WEEK - SEC, reason: "spam".into() });
        let v = board(&s, 0, t);
        assert_eq!(v.rows.iter().map(|r| (r.qualified, r.prize)).collect::<Vec<_>>(), vec![(false, 0), (true, 30 * E8S / 100), (false, 0)], "p3 is still banned now");
        let before: Vec<u64> = (1..=3).map(|k| balance(&s, p(k))).collect();
        s.tick(JUNE + WEEK);
        assert_eq!(balance(&s, p(1)), before[0], "a suspended player is not paid");
        assert_eq!(balance(&s, p(2)), before[1] + 30 * E8S / 100, "the next player takes first place");
        assert_eq!(balance(&s, p(3)), before[2] + 20 * E8S / 100);
        assert_eq!(s.rewards().board_pools[0], E8S - 50 * E8S / 100, "the rest stays in the pool");
        let w = &board(&s, 0, JUNE + WEEK).last_winners;
        assert_eq!(w.iter().map(|w| (w.player, w.rank)).collect::<Vec<_>>(), vec![(p(2), 1), (p(3), 2)]);
        assert!(!s.users.get(&p(1)).unwrap().medals.keys().any(|k| k.starts_with("lb:")));
        // A permanent ban counts too.
        s.users.get_mut(&p(2)).unwrap().ban = Some(Ban { until: 0, reason: "x".into() });
        s.earn_rake(&p(2), 100_000, 0, JUNE + WEEK + SEC);
        assert!(!board(&s, 0, JUNE + WEEK + SEC).rows[0].qualified);
    }

    #[test]
    fn a_month_ending_on_a_monday_still_subtracts_its_earlier_weekly_prizes() {
        // June 2026 starts on a Monday, so W22 and May close in the same tick.
        let may4 = JUNE - 4 * WEEK; // Monday of 2026-W19
        let mut s = setup(2);
        let (a, b) = (p(1), p(2));
        s.rewards_mut().board_pools = [0, 1_000_000, 0, 100 * E8S];
        let start = balance(&s, b);
        let early = dump(&mut s, a, b, 500, false, may4 + DAY_NS) / 2; // 2,500,000
        s.tick(may4 + WEEK);
        assert_eq!(balance(&s, b) - start, 300_000, "W19 pays in May");
        let late = dump(&mut s, a, b, 200, false, JUNE - WEEK + DAY_NS) / 2; // 1,000,000
        s.tick(JUNE);
        let last = |s: &State, b: usize| board(s, b, JUNE).last_winners.iter().map(|w| w.prize).collect::<Vec<_>>();
        assert_eq!(last(&s, 1), vec![210_000], "W22 pays 30% of the 700,000 left");
        assert_eq!(last(&s, 3), vec![early + late - 300_000], "May's cap less the W19 prize paid in May");
        assert_eq!(balance(&s, b) - start, 300_000 + 210_000 + early + late - 300_000);
    }
}
