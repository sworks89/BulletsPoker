import { describe, expect, it } from 'vitest';
import {
  DISCLAIMER, D1_LINE, VERIFY_WINDOW_NS, groupVerifiable, logStepLabel, potAt, revealCountdown, verifyState, verificationRows, windowText,
} from './replay';
import type { VerificationReport } from './fair/verify';

const H = 3_600_000_000_000n;
const M = 60_000_000_000n;
const entry = (o: Partial<{ kind: unknown; seat: number | undefined; text: string; amount: bigint }>) => ({
  kind: (o.kind ? [o.kind] : []) as [] | [never],
  seat: (o.seat === undefined ? [] : [o.seat]) as [] | [number],
  text: o.text ?? '',
  amount: o.amount ?? 0n,
});
const check = (status: 'pass' | 'fail' | 'skipped', basis: 'cryptographic' | 'consistency') => ({ status, basis, detail: 'd' });
const report = (overall: VerificationReport['overall'], status: 'pass' | 'fail' | 'skipped' = 'pass'): VerificationReport => ({
  overall, version: 1, trusted: [], errors: [], warnings: [],
  checks: {
    commitment: check(status, 'cryptographic'), shuffle: check(status, 'cryptographic'), dealing: check(status, 'cryptographic'),
    replay: check(status, 'consistency'), settlement: check(status, 'consistency'),
  },
});

describe('verificationRows', () => {
  it('lists the five checks then Overall with labels, status and basis', () => {
    const rows = verificationRows(report('VERIFIED'));
    expect(rows.map((r) => r.label)).toEqual(['Seed commitment', 'Deterministic shuffle', 'Dealt cards', 'Replay consistency', 'Pot settlement', 'Overall']);
    expect(rows[0]).toMatchObject({ statusText: 'Passed', basisText: 'cryptographic', tone: 'pass' });
    expect(rows[3].basisText).toBe('consistency');
    expect(rows[5]).toMatchObject({ statusText: 'Verified', tone: 'pass' });
  });
  it('words the trusts-canister basis', () => {
    const r = report('VERIFIED');
    (r.checks.settlement as { basis: string }).basis = 'trusts-canister';
    expect(verificationRows(r)[4].basisText).toBe('trusts the canister');
  });
  it('shows failures and incomplete results', () => {
    expect(verificationRows(report('FAILED', 'fail'))[5]).toMatchObject({ statusText: 'Failed', tone: 'fail' });
    expect(verificationRows(report('INCOMPLETE', 'skipped'))[1].statusText).toBe('Not checked');
    expect(verificationRows(report('INCOMPLETE', 'skipped'))[5]).toMatchObject({ statusText: 'Incomplete', tone: 'warn' });
  });
  it('words legacy as not verifiable, never as a failure', () => {
    const rows = verificationRows(report('LEGACY', 'skipped'));
    expect(rows[5]).toMatchObject({ statusText: 'Legacy / Not verifiable', tone: 'neutral' });
    expect(rows[0].statusText).toBe('Not verifiable');
    expect(rows.some((r) => r.tone === 'fail')).toBe(false);
  });
  it('words unsupported versions', () => {
    const rows = verificationRows(report('UNSUPPORTED', 'skipped'));
    expect(rows[5].statusText).toBe('Unsupported shuffle version');
    expect(rows[0].statusText).toBe('Not checked');
  });
});

describe('verifyState', () => {
  const ev = (o: object = {}) => ({ reveal_at: 100n * H, revealed_seed: [] as [] | [number[]], outcome: 'settled', ...o });
  it('is none when the record is missing', () => expect(verifyState(null, 0n).kind).toBe('none'));
  it('is legacy without fair', () => expect(verifyState({ fair: [] }, 0n).kind).toBe('legacy'));
  it('is pending before reveal_at with the countdown', () => {
    const s = verifyState({ fair: [ev()] }, 100n * H - (23n * H + 41n * M));
    expect(s).toMatchObject({ kind: 'pending', message: 'Verification available in 23 h 41 m' });
  });
  it('is ready when the seed is revealed', () => {
    expect(verifyState({ fair: [ev({ revealed_seed: [[1, 2]] })] }, 101n * H).kind).toBe('ready');
  });
  it('treats no seed after reveal_at as unavailable, not ready', () => {
    expect(verifyState({ fair: [ev()] }, 101n * H).kind).toBe('unavailable');
  });
  it('ends the window 7 days after reveal_at but keeps the verify option when the seed is there', () => {
    const now = 100n * H + VERIFY_WINDOW_NS + 1n;
    expect(verifyState({ fair: [ev({ revealed_seed: [[1]] })] }, now)).toMatchObject({
      kind: 'ended',
      message: 'Verification window ended — use exported evidence',
      canVerify: true,
    });
  });
  it('a seed never published is shown as withheld, not as a normal end of the window', () => {
    const now = 100n * H + VERIFY_WINDOW_NS + 1n;
    expect(verifyState({ fair: [ev()] }, now)).toMatchObject({ kind: 'unavailable', canVerify: false });
  });
  it('labels voided hands', () => {
    const s = verifyState({ fair: [ev({ revealed_seed: [[1]], outcome: 'voided: dealer error' })] }, 101n * H);
    expect(s).toMatchObject({ kind: 'ready', voided: true, voidedLabel: 'Voided — cards verifiable, nothing was settled' });
  });
});

