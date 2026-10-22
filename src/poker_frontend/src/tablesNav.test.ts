import { describe, expect, it } from 'vitest';
import { afterClosing, reconcileSeats } from './tablesNav';

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

describe('reconcileSeats', () => {
  it('adds a seated table even at the watch limit', () => {
    const r = reconcileSeats([1n, 2n, 3n, 4n], [9n], 4);
    expect(r.added).toEqual([9n]);
    expect(r.open).toContain(9n);
    expect(r.open).toHaveLength(5);
  });
  it('keeps every seat and reports nothing new when they are already open', () => {
    expect(reconcileSeats([5n, 6n], [5n, 6n], 4)).toEqual({ open: [5n, 6n], added: [] });
  });
  it('evicts the oldest watched table, never a seated one', () => {
    const r = reconcileSeats([1n, 2n, 3n, 4n, 5n, 6n], [1n], 4);
    expect(r.open).toEqual([1n, 3n, 4n, 5n, 6n]);
  });
  it('does not count seated tables toward the watch limit', () => {
    const r = reconcileSeats([1n, 2n, 3n, 4n], [1n, 2n, 3n, 4n, 8n], 4);
    expect(r.open).toEqual([1n, 2n, 3n, 4n, 8n]);
  });
});
