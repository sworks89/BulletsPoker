//! Rewards: the house's rake is split between the Fish Buffet pool, the four
//! leaderboard prize pools and the treasury.
//!
//! Board index order everywhere: 0 weekly points, 1 weekly winnings,
//! 2 monthly points, 3 monthly winnings.

use crate::leaderboards::{month_of, week_of, DAY_NS};
use crate::state::State;
use candid::{CandidType, Deserialize, Principal};
use std::collections::VecDeque;

/// Share of the house's rake that funds the Fish Buffet cashback pool (30%).
pub const FISH_POOL_BPS: u64 = 3_000;
/// Share of the house's rake that funds the four leaderboard pools together (10%).
pub const BOARD_POOL_BPS: u64 = 1_000;
/// Number of leaderboards.
pub const BOARDS: usize = 4;

#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct RewardsState {
    /// ICP (e8s) backing the cashback wheel.
    pub fish_pool: u64,
    /// ICP (e8s) prize pool per leaderboard.
    pub board_pools: [u64; BOARDS],
    /// The live list of each leaderboard. Absent in snapshots saved before the boards.
    pub boards: Option<[crate::leaderboards::BoardList; BOARDS]>,
    /// The paid winners of each board's last closed period.
    pub last_winners: Option<[Vec<crate::leaderboards::BoardRow>; BOARDS]>,
}

impl RewardsState {
    pub fn pools_total(&self) -> u64 {
        self.fish_pool + self.board_pools.iter().sum::<u64>()
    }
}

/// Attributed rake (e8s) per point: 0.001 ICP, so a regular at the lowest
/// stakes (0.01/0.02) earns about 60 points an hour.
pub const E8S_PER_POINT: u64 = 100_000;
/// Banked spins wait up to this many; further steps bank nothing.
pub const MAX_SPINS: usize = 50;
/// Days without earning before tier points start to decay.
pub const DECAY_GRACE_DAYS: u64 = 7;
/// Tier points kept per day of decay, out of 100.
pub const DECAY_KEEP_PCT: u128 = 95;

/// A Fish Buffet tier: reached at `from` tier points, a spin every `step`
/// points, and the wheel's expected value as a rate of the spin's stake.
pub struct Tier {
    pub name: &'static str,
    /// Shown in the tier-up chat line.
    pub icon: &'static str,
    pub from: u64,
    pub step: u64,
    pub rate_bps: u64,
}

pub const TIERS: [Tier; 7] = [
    Tier { name: "Plankton", icon: "🦠", from: 0, step: 100, rate_bps: 800 },
    Tier { name: "Shrimp", icon: "🦐", from: 500, step: 400, rate_bps: 1_100 },
    Tier { name: "Crab", icon: "🦀", from: 2_500, step: 1_500, rate_bps: 1_400 },
    Tier { name: "Octopus", icon: "🐙", from: 10_000, step: 6_000, rate_bps: 1_700 },
    Tier { name: "Dolphin", icon: "🐬", from: 40_000, step: 20_000, rate_bps: 2_000 },
    Tier { name: "Orca", icon: "🐋", from: 150_000, step: 70_000, rate_bps: 2_400 },
    Tier { name: "Shark", icon: "🦈", from: 500_000, step: 200_000, rate_bps: 2_800 },
];

/// The tier reached with `points` tier points.
pub fn tier_index(points: u64) -> usize {
    TIERS.iter().rposition(|t| points >= t.from).unwrap_or(0)
}

/// Tier points after decay: unchanged for 7 days after `last_earned_at`, then
/// ×0.95 (rounded down) per whole extra day.
pub fn decayed(points: u64, last_earned_at: u64, now: u64) -> u64 {
    let days = now.saturating_sub(last_earned_at) / DAY_NS;
    let mut p = points as u128;
    for _ in DECAY_GRACE_DAYS..days {
        if p == 0 {
            break;
        }
        p = p * DECAY_KEEP_PCT / 100;
    }
    p as u64
}

