import { describe, expect, it } from 'vitest';
import { CANISTER, caseNamed, recordOf, withoutRates } from './fixtures';
import { EVIDENCE_FORMAT, fromEvidenceJson, toEvidenceJson } from './evidence';
import { verifyHand } from './verify';

const NAMES = ['everyone_folds_preflop', 'rake_cap', 'uncontested_after_flop', 'showdown_default_rates'];
const NOW = 2n ** 62n;
const make = (name: string, seat = 0) => {
  const record = recordOf(caseNamed(name), seat);
  return { record, evidence: record.fair[0]! };
};
/** Order-independent, byte-aware dump for comparing decoded values. */
const norm = (x: unknown): unknown => {
  if (typeof x === 'bigint') return `${x}n`;
  if (x instanceof Uint8Array) return Array.from(x);
  if (Array.isArray(x)) return x.map(norm);
  if (x && typeof x === 'object') {
    if ('toText' in x && typeof x.toText === 'function') return x.toText();
    return Object.fromEntries(
      Object.entries(x)
        .sort(([a], [b]) => (a < b ? -1 : 1))
        .map(([k, v]) => [k, norm(v)]),
    );
  }
  return x;
};
const show = (x: unknown) => JSON.stringify(norm(x));
const parsed = (text: string) => {
  const r = fromEvidenceJson(text);
  if ('error' in r) throw new Error(r.error);
  return r;
};

describe('evidence JSON', () => {
  it('has the documented shape: format, hex bytes, decimal strings, sorted keys', () => {
    const { record, evidence } = make('showdown_default_rates');
    const text = toEvidenceJson(record, evidence);
    const j = JSON.parse(text);
    expect(j.format).toBe(EVIDENCE_FORMAT);
    expect(j.canister).toBe(CANISTER);
    expect(j.evidence.revealed_seed).toMatch(/^[0-9a-f]{64}$/);
    expect(j.evidence.table_id).toBe(String(evidence.table_id));
    expect(j.record.pot).toBe(String(record.pot));
    expect(j.record.currency).toBe('Icp');
    expect(Object.keys(j)).toEqual([...Object.keys(j)].sort());
    expect(text).not.toMatch(/\s\s|\n/);
  });

  for (const name of NAMES) {
    it(`${name}: a round trip gives the same values and the same report`, async () => {
      for (const seat of [0, 2]) {
        const { record, evidence } = make(name, seat);
        const text = toEvidenceJson(record, evidence);
        const back = parsed(text);
        expect(show(back.record)).toBe(show(record));
        expect(show(back.evidence)).toBe(show(evidence));
        const a = await verifyHand(record, evidence, CANISTER, { now: NOW });
        const b = await verifyHand(back.record, back.evidence, back.canister, { now: NOW });
        expect(a.overall).toBe('VERIFIED');
        expect(show(b)).toBe(show(a));
        expect(toEvidenceJson(back.record, back.evidence)).toBe(text);
      }
    });
  }

  it('is byte-identical whatever order the input keys were built in', () => {
    const { record, evidence } = make('showdown_default_rates');
    const reorder = <T extends object>(o: T): T => Object.fromEntries(Object.entries(o).reverse()) as T;
    const shuffled = reorder({ ...record, players: record.players.map(reorder), log: record.log.map(reorder) });
    expect(toEvidenceJson(shuffled, reorder(evidence))).toBe(toEvidenceJson(record, evidence));
  });

  it('keeps evidence without fee rates (null options) verifying', async () => {
    const { record, evidence } = make('showdown_default_rates');
    const old = withoutRates(evidence);
    const back = parsed(toEvidenceJson(record, old));
    const r = await verifyHand(back.record, back.evidence, back.canister, { now: NOW });
    expect(r.overall).toBe('VERIFIED');
    expect(r.trusted.some((t) => t.what.includes('rake'))).toBe(true);
  });

  it('keeps a tampered file failing, not erroring', async () => {
    const { record, evidence } = make('showdown_default_rates');
    const j = JSON.parse(toEvidenceJson(record, evidence));
    const first = j.record.board.slice(0, 2);
    j.record.board = (first === '00' ? '01' : '00') + j.record.board.slice(2);
    const back = parsed(JSON.stringify(j));
    expect((await verifyHand(back.record, back.evidence, back.canister, { now: NOW })).overall).toBe('FAILED');
  });

  describe('errors', () => {
    const err = (t: string) => {
      const r = fromEvidenceJson(t);
      if (!('error' in r)) throw new Error('expected an error');
      return r.error;
    };
    it('names malformed JSON', () => {
      expect(err('{"format": ')).toMatch(/not valid JSON/);
      expect(err('')).toMatch(/not valid JSON/);
    });
    it('rejects non-objects and a wrong or missing format', () => {
      expect(err('[]')).toMatch(/must be an object/);
      expect(err('{}')).toMatch(/format is missing/);
      expect(err('{"format":"other/v9"}')).toMatch(/Unsupported evidence format "other\/v9"/);
    });
    it('names the bad field', () => {
      const { record, evidence } = make('showdown_default_rates');
      const edit = (f: (j: any) => void) => {
        const j = JSON.parse(toEvidenceJson(record, evidence));
        f(j);
        return err(JSON.stringify(j));
      };
      expect(edit((j) => delete j.record.pot)).toMatch(/record\.pot is missing/);
      expect(edit((j) => (j.record.pot = 5))).toMatch(/record\.pot must be a decimal integer string/);
      expect(edit((j) => (j.evidence.commitment = 'XYZ'))).toMatch(/evidence\.commitment must be lowercase hex/);
      expect(edit((j) => (j.record.players[0].player = 'nope'))).toMatch(/record\.players\[0\]\.player is not a valid principal/);
      expect(edit((j) => (j.record.log[1].phase = 'Nope'))).toMatch(/record\.log\[1\]\.phase must be one of/);
      expect(edit((j) => (j.canister = 'aaaaa-aa'))).toMatch(/does not match/);
    });
  });
});
