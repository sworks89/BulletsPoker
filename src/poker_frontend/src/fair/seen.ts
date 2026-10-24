// Commitments this device saw at the table while a hand was played.
//
// The table view shows the commitment of the hand being played. Keeping a
// copy here lets the verifier later check that the published commitment is
// the one shown during the hand (before the flop and before any betting),
// instead of relying only on the canister's own log order.
//
// Stored in localStorage under SEEN_KEY as a JSON array of [key, hex] pairs,
// oldest first, at most SEEN_MAX entries. The first value seen for a hand is
// kept, so a later different one shows up as a mismatch. Never throws.

export const SEEN_KEY = 'bp.fairSeen';
export const SEEN_MAX = 500;

export type StoreLike = { getItem(k: string): string | null; setItem(k: string, v: string): void };
type Bytes = Uint8Array | number[];
export interface ViewLike {
  id: bigint;
  hand_no: bigint;
  commitment: [] | [Bytes];
}

const hex = (b: Bytes) => Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');

export const seenKey = (canister: string, tableId: bigint, handNo: bigint) => `${canister}:${tableId}:${handNo}`;

/** The browser's localStorage, or null where it is unavailable. */
export function defaultStore(): StoreLike | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

function load(store: StoreLike): [string, string][] {
  try {
    const raw = JSON.parse(store.getItem(SEEN_KEY) ?? '[]');
    if (!Array.isArray(raw)) return [];
    return raw.filter((e): e is [string, string] => Array.isArray(e) && typeof e[0] === 'string' && typeof e[1] === 'string' && /^[0-9a-f]*$/.test(e[1]));
  } catch {
    return [];
  }
}

/** Remembers the commitment each view shows for its current hand. */
export function recordSeen(store: StoreLike | null, canister: string, views: readonly ViewLike[]): void {
  if (!store || !canister) return;
  try {
    const entries = load(store);
    const known = new Set(entries.map((e) => e[0]));
    let changed = false;
    for (const v of views) {
      const c = v.commitment[0];
      if (!c || c.length === 0) continue;
      const key = seenKey(canister, v.id, v.hand_no);
      if (known.has(key)) continue;
      entries.push([key, hex(c)]);
      known.add(key);
      changed = true;
    }
    if (changed) store.setItem(SEEN_KEY, JSON.stringify(entries.slice(-SEEN_MAX)));
  } catch {
    // storage full or blocked: the verifier then reports the commitment as not seen
  }
}

/** The commitment this device saw at the table for that hand, if any. */
export function seenCommitment(store: StoreLike | null, canister: string, tableId: bigint, handNo: bigint): Uint8Array | undefined {
  if (!store) return undefined;
  const key = seenKey(canister, tableId, handNo);
  const hit = load(store).find((e) => e[0] === key);
  if (!hit || hit[1].length % 2 !== 0) return undefined;
  return Uint8Array.from(hit[1].match(/../g) ?? [], (x) => parseInt(x, 16));
}
