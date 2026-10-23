// Independent hand evaluator for the verifier. It does not port the engine's
// 5-card combination search (cards.rs `best_hand`): it reads the best hand
// straight from 5 to 7 cards by rank and suit counts. Only the ordering of
// scores matters; equal scores split a pot.
//
// A card is 0..51: rank = card / 4 (0 = 2 .. 12 = A), suit = card % 4 (c, d, h, s).

const RANKS = '23456789TJQKA';
const SUITS = 'cdhs';

export const CATEGORIES = [
  'High Card',
  'Pair',
  'Two Pair',
  'Three of a Kind',
  'Straight',
  'Flush',
  'Full House',
  'Four of a Kind',
  'Straight Flush',
] as const;

export function cardName(c: number): string {
  return `${RANKS[Math.floor(c / 4)]}${SUITS[c % 4]}`;
}

export function parseCards(s: string): number[] {
  return s
    .split(/\s+/)
    .filter(Boolean)
    .map((t) => RANKS.indexOf(t[0]) * 4 + SUITS.indexOf(t[1]));
}

/** category in the top bits, then up to five ranks, most significant first. */
function pack(cat: number, ranks: number[]): number {
  let s = cat * 2 ** 20;
  ranks.slice(0, 5).forEach((r, i) => (s += r * 16 ** (4 - i)));
  return s;
}

export function categoryOf(score: number): string {
  return CATEGORIES[Math.floor(score / 2 ** 20)];
}

/** Highest straight in a set of ranks (the ace also plays low), or -1. */
function straightHigh(has: (r: number) => boolean): number {
  for (let high = 12; high >= 3; high--) {
    let ok = true;
    for (let k = 0; k < 5 && ok; k++) {
      const r = high - k;
      ok = has(r < 0 ? 12 : r);
    }
    if (ok) return high;
  }
  return -1;
}

/** Best five-card score among 5 to 7 cards. */
export function bestScore(cards: readonly number[]): number {
  if (cards.length < 5 || cards.length > 7) throw new Error(`need 5 to 7 cards, got ${cards.length}`);
  const count = new Array<number>(13).fill(0);
  const bySuit: number[][] = [[], [], [], []];
  for (const c of cards) {
    count[Math.floor(c / 4)]++;
    bySuit[c % 4].push(Math.floor(c / 4));
  }
  const desc = (a: number, b: number) => b - a;
  const flushRanks = bySuit.find((s) => s.length >= 5)?.sort(desc);
  if (flushRanks) {
    const sf = straightHigh((r) => flushRanks.includes(r));
    if (sf >= 0) return pack(8, [sf]);
  }
  const ranksWith = (n: number) => [...Array(13).keys()].filter((r) => count[r] >= n).sort(desc);
  const kickers = (exclude: number[], n: number) =>
    [...Array(13).keys()].filter((r) => count[r] > 0 && !exclude.includes(r)).sort(desc).slice(0, n);

  const quads = ranksWith(4);
  if (quads.length) return pack(7, [quads[0], ...kickers([quads[0]], 1)]);
  const trips = ranksWith(3);
  if (trips.length) {
    const pair = ranksWith(2).filter((r) => r !== trips[0]);
    if (pair.length) return pack(6, [trips[0], pair[0]]);
  }
  if (flushRanks) return pack(5, flushRanks.slice(0, 5));
  const st = straightHigh((r) => count[r] > 0);
  if (st >= 0) return pack(4, [st]);
  if (trips.length) return pack(3, [trips[0], ...kickers([trips[0]], 2)]);
  const pairs = ranksWith(2);
  if (pairs.length >= 2) return pack(2, [pairs[0], pairs[1], ...kickers([pairs[0], pairs[1]], 1)]);
  if (pairs.length === 1) return pack(1, [pairs[0], ...kickers([pairs[0]], 3)]);
  return pack(0, kickers([], 5));
}
