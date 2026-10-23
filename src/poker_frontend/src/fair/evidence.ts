// Hand evidence files: a hand's record and its BP-FAIR evidence as canonical
// JSON, so anyone holding the file can run the verifier without the canister.
//
// Format id: "bulletspoker-hand-evidence/v1". Top level:
//   { canister, evidence, format, record }
// Canonical text: UTF-8 JSON with every object's keys sorted (by UTF-16 code
// unit), no whitespace at all, arrays in their natural order. Equal content
// always gives byte-identical text.
//
// Candid -> JSON mapping:
//   nat64 / nat32 / nat8 (bigint or number)  decimal string, e.g. "1000"
//   blob (Uint8Array | number[])             lowercase hex string, "" if empty
//   Principal                                its text form
//   opt T ([] | [T])                         null, or the value (T is never opt)
//   unit variant ({ Flop: null })            the tag as a string, "Flop"
//   variant with payload                     { Tag: payload }, payload mapped
//   bool / text / record / vec               as JSON
// `record.fair` is not duplicated: it is `evidence`. `fromEvidenceJson` puts
// it back as `fair: [evidence]`. Hole cards are exactly what the exporting
// player saw (other players' unshown cards are empty).
import { Principal } from '@icp-sdk/core/principal';
import type { Currency, FairEvidence, HandPlayer, HandRecord, LogEntry, LogKind, Phase } from '../declarations/poker_backend/poker_backend.did';

export const EVIDENCE_FORMAT = 'bulletspoker-hand-evidence/v1';

type Bytes = Uint8Array | number[];
type Json = null | string | number | boolean | Json[] | { [k: string]: Json };

const hex = (b: Bytes) => Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
const int = (n: bigint | number) => n.toString();
const opt = <T>(o: [] | [T], f: (x: T) => Json): Json => (o.length ? f(o[0]) : null);
const unit = (v: object): string => Object.keys(v)[0];

// ---------------------------------------------------------------- encode

function logKind(k: LogKind): Json {
  if ('ShuffleCommitted' in k) return { ShuffleCommitted: { commitment: hex(k.ShuffleCommitted.commitment) } };
  return unit(k);
}

function encodeEvidence(e: FairEvidence): Json {
  return {
    burns: int(e.burns),
    canister: e.canister.toText(),
    commitment: hex(e.commitment),
    dealt_order: hex(e.dealt_order),
    hand_no: int(e.hand_no),
    jackpot_bps: opt(e.jackpot_bps, int),
    jackpot_cap: opt(e.jackpot_cap, int),
    outcome: e.outcome,
    params: hex(e.params),
    rake_bps: opt(e.rake_bps, int),
    rake_cap: opt(e.rake_cap, int),
    reveal_at: int(e.reveal_at),
    revealed_seed: opt(e.revealed_seed, hex),
    scheme: e.scheme,
    settled_at: int(e.settled_at),
    table_id: int(e.table_id),
    version: int(e.version),
  };
}

function encodeRecord(r: HandRecord): Json {
  return {
    big_blind: int(r.big_blind),
    board: hex(r.board),
    button: int(r.button),
    currency: unit(r.currency),
    hand_no: int(r.hand_no),
    id: int(r.id),
    log: r.log.map((e) => ({
      amount: int(e.amount),
      kind: opt(e.kind, logKind),
      phase: unit(e.phase),
      seat: opt(e.seat, int),
      text: e.text,
    })),
    players: r.players.map((p) => ({
      cards: hex(p.cards),
      committed: int(p.committed),
      player: p.player.toText(),
      seat: int(p.seat),
      shown: p.shown,
      username: p.username,
      won: int(p.won),
    })),
    pot: int(r.pot),
    rake: int(r.rake),
    small_blind: int(r.small_blind),
    table_id: int(r.table_id),
    table_name: r.table_name,
    time: int(r.time),
  };
}

