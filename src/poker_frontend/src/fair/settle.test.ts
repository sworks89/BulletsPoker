import { describe, expect, it } from 'vitest';
import vectors from './settlement-vectors.json';
import { logOf } from './fixtures';
import { feesFor, readLog, settle, type Fees } from './settle';

type VCase = (typeof vectors.cases)[number];

const fees = (c: VCase): Fees => ({
  rake_bps: c.fees.rake_bps,
  rake_cap: BigInt(c.fees.rake_cap),
  jackpot_bps: c.fees.jackpot_bps,
  jackpot_cap: BigInt(c.fees.jackpot_cap),
});

function inputs(c: VCase) {
  const replay = readLog(logOf(c));
  return {
    replay,
    players: c.record.players.map((p) => ({
      seat: p.seat,
      committed: replay.committed.get(p.seat) ?? 0n,
      folded: replay.folded.has(p.seat),
      hole: p.cards,
    })),
    board: c.record.board,
    button: c.record.button,
  };
}

describe('settlement vectors (real engine hands)', () => {
  it('cover the cases the plan asks for', () => {
    const names = vectors.cases.map((c) => c.name);
    for (const n of ['split_pot', 'three_way_all_in_side_pots', 'everyone_folds_preflop', 'rake_cap', 'odd_chip_split']) {
      expect(names).toContain(n);
    }
  });

  for (const c of vectors.cases) {
    describe(c.name, () => {
      it('commitments read from the log match the record', () => {
        const { replay } = inputs(c);
        for (const p of c.record.players) {
          expect(replay.committed.get(p.seat) ?? 0n, `seat ${p.seat}`).toBe(BigInt(p.committed));
        }
      });

      it('rake and drop from the table rates match the engine', () => {
        const pot = BigInt(c.expected.pot);
        expect(feesFor(pot, c.record.board.length, fees(c))).toEqual({
          rake: BigInt(c.expected.rake),
          drop: BigInt(c.expected.jackpot_drop),
        });
      });

      it('pays every seat what the engine paid', () => {
        const { players, board, button } = inputs(c);
        const r = settle({ players, board, button, take: { fees: fees(c) } });
        expect(r.pot).toBe(BigInt(c.expected.pot));
        expect(r.rake).toBe(BigInt(c.expected.rake));
        expect(r.drop).toBe(BigInt(c.expected.jackpot_drop));
        for (const w of c.expected.won) {
          expect(r.won.get(w.seat) ?? 0n, `seat ${w.seat}`).toBe(BigInt(w.won));
        }
      });

      it('splits the same net pot when rake and drop are given as amounts', () => {
        const { players, board, button } = inputs(c);
        const take = { rake: BigInt(c.expected.rake), drop: BigInt(c.expected.jackpot_drop) };
        const r = settle({ players, board, button, take });
        for (const w of c.expected.won) {
          expect(r.won.get(w.seat) ?? 0n, `seat ${w.seat}`).toBe(BigInt(w.won));
        }
      });
    });
  }

  it('gives the odd chip to the first winner left of the button, not the lowest seat', () => {
    const c = vectors.cases.find((x) => x.name === 'odd_chip_split')!;
    const { players, board } = inputs(c);
    const won = (button: number) => settle({ players, board, button, take: { rake: 0n, drop: 0n } }).won;
    expect(won(0).get(4)! - won(0).get(0)!).toBe(1n);
    // Moving the button to seat 2 puts seat 4 first again; to seat 4, seat 0 is first.
    expect(won(4).get(0)! - won(4).get(4)!).toBe(1n);
  });

  it('takes no rake or drop when no flop was dealt', () => {
    expect(feesFor(1_000_000n, 0, { rake_bps: 1000, rake_cap: 10n ** 12n, jackpot_bps: 300, jackpot_cap: 10n ** 12n })).toEqual({
      rake: 0n,
      drop: 0n,
    });
  });
});