/// Splits `rake` over the players in proportion to what each committed,
/// rounding down. The remainder goes to no one.
pub fn attribute(rake: u64, players: &[(Principal, u64)]) -> Vec<(Principal, u64)> {
    let total: u128 = players.iter().map(|(_, c)| *c as u128).sum();
    players
        .iter()
        .map(|(p, c)| (*p, if total == 0 { 0 } else { (rake as u128 * *c as u128 / total) as u64 }))
        .collect()
}

/// A spin waiting to be taken: its stake (house-share e8s) and the tier it was banked at.
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BankedSpin {
    pub stake: u64,
    pub tier: u8,
}

/// One spin of the wheel, as shown in the player's history.
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SpinRecord {
    pub time: u64,
    pub tier: String,
    pub stake: u64,
    pub multiplier_x10: u64,
    /// Index into the wheel's segments (for the animation).
    pub segment: u8,
    /// Before the pool cap.
    pub prize: u64,
    pub paid: u64,
    pub capped: bool,
}

/// A player's leaderboard counters for the current week and month. They reset
/// lazily when `week` or `month` no longer matches the current period.
#[derive(CandidType, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct PeriodCounters {
    pub week: u64,
    pub month: u64,
    pub w_points: u64,
    pub m_points: u64,
    pub w_net: i64,
    pub m_net: i64,
    pub w_hands: u32,
    pub m_hands: u32,
    pub w_rake: u64,
    pub m_rake: u64,
    /// The house's share of the rake paid (after a room host's cut; tournament
    /// fees in full): the cap on a winnings prize. Absent in counters saved before it.
    pub w_house_rake: Option<u64>,
    pub m_house_rake: Option<u64>,
    /// (month, e8s): weekly winnings prizes paid to the player during that
    /// month, which come off that month's winnings cap. Keyed by its own month
    /// so paying a week never has to roll the month's counters.
    pub m_weekly_prizes: Option<(u64, u64)>,
}

impl PeriodCounters {
    /// Starts new counters for any period that has changed since they were written.
    pub fn roll(&mut self, now: u64) {
        let (week, month) = (week_of(now), month_of(now));
        if self.week != week {
            (self.week, self.w_points, self.w_net, self.w_hands, self.w_rake, self.w_house_rake) = (week, 0, 0, 0, 0, None);
        }
        if self.month != month {
            (self.month, self.m_points, self.m_net, self.m_hands, self.m_rake, self.m_house_rake) = (month, 0, 0, 0, 0, None);
        }
    }

    /// Weekly winnings prizes paid to the player during `month`.
    pub fn weekly_prizes_in(&self, month: u64) -> u64 {
        self.m_weekly_prizes.filter(|w| w.0 == month).map_or(0, |w| w.1)
    }

    /// Counts a weekly winnings prize against the month it is paid in.
    pub fn add_weekly_prize(&mut self, month: u64, prize: u64) {
        let total = self.weekly_prizes_in(month).saturating_add(prize);
        self.m_weekly_prizes = Some((month, total));
    }
}

/// A player's Fish Buffet account.
#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct RewardsAccount {
    /// Decaying points that decide the tier (decay is applied when points are earned).
    pub tier_points: u64,
    /// Points ever earned; never decay.
    pub lifetime_points: u64,
    /// When points were last earned (and decay last applied).
    pub last_earned_at: u64,
    /// Attributed rake (e8s) not yet worth a whole point.
    pub carry_e8s: u64,
    /// Points toward the next spin.
    pub meter: u64,
    /// House-share e8s gathered toward the next spin.
    pub meter_stake: u64,
    pub spins: Vec<BankedSpin>,
    pub history: VecDeque<SpinRecord>,
    pub period: PeriodCounters,
    /// Spins taken so far (mixed into each draw). Absent in accounts saved before the wheel.
    pub spins_taken: Option<u64>,
}

/// Wheel segments as (multiplier ×10, weight). The expected multiplier is exactly 1×.
pub const WHEEL: [(u64, u64); 6] = [(5, 60), (10, 24), (15, 6), (20, 6), (50, 3), (100, 1)];
/// Spin results kept in a player's history.
pub const HISTORY_MAX: usize = 20;

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TierInfo {
    pub name: String,
    pub from: u64,
    pub rate_bps: u64,
}

