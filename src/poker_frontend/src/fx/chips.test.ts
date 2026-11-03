import { describe, expect, it } from 'vitest';
import type { TableView } from '../ic';
import { NARROW_BET_DY, betSpot, chipColumns, chipMotionBetween, chipSnapOf, type ChipSnap } from './chips';

const snap = (over: Partial<ChipSnap> = {}): ChipSnap => ({
  hand: 1n,
  street: 0,
  bets: new Map(),
  collected: 0n,
  over: false,
  winners: [],
  ...over,
});

describe('chipColumns', () => {
  // Big blind 50: chip values 10, 50, 250, 1000, 5000, 25000.
  it('breaks an amount into chip values relative to the big blind, largest first', () => {
    expect(chipColumns(150n, 50n)).toEqual([{ tier: 1, n: 3 }]);
    expect(chipColumns(1_310n, 50n)).toEqual([
      { tier: 3, n: 1 },
      { tier: 2, n: 1 },
      { tier: 1, n: 1 },
      { tier: 0, n: 1 },
    ]);
  });

  it('looks the same at any stakes', () => {
    expect(chipColumns(3_000_000n, 1_000_000n)).toEqual(chipColumns(150n, 50n));
  });

  it('shows a remainder below the smallest chip as one smallest chip', () => {
    expect(chipColumns(25n, 50n)).toEqual([{ tier: 0, n: 3 }]);
    expect(chipColumns(3n, 50n)).toEqual([{ tier: 0, n: 1 }]);
  });

  it('caps a column at 8 chips and keeps the 4 largest values', () => {
    expect(chipColumns(10_000_000n, 50n)).toEqual([{ tier: 5, n: 8 }]);
    expect(chipColumns(31_310n, 50n).map((c) => c.tier)).toEqual([5, 4, 3, 2]);
  });

  it('shows nothing for nothing', () => {
    expect(chipColumns(0n, 50n)).toEqual([]);
  });

  it('copes with a big blind too small to divide', () => {
    expect(chipColumns(7n, 1n)).toEqual([{ tier: 1, n: 1 }, { tier: 0, n: 2 }]);
  });
});

describe('chipMotionBetween', () => {
  it('moves nothing on the first view', () => {
    expect(chipMotionBetween(null, snap({ bets: new Map([[0, 50n]]) }))).toEqual({ toss: [], sweep: [], push: [] });
  });

  it('tosses what a player added to their bet', () => {
    const prev = snap({ bets: new Map([[0, 25n], [1, 50n]]) });
    const next = snap({ bets: new Map([[0, 150n], [1, 50n], [2, 150n]]) });
    expect(chipMotionBetween(prev, next).toss).toEqual([
      { seat: 0, amount: 125n, onto: true },
      { seat: 2, amount: 150n, onto: false },
    ]);
  });

  it('tosses the blinds of a new hand', () => {
    const prev = snap({ hand: 1n, over: true, collected: 400n });
    const next = snap({ hand: 2n, bets: new Map([[3, 25n], [4, 50n]]) });
    expect(chipMotionBetween(prev, next)).toEqual({
      toss: [{ seat: 3, amount: 25n, onto: false }, { seat: 4, amount: 50n, onto: false }],
      sweep: [],
      push: [],
    });
  });

  it('sweeps the bets into the pot when a street ends', () => {
    const prev = snap({ bets: new Map([[0, 150n], [1, 150n], [2, 150n]]) });
    const next = snap({ street: 3, collected: 450n });
    expect(chipMotionBetween(prev, next)).toEqual({
      toss: [],
      sweep: [{ seat: 0, amount: 150n }, { seat: 1, amount: 150n }, { seat: 2, amount: 150n }],
      push: [],
    });
  });

  it('after a sweep, tosses a bet already made on the next street in full', () => {
    const prev = snap({ bets: new Map([[0, 150n], [1, 150n]]) });
    const next = snap({ street: 3, collected: 300n, bets: new Map([[1, 200n]]) });
    const m = chipMotionBetween(prev, next);
    expect(m.sweep).toEqual([{ seat: 0, amount: 150n }, { seat: 1, amount: 150n }]);
    expect(m.toss).toEqual([{ seat: 1, amount: 200n, onto: false }]);
  });

  it('sweeps the last bets and pushes the pot to the winners when the hand ends', () => {
    const prev = snap({ collected: 1_650n, bets: new Map([[1, 800n], [2, 800n]]) });
    const next = snap({
      collected: 3_250n,
      over: true,
      winners: [{ seat: 1, amount: 1_625n }, { seat: 2, amount: 1_625n }],
    });
    expect(chipMotionBetween(prev, next)).toEqual({
      toss: [],
      sweep: [{ seat: 1, amount: 800n }, { seat: 2, amount: 800n }],
      push: [{ seat: 1, amount: 1_625n }, { seat: 2, amount: 1_625n }],
    });
  });

  it('keeps the bets where they are when a folded player leaves mid-street', () => {
    // The leaver's seat is hidden but their chips stay in the pot, so the pot grows.
    const prev = snap({ street: 3, collected: 500n, bets: new Map([[0, 100n], [1, 100n], [2, 300n]]) });
    const next = snap({ street: 3, collected: 600n, bets: new Map([[0, 100n], [2, 300n]]) });
    expect(chipMotionBetween(prev, next)).toEqual({ toss: [], sweep: [], push: [] });
  });

  it('pushes only once per hand', () => {
    const over = snap({ collected: 300n, over: true, winners: [{ seat: 0, amount: 300n }] });
    expect(chipMotionBetween(over, over).push).toEqual([]);
  });

  it('does not replay a hand that ended between polls of a new one', () => {
    const prev = snap({ hand: 1n, bets: new Map([[0, 150n]]) });
    const next = snap({ hand: 2n, over: true, collected: 300n, winners: [{ seat: 0, amount: 300n }] });
    expect(chipMotionBetween(prev, next)).toEqual({ toss: [], sweep: [], push: [] });
  });
});

