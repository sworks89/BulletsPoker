import { describe, expect, it } from 'vitest';
import type { TableSummary } from './ic';
import { groupTables } from './lobby';

const t = (id: number, over: Partial<TableSummary> = {}): TableSummary => ({
  id: BigInt(id),
  game: BigInt(id),
  number: 1,
  game_at_cap: false,
  name: `Table ${id}`,
  currency: { Chips: null },
  small_blind: 25n,
  big_blind: 50n,
  min_buy_in: 1000n,
  max_buy_in: 5000n,
  players: 0,
  max_seats: 2,
  phase: { Waiting: null },
  avg_pot: 0n,
  room_owner: [],
  locked: false,
  ...over,
});

describe('groupTables', () => {
  it('folds copies under their game, ordered by table number', () => {
    const rows = groupTables([
      t(3, { name: 'HU', players: 2 }),
      t(12, { game: 3n, number: 3, name: 'HU', players: 1 }),
      t(11, { game: 3n, number: 2, name: 'HU', players: 2 }),
    ]);
    expect(rows).toHaveLength(1);
    expect(rows[0].game).toBe(3n);
    expect(rows[0].tables.map((x) => x.number)).toEqual([1, 2, 3]);
    expect(rows[0].first.id).toBe(3n);
  });

  it('adds up players and open seats across the tables', () => {
    const [row] = groupTables([t(3, { players: 2 }), t(11, { game: 3n, number: 2, players: 1 })]);
    expect(row.players).toBe(3);
    expect(row.open).toBe(1);
  });

  it('never merges rooms, even with the same name', () => {
    const rows = groupTables([t(40, { name: 'Same', room_owner: ['a'] }), t(41, { name: 'Same', room_owner: ['b'] })]);
    expect(rows).toHaveLength(2);
    expect(rows.every((r) => r.room)).toBe(true);
  });

  it('keeps games in the order they first appear', () => {
    const rows = groupTables([t(2), t(3), t(11, { game: 3n, number: 2 }), t(1)]);
    expect(rows.map((r) => r.game)).toEqual([2n, 3n, 1n]);
  });

  it('is full only when no seat is open and no copy can open', () => {
    const fullAtCap = groupTables([t(3, { players: 2, game_at_cap: true })])[0];
    const fullNotCap = groupTables([t(3, { players: 2, game_at_cap: false })])[0];
    const openAtCap = groupTables([t(3, { players: 1, game_at_cap: true })])[0];
    expect([fullAtCap.full, fullNotCap.full, openAtCap.full]).toEqual([true, false, false]);
  });

  it('still forms a row for copies whose original is missing', () => {
    const rows = groupTables([t(11, { game: 3n, number: 2 })]);
    expect(rows).toHaveLength(1);
    expect(rows[0].first.id).toBe(11n);
  });
});
