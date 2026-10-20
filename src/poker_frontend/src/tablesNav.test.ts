import { describe, expect, it } from 'vitest';
import { afterClosing } from './tablesNav';

describe('afterClosing', () => {
  it('moves off the table you were looking at to the last one still open', () => {
    expect(afterClosing([1n, 2n, 3n], 2n, [2n])).toEqual({ open: [1n, 3n], goTo: 3n });
  });
  it('goes to the lobby when nothing is left open', () => {
    expect(afterClosing([2n], 2n, [2n])).toEqual({ open: [], goTo: 'lobby' });
  });
  it('stays put when a table in the background closes', () => {
    expect(afterClosing([1n, 2n], 1n, [2n])).toEqual({ open: [1n], goTo: null });
  });
  it('stays put when you are not looking at a table', () => {
    expect(afterClosing([1n, 2n], null, [1n, 2n])).toEqual({ open: [], goTo: null });
  });
});
