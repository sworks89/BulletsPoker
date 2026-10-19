import { describe, expect, it } from 'vitest';
import { THROW_FX } from './SeatFx';
import { giftArrivals } from './gifts';

type Seat = Parameters<typeof giftArrivals>[1][number];
const seat = (n: number, name: string, gift?: { icon: string; from: string; at: bigint }): Seat => ({
  seat: n,
  username: name,
  gift: gift ? [gift.icon] : [],
  gift_from: gift ? [gift.from] : [],
  gift_sent_at: gift ? [gift.at] : [],
});

describe('gift arrivals', () => {
  it('skips gifts already there on first load', () => {
    const { arrivals } = giftArrivals(null, [seat(0, 'ann', { icon: '🍺', from: 'bob', at: 1n }), seat(1, 'bob')]);
    expect(arrivals).toEqual([]);
  });
  it('flies a new gift from a seated sender', () => {
    const first = giftArrivals(null, [seat(0, 'ann'), seat(3, 'bob')]);
    const { arrivals } = giftArrivals(first.next, [seat(0, 'ann', { icon: '🍾', from: 'bob', at: 5n }), seat(3, 'bob')]);
    expect(arrivals).toEqual([{ seat: 0, icon: '🍾', fromSeat: 3 }]);
  });
  it('drops in a gift from someone not seated', () => {
    const first = giftArrivals(null, [seat(0, 'ann')]);
    const { arrivals } = giftArrivals(first.next, [seat(0, 'ann', { icon: '🍺', from: 'zed', at: 5n })]);
    expect(arrivals).toEqual([{ seat: 0, icon: '🍺', fromSeat: null }]);
  });
  it('plays a repeat of the same gift again', () => {
    const a = giftArrivals(null, [seat(0, 'ann', { icon: '🍺', from: 'bob', at: 5n }), seat(1, 'bob')]);
    const { arrivals } = giftArrivals(a.next, [seat(0, 'ann', { icon: '🍺', from: 'bob', at: 9n }), seat(1, 'bob')]);
    expect(arrivals).toEqual([{ seat: 0, icon: '🍺', fromSeat: 1 }]);
  });
  it('does nothing while the gift is unchanged', () => {
    const a = giftArrivals(null, [seat(0, 'ann', { icon: '🍺', from: 'bob', at: 5n })]);
    const b = giftArrivals(a.next, [seat(0, 'ann', { icon: '🍺', from: 'bob', at: 5n })]);
    expect(b.arrivals).toEqual([]);
  });
});

describe('throw effects', () => {
  it('has a mark for every default throwable', () => {
    for (const e of ['🍅', '🥧', '❄️', '🐟', '💋', '💸', '💣']) expect(THROW_FX[e], e).toBeDefined();
  });
  it('makes money a happy hit and the rest a flinch', () => {
    expect(THROW_FX['💸'].expression).toBe('happy');
    expect(THROW_FX['🍅'].expression).toBe('hit');
    expect(THROW_FX['💣'].expression).toBe('hit');
  });
});