/// What the rewards screen shows for a player.
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RewardsView {
    pub tier: u8,
    pub tier_name: String,
    /// After decay.
    pub tier_points: u64,
    pub next_tier_at: Option<u64>,
    pub step: u64,
    pub meter: u64,
    pub spins_banked: u32,
    pub lifetime_points: u64,
    pub days_since_earned: u64,
    /// Days of grace left before decay starts (0 = decaying now); none without points.
    pub decay_starts_in_days: Option<u64>,
    pub fish_pool: u64,
    pub history: Vec<SpinRecord>,
    pub tiers: Vec<TierInfo>,
}

impl State {
    /// Spins the oldest banked spin: draws a segment from secret entropy, pays
    /// stake x tier rate x multiplier out of the Fish pool (capped by what is in it).
    pub fn spin_wheel(&mut self, caller: Principal, now: u64) -> crate::state::R<SpinRecord> {
        use sha2::{Digest, Sha256};
        let entropy = self.entropy().to_vec();
        let (spin, lifetime, taken) = {
            let u = self.users.get(&caller).ok_or_else(|| "Please sign in first".to_string())?;
            let a = u.rewards.unwrap_or_default();
            if a.spins.is_empty() {
                return Err("No spins yet".into());
            }
            (a.spins[0].clone(), a.lifetime_points, a.spins_taken.unwrap_or(0))
        };
        let mut h = Sha256::new();
        h.update(&entropy);
        h.update(caller.as_slice());
        h.update(lifetime.to_le_bytes());
        h.update(taken.to_le_bytes());
        let d = h.finalize();
        let total: u64 = WHEEL.iter().map(|w| w.1).sum();
        let mut roll = u128::from_le_bytes(d[..16].try_into().unwrap()) % total as u128;
        let mut segment = 0;
        for (i, w) in WHEEL.iter().enumerate() {
            if roll < w.1 as u128 {
                segment = i;
                break;
            }
            roll -= w.1 as u128;
        }
        let mult = WHEEL[segment].0;
        let tier = &TIERS[(spin.tier as usize).min(TIERS.len() - 1)];
        let prize = (spin.stake as u128 * tier.rate_bps as u128 / 10_000 * mult as u128 / 10).min(u64::MAX as u128) as u64;
        let paid = prize.min(self.rewards.as_ref().map_or(0, |r| r.fish_pool));
        if paid > 0 {
            self.credit(&caller, paid)?;
            self.rewards_mut().fish_pool -= paid;
        }
        let rec = SpinRecord {
            time: now,
            tier: tier.name.to_string(),
            stake: spin.stake,
            multiplier_x10: mult,
            segment: segment as u8,
            prize,
            paid,
            capped: paid < prize,
        };
        let mut u = self.users.get_mut(&caller).ok_or_else(|| "Please sign in first".to_string())?;
        let a = u.rewards.get_or_insert_with(RewardsAccount::default);
        a.spins.remove(0);
        a.spins_taken = Some(taken + 1);
        a.history.push_back(rec.clone());
        while a.history.len() > HISTORY_MAX {
            a.history.pop_front();
        }
        Ok(rec)
    }

    pub fn rewards_view(&self, p: &Principal, now: u64) -> RewardsView {
        let a = self.users.get(p).and_then(|u| u.rewards).unwrap_or_default();
        let points = decayed(a.tier_points, a.last_earned_at, now);
        let tier = tier_index(points);
        let days = if a.last_earned_at == 0 && a.tier_points == 0 { 0 } else { now.saturating_sub(a.last_earned_at) / DAY_NS };
        RewardsView {
            tier: tier as u8,
            tier_name: TIERS[tier].name.to_string(),
            tier_points: points,
            next_tier_at: TIERS.get(tier + 1).map(|t| t.from),
            step: TIERS[tier].step,
            meter: a.meter,
            spins_banked: a.spins.len() as u32,
            lifetime_points: a.lifetime_points,
            days_since_earned: days,
            decay_starts_in_days: (points > 0).then(|| DECAY_GRACE_DAYS.saturating_sub(days)),
            fish_pool: self.rewards.as_ref().map_or(0, |r| r.fish_pool),
            history: a.history.iter().rev().cloned().collect(),
            tiers: TIERS.iter().map(|t| TierInfo { name: t.name.to_string(), from: t.from, rate_bps: t.rate_bps }).collect(),
        }
    }
}

