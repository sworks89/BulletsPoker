// Independent settlement for the verifier: commitments read from the hand log,
// main and side pots, rake and jackpot drop, and who wins what. Written from
// the engine's rules (table.rs `run_out_and_showdown`, `take_rake`), checked
// against real engine hands in settlement-vectors.json.
import { bestScore } from './evaluate';

export interface Fees {
  rake_bps: number;
  rake_cap: bigint;
  jackpot_bps: number;
  jackpot_cap: bigint;
}

/** A hand-log entry as `get_hand` returns it (candid `LogEntry`). */
export interface LogLike {
  phase: object;
  seat: [] | [number];
  text: string;
  amount: bigint;
  kind: [] | [object];
}

/** Log texts that put chips in; `amount` is the seat's total bet on that street. */
const BET_TEXTS = new Set(['Small blind', 'Big blind', 'Posts big blind', 'Call', 'Bet', 'Raise', 'All-in', 'Check']);
export const ACTION_TEXTS = new Set([...BET_TEXTS, 'Fold']);
export const STREETS = ['Flop', 'Turn', 'River'] as const;

export interface LogReplay {
  /** Chips each seat put in over the hand, after any uncalled bet came back. */
  committed: Map<number, bigint>;
  folded: Set<number>;
  /** Seats that stood up after folding (their chips stay in the pot). */
  left: Set<number>;
  /** What the log says each seat won. */
  wins: Map<number, bigint>;
  /** Street lines in log order. */
  streets: string[];
}

const phaseOf = (e: LogLike) => Object.keys(e.phase)[0] ?? '';
const add = (m: Map<number, bigint>, k: number, v: bigint) => m.set(k, (m.get(k) ?? 0n) + v);

export function readLog(log: readonly LogLike[]): LogReplay {
  const streetBet = new Map<string, bigint>(); // "seat/phase" -> max total bet on that street
  const committed = new Map<number, bigint>();
  const wins = new Map<number, bigint>();
  const folded = new Set<number>();
  const left = new Set<number>();
  const streets: string[] = [];
  for (const e of log) {
    if (e.kind.length) continue; // protocol events carry no chips
    if (e.seat.length === 0) {
      if ((STREETS as readonly string[]).includes(e.text)) streets.push(e.text);
      continue;
    }
    const seat = e.seat[0];
    if (e.text === 'Fold') folded.add(seat);
    if (e.text === 'Left the table') left.add(seat);
    if (BET_TEXTS.has(e.text)) {
      const key = `${seat}/${phaseOf(e)}`;
      if (e.amount > (streetBet.get(key) ?? 0n)) streetBet.set(key, e.amount);
    } else if (e.text === 'Uncalled bet returned') {
      add(committed, seat, -e.amount);
    } else if (e.text === 'Wins (uncontested)' || e.text.startsWith('Wins with')) {
      add(wins, seat, e.amount);
    }
  }
  for (const [key, bet] of streetBet) add(committed, Number(key.split('/')[0]), bet);
  return { committed, folded, left, wins, streets };
}

/**
 * The engine hands back the part of the largest commitment nobody matched
 * (table.rs `return_uncalled`), unless that player left the table. Returns
 * the seat and the amount that should have come back, or null.
 */
export function unreturnedBet(committed: ReadonlyMap<number, bigint>, left: ReadonlySet<number>): { seat: number; excess: bigint } | null {
  const sorted = [...committed].sort((a, b) => (a[1] > b[1] ? -1 : a[1] < b[1] ? 1 : 0));
  if (sorted.length < 2) return null;
  const [[seat, top], [, second]] = sorted;
  return top > second && !left.has(seat) ? { seat, excess: top - second } : null;
}

const pct = (amount: bigint, bps: number, cap: bigint) => {
  const v = (amount * BigInt(bps)) / 10_000n;
  return v < cap ? v : cap;
};

/** Rake, then the jackpot drop from what is left. Nothing without a flop. */
export function feesFor(pot: bigint, boardLen: number, fees: Fees): { rake: bigint; drop: bigint } {
  if (boardLen < 3) return { rake: 0n, drop: 0n };
  const rake = pct(pot, fees.rake_bps, fees.rake_cap);
  return { rake, drop: pct(pot - rake, fees.jackpot_bps, fees.jackpot_cap) };
}

