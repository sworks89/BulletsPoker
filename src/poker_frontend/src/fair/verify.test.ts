import { describe, expect, it } from 'vitest';
import type { FairEvidence, HandRecord } from '../declarations/poker_backend/poker_backend.did';
import { CANISTER, caseNamed, recordOf, withoutRates, type VCase } from './fixtures';
import { verifyHand, type FairEvidenceLike, type HandRecordLike, type VerificationReport } from './verify';

// Compile-time: get_hand's reply goes straight into the verifier.
export const _acceptsCandid: (r: HandRecord, e: FairEvidence) => [HandRecordLike, FairEvidenceLike] = (r, e) => [r, e];

const UNRIGGED = ['everyone_folds_preflop', 'rake_cap', 'uncontested_after_flop', 'showdown_default_rates'];

function hand(name: string, viewerSeat = 0): { c: VCase; record: HandRecord; evidence: FairEvidence } {
  const c = caseNamed(name);
  const record = recordOf(c, viewerSeat);
  return { c, record, evidence: record.fair[0]! };
}

/** Verified "now": well after the vectors' reveal time unless given. */
const run = (record: HandRecord, evidence: FairEvidence | null, now?: bigint) => verifyHand(record, evidence, CANISTER, { now });

const failed = (r: VerificationReport) => Object.entries(r.checks).filter(([, v]) => v.status === 'fail').map(([k]) => k);
const allText = (r: VerificationReport) => [...r.errors, ...r.warnings].join('\n');

