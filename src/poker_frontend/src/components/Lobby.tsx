import { useState } from 'react';
import { useApp, usePoll } from '../context';
import type { FriendInfo, TableSummary } from '../ic';
import { amount, isChips } from '../format';
import { Chat } from './Chat';
import { CreateRoom } from './CreateRoom';
import { JackpotBanner } from './Jackpot';
import { CHIPS, ICP } from '../format';
import { Avatar } from '../art/Avatar';

export function Lobby() {
  const { backend, go, profile } = useApp();
  const [tables, setTables] = useState<TableSummary[]>([]);
  const [friends, setFriends] = useState<FriendInfo[]>([]);
  const [mode, setMode] = useState<'chips' | 'icp'>('chips');
  const [creating, setCreating] = useState(false);

  usePoll(
    async () => {
      const [t, f] = await Promise.all([backend.list_tables(), profile ? backend.get_friends() : Promise.resolve([])]);
      setTables(t);
      setFriends(f);
    },
    3000,
    [backend, profile?.id.toText()],
  );

  const shown = tables.filter((t) => isChips(t.currency) === (mode === 'chips'));

  return (
    <div className="lobby">
      <section className="panel tables">
        <div className="panel-title row">
          <span>Cash Tables · No-Limit Hold'em</span>
          <div className="tabs">
            <button className={`chip ${mode === 'chips' ? 'active' : ''}`} onClick={() => setMode('chips')}>
              🎲 Play chips
            </button>
            <button className={`chip ${mode === 'icp' ? 'active' : ''}`} onClick={() => setMode('icp')}>
              ∞ ICP
            </button>
            <button className="chip" onClick={() => go({ name: 'tournaments' })}>
              🏆 Tournaments
            </button>
            {profile && (
              <button className="btn small" onClick={() => setCreating(true)}>
                + Host a table
              </button>
            )}
          </div>
        </div>
        {mode === 'chips' ? (
          <p className="muted small">Free play chips: no deposit needed. You get 20,000 to start and a daily bonus that grows with your streak.</p>
        ) : (
          <p className="muted small">Real ICP tables. Deposit from the Wallet page; 2.5% rake (capped) applies only when there's a flop.</p>
        )}
        <JackpotBanner currency={mode === 'chips' ? CHIPS : ICP} />
        <table className="grid">
          <thead>
            <tr>
              <th>Table</th>
              <th>Stakes ({mode === 'chips' ? 'chips' : 'ICP'})</th>
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
                  {t.locked && <span className="tag">🔒</span>}
                  {t.room_owner[0] !== undefined && <div className="muted small">hosted by {t.room_owner[0]}</div>}
                </td>
                <td>
                  {amount(t.small_blind, t.currency)} / {amount(t.big_blind, t.currency)}
                </td>
                <td>
                  {amount(t.min_buy_in, t.currency)} – {amount(t.max_buy_in, t.currency)}
                </td>
                <td>
                  <div className="seatbar">
                    {Array.from({ length: t.max_seats }, (_, i) => (
                      <span key={i} className={i < t.players ? 'on' : ''} />
                    ))}
                  </div>
                  {t.players}/{t.max_seats}
                </td>
                <td>{t.avg_pot > 0n ? amount(t.avg_pot, t.currency) : '—'}</td>
                <td>
                  <button className="btn small">{t.players >= t.max_seats ? 'Watch' : 'Join'}</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      {creating && <CreateRoom onClose={() => setCreating(false)} />}
      <aside className="side">
        {profile && (
          <div className="panel">
            <div className="panel-title">Friends online</div>
            {friends.filter((f) => f.online).length === 0 && <div className="muted small">Nobody yet — add friends from a table or the Friends tab.</div>}
            {friends
              .filter((f) => f.online)
              .map((f) => (
                <div key={f.id.toText()} className="friend-row">
                  <Avatar icon={f.avatar} size="sm" />
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
