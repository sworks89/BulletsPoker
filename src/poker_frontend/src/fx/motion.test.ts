import { describe, expect, it } from 'vitest';
import { bestFive } from './poker';
import { dealOrder, motionBetween, showdownHighlight, type MotionSnap } from './motion';

const RANKS = '23456789TJQKA';
const SUITS = 'cdhs';
const c = (s: string) => RANKS.indexOf(s[0]) * 4 + SUITS.indexOf(s[1]);
const cs = (s: string) => s.split(' ').map(c);

describe('bestFive', () => {
  it('picks the cards that make the best hand', () => {
    const cards = cs('As Ks Kd 7c 2s 9h Kh');
    expect(new Set(bestFive(cards))).toEqual(new Set(cs('As Ks Kd 9h Kh')));
  });
  it('returns all five of five', () => {
    expect(bestFive(cs('2c 3d 4s 5d 6h')).length).toBe(5);
  });
});

const snap = (o: Partial<MotionSnap> = {}): MotionSnap => ({ hand: 1n, board: 0, dealt: [0, 1], faceUp: [0], ...o });

describe('motionBetween', () => {
  it('animates nothing on the first view, so opening a table mid-hand replays nothing', () => {
    expect(motionBetween(null, snap({ board: 3 }))).toEqual({ deal: [], board: [], reveal: [] });
  });
  it('deals every seat with cards when a new hand starts', () => {
    expect(motionBetween(snap({ hand: 1n, board: 5 }), snap({ hand: 2n, dealt: [0, 1, 3] }))).toEqual({ deal: [0, 1, 3], board: [], reveal: [] });
  });
  it('animates only the new board cards', () => {
    expect(motionBetween(snap({ board: 3 }), snap({ board: 4 })).board).toEqual([3]);
    expect(motionBetween(snap({ board: 0 }), snap({ board: 3 })).board).toEqual([0, 1, 2]);
    expect(motionBetween(snap({ board: 4 }), snap({ board: 4 })).board).toEqual([]);
  });
  it('flips seats whose cards were just shown', () => {
    expect(motionBetween(snap({ faceUp: [0] }), snap({ faceUp: [0, 1] })).reveal).toEqual([1]);
  });
  it('treats a new hand that arrives with a board as all new', () => {
    expect(motionBetween(snap({ hand: 1n, board: 5 }), snap({ hand: 2n, board: 3 })).board).toEqual([0, 1, 2]);
  });
});

describe('dealOrder', () => {
  it('starts left of the button and goes round twice', () => {
    expect(dealOrder([1, 3, 5], 3, 9)).toEqual([
      [5, 0], [1, 0], [3, 0],
      [5, 1], [1, 1], [3, 1],
    ]);
  });
});

describe('showdownHighlight', () => {
  const result = {
    board: cs('Kd 7c 2s 9h Kh'),
    shown: [
      { seat: 0, cards: cs('As Ks') },
      { seat: 4, cards: cs('Qh Qd') },
    ],
    winners: [{ seat: 0 }],
  };
  it("marks the winner's best five on the board and in their hand", () => {
    const h = showdownHighlight(result)!;
    expect([...h.board].sort()).toEqual([0, 3, 4]);
    expect([...(h.seats.get(0) ?? [])].sort()).toEqual([0, 1]);
    expect(h.seats.get(4)?.size ?? 0).toBe(0);
  });
  it('highlights nothing without a showdown', () => {
    expect(showdownHighlight({ ...result, shown: [] })).toBeNull();
  });
  it("highlights nothing when no winner's cards were shown", () => {
    expect(showdownHighlight({ ...result, winners: [{ seat: 7 }] })).toBeNull();
  });
});
