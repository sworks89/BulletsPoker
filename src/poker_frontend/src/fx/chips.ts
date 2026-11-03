// Chips on the felt: which chips show for an amount, where a seat's bet sits,
// and what moves between two views of a table. Like the cards, only changes
// seen while watching move: the first view shows the table as it is.

/** Chip values in units of a fifth of the big blind, smallest first. */
const TIERS = [1n, 5n, 25n, 100n, 500n, 2500n];
const MAX_PER_COLUMN = 8;
const MAX_COLUMNS = 4;

/** Chip colors by tier: white, red, green, black, purple, orange. */
export const CHIP_COLORS = ['#e5e7eb', '#dc2626', '#15803d', '#232323', '#7c3aed', '#ea580c'];

export interface ChipColumn {
  tier: number;
  n: number;
}

/**
 * The stacks to draw for an amount, largest value first. Values follow the big
 * blind, so a bet looks the same at any stakes; anything below the smallest
 * chip shows as one more smallest chip.
 */
export function chipColumns(amount: bigint, bigBlind: bigint): ChipColumn[] {
  if (amount <= 0n) return [];
  const unit = bigBlind / 5n > 0n ? bigBlind / 5n : 1n;
  const out: ChipColumn[] = [];
  let left = amount;
  for (let tier = TIERS.length - 1; tier >= 0; tier--) {
    const value = TIERS[tier] * unit;
    const n = left / value;
    if (n > 0n) {
      out.push({ tier, n: Number(n > BigInt(MAX_PER_COLUMN) ? BigInt(MAX_PER_COLUMN) : n) });
      left -= n * value;
    }
  }
  if (left > 0n) {
    const smallest = out.find((c) => c.tier === 0);
    if (smallest) smallest.n = Math.min(MAX_PER_COLUMN, smallest.n + 1);
    else out.push({ tier: 0, n: 1 });
  }
  return out.slice(0, MAX_COLUMNS);
}

/** How far from the middle toward its seat a bet sits, across and down. */
const BET_REACH_X = 0.6;
const BET_REACH_Y = 0.5;
/** On a narrow screen, how far below its seat's center a bet sits (px). */
export const NARROW_BET_DY = 72;

/**
 * Where a seat's bet sits: `left` and `top` in % of the table, plus `dy` px.
 * The bottom seat is the viewer's, whose full-size cards fill the space toward
 * the middle, so its bet sits beside them. On a narrow screen the seats fill
 * the table, so a bet sits just below its seat.
 */
export function betSpot(seat: { left: number; top: number }, narrow: boolean): { left: number; top: number; dy: number } {
  if (narrow) return { ...seat, dy: NARROW_BET_DY };
  if (seat.top >= 85) return { left: seat.left + 13, top: seat.top - 13, dy: 0 };
  return { left: 50 + (seat.left - 50) * BET_REACH_X, top: 50 + (seat.top - 50) * BET_REACH_Y, dy: 0 };
}

/** The parts of a table view that chip motion depends on. */
export interface ChipSnap {
  hand: bigint;
  /** Each seat's bet on this street. */
  bets: Map<number, bigint>;
  /** Chips already in the pot, not counting this street's bets. */
  collected: bigint;
  /** The hand is over and paid out. */
  over: boolean;
  winners: { seat: number; amount: bigint }[];
}

export interface ChipMotion {
  /** Chips going from a seat to its bet; `onto` when a bet was already there. */
  toss: { seat: number; amount: bigint; onto: boolean }[];
  /** Bets going into the pot. */
  sweep: { seat: number; amount: bigint }[];
  /** The pot going to its winners. */
  push: { seat: number; amount: bigint }[];
}

const bySeat = (m: Map<number, bigint>) => [...m].filter(([, a]) => a > 0n).sort((a, b) => a[0] - b[0]);

export function chipMotionBetween(prev: ChipSnap | null, next: ChipSnap): ChipMotion {
  const none: ChipMotion = { toss: [], sweep: [], push: [] };
  if (!prev) return none;
  if (next.hand !== prev.hand) {
    if (next.over) return none;
    return { ...none, toss: bySeat(next.bets).map(([seat, amount]) => ({ seat, amount, onto: false })) };
  }
  const swept = next.collected > prev.collected;
  const toss: ChipMotion['toss'] = [];
  for (const [seat, bet] of bySeat(next.bets)) {
    const before = swept ? 0n : (prev.bets.get(seat) ?? 0n);
    if (bet > before) toss.push({ seat, amount: bet - before, onto: before > 0n });
  }
  return {
    toss,
    sweep: swept ? bySeat(prev.bets).map(([seat, amount]) => ({ seat, amount })) : [],
    push: !prev.over && next.over ? next.winners.map((w) => ({ seat: w.seat, amount: w.amount })) : [],
  };
}
