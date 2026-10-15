import { useState } from 'react';
import { useApp, usePoll } from '../context';
import type { FriendInfo, TableSummary } from '../ic';
import { icp } from '../format';
import { Chat } from './Chat';

export function Lobby() {
  const { backend, go, profile } = useApp();
  const [tables, setTables] = useState<TableSummary[]>([]);
  const [friends, setFriends] = useState<FriendInfo[]>([]);
  const [filter, setFilter] = useState<'all' | 'micro' | 'low' | 'high'>('all');

  usePoll(
    async () => {
      const [t, f] = await Promise.all([backend.list_tables(), profile ? backend.get_friends() : Promise.resolve([])]);
      setTables(t);
      setFriends(f);
    },
    3000,
    [backend, profile?.id.toText()],
  );

  const shown = tables.filter((t) => {
    const bb = Number(t.big_blind);
    if (filter === 'micro') return bb <= 20_000;
    if (filter === 'low') return bb > 20_000 && bb <= 200_000;
    if (filter === 'high') return bb > 200_000;
    return true;
  });

  return (
    <div className="lobby">
      <section className="panel tables">
        <div className="panel-title row">
          <span>Cash Tables · No-Limit Hold'em</span>
          <div className="tabs">
            {(['all', 'micro', 'low', 'high'] as const).map((f) => (
              <button key={f} className={`chip ${filter === f ? 'active' : ''}`} onClick={() => setFilter(f)}>
                {f}
              </button>
            ))}
          </div>
        </div>
        <table className="grid">
          <thead>
            <tr>
              <th>Table</th>
              <th>Stakes (ICP)</th>
              <th>Buy-in</th>
              <th>Players</th>
              <th>Last pot</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {shown.map((t) => (
              <tr key={String(t.id)} onClick={() => go({ name: 'table', id: t.id })}>
                <td>
                  <b>{t.name}</b>
                  {t.max_seats === 2 && <span className="tag">HU</span>}
                </td>
                <td>
                  {icp(t.small_blind)} / {icp(t.big_blind)}
                </td>
                <td>
                  {icp(t.min_buy_in)} – {icp(t.max_buy_in)}
                </td>
                <td>
                  <div className="seatbar">
                    {Array.from({ length: t.max_seats }, (_, i) => (
                      <span key={i} className={i < t.players ? 'on' : ''} />
                    ))}
                  </div>
                  {t.players}/{t.max_seats}
                </td>
                <td>{t.avg_pot > 0n ? icp(t.avg_pot, 4) : '—'}</td>
                <td>
                  <button className="btn small">{t.players >= t.max_seats ? 'Watch' : 'Join'}</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <aside className="side">
        {profile && (
          <div className="panel">
            <div className="panel-title">Friends online</div>
            {friends.filter((f) => f.online).length === 0 && <div className="muted small">Nobody yet — add friends from a table or the Friends tab.</div>}
            {friends
              .filter((f) => f.online)
              .map((f) => (
                <div key={f.id.toText()} className="friend-row">
                  <span className="avatar sm">{f.avatar}</span>
                  <span className="grow">{f.username}</span>
                  {f.tables.length > 0 && (
                    <button className="btn small" onClick={() => go({ name: 'table', id: f.tables[0] })}>
                      Join
                    </button>
                  )}
                </div>
              ))}
          </div>
        )}
        <Chat channel={{ Lobby: null }} title="Lobby chat" />
      </aside>
    </div>
  );
}
