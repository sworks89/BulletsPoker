import { describe, expect, it } from 'vitest';
import type { RewardsView } from './ic';
import {
  formatMultiplier,
  handsNeeded,
  leaderboardMedal,
  meterProgress,
  periodCountdown,
  tierProgress,
  wheelAngle,
} from './rewards';

// The backend's TIERS (src/poker_backend/src/rewards.rs).
const TIERS = [
  ['Plankton', 0n, 800n],
  ['Shrimp', 500n, 1_100n],
  ['Crab', 2_500n, 1_400n],
  ['Octopus', 10_000n, 1_700n],
  ['Dolphin', 40_000n, 2_000n],
  ['Orca', 150_000n, 2_400n],
  ['Shark', 500_000n, 2_800n],
] as const;

function view(tier: number, tier_points: bigint, extra: Partial<RewardsView> = {}): RewardsView {
  return {
    tier,
    tier_name: TIERS[tier][0],
    tier_points,
    next_tier_at: tier < 6 ? [TIERS[tier + 1][1]] : [],
    tiers: TIERS.map(([name, from, rate_bps]) => ({ name, from, rate_bps })),
    step: 100n,
    meter: 0n,
    history: [],
    lifetime_points: tier_points,
    days_since_earned: 0n,
    decay_starts_in_days: [],
    fish_pool: 0n,
    spins_banked: 0,
    ...extra,
  };
}

describe('tierProgress', () => {
  it('is halfway between Shrimp (500) and Crab (2,500) at 1,500', () => {
    expect(tierProgress(view(1, 1_500n))).toBe(0.5);
  });

  it('is 1 at Shark', () => {
    expect(tierProgress(view(6, 600_000n))).toBe(1);
  });

  it('is 0 at the start of a tier', () => {
    expect(tierProgress(view(0, 0n))).toBe(0);
  });
});

describe('meterProgress', () => {
  it('is the share of the spin step filled', () => {
    expect(meterProgress(view(0, 0n, { meter: 25n, step: 100n }))).toBe(0.25);
  });

  it('never exceeds 1', () => {
    expect(meterProgress(view(0, 0n, { meter: 250n, step: 100n }))).toBe(1);
  });
});

describe('wheelAngle', () => {
  it('lands the centre of segment 0 under the top pointer after at least 5 turns', () => {
    const a = wheelAngle(0, 6);
    expect(a).toBeGreaterThanOrEqual(5 * 360);
    // Segment 0's centre sits 30° clockwise from the top before spinning.
    expect((a + 30) % 360).toBe(0);
  });

  it('lands every segment', () => {
    for (let i = 0; i < 6; i++) {
      const centre = (i + 0.5) * 60;
      expect((wheelAngle(i, 6) + centre) % 360, `segment ${i}`).toBe(0);
    }
  });

  it('honours the number of turns', () => {
    expect(wheelAngle(0, 6, 2)).toBeGreaterThanOrEqual(720);
    expect(wheelAngle(0, 6, 2)).toBeLessThan(1080);
  });
});

describe('formatMultiplier', () => {
  it('formats tenths', () => {
    expect(formatMultiplier(15n)).toBe('1.5×');
    expect(formatMultiplier(100n)).toBe('10×');
    expect(formatMultiplier(5n)).toBe('0.5×');
  });
});

describe('leaderboardMedal', () => {
  it('names a leaderboard medal by board, place and period', () => {
    expect(leaderboardMedal('lb:weekly_points:1:2026-W42')).toEqual({ name: 'Weekly Points 🥇 · 2026-W42', icon: '🥇' });
    expect(leaderboardMedal('lb:monthly_winnings:3:2026-06')).toEqual({ name: 'Monthly Winnings 🥉 · 2026-06', icon: '🥉' });
    expect(leaderboardMedal('lb:weekly_winnings:2:2026-W43')?.icon).toBe('🥈');
  });

  it('rejects anything else', () => {
    expect(leaderboardMedal('lb:weekly_points:4:2026-W23')).toBeNull();
    expect(leaderboardMedal('lb:nope:1:2026-W23')).toBeNull();
    expect(leaderboardMedal('lb:weekly_points:1:')).toBeNull();
    expect(leaderboardMedal('first_win')).toBeNull();
  });
});

describe('periodCountdown', () => {
  const now = 1_000_000_000_000; // ms
  const at = (ms: number) => BigInt(now + ms) * 1_000_000n;

  it('shows days and hours for a long period', () => {
    expect(periodCountdown(at((3 * 24 + 4) * 3_600_000 + 60_000), now)).toBe('3d 4h');
  });

  it('shows hours and minutes on the last day', () => {
    expect(periodCountdown(at(5 * 3_600_000 + 12 * 60_000), now)).toBe('5h 12m');
    expect(periodCountdown(at(12 * 60_000 + 5_000), now)).toBe('12m');
  });

  it('says the board is closing once the end has passed', () => {
    expect(periodCountdown(at(0), now)).toBe('closing…');
    expect(periodCountdown(at(-60_000), now)).toBe('closing…');
  });
});

describe('handsNeeded', () => {
  it('counts the hands still needed on winnings boards', () => {
    expect(handsNeeded('weekly_winnings', 150)).toBe(50);
    expect(handsNeeded('monthly_winnings', 100)).toBe(400);
    expect(handsNeeded('weekly_winnings', 250)).toBe(0);
  });

  it('is 0 on points boards', () => {
    expect(handsNeeded('weekly_points', 0)).toBe(0);
  });
});
