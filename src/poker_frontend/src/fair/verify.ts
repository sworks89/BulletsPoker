// BP-FAIR-v1 hand verifier. It checks a hand from the published record and
// evidence alone. It rebuilds the deck and the settlement itself, and never
// relies on a result computed by the canister.
//
// Bases:
// - cryptographic: commitment, shuffle, dealing. These follow from SHA-256 /
//   HKDF / HMAC over the revealed seed.
// - consistency: replay, settlement. The log, the cards and the payouts agree
//   with each other under the engine's rules.
// - trusts-canister: where the seed came from, and that nobody saw the cards
//   during play. Listed in `trusted`, never reported as passed.
import { cardName } from './evaluate';
import { SHUFFLE_VERSION, commitment as commit, handIdBytes, paramsBytes, shuffleDeck } from './protocol';
import { ACTION_TEXTS, STREETS, feesFor, leftOfButton, readLog, settle, unreturnedBet, type Fees, type LogLike } from './settle';

type Bytes = Uint8Array | number[];

export interface FairEvidenceLike {
  version: number;
  scheme: string;
  canister: { toText(): string } | string;
  table_id: bigint;
  hand_no: bigint;
  commitment: Bytes;
  revealed_seed: [] | [Bytes];
  params: Bytes;
  outcome: string;
  dealt_order: Bytes;
  burns: number;
  settled_at: bigint;
  reveal_at: bigint;
  /** The table's rates when the hand was dealt; absent on older evidence. */
  rake_bps?: [] | [number];
  rake_cap?: [] | [bigint];
  jackpot_bps?: [] | [number];
  jackpot_cap?: [] | [bigint];
}

export interface HandRecordLike {
  table_id: bigint;
  hand_no: bigint;
  button: number;
  board: Bytes;
  pot: bigint;
  rake: bigint;
  players: readonly { seat: number; username: string; cards: Bytes; committed: bigint; won: bigint }[];
  log: readonly LogLike[];
}

export type Overall = 'VERIFIED' | 'FAILED' | 'INCOMPLETE' | 'LEGACY' | 'UNSUPPORTED';
export type Basis = 'cryptographic' | 'consistency' | 'trusts-canister';

export interface CheckResult {
  status: 'pass' | 'fail' | 'skipped';
  basis: Basis;
  detail: string;
}

export interface VerificationReport {
  overall: Overall;
  version: number | null;
  checks: { commitment: CheckResult; shuffle: CheckResult; dealing: CheckResult; replay: CheckResult; settlement: CheckResult };
  /** What this report does not check and takes on trust from the canister. */
  trusted: { what: string; basis: 'trusts-canister'; detail: string }[];
  errors: string[];
  warnings: string[];
  /** When the seed is published (ns since the epoch), if it is still pending. */
  revealAt?: bigint;
}

export interface VerifyOptions {
  /** The current time in ns since the epoch (default: the browser clock). */
  now?: bigint;
  /**
   * The commitment this device saw at the table while the hand was played
   * (fair/seen.ts). When given, the published commitment must equal it.
   */
  observedCommitment?: Uint8Array;
}

/** The rates published with the evidence, if this build recorded them. */
function ratesOf(e: FairEvidenceLike): Fees | null {
  const one = <T>(x: [] | [T] | undefined): T | undefined => (x && x.length ? x[0] : undefined);
  const [rake_bps, rake_cap, jackpot_bps, jackpot_cap] = [one(e.rake_bps), one(e.rake_cap), one(e.jackpot_bps), one(e.jackpot_cap)];
  if (rake_bps === undefined || rake_cap === undefined || jackpot_bps === undefined || jackpot_cap === undefined) return null;
  return { rake_bps, rake_cap, jackpot_bps, jackpot_cap };
}

type CheckName = keyof VerificationReport['checks'];

