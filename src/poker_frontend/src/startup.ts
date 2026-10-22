/** Where the app is in connecting to the canisters. */
export type Startup = 'connecting' | 'failed' | 'ready';

/**
 * Where the signed-in player's profile lookup stands. `none` means a lookup
 * succeeded and found no profile; a failed lookup is `failed`, never `none`.
 */
export type ProfileLoad = 'unknown' | 'loading' | 'none' | 'found' | 'failed';

const BASE_MS = 1000;
const MAX_MS = 30_000;

/** Delay before retry number `attempt` (0 = first retry): 1s, 2s, 4s, … capped at 30s. */
export function nextRetryMs(attempt: number): number {
  if (!Number.isFinite(attempt) || attempt < 0) return BASE_MS;
  // Past 2^5 the cap applies; avoid computing huge powers.
  if (attempt >= 5) return MAX_MS;
  return Math.min(MAX_MS, BASE_MS * 2 ** Math.floor(attempt));
}

/** The registration prompt is only for a successful lookup that found no profile. */
export function shouldOfferRegistration(load: ProfileLoad): boolean {
  return load === 'none';
}