describe('chipSnapOf', () => {
  const seat = (n: number, bet: bigint) => [{ seat: n, bet }];
  const view = (phase: string, pot: bigint, bets: bigint[], board = 0, result?: object) =>
    ({
      hand_no: 7n,
      phase: { [phase]: null },
      pot,
      board: new Uint8Array(board),
      seats: [...bets.map((b, i) => seat(i, b)), []],
      last_result: result ? [result] : [],
    }) as unknown as TableView;
  const result = { winners: [{ seat: 1, amount: 380n }] };

  it('counts only chips already in the pot as collected while betting', () => {
    const s = chipSnapOf(view('Flop', 500n, [100n, 0n, 100n], 3));
    expect(s).toEqual({ hand: 7n, street: 3, bets: new Map([[0, 100n], [2, 100n]]), collected: 300n, over: false, winners: [] });
  });

  it('puts the whole pot in and lists the winners when the hand is over', () => {
    const s = chipSnapOf(view('HandOver', 400n, [0n, 0n], 5, result));
    expect(s.over).toBe(true);
    expect(s.collected).toBe(400n);
    expect(s.winners).toEqual([{ seat: 1, amount: 380n }]);
  });

  it('shows no pot once the table is waiting for players, though the last result is kept', () => {
    const s = chipSnapOf(view('Waiting', 400n, [0n, 0n], 5, result));
    expect(s.collected).toBe(0n);
    expect(s.over).toBe(false);
  });
});

describe('betSpot', () => {
  it('puts a bet between the seat and the middle of the table', () => {
    const top = betSpot({ left: 50, top: 10 }, false);
    expect(top.left).toBeCloseTo(50);
    expect(top.top).toBeCloseTo(30);
    const side = betSpot({ left: 7, top: 50 }, false);
    expect(side.left).toBeCloseTo(24.2);
    expect(side.top).toBeCloseTo(50);
    expect(side.dy).toBe(0);
  });

  it('puts the bottom seat’s bet beside its cards, which fill the space toward the middle', () => {
    expect(betSpot({ left: 50, top: 90 }, false)).toEqual({ left: 63, top: 77, dy: 0 });
    // Below the seat would be on top of the action buttons.
    expect(betSpot({ left: 50, top: 90 }, true)).toEqual({ left: 63, top: 77, dy: 0 });
  });

  it('on a narrow screen, where seats fill the table, puts a bet just below its seat', () => {
    expect(betSpot({ left: 14, top: 30 }, true)).toEqual({ left: 14, top: 30, dy: NARROW_BET_DY });
  });
});
