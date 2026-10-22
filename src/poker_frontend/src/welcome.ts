import type { WelcomeView } from './ic';

const NS_PER_MS = 1_000_000n;
const DAY_MS = 86_400_000;

/** The share of the bonus released so far, from 0 to 1 (0 when there is no bonus). */
export function welcomeProgress(v: WelcomeView): number {
  if (v.amount === 0n) return 0;
  const p = Number((v.released * 10_000n) / v.amount) / 10_000;
  return Math.min(1, Math.max(0, p));
}

/** Days until the bonus expires, rounded up so the last day reads 1; 0 once it has expired. */
export function daysLeft(expiresAtNs: bigint, nowMs: number): number {
  const leftMs = Number(expiresAtNs / NS_PER_MS) - nowMs;
  return leftMs > 0 ? Math.ceil(leftMs / DAY_MS) : 0;
}

export type WelcomeStatus = 'offer' | 'active' | 'done' | 'expired' | 'none';

/** Which face of the bonus to show. */
export function welcomeStatus(v: WelcomeView): WelcomeStatus {
  if (v.eligible) return 'offer';
  if (v.amount > 0n && v.released >= v.amount) return 'done';
  if (v.expired) return 'expired';
  if (v.active) return 'active';
  return 'none';
}

const KEY = 'bp.welcome';

/** The status last announced to this player, so each is announced once. */
export function lastAnnounced(who: string): WelcomeStatus | null {
  try {
    return (JSON.parse(localStorage.getItem(KEY) ?? '{}')[who] as WelcomeStatus | undefined) ?? null;
  } catch {
    return null;
  }
}

export function markAnnounced(who: string, s: WelcomeStatus): void {
  try {
    let all: Record<string, string> = {};
    try {
      all = JSON.parse(localStorage.getItem(KEY) ?? '{}') ?? {};
    } catch {
      /* start afresh */
    }
    all[who] = s;
    localStorage.setItem(KEY, JSON.stringify(all));
  } catch {
    /* storage unavailable: the toast may repeat */
  }
}

/**
 * The toast to show when the status moves from `prev` (last announced here) to
 * `status`, or null. With nothing announced on this device, done/expired are
 * stale news (a new device or cleared storage), so they stay quiet.
 */
export function toastFor(prev: WelcomeStatus | null, status: WelcomeStatus): 'active' | 'done' | 'expired' | null {
  if (prev === status) return null;
  if (status === 'active') return 'active';
  if ((status === 'done' || status === 'expired') && prev !== null) return status;
  return null;
}
