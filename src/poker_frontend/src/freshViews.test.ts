import { describe, expect, it } from 'vitest';
import { freshest } from './freshViews';

const v = (id: number, now: number, board = 0) => ({ id: BigInt(id), now: BigInt(now), board });

describe('freshest', () => {
  it('keeps the view we hold when a lagging replica answers with an older one', () => {
    const held = new Map([['1', v(1, 200, 3)]]);
    const out = freshest(held, [v(1, 150, 0)]);
    expect(out.get('1')).toEqual(v(1, 200, 3));
  });

  it('takes a newer view, and a same-time one', () => {
    const held = new Map([['1', v(1, 200, 3)], ['2', v(2, 200, 0)]]);
    const out = freshest(held, [v(1, 250, 4), v(2, 200, 1)]);
    expect(out.get('1')).toEqual(v(1, 250, 4));
    expect(out.get('2')).toEqual(v(2, 200, 1));
  });

  it('lists only the tables just fetched, so a closed table still drops out', () => {
    const held = new Map([['1', v(1, 200)], ['2', v(2, 200)]]);
    const out = freshest(held, [v(2, 100)]);
    expect([...out.keys()]).toEqual(['2']);
    expect(out.get('2')).toEqual(v(2, 200));
  });
});
