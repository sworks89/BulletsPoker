import { describe, expect, it } from 'vitest';
import { handReactions, handScore, type ReactionInput } from './poker';

const RANKS = '23456789TJQKA';
const SUITS = 'cdhs';
/** 'As' → the backend's card number (rank * 4 + suit). */
const c = (s: string) => RANKS.indexOf(s[0]) * 4 + SUITS.indexOf(s[1]);
const cs = (s: string) => s.split(' ').map(c);

describe('handScore', () => {
  it('ranks a royal flush above four of a kind', () => {
    expect(handScore(cs('As Ks Qs Js Ts'))).toBeGreaterThan(handScore(cs('Ah Ac Ad As Kd')));
  });
  it('ranks a full house above a flush', () => {
    expect(handScore(cs('3h 3c 3d 2s 2d'))).toBeGreaterThan(handScore(cs('Ah Kh 9h 7h 2h')));
  });
  it('ranks the wheel below a six-high straight', () => {
    expect(handScore(cs('Ah 2c 3d 4s 5d'))).toBeLessThan(handScore(cs('2c 3d 4s 5d 6h')));
  });
  it('breaks equal two pair on the kicker', () => {
    expect(handScore(cs('Kh Kc 7d 7s Ad'))).toBeGreaterThan(handScore(cs('Kd Ks 7h 7c Qd')));
  });
  it('picks the best five of seven', () => {
    expect(handScore(cs('As Ks Qs Js Ts 2d 3c'))).toBe(handScore(cs('As Ks Qs Js Ts')));
  });
  it('ties the same ranks in different suits', () => {
    expect(handScore(cs('Ah Kh 9c 7d 2s'))).toBe(handScore(cs('Ad Kd 9h 7s 2c')));
  });
});

const BB = 50n;
const input = (over: Partial<ReactionInput['result']> = {}, allIn: number[] = []): ReactionInput => ({
  result: {
    board: cs('Kh 7d 2c 9s Ah'),
    shown: [
      { seat: 1, cards: cs('Ks Kd') },
      { seat: 2, cards: cs('Ac Ad') },
    ],
    winners: [{ seat: 2, amount: 400n, hand: 'Three of a Kind' }],
    ...over,
  },
  bigBlind: BB,
  allInSeats: new Set(allIn),
});

describe('handReactions', () => {
  it('flags the loser who was ahead before the river, and the winner gets the big win', () => {
    const r = handReactions(input());
    expect(r.get(1)).toBe('badBeat');
    expect(r.get(2)).toBe('bigWin');
  });
  it('gives no bad beat to a player who took a side pot', () => {
    const r = handReactions(input({ winners: [{ seat: 2, amount: 400n, hand: 'Three of a Kind' }, { seat: 1, amount: 100n, hand: 'Three of a Kind' }] }));
    expect(r.get(1)).not.toBe('badBeat');
  });
  it('gives no bad beat to a mucked loser', () => {
    const r = handReactions(input({ shown: [{ seat: 2, cards: cs('Ac Ad') }] }));
    expect(r.get(1)).toBeUndefined();
  });
  it('gives no bad beat before the river', () => {
    const r = handReactions(input({ board: cs('Kh 7d 2c 9s') }));
    expect(r.get(1)).toBeUndefined();
  });
  it('gives no bad beat when the hands were tied on the turn', () => {
    const r = handReactions(
      input({
        board: cs('Kh 7d 2c 9s 9h'),
        shown: [
          { seat: 1, cards: cs('3c 4d') },
          { seat: 2, cards: cs('3d 4c') },
        ],
        winners: [{ seat: 2, amount: 10n, hand: 'Pair' }],
      }),
    );
    expect(r.get(1)).toBeUndefined();
  });
  it('gives an uncontested winner only the small cheer, however big the pot', () => {
    const r = handReactions(input({ shown: [], winners: [{ seat: 2, amount: 1000n * BB, hand: 'Uncontested' }] }));
    expect(r.get(2)).toBe('win');
    expect(r.size).toBe(1);
  });
  it('makes a 40 big blind win a big win, and 39 a normal one', () => {
    const plain = { board: cs('Kh 7d 2c 9s 3h'), shown: [{ seat: 1, cards: cs('5c 4d') }, { seat: 2, cards: cs('Ac Ad') }] };
    expect(handReactions(input({ ...plain, winners: [{ seat: 2, amount: 40n * BB, hand: 'Pair' }] })).get(2)).toBe('bigWin');
    expect(handReactions(input({ ...plain, winners: [{ seat: 2, amount: 39n * BB, hand: 'Pair' }] })).get(2)).toBe('win');
  });
  it('makes an all-in showdown a big win', () => {
    const plain = { board: cs('Kh 7d 2c 9s 3h'), shown: [{ seat: 1, cards: cs('5c 4d') }, { seat: 2, cards: cs('Ac Ad') }] };
    const r = handReactions(input({ ...plain, winners: [{ seat: 2, amount: 2n * BB, hand: 'Pair' }] }, [1]));
    expect(r.get(2)).toBe('bigWin');
  });
});
