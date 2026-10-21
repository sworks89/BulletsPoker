import type { RewardsView } from './ic';
import { icp } from './format';

// Pure helpers for the Rewards page (Fish Buffet and the leaderboards).

/** The wheel's multipliers ×10, in segment order: the backend's `WHEEL` in src/poker_backend/src/rewards.rs. */
export const WHEEL_X10 = [5n, 10n, 15n, 20n, 50n, 100n];

/** Hands a player must play in the period to qualify for a winnings board. */
export const MIN_HANDS: Record<string, number> = { weekly_winnings: 200, monthly_winnings: 500 };

export const BOARD_NAMES: Record<string, string> = {
  weekly_points: 'Weekly Points',
  weekly_winnings: 'Weekly Winnings',
  monthly_points: 'Monthly Points',
  monthly_winnings: 'Monthly Winnings',
};

const clamp01 = (x: number) => Math.min(1, Math.max(0, x));

/** Progress from the current tier's threshold toward the next tier, 0..1 (1 at Shark). */
export function tierProgress(view: RewardsView): number {
  const next = view.next_tier_at[0];
  if (next === undefined) return 1;
  const from = view.tiers[view.tier]?.from ?? 0n;
  if (next <= from) return 1;
  return clamp01(Number(view.tier_points - from) / Number(next - from));
}

/** How full the spin meter is toward the next banked spin, 0..1. */
export function meterProgress(view: RewardsView): number {
  if (view.step <= 0n) return 0;
  return clamp01(Number(view.meter) / Number(view.step));
}

/** Centre of `segment`, in degrees clockwise from the top, when the wheel is at rest. */
export function segmentCentre(segment: number, segments: number): number {
  return ((segment + 0.5) * 360) / segments;
}

/**
 * The clockwise rotation (degrees) that brings the middle of `segment` under the
 * pointer at the top, after `turns` full turns. Segments are laid out clockwise
 * from the top.
 */
export function wheelAngle(segment: number, segments: number, turns = 5): number {
  return turns * 360 + ((360 - segmentCentre(segment, segments)) % 360);
}

/** A wheel multiplier sent as tenths: 15n → "1.5×". */
export function formatMultiplier(x10: bigint): string {
  return `${Number(x10) / 10}×`;
}

const MEDALS: Record<string, string> = { '1': '🥇', '2': '🥈', '3': '🥉' };

/** A leaderboard medal `lb:<board>:<rank 1-3>:<period>`, named as the backend names it. */
export function leaderboardMedal(id: string): { name: string; icon: string } | null {
  const m = id.match(/^lb:([a-z_]+):(\d+):(.+)$/);
  if (!m) return null;
  const [, board, rank, label] = m;
  const name = BOARD_NAMES[board];
  const icon = MEDALS[rank];
  if (!name || !icon) return null;
  return { name: `${name} ${icon} · ${label}`, icon };
}

/** Time left until a board resets (`endsNs` in ns); "closing…" once the end has passed. */
export function periodCountdown(endsNs: bigint, nowMs = Date.now()): string {
  const ms = Number(endsNs / 1_000_000n) - nowMs;
  if (ms <= 0) return 'closing…';
  const d = Math.floor(ms / 86_400_000);
  const h = Math.floor((ms % 86_400_000) / 3_600_000);
  const m = Math.floor((ms % 3_600_000) / 60_000);
  if (d > 0) return `${d}d ${h}h`;
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

/** Hands still needed to qualify for `board` (0 on points boards or once qualified). */
export function handsNeeded(board: string, hands: number): number {
  return Math.max(0, (MIN_HANDS[board] ?? 0) - hands);
}

export const isWinningsBoard = (board: string): boolean => board in MIN_HANDS;

/** An ICP prize or pool: 4 decimals, or all 8 when 4 would show a non-zero amount as 0. */
export function prizeIcp(e8s: bigint): string {
  const short = icp(e8s, 4);
  return e8s !== 0n && /^-?0$/.test(short) ? icp(e8s, 8) : short;
}

/** The tier last seen, and whose it was. */
export type TierSeen = { who: string; tier: number } | null;

/** Whether `tier` is a rise for `who` over what was last seen. The first look
 *  at an account (after signing in or switching) is never a rise. */
export function tierRose(seen: TierSeen, who: string, tier: number): boolean {
  return seen !== null && seen.who === who && tier > seen.tier;
}
