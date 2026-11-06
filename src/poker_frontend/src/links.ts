/** Longest decimal a u64 tournament id can be. */
const MAX_ID_DIGITS = 20;

/** The tournament an alert's link names (`?tournament=7`), or null if it names none. */
export function readTournamentParam(search: string): bigint | null {
  const v = new URLSearchParams(search).get('tournament')?.trim();
  return v && v.length <= MAX_ID_DIGITS && /^\d+$/.test(v) ? BigInt(v) : null;
}