describe('verifyHand', () => {
  for (const name of UNRIGGED) {
    it(`${name}: a real engine hand verifies, for every player`, async () => {
      for (const seat of [0, 2, 4]) {
        const { record, evidence } = hand(name, seat);
        const r = await run(record, evidence);
        expect(r.overall, JSON.stringify(r, (_k, v) => (typeof v === 'bigint' ? `${v}` : v), 1)).toBe('VERIFIED');
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
        // The rates come from the evidence: rake and drop are recomputed, not trusted.
        expect(r.trusted.map((t) => t.what).join('|')).not.toMatch(/rake/i);
        expect(r.checks.settlement.detail).toMatch(/rates recorded at the deal/);
      }
    });

    it(`${name}: old evidence without rates still verifies, and says rake and drop came from the record`, async () => {
      const { record, evidence } = hand(name);
      const r = await run(record, withoutRates(evidence));
      expect(r.overall).toBe('VERIFIED');
      expect(r.trusted.map((t) => t.what).join('|')).toMatch(/rake/i);
      expect(r.checks.settlement.detail).toMatch(/taken from the record/);
    });
  }

  it('the dealing detail says the order check relies on the record', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    expect((await run(record, evidence)).checks.dealing.detail).toMatch(/button and player list/);
  });

  it('lists the seed origin and in-hand confidentiality as trusted, never as passed checks', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const r = await run(record, evidence);
    expect(r.trusted.every((t) => t.basis === 'trusts-canister')).toBe(true);
    const what = r.trusted.map((t) => t.what).join('|');
    expect(what).toMatch(/seed origin/i);
    expect(what).toMatch(/confidential/i);
    expect(Object.keys(r.checks)).toEqual(['commitment', 'shuffle', 'dealing', 'replay', 'settlement']);
    const text = JSON.stringify(r, (_k, v) => (typeof v === 'bigint' ? v.toString() : v)).toLowerCase();
    for (const banned of ['trustless', 'tamper-proof', 'provably random']) expect(text).not.toContain(banned);
  });

  it('a changed seed fails the commitment', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const seed = Uint8Array.from(evidence.revealed_seed[0]!);
    seed[0] ^= 1;
    const r = await run(record, { ...evidence, revealed_seed: [seed] });
    expect(r.overall).toBe('FAILED');
    expect(r.checks.commitment.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/commitment/i);
  });

  it('a changed commitment fails', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const commitment = Uint8Array.from(evidence.commitment);
    commitment[31] ^= 0x80;
    const r = await run(record, { ...evidence, commitment });
    expect(r.overall).toBe('FAILED');
    expect(r.checks.commitment.status).toBe('fail');
  });

  it('a commitment in the log that differs from the evidence fails', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const log = record.log.map((e, i) =>
      i === 0 ? { ...e, kind: [{ ShuffleCommitted: { commitment: new Uint8Array(32) } }] as typeof e.kind } : e,
    );
    const r = await run({ ...record, log }, evidence);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.commitment.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/log/i);
  });

  it('a commitment logged after the hand started fails', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const [first, ...rest] = record.log;
    const log = [rest[0], first, ...rest.slice(1)];
    const r = await run({ ...record, log }, evidence);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.commitment.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/ShuffleCommitted.*entry 1/);
  });

  it('evidence for another canister, table or hand fails', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    expect((await verifyHand(record, evidence, 'aaaaa-aa')).checks.commitment.status).toBe('fail');
    expect((await run(record, { ...evidence, hand_no: evidence.hand_no + 1n })).checks.commitment.status).toBe('fail');
    expect((await run(record, { ...evidence, table_id: 99n })).checks.commitment.status).toBe('fail');
  });

  it('a tampered hole card fails dealing and names the seat, the card and its position', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const players = record.players.map((p) => (p.seat === 2 ? { ...p, cards: Uint8Array.from([p.cards[0], (Number(p.cards[1]) + 1) % 52]) } : p));
    const r = await run({ ...record, players }, evidence);
    expect(r.overall).toBe('FAILED');
    expect(failed(r)).toContain('dealing');
    expect(r.errors.join('\n')).toMatch(/seat 2 \(player2\) hole card 2: expected [2-9TJQKA][cdhs], record [2-9TJQKA][cdhs]/);
  });

  it('a tampered board card fails dealing and names the street', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const board = Uint8Array.from(record.board);
    board[3] = (board[3] + 1) % 52;
    const r = await run({ ...record, board }, evidence);
    expect(r.overall).toBe('FAILED');
    expect(failed(r)).toContain('dealing');
    expect(r.errors.join('\n')).toMatch(/board card 4 \(turn\): expected [2-9TJQKA][cdhs], record [2-9TJQKA][cdhs]/);
  });

  it('cards that do not follow from the seed fail dealing (a rigged engine hand)', async () => {
    const { record, evidence } = hand('split_pot');
    const r = await run(record, evidence);
    expect(r.overall).toBe('FAILED');
    expect(failed(r)).toContain('dealing');
  });

  it('a dealing order that does not start left of the button fails', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const dealt_order = Uint8Array.from([4, 2, 0, 4, 2, 0]);
    const r = await run(record, { ...evidence, dealt_order });
    expect(r.overall).toBe('FAILED');
    expect(r.checks.dealing.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/dealt_order/);
  });

  it('a missing ShuffleCommitted event gives INCOMPLETE', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const r = await run({ ...record, log: record.log.slice(1) }, evidence);
    expect(r.overall).toBe('INCOMPLETE');
    expect(allText(r)).toMatch(/ShuffleCommitted/);
  });

  it('dropping the ShuffleCommitted event does not hide a bad seed: FAILED', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const seed = Uint8Array.from(evidence.revealed_seed[0]!);
    seed[5] ^= 1;
    const r = await run({ ...record, log: record.log.slice(1) }, { ...evidence, revealed_seed: [seed] });
    expect(r.overall).toBe('FAILED');
    expect(r.checks.commitment.status).toBe('fail');
  });

  it('a changed payout fails settlement and names the player with both amounts', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const players = record.players.map((p) => (p.seat === 0 ? { ...p, won: p.won - 100n } : p.seat === 2 ? { ...p, won: p.won + 100n } : p));
    const r = await run({ ...record, players }, evidence);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.settlement.status).toBe('fail');
    const want = BigInt(c.expected.won.find((w) => w.seat === 0)!.won);
    expect(r.errors.join('\n')).toContain(`seat 0 (player1) won: expected ${want}, record ${want - 100n}`);
    expect(r.errors.join('\n')).toContain(`seat 2 (player2) won: expected 0, record 100`);
  });

  it('a single winner paid less (the difference hidden as jackpot drop) fails with the rates on the evidence', async () => {
    const { c, record, evidence } = hand('showdown_default_rates');
    const players = record.players.map((p) => (p.seat === 0 ? { ...p, won: p.won - 1000n } : p));
    // Rewrite the log's win line too, so only the settlement can catch it.
    const log = record.log.map((e) => (e.text.startsWith('Wins') && e.seat[0] === 0 ? { ...e, amount: e.amount - 1000n } : e));
    const r = await run({ ...record, players, log }, evidence);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.replay.status).toBe('pass');
    expect(r.checks.settlement.status).toBe('fail');
    const want = BigInt(c.expected.won.find((w) => w.seat === 0)!.won);
    expect(r.errors.join('\n')).toContain(`seat 0 (player1) won: expected ${want}, record ${want - 1000n}`);
  });

  it('a changed rake fails settlement', async () => {
    const { record, evidence } = hand('rake_cap');
    const r = await run({ ...record, rake: record.rake + 1n }, evidence);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.settlement.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/rake: expected 2000000, record 2000001/);
  });

  it('rake on a hand with no flop fails even without the rates', async () => {
    const { record, evidence } = hand('everyone_folds_preflop');
    const players = record.players.map((p) => (p.won > 0n ? { ...p, won: p.won - 5n } : p));
    const r = await run({ ...record, rake: 5n, players }, withoutRates(evidence));
    expect(r.overall).toBe('FAILED');
    expect(r.checks.settlement.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/no flop/i);
  });

  it('a bet nobody called but nobody got back fails settlement with the expected return', async () => {
    const { record, evidence } = hand('uncontested_after_flop');
    const returned = record.log.find((e) => e.text === 'Uncalled bet returned')!;
    const log = record.log.filter((e) => e !== returned);
    const r = await run({ ...record, log }, evidence);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.settlement.status).toBe('fail');
    expect(r.errors.join('\n')).toContain(`seat 2 (player2) has ${returned.amount} more in than anyone else and no "Left the table" line: expected ${returned.amount} returned`);
  });

  it('a changed commitment amount in the record fails replay, naming the player', async () => {
    const { record, evidence } = hand('rake_cap');
    const players = record.players.map((p) => (p.seat === 4 ? { ...p, committed: p.committed + 1n } : p));
    const r = await run({ ...record, players }, evidence);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.replay.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/seat 4 \(player3\) committed: log says 50000000, record 50000001/);
  });

  it('a board longer than the streets in the log fails replay', async () => {
    const { record, evidence } = hand('uncontested_after_flop');
    const log = record.log.filter((e) => e.text !== 'Flop');
    const r = await run({ ...record, log }, evidence);
    expect(r.checks.replay.status).toBe('fail');
    expect(r.overall).toBe('FAILED');
  });

  it('a showdown with a short board is FAILED, not a rejected promise', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const r = await run({ ...record, board: Uint8Array.from(record.board).slice(0, 4) }, evidence);
    expect(r.overall).toBe('FAILED');
    expect(r.checks.settlement.status).toBe('fail');
  });

  it('a dealt_order missing a live seat is FAILED, not a rejected promise', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const dealt_order = Uint8Array.from(Array.from(evidence.dealt_order).filter((s) => s !== 4));
    const r = await run(record, { ...evidence, dealt_order });
    expect(r.overall).toBe('FAILED');
    expect(r.checks.dealing.status).toBe('fail');
    expect(r.checks.settlement.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/seat 4/);
  });

  it('a legacy record (no evidence) is LEGACY, never FAILED', async () => {
    const { record } = hand('showdown_default_rates');
    const r = await run({ ...record, fair: [] }, null);
    expect(r.overall).toBe('LEGACY');
    expect(r.errors).toEqual([]);
  });

  it('an unknown version is UNSUPPORTED', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const r = await run(record, { ...evidence, version: 2 });
    expect(r.overall).toBe('UNSUPPORTED');
    expect(r.version).toBe(2);
  });

  it('before reveal_at a missing seed is INCOMPLETE (pending reveal), even with a bad payout', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const pending = { ...evidence, revealed_seed: [] as [] };
    const before = evidence.reveal_at - 1n;
    const r = await run(record, pending, before);
    expect(r.overall).toBe('INCOMPLETE');
    expect(r.errors).toEqual([]);
    expect(r.warnings.join('\n')).toMatch(/pending reveal/i);
    expect(r.warnings.join('\n')).toMatch(/check again/i);
    expect(r.warnings.join('\n')).toContain(new Date(Number(evidence.reveal_at / 1_000_000n)).toISOString());
    expect(r.revealAt).toBe(evidence.reveal_at);
    const players = record.players.map((p) => ({ ...p, won: p.won + 7n }));
    const bad = await run({ ...record, players }, pending, before);
    expect(bad.overall).toBe('INCOMPLETE');
    expect(bad.checks.settlement.status).toBe('fail');
  });

  it('from reveal_at on a missing seed is INCOMPLETE, but not "pending"', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const r = await run(record, { ...evidence, revealed_seed: [] }, evidence.reveal_at);
    expect(r.overall).toBe('INCOMPLETE');
    expect(r.warnings.join('\n')).toMatch(/seed not published although reveal_at has passed/);
    expect(allText(r)).not.toMatch(/pending reveal|check again/i);
    // And a proven fail is no longer covered by a pending reveal.
    const players = record.players.map((p) => ({ ...p, won: p.won + 7n }));
    const bad = await run({ ...record, players }, { ...evidence, revealed_seed: [] }, evidence.reveal_at);
    expect(bad.overall).toBe('FAILED');
  });

  it('an empty dealt_order (older build) skips the hole cards: INCOMPLETE, never FAILED', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const r = await run(record, { ...evidence, dealt_order: new Uint8Array(0) });
    expect(r.overall).toBe('INCOMPLETE');
    expect(r.checks.dealing.status).toBe('skipped');
    expect(r.warnings.join('\n')).toMatch(/dealt_order/);
    expect(r.errors).toEqual([]);
  });

  it('an empty dealt_order does not hide a tampered board: FAILED', async () => {
    const { record, evidence } = hand('showdown_default_rates');
    const board = Uint8Array.from(record.board);
    board[0] = (board[0] + 1) % 52;
    const r = await run({ ...record, board }, { ...evidence, dealt_order: new Uint8Array(0) });
    expect(r.overall).toBe('FAILED');
    expect(r.checks.dealing.status).toBe('fail');
    expect(r.errors.join('\n')).toMatch(/board card 1 \(flop\)/);
  });

  function voidedOf(name: string) {
    const { record, evidence } = hand(name);
    // As `void_unsettled_hand` publishes it: nothing paid, the log cut where the table closed.
    const end = record.log.findIndex((e) => e.text === 'Turn');
    const voided: HandRecord = {
      ...record,
      board: Uint8Array.from(record.board).slice(0, 3),
      pot: 0n,
      rake: 0n,
      players: record.players.map((p) => ({ ...p, committed: 0n, won: 0n })),
      log: record.log.slice(0, end),
    };
    return { record: voided, evidence: { ...evidence, outcome: 'voided: table closed', burns: 1 } };
  }

  it('a voided hand is INCOMPLETE, never FAILED, when its cards check out', async () => {
    const { record, evidence } = voidedOf('rake_cap');
    const r = await run(record, evidence);
    expect(r.overall).toBe('INCOMPLETE');
    expect(r.errors).toEqual([]);
    expect(r.checks.commitment.status).toBe('pass');
    expect(r.checks.dealing.status).toBe('pass');
    expect(r.checks.settlement.status).toBe('skipped');
    expect(r.warnings.join('\n')).toMatch(/voided/);
  });

  it('a voided hand with a tampered board is FAILED', async () => {
    const { record, evidence } = voidedOf('rake_cap');
    const board = Uint8Array.from(record.board);
    board[1] = (board[1] + 1) % 52;
    const r = await run({ ...record, board }, evidence);
    expect(r.overall).toBe('FAILED');
    expect(r.errors.join('\n')).toMatch(/board card 2 \(flop\)/);
  });
});