impl State {
    /// A copy of the reward pools (tests only; code reads `self.rewards.as_ref()`).
    #[cfg(test)]
    pub(crate) fn rewards(&self) -> RewardsState {
        self.rewards.clone().unwrap_or_default()
    }

    pub(crate) fn rewards_mut(&mut self) -> &mut RewardsState {
        self.rewards.get_or_insert_with(RewardsState::default)
    }

    /// Splits the house's share of the rake: 30% to the Fish pool, 2.5% to
    /// each leaderboard pool, the rest (including rounding) to the treasury.
    pub(crate) fn fund_rewards(&mut self, house_rake: u64) {
        let fish = house_rake * FISH_POOL_BPS / 10_000;
        let board = house_rake * (BOARD_POOL_BPS / BOARDS as u64) / 10_000;
        let r = self.rewards_mut();
        r.fish_pool += fish;
        for p in r.board_pools.iter_mut() {
            *p += board;
        }
        self.treasury += house_rake - fish - board * BOARDS as u64;
    }

    /// Credits a player with rake they paid: `rake_e8s` earns points (1 per
    /// 1,000 e8s, leftovers carried), `house_e8s` is the house's share of it
    /// and stakes the spins the points bank. Unregistered players are ignored.
    ///
    /// Returns the tier reached when it rose (after decay on both sides), for
    /// the caller to announce where the player sits.
    pub(crate) fn earn_rake(&mut self, p: &Principal, rake_e8s: u64, house_e8s: u64, now: u64) -> Option<usize> {
        if rake_e8s == 0 && house_e8s == 0 {
            return None;
        }
        self.close_periods(now);
        let risen;
        {
            let mut u = self.users.get_mut(p)?;
            let a = u.rewards.get_or_insert_with(RewardsAccount::default);
            let before = tier_index(decayed(a.tier_points, a.last_earned_at, now));
            a.period.roll(now);
            a.carry_e8s += rake_e8s;
            let points = a.carry_e8s / E8S_PER_POINT;
            a.carry_e8s %= E8S_PER_POINT;
            if points > 0 {
                // Decay is applied (and its clock restarted) only when whole
                // points are earned, so no stretch of idle days decays twice.
                a.tier_points = decayed(a.tier_points, a.last_earned_at, now).saturating_add(points);
                a.last_earned_at = now;
            }
            a.lifetime_points = a.lifetime_points.saturating_add(points);
            a.period.w_points += points;
            a.period.m_points += points;
            a.period.w_rake += rake_e8s;
            a.period.m_rake += rake_e8s;
            a.period.w_house_rake = Some(a.period.w_house_rake.unwrap_or(0).saturating_add(house_e8s));
            a.period.m_house_rake = Some(a.period.m_house_rake.unwrap_or(0).saturating_add(house_e8s));

            // The meter banks a spin for each step crossed at the tier reached now.
            a.meter = a.meter.saturating_add(points);
            a.meter_stake = a.meter_stake.saturating_add(house_e8s);
            let tier = tier_index(a.tier_points);
            let step = TIERS[tier].step;
            let crossed = a.meter / step;
            if crossed > 0 {
                let total = a.meter;
                a.meter %= step;
                // The points left on the meter keep their part of the stake;
                // the spins share the rest evenly.
                let keep = (a.meter_stake as u128 * a.meter as u128 / total as u128) as u64;
                let rest = a.meter_stake - keep;
                let stake = rest / crossed;
                // The division's remainder stays toward the next spin.
                a.meter_stake = keep + rest % crossed;
                let room = MAX_SPINS.saturating_sub(a.spins.len()) as u64;
                for _ in 0..crossed.min(room) {
                    a.spins.push(BankedSpin { stake, tier: tier as u8 });
                }
            }
            let after = tier_index(decayed(a.tier_points, a.last_earned_at, now));
            risen = (after > before).then_some(after);
        }
        self.on_points(p, now);
        self.referral_cut(p, house_e8s, now);
        self.release_welcome(p, house_e8s, now);
        risen
    }

