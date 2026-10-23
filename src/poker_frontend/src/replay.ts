// Pure helpers behind the hand replay and its Verify Hand panel.
import type { VerificationReport } from './fair/verify';

export const DISCLAIMER =
  'This check proves the cards dealt match a seed committed before the hand and that the pot was paid by the rules. It does not prove the seed came from ICP randomness, and it does not prove nobody could see the cards during play — the game canister holds them.';
export const D1_LINE = '24 hours after a hand, the players in it can verify the shuffle, which reveals every card dealt in that hand.';

const NS_PER_MS = 1_000_000n;
const HOUR_NS = 3_600_000_000_000n;
const MIN_NS = 60_000_000_000n;
/** How long a revealed hand stays verifiable on the canister after reveal_at. */
export const VERIFY_WINDOW_NS = 7n * 24n * HOUR_NS;

const utc = (ns: bigint) => new Date(Number(ns / NS_PER_MS)).toISOString().slice(0, 16).replace('T', ' ') + ' UTC';
const hex = (b: Uint8Array | number[]) => Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');

// ------------------------------------------------------------ report rows

export interface VerificationRow {
  key: string;
  label: string;
  statusText: string;
  basisText: string;
  tone: 'pass' | 'fail' | 'warn' | 'neutral';
  detail: string;
}

const CHECK_LABELS = [
  ['commitment', 'Seed commitment'],
  ['shuffle', 'Deterministic shuffle'],
  ['dealing', 'Dealt cards'],
  ['replay', 'Replay consistency'],
  ['settlement', 'Pot settlement'],
] as const;

const BASIS_TEXT = { cryptographic: 'cryptographic', consistency: 'consistency', 'trusts-canister': 'trusts the canister' } as const;

const OVERALL: Record<VerificationReport['overall'], { text: string; tone: VerificationRow['tone'] }> = {
  VERIFIED: { text: 'Verified', tone: 'pass' },
  FAILED: { text: 'Failed', tone: 'fail' },
  INCOMPLETE: { text: 'Incomplete', tone: 'warn' },
  LEGACY: { text: 'Legacy / Not verifiable', tone: 'neutral' },
  UNSUPPORTED: { text: 'Unsupported shuffle version', tone: 'warn' },
};

export function verificationRows(report: VerificationReport): VerificationRow[] {
  const legacy = report.overall === 'LEGACY';
  const rows: VerificationRow[] = CHECK_LABELS.map(([key, label]) => {
    const c = report.checks[key];
    const statusText = c.status === 'pass' ? 'Passed' : c.status === 'fail' ? 'Failed' : legacy ? 'Not verifiable' : 'Not checked';
    const tone = c.status === 'pass' ? 'pass' : c.status === 'fail' ? 'fail' : 'neutral';
    return { key, label, statusText, basisText: BASIS_TEXT[c.basis], tone, detail: c.detail };
  });
  const o = OVERALL[report.overall];
  const detail = report.errors[0] ?? (legacy ? 'This hand was dealt before the shuffle commitment existed. That is not a failure.' : '');
  rows.push({ key: 'overall', label: 'Overall', statusText: o.text, basisText: '', tone: o.tone, detail });
  return rows;
}

// ------------------------------------------------------ verify button state

export function revealCountdown(revealAt: bigint, now: bigint): string {
  const left = revealAt - now;
  if (left < MIN_NS) return 'less than a minute';
  const h = left / HOUR_NS;
  const m = (left % HOUR_NS) / MIN_NS;
  return h > 0n ? `${h} h ${m} m` : `${m} m`;
}

export function windowText(revealAt: bigint): string {
  return `Verifiable until ${utc(revealAt + VERIFY_WINDOW_NS)}; export the evidence to keep it longer`;
}

interface StateEvidence {
  reveal_at: bigint;
  revealed_seed: [] | [unknown];
  outcome: string;
}

export type VerifyState =
  | { kind: 'none' }
  | { kind: 'legacy'; message: string }
  | { kind: 'pending'; message: string; revealAt: bigint }
  | { kind: 'ready' | 'ended' | 'unavailable'; message: string; revealAt: bigint; canVerify: boolean; voided: boolean; voidedLabel?: string };

const VOIDED_LABEL = 'Voided — cards verifiable, nothing was settled';

/** What the Verify area of a replay should offer. `now` is in ns since the epoch. */
export function verifyState(hand: { fair: [] | [StateEvidence] } | null, now: bigint): VerifyState {
  if (!hand) return { kind: 'none' };
  const ev = hand.fair[0];
  if (!ev) return { kind: 'legacy', message: 'Legacy / Not verifiable' };
  const revealAt = ev.reveal_at;
  if (now < revealAt) return { kind: 'pending', message: `Verification available in ${revealCountdown(revealAt, now)}`, revealAt };
  const canVerify = ev.revealed_seed.length > 0;
  const voided = ev.outcome.startsWith('voided');
  const base = { revealAt, canVerify, voided, ...(voided ? { voidedLabel: VOIDED_LABEL } : {}) };
  if (now > revealAt + VERIFY_WINDOW_NS) return { kind: 'ended', message: 'Verification window ended — use exported evidence', ...base };
  if (!canVerify) return { kind: 'unavailable', message: 'The seed is not published for this hand. Use exported evidence if you have it.', ...base };
  return { kind: 'ready', message: 'Verify hand', ...base };
}

// ----------------------------------------------------------- log rendering

export interface LogLike {
  kind: [] | [Record<string, unknown>];
  seat: [] | [number];
  text: string;
  amount: bigint;
}

export type LogStep =
  | { type: 'protocol'; text: string }
  | { type: 'street'; text: string }
  | { type: 'action'; seat: number; text: string; amount: bigint };

/** A log entry as a replay step. Protocol entries never carry chips: ShuffleRevealed's amount is reveal_at. */
export function logStepLabel(e: LogLike): LogStep {
  const k = e.kind[0];
  if (k) {
    if ('ShuffleCommitted' in k) {
      const c = (k.ShuffleCommitted as { commitment: Uint8Array | number[] }).commitment;
      return { type: 'protocol', text: `Shuffle committed: ${hex(c).slice(0, 16)}…` };
    }
    if ('ShuffleRevealed' in k) return { type: 'protocol', text: `Seed reveal at ${utc(e.amount)}` };
    if ('HandVerificationAvailable' in k) return { type: 'protocol', text: 'Hand verification available' };
    return { type: 'protocol', text: e.text };
  }
  if (e.seat.length) return { type: 'action', seat: e.seat[0], text: e.text.toLowerCase(), amount: e.amount };
  return { type: 'street', text: e.text };
}

const STREETS = new Set(['Flop', 'Turn', 'River']);

/** Pot size after the first `step` log entries. Protocol entries are ignored. */
export function potAt(log: readonly LogLike[], step: number): bigint {
  let banked = 0n;
  const street = new Map<number, bigint>();
  for (const e of log.slice(0, step)) {
    if (e.kind.length) continue;
    const seat = e.seat[0];
    if (seat === undefined) {
      if (STREETS.has(e.text)) {
        for (const v of street.values()) banked += v;
        street.clear();
      }
    } else if (e.text === 'Uncalled bet returned') {
      street.set(seat, (street.get(seat) ?? 0n) - e.amount);
    } else if (!e.text.startsWith('Wins') && !e.text.startsWith('Shows') && e.amount > 0n) {
      street.set(seat, e.amount);
    }
  }
  for (const v of street.values()) banked += v;
  return banked;
}
