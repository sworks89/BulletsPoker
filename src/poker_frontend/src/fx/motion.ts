// What the table should animate between two views of it. Only changes seen
// while watching animate: the first view (opening a table, mid-hand or not)
// shows the table as it is.

import { bestFive } from './poker';

/** The parts of a table view that card motion depends on. */
export interface MotionSnap {
  hand: bigint;
  /** Board cards out. */
  board: number;
  /** Seats holding cards (face up or down). */
  dealt: number[];
  /** Seats whose cards this viewer can see. */
  faceUp: number[];
}

export interface Motion {
  /** Seats dealt in by a new hand. */
  deal: number[];
  /** Board cards that just came out, by index. */
  board: number[];
  /** Seats whose cards were just turned face up. */
  reveal: number[];
}

const range = (from: number, to: number) => Array.from({ length: Math.max(0, to - from) }, (_, i) => from + i);

export function motionBetween(prev: MotionSnap | null, next: MotionSnap): Motion {
  if (!prev) return { deal: [], board: [], reveal: [] };
  if (next.hand !== prev.hand) return { deal: [...next.dealt], board: range(0, next.board), reveal: [] };
  return {
    deal: [],
    board: range(prev.board, next.board),
    reveal: next.faceUp.filter((s) => !prev.faceUp.includes(s)),
  };
}

/** The order hole cards go out: [seat, card] from the seat left of the button, twice round. */
export function dealOrder(seats: number[], button: number, seatCount: number): [number, number][] {
  const after = (s: number) => (s - button - 1 + seatCount) % seatCount;
  const round = [...seats].sort((a, b) => after(a) - after(b));
  return [0, 1].flatMap((k) => round.map((s) => [s, k] as [number, number]));
}

export interface ShowdownInput {
  board: number[];
  shown: { seat: number; cards: number[] }[];
  winners: { seat: number }[];
}

/** The winning cards at a showdown: board indices, and per seat the hole card indices. */
export interface Highlight {
  board: Set<number>;
  seats: Map<number, Set<number>>;
}

/** Which cards made the winning hands, or null without a showdown to show. */
export function showdownHighlight({ board, shown, winners }: ShowdownInput): Highlight | null {
  const out: Highlight = { board: new Set(), seats: new Map() };
  let any = false;
  for (const w of winners) {
    const hand = shown.find((s) => s.seat === w.seat);
    if (!hand) continue;
    const five = bestFive([...hand.cards, ...board]);
    if (five.length === 0) continue;
    any = true;
    board.forEach((c, i) => five.includes(c) && out.board.add(i));
    const mine = out.seats.get(w.seat) ?? new Set<number>();
    hand.cards.forEach((c, k) => five.includes(c) && mine.add(k));
    out.seats.set(w.seat, mine);
  }
  return any ? out : null;
}