    /// Posts "🐬 Name reached Dolphin in the Fish Buffet!" in `ch`.
    pub(crate) fn announce_tier_up(&mut self, p: &Principal, tier: usize, ch: crate::state::Channel, now: u64) {
        let Some(name) = self.users.get(p).map(|u| u.username.clone()) else { return };
        let t = &TIERS[tier.min(TIERS.len() - 1)];
        let text = format!("{} {name} reached {} in the Fish Buffet!", t.icon, t.name);
        self.post(ch, None, String::new(), text, crate::state::MessageKind::System, now);
    }

    /// Counts a hand toward the player's period counters, with its net ICP
    /// result (0 for tournament hands). Unregistered players are ignored.
    pub(crate) fn record_icp_hand(&mut self, p: &Principal, net: i64, now: u64) {
        self.close_periods(now);
        {
            let Some(mut u) = self.users.get_mut(p) else { return };
            let a = u.rewards.get_or_insert_with(RewardsAccount::default);
            a.period.roll(now);
            a.period.w_hands += 1;
            a.period.m_hands += 1;
            a.period.w_net += net;
            a.period.m_net += net;
        }
        self.on_net(p, now);
    }

    /// Adds a net ICP result that is not a hand (an ICP tournament's prize
    /// minus its costs) to the player's period counters.
    pub(crate) fn record_icp_net(&mut self, p: &Principal, net: i64, now: u64) {
        self.close_periods(now);
        {
            let Some(mut u) = self.users.get_mut(p) else { return };
            let a = u.rewards.get_or_insert_with(RewardsAccount::default);
            a.period.roll(now);
            a.period.w_net += net;
            a.period.m_net += net;
        }
        self.on_net(p, now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leaderboards::DAY_NS as DAY;
    use candid::Principal;

    fn p(n: u8) -> Principal {
        Principal::from_slice(&[n; 10])
    }

    #[test]
    fn tier_thresholds() {
        assert_eq!(tier_index(0), 0);
        assert_eq!(tier_index(499), 0);
        assert_eq!(tier_index(500), 1);
        assert_eq!(tier_index(2_500), 2);
        assert_eq!(tier_index(499_999), 5);
        assert_eq!(tier_index(500_000), 6);
        assert_eq!(tier_index(u64::MAX), 6);
        assert_eq!(TIERS[0].name, "Plankton");
        assert_eq!(TIERS[6].name, "Shark");
    }

    #[test]
    fn decay_waits_seven_days_then_drops_five_percent_a_day() {
        assert_eq!(decayed(10_000, 0, 7 * DAY), 10_000);
        assert_eq!(decayed(10_000, 0, 8 * DAY - 1), 10_000);
        assert_eq!(decayed(10_000, 0, 8 * DAY), 9_500);
        assert_eq!(decayed(10_000, 0, 9 * DAY), 9_025);
        assert_eq!(decayed(10_000, 0, 400 * DAY), 0);
        assert_eq!(decayed(u64::MAX, 0, 8 * DAY), (u64::MAX as u128 * 95 / 100) as u64);
        assert_eq!(decayed(u64::MAX, 0, 100_000 * DAY), 0);
        // A clock that runs backwards never decays.
        assert_eq!(decayed(10_000, 9 * DAY, 0), 10_000);
    }

    #[test]
    fn attribute_splits_by_contribution() {
        let (a, b, c) = (p(1), p(2), p(3));
        assert_eq!(attribute(100, &[(a, 300), (b, 100)]), vec![(a, 75), (b, 25)]);
        assert_eq!(attribute(10, &[(a, 1), (b, 1), (c, 1)]), vec![(a, 3), (b, 3), (c, 3)]);
        assert_eq!(attribute(10, &[(a, 0), (b, 0)]), vec![(a, 0), (b, 0)]);
        // Big pots don't overflow.
        let big = 100_000_000_000_000u64;
        assert_eq!(attribute(big, &[(a, big), (b, big)]), vec![(a, big / 2), (b, big / 2)]);
    }
}
