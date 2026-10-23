// Test fixtures: settlement-vectors.json records in the shape `get_hand` returns.
// Only imported by tests.
import { Principal } from '@icp-sdk/core/principal';
import type { FairEvidence, HandRecord, LogEntry, LogKind } from '../declarations/poker_backend/poker_backend.did';
import vectors from './settlement-vectors.json';
import type { LogLike } from './settle';

export type VCase = (typeof vectors.cases)[number];
type VRecord = VCase['record'];
type VLog = VRecord['log'][number];

export const CANISTER = vectors.canister;

export const fromHex = (h: string) => new Uint8Array((h.match(/../g) ?? []).map((x) => parseInt(x, 16)));

export const caseNamed = (name: string): VCase => {
  const c = vectors.cases.find((x) => x.name === name);
  if (!c) throw new Error(`no settlement vector ${name}`);
  return c;
};

function kindOf(k: VLog['kind']): LogEntry['kind'] {
  if (k === null) return [];
  if (typeof k === 'string') return [{ [k]: null } as LogKind];
  return [{ ShuffleCommitted: { commitment: fromHex(k.ShuffleCommitted.commitment) } }];
}

export function logOf(c: VCase): LogLike[] {
  return toLog(c.record.log);
}

function toLog(log: VLog[]): LogEntry[] {
  return log.map((e) => ({
    phase: { [e.phase]: null } as LogEntry['phase'],
    seat: e.seat === null ? [] : [e.seat],
    text: e.text,
    amount: BigInt(e.amount),
    kind: kindOf(e.kind),
  }));
}

function toEvidence(f: NonNullable<VRecord['fair']>): FairEvidence {
  return {
    version: f.version,
    scheme: f.scheme,
    canister: Principal.fromText(f.canister),
    table_id: BigInt(f.table_id),
    hand_no: BigInt(f.hand_no),
    commitment: fromHex(f.commitment),
    revealed_seed: f.revealed_seed === null ? [] : [fromHex(f.revealed_seed)],
    params: fromHex(f.params),
    outcome: f.outcome,
    dealt_order: Uint8Array.from(f.dealt_order),
    burns: f.burns,
    settled_at: BigInt(f.settled_at),
    reveal_at: BigInt(f.reveal_at),
  };
}

/**
 * The record as `get_hand` returns it to the player at `viewerSeat` once the
 * seed is revealed: other players' unshown hole cards are cleared.
 */
export function recordOf(c: VCase, viewerSeat = 0): HandRecord {
  const r = c.record;
  return {
    id: BigInt(r.id),
    table_id: BigInt(r.table_id),
    table_name: 'Low Rollers',
    currency: { Icp: null },
    small_blind: BigInt(r.small_blind),
    big_blind: BigInt(r.big_blind),
    hand_no: BigInt(r.hand_no),
    time: 0n,
    button: r.button,
    board: Uint8Array.from(r.board),
    pot: BigInt(r.pot),
    rake: BigInt(r.rake),
    players: r.players.map((p) => ({
      seat: p.seat,
      player: Principal.fromText(p.player),
      username: p.username,
      cards: p.seat === viewerSeat || p.shown ? Uint8Array.from(p.cards) : new Uint8Array(0),
      shown: p.shown,
      committed: BigInt(p.committed),
      won: BigInt(p.won),
    })),
    log: toLog(r.log),
    fair: r.fair === null ? [] : [toEvidence(r.fair)],
  };
}

/** The fees the case was played at. */
export const feesOf = (c: VCase) => ({
  rake_bps: c.fees.rake_bps,
  rake_cap: BigInt(c.fees.rake_cap),
  jackpot_bps: c.fees.jackpot_bps,
  jackpot_cap: BigInt(c.fees.jackpot_cap),
});