describe('revealCountdown / windowText', () => {
  it('formats hours and minutes', () => {
    expect(revealCountdown(24n * H, 0n)).toBe('24 h 0 m');
    expect(revealCountdown(5n * M, 0n)).toBe('5 m');
    expect(revealCountdown(1n, 0n)).toBe('less than a minute');
  });
  it('states the verification window', () => {
    expect(windowText(0n)).toBe('Verifiable until 1970-01-08 00:00 UTC; export the evidence to keep it longer');
  });
});

describe('logStepLabel', () => {
  it('renders protocol steps readably', () => {
    const commit = entry({ kind: { ShuffleCommitted: { commitment: new Uint8Array([0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xff]) } }, text: 'x' });
    expect(logStepLabel(commit)).toEqual({ type: 'protocol', text: 'Shuffle committed: abcdef0123456789…' });
    const rev = entry({ kind: { ShuffleRevealed: null }, amount: 0n });
    expect(logStepLabel(rev)).toEqual({ type: 'protocol', text: 'Seed reveal at 1970-01-01 00:00 UTC' });
    expect(logStepLabel(entry({ kind: { HandVerificationAvailable: null } })).text).toBe('Hand verification available');
    expect(logStepLabel(entry({ kind: { HandVerificationAvailable: null } }), 0n).text).toBe('Hand verification available from 1970-01-01 00:00 UTC');
  });
  it('keeps ordinary actions and streets', () => {
    expect(logStepLabel(entry({ seat: 2, text: 'Raises', amount: 5n }))).toEqual({ type: 'action', seat: 2, text: 'raises', amount: 5n });
    expect(logStepLabel(entry({ text: 'Flop' }))).toEqual({ type: 'street', text: 'Flop' });
  });
});

describe('potAt', () => {
  it('ignores fair entries even if they carry a seat or an amount', () => {
    const log = [
      entry({ kind: { ShuffleCommitted: { commitment: [] } }, seat: 0, amount: 999n }),
      entry({ seat: 1, text: 'Raises', amount: 10n }),
      entry({ kind: { ShuffleRevealed: null }, seat: 1, amount: 5_000_000_000n }),
    ];
    expect(potAt(log, 3)).toBe(10n);
  });
});

describe('copy', () => {
  it('has the exact disclaimer and D1 line', () => {
    expect(DISCLAIMER).toBe(
      "This check proves the cards dealt match the seed behind the published commitment and that the pot was paid by the rules. If this device saw that commitment at the table during the hand, the deck was fixed before the flop and before any betting; otherwise that order rests on the canister's own log. It does not prove the seed came from ICP randomness, and it does not prove nobody could see the cards during play — the game canister holds them.",
    );
    expect(DISCLAIMER).not.toMatch(/committed before the hand/);
    expect(D1_LINE).toBe('24 hours after a hand, the players in it can verify the shuffle, which reveals every card dealt in that hand.');
  });
});

describe('groupVerifiable', () => {
  const h = (id: number, reveal: bigint | null) => ({ id: BigInt(id), reveal_at: (reveal === null ? [] : [reveal]) as [] | [bigint] });
  it('lists revealed hands first (newest first), then pending ones soonest first with a countdown', () => {
    const now = 100n * H;
    const g = groupVerifiable([h(5, 101n * H), h(4, 120n * H + 30n * M), h(3, 99n * H), h(2, 100n * H), h(1, 50n * H), h(0, null)], now);
    expect(g.ready.map((x) => x.id)).toEqual([3n, 2n, 1n]);
    expect(g.pending.map((x) => x.id)).toEqual([5n, 4n]);
    expect(g.pending.map((x) => x.countdown)).toEqual(['1 h 0 m', '20 h 30 m']);
  });
  it('drops hands with no evidence and is empty for an empty list', () => {
    expect(groupVerifiable([h(1, null)], 0n)).toEqual({ ready: [], pending: [] });
    expect(groupVerifiable([], 0n)).toEqual({ ready: [], pending: [] });
  });
});
