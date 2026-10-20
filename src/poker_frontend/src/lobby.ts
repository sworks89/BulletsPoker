import type { TableSummary } from './ic';

/** One lobby row: a game and every open table of it (the original and its copies). */
export interface GameRow {
  game: bigint;
  /** The original, or the lowest-numbered table present. */
  first: TableSummary;
  /** Sorted by table number. */
  tables: TableSummary[];
  players: number;
  open: number;
  /** A player-hosted room: always its own row. */
  room: boolean;
  /** Every seat taken and no copy can open. */
  full: boolean;
}

/** Groups the lobby's tables into one row per game, in the order games first appear. */
export function groupTables(rows: TableSummary[]): GameRow[] {
  const byGame = new Map<string, TableSummary[]>();
  for (const t of rows) {
    // Rooms never group; key them by their own id so equal game ids can't collide.
    const key = t.room_owner.length > 0 ? `room-${t.id}` : `game-${t.game}`;
    const list = byGame.get(key);
    if (list) list.push(t);
    else byGame.set(key, [t]);
  }
  return [...byGame.values()].map((list) => {
    const tables = [...list].sort((a, b) => a.number - b.number);
    const first = tables[0];
    const players = tables.reduce((n, t) => n + t.players, 0);
    const open = tables.reduce((n, t) => n + Math.max(0, t.max_seats - t.players), 0);
    return {
      game: first.game,
      first,
      tables,
      players,
      open,
      room: first.room_owner.length > 0,
      full: open === 0 && first.game_at_cap,
    };
  });
}
