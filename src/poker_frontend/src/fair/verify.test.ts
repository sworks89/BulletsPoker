import { describe, expect, it } from 'vitest';
import type { FairEvidence, HandRecord } from '../declarations/poker_backend/poker_backend.did';
import { CANISTER, caseNamed, feesOf, recordOf, type VCase } from './fixtures';
import { verifyHand, type FairEvidenceLike, type HandRecordLike, type VerificationReport } from './verify';

// Compile-time: get_hand's reply goes straight into the verifier.
export const _acceptsCandid: (r: HandRecord, e: FairEvidence) => [HandRecordLike, FairEvidenceLike] = (r, e) => [r, e];

const UNRIGGED = ['everyone_folds_preflop', 'rake_cap', 'uncontested_after_flop', 'showdown_default_rates'];

function hand(name: string, viewerSeat = 0): { c: VCase; record: HandRecord; evidence: FairEvidence } {
  const c = caseNamed(name);
  const record = recordOf(c, viewerSeat);
  return { c, record, evidence: record.fair[0]! };
}

const run = (record: HandRecord, evidence: FairEvidence | null, c?: VCase) =>
  verifyHand(record, evidence, CANISTER, c ? { fees: feesOf(c) } : undefined);

const failed = (r: VerificationReport) => Object.entries(r.checks).filter(([, v]) => v.status === 'fail').map(([k]) => k);