export interface SettlePlayer {
  seat: number;
  committed: bigint;
  folded: boolean;
  /** Needed for every live player when two or more reach a showdown. */
  hole?: readonly number[];
}

export interface SettleInput {
  players: readonly SettlePlayer[];
  board: readonly number[];
  button: number;
  /** The table's rates, or rake and drop amounts taken as given. */
  take: { fees: Fees } | { rake: bigint; drop: bigint };
}

export interface Pot {
  amount: bigint;
  eligible: number[];
  winners: number[];
}

export interface Settlement {
  pot: bigint;
  rake: bigint;
  drop: bigint;
  won: Map<number, bigint>;
  pots: Pot[];
}

/** Seats from the one left of the button round to the button (odd chips go first). */
export const leftOfButton = (button: number) => (a: number, b: number) => {
  const key = (s: number) => (s > button ? s - button : s + 256 - button);
  return key(a) - key(b);
};

export function settle({ players, board, button, take }: SettleInput): Settlement {
  const pot = players.reduce((n, p) => n + p.committed, 0n);
  const { rake, drop } = 'fees' in take ? feesFor(pot, board.length, take.fees) : take;
  const won = new Map<number, bigint>(players.map((p) => [p.seat, 0n]));
  const live = players.filter((p) => !p.folded);
  if (live.length === 0) throw new Error('no player is left in the hand');

  if (live.length === 1) {
    const w = live[0].seat;
    won.set(w, pot - rake - drop);
    return { pot, rake, drop, won, pots: [{ amount: pot - rake - drop, eligible: [w], winners: [w] }] };
  }

  // Main and side pots at each live player's commitment level; folded chips count up to each level.
  const levels = [...new Set(live.map((p) => p.committed))].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  const pots: Pot[] = [];
  let prev = 0n;
  const min = (a: bigint, b: bigint) => (a < b ? a : b);
  for (const level of levels) {
    const amount = players.reduce((n, p) => n + min(p.committed, level) - min(p.committed, prev), 0n);
    if (amount > 0n) pots.push({ amount, eligible: live.filter((p) => p.committed >= level).map((p) => p.seat), winners: [] });
    prev = level;
  }
  // Chips above every live level (from players who folded or left) join the last pot.
  const assigned = pots.reduce((n, p) => n + p.amount, 0n);
  if (pots.length) pots[pots.length - 1].amount += pot - assigned;

  // Rake and drop come out of each pot in proportion; rounding leftovers from the largest pots.
  const takeAll = rake + drop;
  const potSum = pots.reduce((n, p) => n + p.amount, 0n);
  let left = takeAll;
  if (potSum > 0n) {
    for (const p of pots) {
      const r = (takeAll * p.amount) / potSum;
      p.amount -= r;
      left -= r;
    }
  }
  const order = pots.map((_, i) => i).sort((a, b) => (pots[a].amount > pots[b].amount ? -1 : pots[a].amount < pots[b].amount ? 1 : a - b));
  for (const i of order) {
    const r = min(left, pots[i].amount);
    pots[i].amount -= r;
    left -= r;
  }

  const score = new Map<number, number>();
  for (const p of live) {
    if (!p.hole || p.hole.length !== 2 || board.length !== 5) {
      throw new Error(`seat ${p.seat}: a showdown needs two hole cards and five board cards`);
    }
    score.set(p.seat, bestScore([...p.hole, ...board]));
  }
  for (const p of pots) {
    const best = Math.max(...p.eligible.map((s) => score.get(s)!));
    p.winners = p.eligible.filter((s) => score.get(s) === best).sort(leftOfButton(button));
    const n = BigInt(p.winners.length);
    let odd = p.amount % n;
    for (const w of p.winners) {
      const extra = odd > 0n ? 1n : 0n;
      odd -= extra;
      won.set(w, won.get(w)! + p.amount / n + extra);
    }
  }
  return { pot, rake, drop, won, pots };
}
