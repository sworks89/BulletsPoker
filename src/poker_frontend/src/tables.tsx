import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useApp, usePoll } from './context';
import { unwrap } from './ic';
import { LEAVE_CONFIRM, leavesAfterHand } from './leave';
import type { TableView } from './ic';
import { isMyTurn } from './turn';
import { afterClosing, reconcileSeats } from './tablesNav';

export { isMyTurn };

/** Most tables a player can watch at once; tables they are seated at always stay open. */
export const MAX_OPEN = 4;

export type Layout = 'tabs' | 'tile';

export interface Notice {
  key: string;
  text: string;
  /** Opening this table clears the notice. */
  table?: bigint;
}

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
  /** Notices that stay until dismissed (moved tables, a failed auto-sit cancel). */
  notices: Notice[];
  dismissNotice: (key: string) => void;
  focus: (id: bigint) => void;
  setLayout: (l: Layout) => void;
  setAutoFocus: (on: boolean) => void;
  refresh: () => Promise<void>;
  /** Table updates have been failing: the connection is probably down. */
  reconnecting: boolean;
}

const Ctx = createContext<TablesState | null>(null);

export function useTables(): TablesState {
  const c = useContext(Ctx);
  if (!c) throw new Error('useTables outside provider');
  return c;
}


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
  const { backend, view, go, notify, principal, refreshProfile } = useApp();
  const [open, setOpen] = useState<bigint[]>(() => load<string[]>('bp.openTables', []).map((s) => BigInt(s)));
  const [views, setViews] = useState<Map<string, TableView>>(new Map());
  const [clockOffset, setClockOffset] = useState(0);
  const [layout, setLayoutState] = useState<Layout>(() => load<Layout>('bp.layout', 'tabs'));
  const [autoFocus, setAutoFocusState] = useState<boolean>(() => load('bp.autoFocus', true));
  const active = view.name === 'table' ? view.id : null;

  useEffect(() => save('bp.openTables', open.map(String)), [open]);

  const [notices, setNotices] = useState<Notice[]>([]);
  const addNotice = useCallback(
    (n: Notice) => setNotices((cur) => [...cur.filter((x) => x.key !== n.key), n]),
    [],
  );
  const dismissNotice = useCallback((key: string) => setNotices((cur) => cur.filter((x) => x.key !== key)), []);
  // Opening the table a notice is about answers it.
  useEffect(() => {
    if (active !== null) setNotices((cur) => (cur.some((n) => n.table === active) ? cur.filter((n) => n.table !== active) : cur));
  }, [active]);

  // The tables I hold a seat at, as the server last said (table id -> tournament id).
  const seats = useRef<Map<string, bigint | null>>(new Map());
  const seatIds = () => [...seats.current.keys()].map((s) => BigInt(s));
  const openRef = useRef(open);
  openRef.current = open;

  const openTable = useCallback((id: bigint) => go({ name: 'table', id }), [go]);

  // Navigating to a table (from the lobby, tournaments, friends…) opens it. Past
  // the watch limit, the oldest table you're only watching makes room; seated
  // tables are never closed for you and don't count toward the limit.
  useEffect(() => {
    if (active === null || open.includes(active)) return;
    setOpen(reconcileSeats([...open, active], seatIds(), MAX_OPEN).open);
    // seats is read for the decision only
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active]);

  // Leaving a seat from the ✕: the same flow as the table's Leave button.
  const leaveSeat = useCallback(
    async (id: bigint) => {
      const v = views.get(String(id));
      const me = v && v.my_seat.length ? v.seats[v.my_seat[0]][0] : undefined;
      if (me && leavesAfterHand(Object.keys(v!.phase)[0], me) && !confirm(LEAVE_CONFIRM)) return;
      try {
        const r = unwrap(await backend.leave_table(id));
        if ('Left' in r) {
          seats.current.delete(String(id));
          const next = afterClosing(openRef.current, active, [id]);
          setOpen(next.open);
          if (next.goTo !== null) go(next.goTo === 'lobby' ? { name: 'lobby' } : { name: 'table', id: next.goTo });
          await refreshProfile();
        } else {
          notify("You'll leave when this hand ends.");
        }
      } catch (e) {
        notify(e instanceof Error ? e.message : String(e), 'err');
      }
    },
    [views, backend, active, go, notify, refreshProfile],
  );

  const closeTable = useCallback(
    async (id: bigint) => {
      // A seat is never just hidden: ✕ on a seated table leaves it (cash) or
      // explains why a tournament seat can't be closed.
      if (seats.current.has(String(id))) {
        if (seats.current.get(String(id)) != null) {
          notify('Tournament seats stay open until you are out. Sit out from the table if you need a break.', 'err');
          go({ name: 'table', id });
        } else {
          await leaveSeat(id);
        }
        return;
      }
      // Closing a table you're waiting at takes you out of its auto-sit line,
      // so you're never seated (and charged) at a table you closed. If it can't
      // be cancelled the table stays open and says so.
      if (views.get(String(id))?.my_auto_sit.length) {
        let failed: unknown;
        for (let attempt = 0; attempt < 2; attempt++) {
          try {
            unwrap(await backend.auto_sit(id, []));
            failed = undefined;
            break;
          } catch (e) {
            failed = e;
            if (attempt === 0) notify(e instanceof Error ? e.message : String(e), 'err');
          }
        }
        if (failed !== undefined) {
          addNotice({ key: `autosit-${id}`, text: "Couldn't leave the waiting line — try again", table: id });
          return;
        }
      }
      const next = afterClosing(openRef.current, active, [id]);
      setOpen(next.open);
      if (next.goTo !== null) go(next.goTo === 'lobby' ? { name: 'lobby' } : { name: 'table', id: next.goTo });
    },
    [active, go, views, backend, notify, addNotice, leaveSeat],
  );

  // Every seat I hold stays open, wherever the server put it: tournament
  // balancing and merges move me between tables, and a new device knows none of
  // them. Checked on sign-in and every few seconds.
  const synced = useRef(false);
  useEffect(() => {
    synced.current = false;
    seats.current = new Map();
  }, [principal]);
  usePoll(
    async () => {
      if (!principal) return;
      const refs = await backend.get_my_seats();
      const now = new Map<string, bigint | null>(refs.map((r) => [String(r.table), r.tournament[0] ?? null]));
      const before = seats.current;
      seats.current = now;
      const ids = refs.map((r) => r.table);
      const { open: next } = reconcileSeats(openRef.current, ids, MAX_OPEN);
      const newTourney = refs.filter((r) => r.tournament[0] !== undefined && !before.has(String(r.table)));
      // A tournament table I was moved away from drops out (a busted player keeps theirs).
      const movedTo = new Set(newTourney.map((r) => r.tournament[0]));
      const left = [...before.entries()]
        .filter(([k, t]) => t !== null && !now.has(k) && movedTo.has(t))
        .map(([k]) => BigInt(k));
      if (synced.current) {
        for (const r of newTourney) {
          addNotice({ key: `moved-${r.table}`, text: `You were moved to table ${r.table}`, table: r.table });
        }
      }
      synced.current = true;
      const done = afterClosing(next, active, left);
      if (done.open.length !== openRef.current.length || done.open.some((id, i) => id !== openRef.current[i])) {
        openRef.current = done.open;
        setOpen(done.open);
      }
      if (done.goTo !== null && left.length) {
        const dest = newTourney.length ? newTourney[newTourney.length - 1].table : done.goTo;
        go(dest === 'lobby' ? { name: 'lobby' } : { name: 'table', id: dest });
      }
    },
    3000,
    [backend, principal, active],
  );

  const requested = useRef<bigint[]>([]);
  const failures = useRef(0);
  const [reconnecting, setReconnecting] = useState(false);
  const refresh = useCallback(async () => {
    if (open.length === 0) return;
    const ids = [...open];
    let vs: TableView[];
    try {
      vs = await backend.get_tables(ids);
    } catch (e) {
      failures.current += 1;
      if (failures.current >= 2) setReconnecting(true);
      throw e;
    }
    failures.current = 0;
    setReconnecting(false);
    requested.current = ids;
    if (vs.length) setClockOffset(Date.now() - Number(vs[0].now / 1_000_000n));
    setViews(new Map(vs.map((v) => [String(v.id), v])));
  }, [backend, open]);

  usePoll(refresh, 1000, [refresh]);

  // React to the browser's own view of the network, and catch up at once on return.
  useEffect(() => {
    const online = () => refresh().catch(() => {});
    const offline = () => setReconnecting(true);
    const visible = () => document.visibilityState === 'visible' && online();
    window.addEventListener('online', online);
    window.addEventListener('offline', offline);
    document.addEventListener('visibilitychange', visible);
    return () => {
      window.removeEventListener('online', online);
      window.removeEventListener('offline', offline);
      document.removeEventListener('visibilitychange', visible);
    };
  }, [refresh]);

  // Tables that no longer exist (closed rooms, emptied table copies, finished
  // tournaments) drop out, with a word about it; with none left, back to the lobby.
  useEffect(() => {
    const gone = requested.current.filter((id) => !views.has(String(id)));
    if (gone.length === 0) return;
    notify(gone.length === 1 ? 'A table you had open has closed.' : `${gone.length} tables you had open have closed.`);
    const next = afterClosing(open, active, gone);
    setOpen(next.open);
    if (next.goTo !== null) go(next.goTo === 'lobby' ? { name: 'lobby' } : { name: 'table', id: next.goTo });
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
      notices,
      dismissNotice,
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
      reconnecting: reconnecting && open.length > 0,
    }),
    [open, active, views, clockOffset, layout, autoFocus, openTable, closeTable, notices, dismissNotice, go, refresh, reconnecting],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
