import { describe, expect, it } from 'vitest';
import { THROW_FX } from './SeatFx';

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
