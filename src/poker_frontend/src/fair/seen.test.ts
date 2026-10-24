import { describe, expect, it } from 'vitest';
import { SEEN_KEY, SEEN_MAX, recordSeen, seenCommitment, seenKey } from './seen';

class MemStore {
  data = new Map<string, string>();
  getItem(k: string) {
    return this.data.get(k) ?? null;
  }
  setItem(k: string, v: string) {
    this.data.set(k, v);
  }
}

const C = 'ryjl3-tyaaa-aaaaa-aaaba-cai';
const view = (id: bigint, hand_no: bigint, c?: number[]) => ({ id, hand_no, commitment: (c ? [Uint8Array.from(c)] : []) as [] | [Uint8Array] });

describe('commitments seen at the table', () => {
  it('stores the commitment shown for a hand and reads it back by canister, table and hand', () => {
    const s = new MemStore();
    recordSeen(s, C, [view(7n, 3n, [1, 2, 255]), view(8n, 1n)]);
    expect(seenCommitment(s, C, 7n, 3n)).toEqual(Uint8Array.from([1, 2, 255]));
    expect(seenCommitment(s, C, 7n, 4n)).toBeUndefined();
    expect(seenCommitment(s, C, 8n, 1n)).toBeUndefined(); // no commitment in that view
    expect(seenCommitment(s, 'aaaaa-aa', 7n, 3n)).toBeUndefined();
    expect(seenKey(C, 7n, 3n)).toBe(`${C}:7:3`);
  });

  it('keeps the first value seen, so a later different one shows up as a mismatch', () => {
    const s = new MemStore();
    recordSeen(s, C, [view(7n, 3n, [1])]);
    recordSeen(s, C, [view(7n, 3n, [2])]);
    expect(seenCommitment(s, C, 7n, 3n)).toEqual(Uint8Array.from([1]));
  });

  it(`keeps only the last ${SEEN_MAX} hands`, () => {
    const s = new MemStore();
    for (let h = 1; h <= SEEN_MAX + 20; h++) recordSeen(s, C, [view(1n, BigInt(h), [h & 255])]);
    expect(JSON.parse(s.getItem(SEEN_KEY)!).length).toBe(SEEN_MAX);
    expect(seenCommitment(s, C, 1n, 20n)).toBeUndefined();
    expect(seenCommitment(s, C, 1n, 21n)).toEqual(Uint8Array.from([21]));
    expect(seenCommitment(s, C, 1n, BigInt(SEEN_MAX + 20))).toBeDefined();
  });

  it('never throws on broken or blocked storage', () => {
    const broken = { getItem: () => '{not json', setItem: () => undefined };
    expect(seenCommitment(broken, C, 1n, 1n)).toBeUndefined();
    expect(() => recordSeen(broken, C, [view(1n, 1n, [1])])).not.toThrow();
    const blocked = {
      getItem: () => {
        throw new Error('denied');
      },
      setItem: () => {
        throw new Error('denied');
      },
    };
    expect(() => recordSeen(blocked, C, [view(1n, 1n, [1])])).not.toThrow();
    expect(seenCommitment(blocked, C, 1n, 1n)).toBeUndefined();
    expect(seenCommitment(null, C, 1n, 1n)).toBeUndefined();
  });
});
