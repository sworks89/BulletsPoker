import { describe, expect, it } from 'vitest';
import vectors from './vectors.json';
import {
  UnsupportedVersion,
  commitment,
  handIdBytes,
  lp,
  paramsBytes,
  shuffleDeck,
  shuffleKey,
  uniformFromWords,
} from './protocol';

const hex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
const fromHex = (h: string) => new Uint8Array((h.match(/../g) ?? []).map((x) => parseInt(x, 16)));

describe('BP-FAIR-v1 vectors', () => {
  expect(vectors.protocol).toBe('BP-FAIR-v1');
  vectors.cases.forEach((c, i) => {
    it(`case ${i} (${c.canister_text}, table ${c.table_id}, hand ${c.hand_no})`, async () => {
      const seed = fromHex(c.seed_hex);
      const tableId = BigInt(c.table_id);
      const handNo = BigInt(c.hand_no);
      const handId = handIdBytes(c.canister_text, tableId, handNo);
      expect(hex(handId)).toBe(c.hand_id_hex);
      expect(hex(paramsBytes(c.version))).toBe(c.params_hex);
      expect(hex(await shuffleKey(seed, handId, c.version))).toBe(c.key_hex);
      expect(hex(await commitment(seed, handId, c.version))).toBe(c.commitment_hex);
      expect(await shuffleDeck(seed, handId, c.version)).toEqual(c.deck);
    });
  });
});

describe('uniform', () => {
  vectors.uniform.forEach((u, i) => {
    it(`case ${i} n=${u.n}`, () => {
      const r = uniformFromWords(u.n, u.words.map((w) => Number(w)));
      expect(r).toEqual({ result: u.result, consumed: u.consumed });
    });
  });
  it('throws when words run out', () => {
    expect(() => uniformFromWords(3, [0xffffffff])).toThrow();
  });
});

describe('encoding', () => {
  it('lp prefixes the length big-endian', () => {
    expect(Array.from(lp(new Uint8Array([97, 98, 99])))).toEqual([0, 0, 0, 3, 97, 98, 99]);
    expect(Array.from(lp(new Uint8Array()))).toEqual([0, 0, 0, 0]);
  });
});

describe('versions', () => {
  it('rejects an unknown version', async () => {
    const seed = new Uint8Array(32);
    const id = handIdBytes('aaaaa-aa', 0n, 0n);
    await expect(shuffleDeck(seed, id, 2)).rejects.toBeInstanceOf(UnsupportedVersion);
    await expect(shuffleDeck(seed, id, 0)).rejects.toBeInstanceOf(UnsupportedVersion);
    await expect(commitment(seed, id, 2)).rejects.toBeInstanceOf(UnsupportedVersion);
  });
});
