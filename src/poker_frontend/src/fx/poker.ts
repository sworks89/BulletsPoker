// Just enough poker to react to a finished hand: a hand evaluator (cards use the
// backend's encoding, rank = card >> 2 with 0 = deuce, suit = card & 3) and the
// rules for which seats cheer and which get a bad beat.

/** Scores exactly five cards; higher is better, equal is a tie. */
function score5(cards: number[]): number {
  const ranks = cards.map((c) => c >> 2).sort((a, b) => b - a);
  const flush = cards.every((c) => (c & 3) === (cards[0] & 3));
  const counts = new Map<number, number>();
  for (const r of ranks) counts.set(r, (counts.get(r) ?? 0) + 1);
  // Ranks ordered by how many there are, then by rank: the tie-break order.
  const groups = [...counts].sort((a, b) => b[1] - a[1] || b[0] - a[0]);
  const order = groups.map((g) => g[0]);
  const unique = counts.size === 5;
  let straightHigh = -1;
  if (unique && ranks[0] - ranks[4] === 4) straightHigh = ranks[0];
  if (unique && ranks[0] === 12 && ranks[1] === 3) straightHigh = 3; // the wheel, A-2-3-4-5
  const shape = groups.map((g) => g[1]).join('');
  let category: number;
  let kickers = order;
  if (straightHigh >= 0) {
    category = flush ? 8 : 4;
    kickers = [straightHigh];
  } else if (shape === '41') category = 7;
  else if (shape === '32') category = 6;
  else if (flush) category = 5;
  else if (shape === '311') category = 3;
  else if (shape === '221') category = 2;
  else if (shape === '2111') category = 1;
  else category = 0;
  let s = category;
  for (let i = 0; i < 5; i++) s = s * 13 + (kickers[i] ?? 0);
  return s;
}

/** Best five-card score from 5–7 cards. */
export function handScore(cards: number[]): number {
  let best = -1;
  const n = cards.length;
  for (let a = 0; a < n; a++)
    for (let b = a + 1; b < n; b++)
      for (let c = b + 1; c < n; c++)
        for (let d = c + 1; d < n; d++)
          for (let e = d + 1; e < n; e++) best = Math.max(best, score5([cards[a], cards[b], cards[c], cards[d], cards[e]]));
  return best;
}

export type Reaction = 'badBeat' | 'win' | 'bigWin';

export interface ReactionInput {
  result: {
    board: number[];
    shown: { seat: number; cards: number[] }[];
    winners: { seat: number; amount: bigint; hand: string }[];
  };
  bigBlind: bigint;
  allInSeats: Set<number>;
}

/** Who reacts to a finished hand, by seat. */
export function handReactions({ result, bigBlind, allInSeats }: ReactionInput): Map<number, Reaction> {
  const out = new Map<number, Reaction>();
  const { board, shown, winners } = result;
  if (winners.every((w) => w.hand === 'Uncontested')) {
    for (const w of winners) out.set(w.seat, 'win');
    return out;
  }
  const winnerSeats = new Set(winners.map((w) => w.seat));
  const shownWinners = shown.filter((s) => winnerSeats.has(s.seat));
  let badBeat = false;
  if (board.length === 5 && shownWinners.length > 0) {
    const turn = board.slice(0, 4);
    const onTurn = (cards: number[]) => handScore([...cards, ...turn]);
    for (const s of shown) {
      if (winnerSeats.has(s.seat)) continue;
      const mine = onTurn(s.cards);
      if (shownWinners.every((w) => mine > onTurn(w.cards))) {
        out.set(s.seat, 'badBeat');
        badBeat = true;
      }
    }
  }
  const allInShowdown = shown.some((s) => allInSeats.has(s.seat));
  for (const w of winners) {
    const beatSomeone = badBeat && shownWinners.some((s) => s.seat === w.seat);
    out.set(w.seat, w.amount >= 40n * bigBlind || allInShowdown || beatSomeone ? 'bigWin' : 'win');
  }
  return out;
}
