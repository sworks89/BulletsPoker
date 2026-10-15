import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useApp, usePoll } from './context';
import type { TableView } from './ic';

/** Most tables a player can have open at once. */
export const MAX_OPEN = 4;

export type Layout = 'tabs' | 'tile';

interface TablesState {
  open: bigint[];
  active: bigint | null;
  views: Map<string, TableView>;
  /** Server clock minus local clock (ms), for action timers. */
  clockOffset: number;
  layout: Layout;
  autoFocus: boolean;
  openTable: (id: bigint) => void;
  closeTable: (id: bigint) => void;
  focus: (id: bigint) => void;
  setLayout: (l: Layout) => void;
  setAutoFocus: (on: boolean) => void;
  refresh: () => Promise<void>;
}

const Ctx = createContext<TablesState | null>(null);

export function useTables(): TablesState {
  const c = useContext(Ctx);
  if (!c) throw new Error('useTables outside provider');
  return c;
}

export const isMyTurn = (v: TableView | undefined): boolean =>
  !!v && v.my_seat.length > 0 && v.to_act.length > 0 && v.my_seat[0] === v.to_act[0];

// Per-browser preferences: which tables are open and how they're laid out.
function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

function save(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // storage unavailable (private mode): preferences just won't persist
  }
}

export function TablesProvider({ children }: { children: ReactNode }) {
  const { backend, view, go, notify } = useApp();
  const [open, setOpen] = useState<bigint[]>(() => load<string[]>('bp.openTables', []).map((s) => BigInt(s)));
  const [views, setViews] = useState<Map<string, TableView>>(new Map());
  const [clockOffset, setClockOffset] = useState(0);
  const [layout, setLayoutState] = useState<Layout>(() => load<Layout>('bp.layout', 'tabs'));
  const [autoFocus, setAutoFocusState] = useState<boolean>(() => load('bp.autoFocus', true));
  const active = view.name === 'table' ? view.id : null;

  useEffect(() => save('bp.openTables', open.map(String)), [open]);

  const openTable = useCallback((id: bigint) => go({ name: 'table', id }), [go]);

  // Navigating to a table (from the lobby, tournaments, friends…) opens it. When
  // the limit is reached, a table you're only watching makes room; tables you're
  // seated at are never closed for you.
  useEffect(() => {
    if (active === null || open.includes(active)) return;
    if (open.length < MAX_OPEN) {
      setOpen([...open, active]);
      return;
    }
    const watching = open.find((id) => !views.get(String(id))?.my_seat.length);
    if (watching === undefined) {
      notify(`You can have ${MAX_OPEN} tables open. Leave one first.`, 'err');
      go({ name: 'table', id: open[open.length - 1] });
      return;
    }
    setOpen([...open.filter((id) => id !== watching), active]);
    // views is read for the decision only
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active]);

  const closeTable = useCallback(
    (id: bigint) => {
      const rest = open.filter((x) => x !== id);
      setOpen(rest);
      if (active === id) go(rest.length ? { name: 'table', id: rest[rest.length - 1] } : { name: 'lobby' });
    },
    [open, active, go],
  );

  const requested = useRef<bigint[]>([]);
  const refresh = useCallback(async () => {
    if (open.length === 0) return;
    const ids = [...open];
    const vs = await backend.get_tables(ids);
    requested.current = ids;
    if (vs.length) setClockOffset(Date.now() - Number(vs[0].now / 1_000_000n));
    setViews(new Map(vs.map((v) => [String(v.id), v])));
  }, [backend, open]);

  usePoll(refresh, 1000, [refresh]);

  // Tables that no longer exist (closed rooms, finished tournaments) drop out.
  useEffect(() => {
    const gone = requested.current.filter((id) => !views.has(String(id)));
    if (gone.length) setOpen((o) => o.filter((id) => !gone.includes(id)));
    // only react to fresh server data
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [views]);

  // Auto-focus: in tab mode, jump to a table where it's your turn (unless the current one also needs you).
  const lastJump = useRef<string>('');
  useEffect(() => {
    if (!autoFocus || layout !== 'tabs' || active === null) return;
    if (isMyTurn(views.get(String(active)))) return;
    const waiting = open.map((id) => views.get(String(id))).find((v) => isMyTurn(v));
    if (!waiting) return;
    const key = `${waiting.id}-${waiting.hand_no}-${Object.keys(waiting.phase)[0]}-${waiting.current_bet}`;
    if (key === lastJump.current) return;
    lastJump.current = key;
    go({ name: 'table', id: waiting.id });
  }, [views, autoFocus, layout, active, open, go]);

  // Browser tab title shows how many tables are waiting on you.
  const waitingCount = open.filter((id) => isMyTurn(views.get(String(id)))).length;
  useEffect(() => {
    document.title = waitingCount > 0 ? `(${waitingCount}) Your turn · Bullets Poker` : 'Bullets Poker';
  }, [waitingCount]);

  const value = useMemo<TablesState>(
    () => ({
      open,
      active,
      views,
      clockOffset,
      layout,
      autoFocus,
      openTable,
      closeTable,
      focus: (id) => go({ name: 'table', id }),
      setLayout: (l) => {
        setLayoutState(l);
        save('bp.layout', l);
      },
      setAutoFocus: (on) => {
        setAutoFocusState(on);
        save('bp.autoFocus', on);
      },
      refresh,
    }),
    [open, active, views, clockOffset, layout, autoFocus, openTable, closeTable, go, refresh],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