describe('verifyHand', () => {
  for (const name of UNRIGGED) {
    it(`${name}: a real engine hand verifies`, async () => {
      for (const seat of [0, 2, 4]) {
        const { c, record, evidence } = hand(name, seat);
        const r = await run(record, evidence, c);
        expect(r.overall, JSON.stringify(r, null, 1)).toBe('VERIFIED');
        expect(r.version).toBe(1);
        expect(r.errors).toEqual([]);
        for (const k of ['commitment', 'shuffle', 'dealing', 'replay', 'settlement'] as const) {
          expect(r.checks[k].status, k).toBe('pass');
        }
        expect(r.checks.commitment.basis).toBe('cryptographic');
        expect(r.checks.shuffle.basis).toBe('cryptographic');
        expect(r.checks.dealing.basis).toBe('cryptographic');
        expect(r.checks.replay.basis).toBe('consistency');
        expect(r.checks.settlement.basis).toBe('consistency');
      }
    });

    it(`${name}: without the table's rates it still verifies, and says the rake is trusted`, async () => {
      const { record, evidence } = hand(name);
      const r = await run(record, evidence);
      expect(r.overall).toBe('VERIFIED');
      expect(r.trusted.map((t) => t.what).join('|')).toMatch(/rake/i);
      expect(r.warnings.join('|')).toMatch(/rake/i);
    });
  }

  it('lists the seed origin and in-hand confidentiality as trusted, never as passed checks', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const r = await run(record, evidence, c);
    expect(r.trusted.every((t) => t.basis === 'trusts-canister')).toBe(true);
    const what = r.trusted.map((t) => t.what).join('|');
    expect(what).toMatch(/seed origin/i);
    expect(what).toMatch(/confidential/i);
    expect(Object.keys(r.checks)).toEqual(['commitment', 'shuffle', 'dealing', 'replay', 'settlement']);
    const text = JSON.stringify(r, (_k, v) => (typeof v === 'bigint' ? v.toString() : v)).toLowerCase();
    for (const banned of ['trustless', 'tamper-proof', 'provably random']) expect(text).not.toContain(banned);
  });

  it('a changed seed fails the commitment', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const seed = Uint8Array.from(evidence.revealed_seed[0]!);
    seed[0] ^= 1;
    const r = await run(record, { ...evidence, revealed_seed: [seed] }, c);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.commitment.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/commitment/i);
  });

  it('a changed commitment fails', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const commitment = Uint8Array.from(evidence.commitment);
    commitment[31] ^= 0x80;
    const r = await run(record, { ...evidence, commitment }, c);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.commitment.status).toBe('fail');
  });

  it('a commitment in the log that differs from the evidence fails', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const log = record.log.map((e, i) =>
      i === 0 ? { ...e, kind: [{ ShuffleCommitted: { commitment: new Uint8Array(32) } }] as typeof e.kind } : e,
    );
    const r = await run({ ...record, log }, evidence, c);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.commitment.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/log/i);
  });

  it('a commitment logged after the hand started fails', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const [first, ...rest] = record.log;
    const log = [rest[0], first, ...rest.slice(1)];
    const r = await run({ ...record, log }, evidence, c);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.commitment.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/ShuffleCommitted.*entry 1/);
  });

  it('evidence for another canister, table or hand fails', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    expect((await verifyHand(record, evidence, 'aaaaa-aa', { fees: feesOf(c) })).checks.commitment.status).toBe('fail');
    expect((await run(record, { ...evidence, hand_no: evidence.hand_no + 1n }, c)).checks.commitment.status).toBe('fail');
    expect((await run(record, { ...evidence, table_id: 99n }, c)).checks.commitment.status).toBe('fail');
  });

  it('a tampered hole card fails dealing and names the seat, the card and its position', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const players = record.players.map((p) => (p.seat === 2 ? { ...p, cards: Uint8Array.from([p.cards[0], (Number(p.cards[1]) + 1) % 52]) } : p));
    const r = await run({ ...record, players }, evidence, c);
    expect(r.overall).toBe('FAILED');
    expect(failed(r)).toContain('dealing');
    expect(r.errors.join('\n')).toMatch(/seat 2 \(player2\) hole card 2: expected [2-9TJQKA][cdhs], record [2-9TJQKA][cdhs]/);
  });

  it('a tampered board card fails dealing and names the street', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const board = Uint8Array.from(record.board);
    board[3] = (board[3] + 1) % 52;
    const r = await run({ ...record, board }, evidence, c);
    expect(r.overall).toBe('FAILED');
    expect(failed(r)).toContain('dealing');
    expect(r.errors.join('\n')).toMatch(/board card 4 \(turn\): expected [2-9TJQKA][cdhs], record [2-9TJQKA][cdhs]/);
  });

  it('cards that do not follow from the seed fail dealing (a rigged engine hand)', async () => {
    const { c, record, evidence } = hand('split_pot');
    const r = await run(record, evidence, c);
    expect(r.overall).toBe('FAILED');
    expect(failed(r)).toContain('dealing');
  });

  it('a dealing order that does not start left of the button fails', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const dealt_order = Uint8Array.from([4, 2, 0, 4, 2, 0]);
    const r = await run(record, { ...evidence, dealt_order }, c);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.dealing.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/dealt_order/);
  });

  it('a missing ShuffleCommitted event gives INCOMPLETE', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const r = await run({ ...record, log: record.log.slice(1) }, evidence, c);
    expect(r.overall).toBe('INCOMPLETE');
    expect(r.errors.join('\n') + r.warnings.join('\n')).toMatch(/ShuffleCommitted/);
  });

  it('a changed payout fails settlement and names the player with both amounts', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const players = record.players.map((p) => (p.seat === 0 ? { ...p, won: p.won - 100n } : p.seat === 2 ? { ...p, won: p.won + 100n } : p));
    const r = await run({ ...record, players }, evidence, c);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.settlement.status).toBe('fail');
    const want = BigInt(c.expected.won.find((w) => w.seat === 0)!.won);
    expect(r.errors.join('\n')).toContain(`seat 0 (player1) won: expected ${want}, record ${want - 100n}`);
    expect(r.errors.join('\n')).toContain(`seat 2 (player2) won: expected 0, record 100`);
  });

  it('a changed rake fails settlement when the rates are known', async () => {
    const { c, record, evidence } = hand('rake_cap');
    const r = await run({ ...record, rake: record.rake + 1n }, evidence, c);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.settlement.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/rake: expected 2000000, record 2000001/);
  });

  it('rake on a hand with no flop fails even without the rates', async () => {
    const { record, evidence } = hand('everyone_folds_preflop');
    const players = record.players.map((p) => (p.won > 0n ? { ...p, won: p.won - 5n } : p));
    const r = await run({ ...record, rake: 5n, players }, evidence);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.settlement.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/no flop/i);
  });

  it('a changed commitment amount in the record fails replay, naming the player', async () => {
    const { c, record, evidence } = hand('rake_cap');
    const players = record.players.map((p) => (p.seat === 4 ? { ...p, committed: p.committed + 1n } : p));
    const r = await run({ ...record, players }, evidence, c);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.replay.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/seat 4 \(player3\) committed: log says 50000000, record 50000001/);
  });

  it('a board longer than the streets in the log fails replay', async () => {
    const { c, record, evidence } = hand('uncontested_after_flop');
    const log = record.log.filter((e) => e.text !== 'Flop');
    const r = await run({ ...record, log }, evidence, c);
    expect(r.checks.replay.status).toBe('fail');
    expect(r.overall).toBe('FAILED');
  });

  it('a legacy record (no evidence) is LEGACY, never FAILED', async () => {
    const { record } = hand('showdown_default_rates');
    const r = await run({ ...record, fair: [] }, null);
    expect(r.overall).toBe('LEGACY');
    expect(r.errors).toEqual([]);
  });

  it('an unknown version is UNSUPPORTED', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const r = await run(record, { ...evidence, version: 2 }, c);
    expect(r.overall).toBe('UNSUPPORTED');
    expect(r.version).toBe(2);
  });

  it('a seed not yet revealed is INCOMPLETE (pending reveal), even with a bad payout', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const pending = { ...evidence, revealed_seed: [] as [] };
    const r = await run(record, pending, c);
    expect(r.overall).toBe('INCOMPLETE');
    expect(r.errors).toEqual([]);
    expect(r.warnings.join('\n')).toMatch(/pending reveal/i);
    expect(r.warnings.join('\n')).toContain(new Date(Number(evidence.reveal_at / 1_000_000n)).toISOString());
    expect(r.revealAt).toBe(evidence.reveal_at);
    const players = record.players.map((p) => ({ ...p, won: p.won + 7n }));
    const bad = await run({ ...record, players }, pending, c);
    expect(bad.overall).toBe('INCOMPLETE');
    expect(bad.checks.settlement.status).toBe('fail');
  });

  it('an empty dealt_order (older build) skips the hole cards: INCOMPLETE, never FAILED', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const r = await run(record, { ...evidence, dealt_order: new Uint8Array(0) }, c);
    expect(r.overall).toBe('INCOMPLETE');
    expect(r.checks.dealing.status).toBe('skipped');
    expect(r.warnings.join('\n')).toMatch(/dealt_order/);
    expect(r.errors).toEqual([]);
  });

  it('an empty dealt_order still checks the board (missing data outranks a fail: INCOMPLETE)', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const board = Uint8Array.from(record.board);
    board[0] = (board[0] + 1) % 52;
    const r = await run({ ...record, board }, { ...evidence, dealt_order: new Uint8Array(0) }, c);
    expect(r.overall).toBe('INCOMPLETE');
    expect(r.checks.dealing.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/board card 1 \(flop\)/);
  });

  it('a voided hand is never FAILED', async () => {
    const { c, record, evidence } = hand('rake_cap');
    // As `void_unsettled_hand` publishes it: nothing paid, the log cut where the table closed.
    const end = record.log.findIndex((e) => e.text === 'Flop');
    const voided: HandRecord = {
      ...record,
      board: new Uint8Array(0),
      pot: 0n,
      rake: 0n,
      players: record.players.map((p) => ({ ...p, committed: 0n, won: 0n })),
      log: record.log.slice(0, end),
    };
    const r = await run(voided, { ...evidence, outcome: 'voided: table closed', burns: 0 }, c);
    expect(r.overall).toBe('INCOMPLETE');
    expect(r.errors).toEqual([]);
    expect(r.checks.commitment.status).toBe('pass');
    expect(r.checks.dealing.status).toBe('pass');
    expect(r.checks.settlement.status).toBe('skipped');
    expect(r.warnings.join('\n')).toMatch(/voided/);
  });
});