const SCHEME = 'icp-raw_rand/v1';
/** The protocol's reveal delay: reveal_at = settled_at + 24 h (REVEAL_DELAY_NS in table.rs). */
const REVEAL_DELAY_NS = 24n * 3_600_000_000_000n;
const MAX_RAKE_BPS = 1000; // TableConfig::validate
const MAX_JACKPOT_BPS = 300;

const hex = (b: Bytes) => Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
const same = (a: Bytes, b: Bytes) => a.length === b.length && Array.from(a).every((x, i) => x === b[i]);
const isoNs = (ns: bigint) => new Date(Number(ns / 1_000_000n)).toISOString();
const sum = (xs: Iterable<bigint>) => [...xs].reduce((n, x) => n + x, 0n);
const STREET_OF_BOARD = ['flop', 'flop', 'flop', 'turn', 'river'];

export async function verifyHand(
  record: HandRecordLike,
  evidence: FairEvidenceLike | null,
  canisterId: string,
  options: VerifyOptions = {},
): Promise<VerificationReport> {
  const skipped = (basis: Basis, detail: string): CheckResult => ({ status: 'skipped', basis, detail });
  const report: VerificationReport = {
    overall: 'INCOMPLETE',
    version: evidence ? evidence.version : null,
    checks: {
      commitment: skipped('cryptographic', 'not checked'),
      shuffle: skipped('cryptographic', 'not checked'),
      dealing: skipped('cryptographic', 'not checked'),
      replay: skipped('consistency', 'not checked'),
      settlement: skipped('consistency', 'not checked'),
    },
    trusted: [
      {
        what: 'seed origin',
        basis: 'trusts-canister',
        detail: `The seed is said to come from the management canister's raw_rand (${SCHEME}). The verifier can only check the seed against the commitment, not where it came from.`,
      },
      {
        what: 'card confidentiality during the hand',
        basis: 'trusts-canister',
        detail: 'The canister held the seed and the deck while the hand was played. Checking after the hand cannot show that nobody saw the cards.',
      },
    ],
    errors: [],
    warnings: [],
  };

  if (!evidence) {
    report.overall = 'LEGACY';
    for (const k of Object.keys(report.checks) as CheckName[]) {
      report.checks[k].detail = 'Legacy hand: dealt before the shuffle commitment, so it cannot be verified.';
    }
    return report;
  }
  if (evidence.version !== SHUFFLE_VERSION) {
    report.overall = 'UNSUPPORTED';
    report.errors.push(`Shuffle version ${evidence.version} is not supported by this verifier (it knows version ${SHUFFLE_VERSION}).`);
    return report;
  }

  const missing: string[] = []; // reasons the result is incomplete
  const problems: Record<CheckName, string[]> = { commitment: [], shuffle: [], dealing: [], replay: [], settlement: [] };
  const fail = (k: CheckName, msg: string) => {
    problems[k].push(msg);
    report.errors.push(`${k}: ${msg}`);
  };
  const set = (k: CheckName, ok: string, skip?: string) => {
    const c = report.checks[k];
    if (problems[k].length) Object.assign(c, { status: 'fail', detail: problems[k].join('; ') });
    else if (skip) Object.assign(c, { status: 'skipped', detail: skip });
    else Object.assign(c, { status: 'pass', detail: ok });
  };
  const players = [...record.players].sort((a, b) => a.seat - b.seat);
  const who = (seat: number) => {
    const p = players.find((x) => x.seat === seat);
    return p ? `seat ${seat} (${p.username})` : `seat ${seat}`;
  };
  const board = Array.from(record.board);
  const voided = evidence.outcome !== 'settled';

  // ---- commitment: bound to this hand, logged first, and opened by the seed
  const canister = typeof evidence.canister === 'string' ? evidence.canister : evidence.canister.toText();
  if (canister !== canisterId) fail('commitment', `the evidence names canister ${canister}, but this hand came from ${canisterId}`);
  if (evidence.table_id !== record.table_id) fail('commitment', `evidence table_id ${evidence.table_id}, record ${record.table_id}`);
  if (evidence.hand_no !== record.hand_no) fail('commitment', `evidence hand_no ${evidence.hand_no}, record ${record.hand_no}`);
  if (!same(evidence.params, paramsBytes(evidence.version))) fail('commitment', `params are not params_bytes(${evidence.version})`);
  if (evidence.scheme !== SCHEME) report.warnings.push(`Unexpected randomness scheme "${evidence.scheme}" (expected ${SCHEME}).`);
  const committedAt = record.log.flatMap((e, i) => (e.kind.length && 'ShuffleCommitted' in e.kind[0] ? [i] : []));
  if (committedAt.length === 0) {
    missing.push('the log has no ShuffleCommitted event, so the verifier cannot confirm the commitment came before the deal');
  } else {
    if (committedAt.length > 1) fail('commitment', `the log has ${committedAt.length} ShuffleCommitted events (entries ${committedAt.join(', ')})`);
    if (committedAt[0] !== 0) fail('commitment', `ShuffleCommitted is log entry ${committedAt[0]}, after the hand started; it must be entry 0`);
    const logged = (record.log[committedAt[0]].kind[0] as { ShuffleCommitted: { commitment: Bytes } }).ShuffleCommitted.commitment;
    if (!same(logged, evidence.commitment)) {
      fail('commitment', `the log's ShuffleCommitted ${hex(logged)} differs from the evidence commitment ${hex(evidence.commitment)}`);
    }
  }
  const observed = options.observedCommitment;
  if (observed && !same(observed, evidence.commitment)) {
    fail('commitment', `the commitment shown at the table differs from the published one: the table showed ${hex(observed)}, the evidence has ${hex(evidence.commitment)}`);
  }
  if (!observed) {
    report.warnings.push("No commitment was recorded on this device for this hand, so its pre-deal timing relies on the canister's log.");
    report.trusted.push({
      what: 'commitment timing',
      basis: 'trusts-canister',
      detail:
        "This device did not see the commitment at the table during the hand. That it was fixed before the cards were dealt rests on the canister's own log, which puts it at entry 0; the log is not externally timestamped.",
    });
  }
  if (evidence.reveal_at !== evidence.settled_at + REVEAL_DELAY_NS) {
    report.warnings.push(
      `reveal_at is not settled_at + 24 h (the protocol's reveal delay): settled_at ${isoNs(evidence.settled_at)}, reveal_at ${isoNs(evidence.reveal_at)}.`,
    );
  }
  const seed = evidence.revealed_seed.length ? Uint8Array.from(evidence.revealed_seed[0]) : null;
  let deck: number[] | null = null;
  const now = options.now ?? BigInt(Date.now()) * 1_000_000n;
  const pending = !seed && now < evidence.reveal_at;
  if (pending) {
    report.revealAt = evidence.reveal_at;
    report.warnings.push(`Pending reveal: the seed is published at ${isoNs(evidence.reveal_at)}. Check again after that.`);
  } else if (!seed) {
    missing.push(`seed not published although reveal_at has passed (${isoNs(evidence.reveal_at)})`);
  } else if (seed.length !== 32) {
    fail('commitment', `the revealed seed is ${seed.length} bytes, not 32`);
  } else {
    const hid = handIdBytes(canister, evidence.table_id, evidence.hand_no);
    const got = await commit(seed, hid, evidence.version);
    if (!same(got, evidence.commitment)) {
      fail('commitment', `SHA-256 of the revealed seed and hand id gives ${hex(got)}, but the published commitment is ${hex(evidence.commitment)}`);
    }
    deck = await shuffleDeck(seed, hid, evidence.version);
  }
  set(
    'commitment',
    observed
      ? 'The revealed seed opens the commitment for this canister, table and hand. The commitment matched what you saw at the table before the flop was dealt, so the deck was fixed by then; the log records it as event 0, before any card was dealt.'
      : "The revealed seed opens the commitment for this canister, table and hand. The log records it as event 0, before any card was dealt; that order is the canister's own log (see Taken on trust), because this device did not see the commitment at the table.",
    pending ? 'Seed not revealed yet.' : !seed ? 'Seed not published.' : committedAt.length === 0 ? 'The seed opens the commitment, but the log has no ShuffleCommitted event.' : undefined,
  );

  // ---- shuffle
  set('shuffle', 'Rebuilt the 52-card deck from the seed (HKDF-SHA256 key, HMAC-SHA256 stream, Fisher-Yates).', deck ? undefined : 'Needs the revealed seed.');

  // ---- dealing: pop from the end, hole cards in dealt_order, burn before each street
  const dealtOrder = Array.from(evidence.dealt_order);
  const holes = new Map<number, number[]>();
  const expectBurns = board.length === 0 ? 0 : board.length <= 3 ? 1 : board.length - 2;
  if (![0, 3, 4, 5].includes(board.length)) fail('dealing', `the board has ${board.length} cards`);
  if (evidence.burns !== expectBurns) fail('dealing', `evidence says ${evidence.burns} burns, a ${board.length}-card board needs ${expectBurns}`);
  if (dealtOrder.length) {
    const round = players.map((p) => p.seat).sort(leftOfButton(record.button));
    const want = [...round, ...round];
    if (!same(dealtOrder, want)) {
      fail('dealing', `dealt_order [${dealtOrder.join(', ')}] is not the dealt seats twice from left of button ${record.button}: expected [${want.join(', ')}]`);
    }
  } else {
    missing.push('dealt_order is empty (hand dealt by an older build), so hole cards cannot be matched to seats');
    report.warnings.push('dealt_order is empty: hole cards were not checked; the board was.');
  }
  if (deck) {
    const d = [...deck];
    if (dealtOrder.length) {
      for (const seat of dealtOrder) holes.set(seat, [...(holes.get(seat) ?? []), d.pop()!]);
    } else {
      d.splice(d.length - 2 * players.length);
    }
    const rebuilt: number[] = [];
    for (let street = 0; street < expectBurns; street++) {
      d.pop(); // burn
      for (let k = 0; k < (street === 0 ? 3 : 1); k++) rebuilt.push(d.pop()!);
    }
    board.forEach((c, i) => {
      if (rebuilt[i] !== c) {
        fail('dealing', `board card ${i + 1} (${STREET_OF_BOARD[i]}): expected ${rebuilt[i] === undefined ? 'none' : cardName(rebuilt[i])}, record ${cardName(c)}`);
      }
    });
    if (dealtOrder.length) {
      for (const p of players) {
        const cards = Array.from(p.cards);
        if (cards.length === 0) continue; // hidden from this viewer
        const want = holes.get(p.seat) ?? [];
        if (cards.length !== want.length) fail('dealing', `${who(p.seat)} has ${cards.length} hole cards, ${want.length} were dealt`);
        cards.forEach((c, i) => {
          if (want[i] !== undefined && want[i] !== c) {
            fail('dealing', `${who(p.seat)} hole card ${i + 1}: expected ${cardName(want[i])}, record ${cardName(c)}`);
          }
        });
      }
    }
  }
  const shownHoles = players.filter((p) => p.cards.length > 0).length;
  set(
    'dealing',
    `Every visible card matches the rebuilt deck: ${board.length} board cards and ${shownHoles} players' hole cards. The dealt_order check relies on the record's button and player list.`,
    !deck ? 'Needs the revealed seed.' : dealtOrder.length === 0 ? `Board checked (${board.length} cards); hole cards skipped (no dealt_order).` : undefined,
  );

  // ---- replay: the log agrees with the board, the folds and all-ins, and the record
  const replay = readLog(record.log);
  const seats = new Set(players.map((p) => p.seat));
  const wantStreets = STREETS.slice(0, expectBurns);
  if (replay.streets.join(',') !== wantStreets.join(',')) {
    fail('replay', `the log deals streets [${replay.streets.join(', ')}], but a ${board.length}-card board needs [${wantStreets.join(', ')}]`);
  }
  const PHASES = ['PreFlop', 'Flop', 'Turn', 'River', 'HandOver'];
  let lastPhase = 0;
  const out = new Map<number, string>(); // seat -> why it can't act any more
  record.log.forEach((e, i) => {
    const ph = PHASES.indexOf(Object.keys(e.phase)[0] ?? '');
    if (ph >= 0) {
      if (ph < lastPhase) fail('replay', `log entry ${i} (${e.text}) goes back to ${PHASES[ph]} after ${PHASES[lastPhase]}`);
      lastPhase = Math.max(lastPhase, ph);
    }
    if (e.seat.length === 0 || e.kind.length) return;
    const seat = e.seat[0];
    if (!seats.has(seat)) {
      fail('replay', `log entry ${i} (${e.text}) is by seat ${seat}, which was not dealt in`);
      return;
    }
    if (ACTION_TEXTS.has(e.text) && !e.text.endsWith('blind')) {
      if (out.has(seat)) fail('replay', `log entry ${i}: ${who(seat)} acts (${e.text}) after ${out.get(seat)}`);
      if (e.text === 'Fold') out.set(seat, 'folding');
      if (e.text === 'All-in') out.set(seat, 'going all-in');
    }
  });
  const live = players.filter((p) => !replay.folded.has(p.seat));
  if (voided) {
    report.warnings.push(`Hand ${evidence.outcome}: it never finished, so nothing was settled.`);
  } else {
    for (const p of players) {
      const c = replay.committed.get(p.seat) ?? 0n;
      if (c !== p.committed) fail('replay', `${who(p.seat)} committed: log says ${c}, record ${p.committed}`);
      const w = replay.wins.get(p.seat) ?? 0n;
      if (w !== p.won) fail('replay', `${who(p.seat)} won: log says ${w}, record ${p.won}`);
    }
    const logPot = sum(replay.committed.values());
    if (logPot !== record.pot) fail('replay', `pot: log commitments add up to ${logPot}, record ${record.pot}`);
    if (live.length === 0) fail('replay', 'every player folded');
    if (live.length >= 2 && board.length !== 5) fail('replay', `${live.length} players reached the end but the board has ${board.length} cards`);
    if (live.length === 1 && !record.log.some((e) => e.text === 'Wins (uncontested)' && e.seat[0] === live[0].seat)) {
      fail('replay', `${who(live[0].seat)} is the only player left but the log has no uncontested win for them`);
    }
  }
  set('replay', `The log's ${record.log.length} entries agree with the board, the folds and all-ins, and the recorded commitments and payouts.`);

  // ---- settlement: pots and winners from the log's commitments and the rebuilt cards
  const rates = ratesOf(evidence);
  if (voided) {
    // A voided hand pays nothing: what was put in goes back to the players.
    if (record.pot !== 0n) fail('settlement', `the hand was voided but the record has a pot of ${record.pot}`);
    if (record.rake !== 0n) fail('settlement', `the hand was voided but the record takes rake ${record.rake}`);
    for (const p of players) if (p.won !== 0n) fail('settlement', `the hand was voided but ${who(p.seat)} won ${p.won}`);
    set('settlement', '', 'The hand was voided: nothing was paid out (pot, rake and payouts are all 0).');
  } else {
    const committed = new Map(players.map((p) => [p.seat, replay.committed.get(p.seat) ?? 0n] as [number, bigint]));
    const pot = sum(committed.values());
    const paid = sum(players.map((p) => p.won));
    const rake = record.rake;
    const unreturned = unreturnedBet(committed, replay.left);
    if (unreturned) {
      fail(
        'settlement',
        `${who(unreturned.seat)} has ${unreturned.excess} more in than anyone else and no "Left the table" line: expected ${unreturned.excess} returned as an uncalled bet`,
      );
    }
    let drop: bigint;
    let takeNote: string;
    if (rates) {
      const want = feesFor(pot, board.length, rates);
      if (want.rake !== rake) fail('settlement', `rake: expected ${want.rake}, record ${rake}`);
      drop = want.drop;
      if (paid + want.rake + drop !== pot) {
        fail('settlement', `payouts ${paid} + rake ${want.rake} + jackpot drop ${drop} = ${paid + want.rake + drop}, but the pot is ${pot}`);
      }
      takeNote =
        `Rake ${want.rake} and jackpot drop ${drop} recomputed from the rates recorded at the deal ` +
        `(rake ${rates.rake_bps} bps, cap ${rates.rake_cap}; jackpot ${rates.jackpot_bps} bps, cap ${rates.jackpot_cap}).`;
    } else {
      drop = pot - rake - paid;
      report.trusted.push({
        what: 'rake and jackpot drop amounts',
        basis: 'trusts-canister',
        detail: `This evidence predates recorded fee rates. The rake (${rake}) is taken from the record, and the drop (${drop}) is inferred as pot − rake − payouts. Only the no-flop rule and the global limits (rake ≤ 10%, drop ≤ 3%) are checked.`,
      });
      report.warnings.push('No fee rates in this evidence: rake and jackpot drop were taken from the record, not recomputed.');
      if (drop < 0n) fail('settlement', `payouts ${paid} + rake ${rake} exceed the pot ${pot}`);
      if (rake > (pot * BigInt(MAX_RAKE_BPS)) / 10_000n) fail('settlement', `rake ${rake} is over 10% of the pot ${pot}`);
      if (drop > ((pot - rake) * BigInt(MAX_JACKPOT_BPS)) / 10_000n) fail('settlement', `inferred jackpot drop ${drop} is over 3% of ${pot - rake}`);
      takeNote = `Rake ${rake} and jackpot drop ${drop} were taken from the record (this evidence has no fee rates), not recomputed.`;
    }
    if (board.length < 3) {
      if (rake !== 0n) fail('settlement', `rake ${rake} was taken with no flop dealt`);
      if (drop !== 0n) fail('settlement', `jackpot drop ${drop} was taken with no flop dealt`);
    }
    let skip: string | undefined;
    if (live.length >= 2 && (!deck || dealtOrder.length === 0)) {
      skip = !deck ? 'Pot totals checked; the winners need the revealed seed.' : 'Pot totals checked; the winners need dealt_order to place the hole cards.';
    } else if (live.length >= 1 && drop >= 0n) {
      try {
        const r = settle({
          players: players.map((p) => ({ seat: p.seat, committed: committed.get(p.seat)!, folded: replay.folded.has(p.seat), hole: holes.get(p.seat) })),
          board,
          button: record.button,
          take: rates ? { fees: rates } : { rake, drop },
        });
        for (const p of players) {
          const want = r.won.get(p.seat) ?? 0n;
          if (want !== p.won) fail('settlement', `${who(p.seat)} won: expected ${want}, record ${p.won}`);
        }
      } catch (e) {
        fail('settlement', `cannot settle this hand: ${e instanceof Error ? e.message : String(e)}`);
      }
    }
    set(
      'settlement',
      `Recomputed ${live.length >= 2 ? 'the main and side pots and the winners from the rebuilt cards' : 'the uncontested pot'}: every payout matches (pot ${pot}). ${takeNote}`,
      skip,
    );
  }

  // ---- overall: a pending reveal first; then a proven fail always wins
  const statuses = Object.values(report.checks).map((c) => c.status);
  report.warnings.push(...missing.map((m) => `Incomplete: ${m}.`));
  if (pending) {
    report.overall = 'INCOMPLETE';
  } else if (statuses.includes('fail')) {
    report.overall = 'FAILED';
  } else if (missing.length || statuses.includes('skipped')) {
    report.overall = 'INCOMPLETE';
  } else {
    report.overall = 'VERIFIED';
  }
  return report;
}
