import { useEffect, useState } from 'react';
import { useApp } from '../context';
import { isMyTurn, MAX_OPEN, useTables } from '../tables';
import { amount } from '../format';
import { TablePage } from './Table';
import { Chat } from './Chat';

/** Strip of open tables, shown on every page so you never miss your turn. */
export function TableStrip() {
  const { view, go } = useApp();
  const { open, active, views, clockOffset, layout, autoFocus, focus, closeTable, setLayout, setAutoFocus } = useTables();
  const [, tick] = useState(0);
  useEffect(() => {
    const t = setInterval(() => tick((x) => x + 1), 500);
    return () => clearInterval(t);
  }, []);
  if (open.length === 0) return null;
  const onTables = view.name === 'table';

  return (
    <div className="table-strip">
      {open.map((id) => {
        const v = views.get(String(id));
        const turn = isMyTurn(v);
        const me = v && v.my_seat.length ? v.seats[v.my_seat[0]][0] : undefined;
        const secs = v && turn ? Math.max(0, Math.ceil((Number(v.action_deadline / 1_000_000n) + clockOffset - Date.now()) / 1000)) : 0;
        return (
          <div
            key={String(id)}
            className={`strip-tab ${onTables && active === id ? 'active' : ''} ${turn ? 'turn' : ''}`}
            onClick={() => focus(id)}
          >
            <span className="strip-name">{v?.tournament[0] ? '🏆 ' : ''}{v?.config.name ?? 'Table'}</span>
            <span className="strip-info">
              {turn ? `Your turn · ${secs}s${v!.time_bank_active ? ' ⏳' : ''}` : me ? amount(me.stack, v!.config.currency) : 'watching'}
            </span>
            <button
              className="strip-close"
              title="Close table"
              onClick={(e) => {
                e.stopPropagation();
                closeTable(id);
              }}
            >
              ✕
            </button>
          </div>
        );
      })}
      {open.length < MAX_OPEN && (
        <button className="strip-add" title="Open another table" onClick={() => go({ name: 'lobby' })}>
          + Table
        </button>
      )}
      <div className="strip-controls">
        <button className={`chip ${layout === 'tabs' ? 'active' : ''}`} onClick={() => setLayout('tabs')} title="One table at a time">
          ▭ Tabs
        </button>
        <button className={`chip ${layout === 'tile' ? 'active' : ''}`} onClick={() => setLayout('tile')} title="All tables side by side">
          ⊞ Tile
        </button>
        {layout === 'tabs' && (
          <label className="small muted strip-auto" title="Switch to a table as soon as it's your turn there">
            <input type="checkbox" checked={autoFocus} onChange={(e) => setAutoFocus(e.target.checked)} /> Auto-switch
          </label>
        )}
      </div>
    </div>
  );
}

/**
 * Hosts every open table. All stay mounted (hidden when not shown) so turn
 * sounds and timers keep working while you look at another table or page.
 */
export function TableArea() {
  const { view } = useApp();
  const { open, active, layout } = useTables();
  const onTables = view.name === 'table';
  const tiled = onTables && layout === 'tile' && open.length > 1;

  if (tiled) {
    return (
      <div className="tile-area">
        <div className={`tile-grid n${open.length}`}>
          {open.map((id) => (
            <div key={String(id)} className={`tile ${active === id ? 'focused' : ''}`}>
              <TablePage id={id} compact />
            </div>
          ))}
        </div>
        {active !== null && <Chat key={String(active)} channel={{ Table: active }} title="Chat · focused table" />}
      </div>
    );
  }
  return (
    <>
      {open.map((id) => (
        <TablePage key={String(id)} id={id} visible={onTables && active === id} />
      ))}
    </>
  );
}
