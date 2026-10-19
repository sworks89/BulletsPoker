import { Fragment, useState } from 'react';
import { useApp, usePoll } from '../context';
import type { FriendInfo, TableSummary } from '../ic';
import { amount, isChips } from '../format';
import { Chat } from './Chat';
import { CreateRoom } from './CreateRoom';
import { JackpotBanner } from './Jackpot';
import { CHIPS, ICP } from '../format';
import { Avatar } from '../art/Avatar';
import { groupTables, type GameRow } from '../lobby';
import { JoinGame } from './JoinGame';

const OPEN_KEY = 'bp.lobby.open';

function SeatBar({ t }: { t: TableSummary }) {
  return (
    <>
      <div className="seatbar">
        {Array.from({ length: t.max_seats }, (_, i) => (
          <span key={i} className={i < t.players ? 'on' : ''} />
        ))}
      </div>
      {t.players}/{t.max_seats}
    </>
  );
}

export function Lobby() {
  const { backend, go, profile } = useApp();
  const [tables, setTables] = useState<TableSummary[]>([]);
  const [friends, setFriends] = useState<FriendInfo[]>([]);
  const [mode, setMode] = useState<'chips' | 'icp'>('chips');
  const [creating, setCreating] = useState(false);
  const [joining, setJoining] = useState<GameRow | null>(null);
  // Which games with several tables are folded open (per browser).
  const [openGames, setOpenGames] = useState<string[]>(() => {
    try {
      return JSON.parse(localStorage.getItem(OPEN_KEY) ?? '[]') as string[];
    } catch {
      return [];
    }
  });
  const isOpen = (game: bigint) => openGames.includes(String(game));
  const toggle = (game: bigint) => {
    const next = isOpen(game) ? openGames.filter((g) => g !== String(game)) : [...openGames, String(game)];
    setOpenGames(next);
    try {
      localStorage.setItem(OPEN_KEY, JSON.stringify(next));
    } catch {
      // storage unavailable: the fold just isn't remembered
    }
  };

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
            {groupTables(shown).map((row) => {
              const t = row.first;
              const many = row.tables.length > 1;
              const unfolded = many && (row.full || isOpen(row.game));
              const watch = (id: bigint) => go({ name: 'table', id });
              return (
                <Fragment key={`${row.room ? 'room' : 'game'}-${t.id}`}>
                  <tr
                    className={many ? 'game-row' : ''}
                    onClick={() => (many ? toggle(row.game) : watch(t.id))}
                  >
                    <td>
                      {many && <span className="caret">{unfolded ? '▾' : '▸'}</span>}
                      <b>{t.name}</b>
                      {t.max_seats === 2 && <span className="tag">HU</span>}
                      {t.locked && <span className="tag">🔒</span>}
                      {many && <span className="tag copies">{row.tables.length} tables</span>}
                      {t.room_owner[0] !== undefined && <div className="muted small">hosted by {t.room_owner[0]}</div>}
                    </td>
                    <td>
                      {amount(t.small_blind, t.currency)} / {amount(t.big_blind, t.currency)}
                    </td>
                    <td>
                      {amount(t.min_buy_in, t.currency)} – {amount(t.max_buy_in, t.currency)}
                    </td>
                    <td>
                      {many ? (
                        <>
                          {row.players} playing · <span className="muted">{row.open === 1 ? '1 seat open' : `${row.open} seats open`}</span>
                        </>
                      ) : (
                        <SeatBar t={t} />
                      )}
                    </td>
                    <td>{!many && t.avg_pot > 0n ? amount(t.avg_pot, t.currency) : '—'}</td>
                    <td onClick={(e) => e.stopPropagation()}>
                      {row.room ? (
                        <button className="btn small" onClick={() => watch(t.id)}>
                          {t.players >= t.max_seats ? 'Watch' : 'Join'}
                        </button>
                      ) : (
                        <div className="row gap">
                          {row.full ? (
                            <button className="btn small" disabled title="Every table is full. Watch one and turn on auto-sit.">
                              Full
                            </button>
                          ) : (
                            <button className="btn small" disabled={!profile} onClick={() => setJoining(row)}>
                              Join
                            </button>
                          )}
                          {!many && (
                            <button className="btn small ghost" onClick={() => watch(t.id)}>
                              Watch
                            </button>
                          )}
                        </div>
                      )}
                    </td>
                  </tr>
                  {unfolded &&
                    row.tables.map((c) => (
                      <tr key={String(c.id)} className="copy-row" onClick={() => watch(c.id)}>
                        <td className="copy-name">Table {c.number}</td>
                        <td />
                        <td />
                        <td>
                          <SeatBar t={c} />
                        </td>
                        <td>{c.avg_pot > 0n ? amount(c.avg_pot, c.currency) : '—'}</td>
                        <td>
                          <button className="btn small ghost">Watch</button>
                        </td>
                      </tr>
                    ))}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </section>

      {creating && <CreateRoom onClose={() => setCreating(false)} />}
      {joining && <JoinGame row={joining} onClose={() => setJoining(null)} />}
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
