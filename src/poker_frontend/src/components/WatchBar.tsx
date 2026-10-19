import { useEffect, useState } from 'react';
import { useApp } from '../context';
import { unwrap, type AutoSitStatus, type TableView } from '../ic';
import { BuyInPicker } from './JoinGame';

/** How often a waiting player's browser checks in (the server drops them after 60s of silence). */
const AUTO_SIT_PING_SECS = 20;
const BUY_IN_KEY = 'bp.autosit.buyin';

function lastBuyIn(): bigint | undefined {
  try {
    const v = localStorage.getItem(BUY_IN_KEY);
    return v ? BigInt(v) : undefined;
  } catch {
    return undefined;
  }
}

function ordinal(n: number): string {
  const s = ['th', 'st', 'nd', 'rd'];
  const v = n % 100;
  return n + (s[(v - 20) % 10] || s[v] || s[0]);
}

/**
 * For a signed-in player watching a cash table: who's here, and a toggle to be
 * seated automatically when a seat opens. Clicking an empty seat still works too.
 */
export function WatchBar({ view, onChange }: { view: TableView; onChange: () => Promise<void> }) {
  const { backend, run, notify } = useApp();
  const [picking, setPicking] = useState(false);
  const queued = view.my_auto_sit[0];
  const open = view.seats.filter((s) => s.length === 0).length;
  const id = view.id;

  // Stay in line while watching; hear back when seated or dropped.
  useEffect(() => {
    if (!queued) return;
    const report = (st: AutoSitStatus) => {
      if ('Seated' in st) notify(`You're in! Seated at Table ${view.number}`);
      else if ('Dropped' in st) notify(st.Dropped.reason, 'err');
    };
    const t = setInterval(async () => {
      try {
        const st = unwrap(await backend.auto_sit_ping(id));
        report(st);
        if (!('Queued' in st)) await onChange();
      } catch {
        // a missed check-in is retried on the next tick
      }
    }, AUTO_SIT_PING_SECS * 1000);
    return () => {
      clearInterval(t);
      // Leaving the line, or this bar going away because we were just seated:
      // one last check-in collects the result so its message isn't lost.
      backend
        .auto_sit_ping(id)
        .then((r) => report(unwrap(r)))
        .catch(() => {});
    };
    // Re-arm only when the line membership changes, not on every poll.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backend, id, !!queued]);

  const turnOff = async () => {
    await run(async () => unwrap(await backend.auto_sit(id, [])));
    await onChange();
  };

  return (
    <div className="watch-bar">
      <span>
        👀 Watching <b>{view.config.name}</b>
        {view.number > 1 && <> (Table {view.number})</>} · {open === 0 ? 'all seats taken' : open === 1 ? '1 seat open' : `${open} seats open`}
      </span>
      <label className="toggle">
        <input type="checkbox" checked={!!queued} onChange={() => (queued ? turnOff() : setPicking(true))} />
        Auto-sit when a seat opens
      </label>
      {queued ? (
        <span className="muted">
          You're {ordinal(queued.place)} in line · {view.auto_sit_waiting} waiting
        </span>
      ) : (
        <span className="muted small">Or click an empty seat yourself when one opens.</span>
      )}
      {picking && (
        <BuyInPicker
          title="Auto-sit buy-in"
          limits={view.config}
          confirm="Wait for a seat"
          initial={lastBuyIn()}
          onClose={() => setPicking(false)}
          onConfirm={async (buyIn) => {
            const st = await run(async () => unwrap(await backend.auto_sit(id, [buyIn])));
            if (st === undefined) return false;
            try {
              localStorage.setItem(BUY_IN_KEY, String(buyIn));
            } catch {
              // not remembered, that's all
            }
            await onChange();
            return true;
          }}
        />
      )}
    </div>
  );
}
