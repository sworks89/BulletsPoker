import { CANISTER_ORIGIN } from './domains';

/** Where a tapped alert ends up. */
export const PLAY_ORIGIN = 'https://www.bulletspoker.com';

/**
 * Where to send a tapped alert, or null to stay put. Alerts have to open the
 * canister address to pass Internet Identity's check (src/poker_backend/src/alerts.rs),
 * but players are signed in, and have the game installed, on the main site.
 */
export function alertForward(hostname: string, search: string): string | null {
  if (hostname !== new URL(CANISTER_ORIGIN).hostname) return null;
  const id = readTournamentParam(search);
  return id === null ? null : `${PLAY_ORIGIN}/?tournament=${id}`;
}

/** Longest decimal a u64 tournament id can be. */
const MAX_ID_DIGITS = 20;

/** The tournament an alert's link names (`?tournament=7`), or null if it names none. */
export function readTournamentParam(search: string): bigint | null {
  const v = new URLSearchParams(search).get('tournament')?.trim();
  return v && v.length <= MAX_ID_DIGITS && /^\d+$/.test(v) ? BigInt(v) : null;
}