/** Sorts object keys recursively and prints with no whitespace. */
function canonical(v: Json): string {
  if (Array.isArray(v)) return `[${v.map(canonical).join(',')}]`;
  if (v !== null && typeof v === 'object') {
    return `{${Object.keys(v)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canonical(v[k])}`)
      .join(',')}}`;
  }
  return JSON.stringify(v);
}

export function toEvidenceJson(record: HandRecord, evidence: FairEvidence): string {
  return canonical({
    canister: evidence.canister.toText(),
    evidence: encodeEvidence(evidence),
    format: EVIDENCE_FORMAT,
    record: encodeRecord(record),
  });
}

// ---------------------------------------------------------------- decode

class Bad extends Error {}

type Obj = { [k: string]: unknown };
const obj = (v: unknown, at: string): Obj => {
  if (v === null || typeof v !== 'object' || Array.isArray(v)) throw new Bad(`${at} must be an object`);
  return v as Obj;
};
const field = (o: Obj, k: string, at: string): unknown => {
  if (!(k in o)) throw new Bad(`${at}.${k} is missing`);
  return o[k];
};
const str = (v: unknown, at: string): string => {
  if (typeof v !== 'string') throw new Bad(`${at} must be a string`);
  return v;
};
const big = (v: unknown, at: string): bigint => {
  if (typeof v !== 'string' || !/^(0|[1-9][0-9]*)$/.test(v)) throw new Bad(`${at} must be a decimal integer string`);
  return BigInt(v);
};
const small = (v: unknown, at: string, max: number): number => {
  const n = big(v, at);
  if (n > BigInt(max)) throw new Bad(`${at} is out of range`);
  return Number(n);
};
const bytes = (v: unknown, at: string): Uint8Array => {
  if (typeof v !== 'string' || !/^([0-9a-f]{2})*$/.test(v)) throw new Bad(`${at} must be lowercase hex`);
  return Uint8Array.from((v.match(/../g) ?? []).map((x) => parseInt(x, 16)));
};
const bool = (v: unknown, at: string): boolean => {
  if (typeof v !== 'boolean') throw new Bad(`${at} must be true or false`);
  return v;
};
const arr = (v: unknown, at: string): unknown[] => {
  if (!Array.isArray(v)) throw new Bad(`${at} must be an array`);
  return v;
};
const optional = <T>(v: unknown, f: (x: unknown) => T): [] | [T] => (v === null ? [] : [f(v)]);
const principal = (v: unknown, at: string): Principal => {
  const text = str(v, at);
  try {
    return Principal.fromText(text);
  } catch {
    throw new Bad(`${at} is not a valid principal`);
  }
};
const tag = <T extends string>(v: unknown, at: string, allowed: readonly T[]): T => {
  if (typeof v !== 'string' || !(allowed as readonly string[]).includes(v)) throw new Bad(`${at} must be one of ${allowed.join(', ')}`);
  return v as T;
};

const PHASES = ['Flop', 'Turn', 'River', 'HandOver', 'Waiting', 'PreFlop'] as const;
const CURRENCIES = ['Icp', 'Chips'] as const;
const U32 = 0xffff_ffff;

function decodeKind(v: unknown, at: string): LogKind {
  if (typeof v === 'string') return { [tag(v, at, ['ShuffleRevealed', 'HandVerificationAvailable'])]: null } as LogKind;
  const c = obj(field(obj(v, at), 'ShuffleCommitted', at), `${at}.ShuffleCommitted`);
  return { ShuffleCommitted: { commitment: bytes(field(c, 'commitment', `${at}.ShuffleCommitted`), `${at}.ShuffleCommitted.commitment`) } };
}

function decodeEvidence(v: unknown): FairEvidence {
  const at = 'evidence';
  const o = obj(v, at);
  const f = (k: string) => field(o, k, at);
  return {
    burns: small(f('burns'), `${at}.burns`, 255),
    canister: principal(f('canister'), `${at}.canister`),
    commitment: bytes(f('commitment'), `${at}.commitment`),
    dealt_order: bytes(f('dealt_order'), `${at}.dealt_order`),
    hand_no: big(f('hand_no'), `${at}.hand_no`),
    jackpot_bps: optional(f('jackpot_bps'), (x) => small(x, `${at}.jackpot_bps`, U32)),
    jackpot_cap: optional(f('jackpot_cap'), (x) => big(x, `${at}.jackpot_cap`)),
    outcome: str(f('outcome'), `${at}.outcome`),
    params: bytes(f('params'), `${at}.params`),
    rake_bps: optional(f('rake_bps'), (x) => small(x, `${at}.rake_bps`, U32)),
    rake_cap: optional(f('rake_cap'), (x) => big(x, `${at}.rake_cap`)),
    reveal_at: big(f('reveal_at'), `${at}.reveal_at`),
    revealed_seed: optional(f('revealed_seed'), (x) => bytes(x, `${at}.revealed_seed`)),
    scheme: str(f('scheme'), `${at}.scheme`),
    settled_at: big(f('settled_at'), `${at}.settled_at`),
    table_id: big(f('table_id'), `${at}.table_id`),
    version: small(f('version'), `${at}.version`, U32),
  };
}

function decodeLog(v: unknown, i: number): LogEntry {
  const at = `record.log[${i}]`;
  const o = obj(v, at);
  const f = (k: string) => field(o, k, at);
  return {
    amount: big(f('amount'), `${at}.amount`),
    kind: optional(f('kind'), (x) => decodeKind(x, `${at}.kind`)),
    phase: { [tag(f('phase'), `${at}.phase`, PHASES)]: null } as Phase,
    seat: optional(f('seat'), (x) => small(x, `${at}.seat`, 255)),
    text: str(f('text'), `${at}.text`),
  };
}

function decodePlayer(v: unknown, i: number): HandPlayer {
  const at = `record.players[${i}]`;
  const o = obj(v, at);
  const f = (k: string) => field(o, k, at);
  return {
    cards: bytes(f('cards'), `${at}.cards`),
    committed: big(f('committed'), `${at}.committed`),
    player: principal(f('player'), `${at}.player`),
    seat: small(f('seat'), `${at}.seat`, 255),
    shown: bool(f('shown'), `${at}.shown`),
    username: str(f('username'), `${at}.username`),
    won: big(f('won'), `${at}.won`),
  };
}

function decodeRecord(v: unknown, evidence: FairEvidence): HandRecord {
  const at = 'record';
  const o = obj(v, at);
  const f = (k: string) => field(o, k, at);
  return {
    big_blind: big(f('big_blind'), `${at}.big_blind`),
    board: bytes(f('board'), `${at}.board`),
    button: small(f('button'), `${at}.button`, 255),
    currency: { [tag(f('currency'), `${at}.currency`, CURRENCIES)]: null } as Currency,
    fair: [evidence],
    hand_no: big(f('hand_no'), `${at}.hand_no`),
    id: big(f('id'), `${at}.id`),
    log: arr(f('log'), `${at}.log`).map(decodeLog),
    players: arr(f('players'), `${at}.players`).map(decodePlayer),
    pot: big(f('pot'), `${at}.pot`),
    rake: big(f('rake'), `${at}.rake`),
    small_blind: big(f('small_blind'), `${at}.small_blind`),
    table_id: big(f('table_id'), `${at}.table_id`),
    table_name: str(f('table_name'), `${at}.table_name`),
    time: big(f('time'), `${at}.time`),
  };
}

export type EvidenceFile = { record: HandRecord; evidence: FairEvidence; canister: string };

/** Reads an evidence file. Never throws: a bad file gives `{ error }`. */
export function fromEvidenceJson(text: string): EvidenceFile | { error: string } {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch (e) {
    return { error: `This is not valid JSON (${e instanceof Error ? e.message : String(e)}).` };
  }
  try {
    const top = obj(raw, 'the file');
    const format = field(top, 'format', 'the file');
    if (format !== EVIDENCE_FORMAT) {
      return { error: `Unsupported evidence format ${JSON.stringify(format)}; this verifier reads "${EVIDENCE_FORMAT}".` };
    }
    const evidence = decodeEvidence(field(top, 'evidence', 'the file'));
    const record = decodeRecord(field(top, 'record', 'the file'), evidence);
    const canister = principal(field(top, 'canister', 'the file'), 'canister').toText();
    if (canister !== evidence.canister.toText()) {
      return { error: `Invalid evidence file: canister ${canister} does not match evidence.canister ${evidence.canister.toText()}.` };
    }
    return { record, evidence, canister };
  } catch (e) {
    if (e instanceof Bad) return { error: `Invalid evidence file: ${e.message}.` };
    throw e;
  }
}
