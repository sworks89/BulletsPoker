import { describe, expect, it } from 'vitest';
import vectors from './settlement-vectors.json';
import { bestScore, cardName, categoryOf, parseCards } from './evaluate';

const score = (s: string) => bestScore(parseCards(s));

describe('evaluate', () => {
  it('names cards like the engine', () => {
    expect(cardName(12)).toBe('5c');
    expect(cardName(51)).toBe('As');
    expect(cardName(0)).toBe('2c');
    expect(parseCards('Td 2c As')).toEqual([33, 0, 51]);
  });

  it('finds every category', () => {
    expect(categoryOf(score('As Ks Qs Js Ts 2c 3d'))).toBe('Straight Flush');
    expect(categoryOf(score('As Ad Ac Ah 2s 3c 4d'))).toBe('Four of a Kind');
    expect(categoryOf(score('As Ad Ac Kh Ks 3c 4d'))).toBe('Full House');
    expect(categoryOf(score('2s 7s 9s Js Ks 3c 4d'))).toBe('Flush');
    expect(categoryOf(score('As 2d 3c 4h 5s Kc Kd'))).toBe('Straight');
    expect(categoryOf(score('7s 7d 7c Kh 2s 3c 9d'))).toBe('Three of a Kind');
    expect(categoryOf(score('7s 7d Kc Kh 2s 3c 9d'))).toBe('Two Pair');
    expect(categoryOf(score('7s 7d Ac Jh 2s 3c 9d'))).toBe('Pair');
    expect(categoryOf(score('7s 8d Ac Jh 2s 3c 9d'))).toBe('High Card');
  });

  it('orders hands and kickers', () => {
    expect(score('As 2d 3c 4h 5s')).toBeLessThan(score('2d 3c 4h 5s 6d'));
    expect(score('Ks Kd Ac 7h 2s')).toBeGreaterThan(score('Ks Kd Qc 7h 2s'));
    expect(score('Ks Kd 9c 9h 2s')).toBeGreaterThan(score('Ks Kd 8c 8h As'));
    expect(score('3s 3d 3c 2h 2s')).toBeGreaterThan(score('2s 2d 2c Ah As'));
    expect(score('2c 3d As Ks Qs Js Ts')).toBe(score('4c 5d As Ks Qs Js Ts'));
    expect(score('2s 7s 9s Js Ks')).toBeGreaterThan(score('Ts Jd Qc Kh As'));
    // Two pair out of three pairs: the best two plus the best kicker.
    expect(score('Ks Kd 9c 9h 4s 4d 2c')).toBe(score('Ks Kd 9c 9h 4s 3d 2c'));
    // Two sets make a full house of the higher set.
    expect(score('9s 9d 9c 4h 4s 4d 2c')).toBeGreaterThan(score('9s 9d 9c 3h 3s Ad 2c'));
    // Best flush of six suited cards uses the top five.
    expect(score('As 9s 7s 5s 3s 2s Kd')).toBeGreaterThan(score('Ks 9s 7s 5s 3s 2s Ad'));
    // A straight flush inside a longer flush.
    expect(categoryOf(score('9h 8h 7h 6h 5h 2h Ac'))).toBe('Straight Flush');
    // Steel wheel.
    expect(categoryOf(score('Ad 2d 3d 4d 5d Kc Qc'))).toBe('Straight Flush');
  });

  it('agrees with every hand the engine showed in the settlement vectors', () => {
    let checked = 0;
    for (const c of vectors.cases) {
      const r = c.record;
      for (const e of r.log) {
        if (e.seat === null || !e.text.startsWith('Shows ')) continue;
        const pl = r.players.find((x) => x.seat === e.seat)!;
        expect(categoryOf(bestScore([...pl.cards, ...r.board])), `${c.name} seat ${e.seat}`).toBe(e.text.slice(6));
        checked++;
      }
    }
    expect(checked).toBeGreaterThan(5);
  });
});
