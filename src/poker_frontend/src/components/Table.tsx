import { useEffect, useRef, useState } from 'react';
import { useApp, usePoll } from '../context';
import { unwrap, type SeatView, type TableView } from '../ic';
import { bytes, cardRank, cardSuit, icp, isRed } from '../format';
import { Chat } from './Chat';

export function Card({ c, back, small }: { c?: number; back?: string; small?: boolean }) {
  if (c === undefined) {
    return <div className={`card back ${small ? 'sm' : ''}`} style={{ background: back || '#b91c1c' }} />;
  }
  return (
    <div className={`card ${isRed(c) ? 'red' : ''} ${small ? 'sm' : ''}`}>
      <span className="rank">{cardRank(c)}</span>
      <span className="suit">{cardSuit(c)}</span>
    </div>
  );
}

function Chips({ amount }: { amount: bigint }) {
  if (amount === 0n) return null;
  return (
    <div className="bet">
      <span className="chip-stack" />
      {icp(amount, 4)}
    </div>
  );
}

export function TablePage({ id }: { id: bigint }) {
  const { backend, profile, run, refreshProfile, go } = useApp();
  const [view, setView] = useState<TableView | null>(null);
  const [clockOffset, setClockOffset] = useState(0);
  const [, setTick] = useState(0);
  const [buyInSeat, setBuyInSeat] = useState<number | null>(null);
  const [selected, setSelected] = useState<SeatView | null>(null);
  const [raiseTo, setRaiseTo] = useState<bigint>(0n);
  const [busy, setBusy] = useState(false);

  const load = async () => {
    const v = (await backend.get_table(id))[0] ?? null;
    if (v) setClockOffset(Date.now() - Number(v.now / 1_000_000n));
    setView(v);
  };
  usePoll(load, 1000, [backend, id]);

  useEffect(() => {
    const t = setInterval(() => setTick((x) => x + 1), 250);
    return () => clearInterval(t);
  }, []);

  const myTurn = view && view.my_seat.length && view.to_act.length && view.my_seat[0] === view.to_act[0];
  const lastTurnKey = useRef('');
  useEffect(() => {
    if (!view || !myTurn) return;
    const key = `${view.hand_no}-${view.phase && Object.keys(view.phase)[0]}-${view.current_bet}`;
    if (key !== lastTurnKey.current) {
      lastTurnKey.current = key;
      setRaiseTo(view.min_raise_to);
    }
  }, [view, myTurn]);

  if (!view) return <div className="splash">Loading table…</div>;

  const n = view.seats.length;
  const narrow = window.innerWidth < 700;
  const mySeat = view.my_seat[0];
  const rotation = mySeat ?? 0;
  const phase = Object.keys(view.phase)[0];
  const remaining = Math.max(0, Number(view.action_deadline / 1_000_000n) + clockOffset - Date.now());
  const timeout = Number(view.config.action_timeout_secs) * 1000;
  const me = mySeat !== undefined ? view.seats[mySeat][0] : undefined;

  const act = async (action: Parameters<typeof backend.act>[1]) => {
    setBusy(true);
    await run(async () => unwrap(await backend.act(id, action)));
    await load();
    setBusy(false);
  };

  const seatPos = (i: number) => {
    const k = (i - rotation + n) % n;
    const angle = Math.PI / 2 + (k * 2 * Math.PI) / n;
    const rx = narrow ? 36 : 43;
    return { left: `${50 + rx * Math.cos(angle)}%`, top: `${50 + 40 * Math.sin(angle)}%` };
  };

  const potRaise = (fraction: number) => {
    const pot = view.pot + view.to_call;
    let to = view.current_bet + BigInt(Math.floor(Number(pot) * fraction));
    if (to < view.min_raise_to) to = view.min_raise_to;
    if (to > view.max_raise_to) to = view.max_raise_to;
    setRaiseTo(to);
  };

  const result = phase === 'HandOver' ? view.last_result[0] : undefined;
  const winnerSeats = new Set(result?.winners.map((w) => w.seat) ?? []);

  return (
    <div className="table-page">
      <div className="table-main">
        <div className="table-header">
          <button className="btn ghost" onClick={() => go({ name: 'lobby' })}>
            ← Lobby
          </button>
          <div>
            <b>{view.config.name}</b>
            <span className="muted">
              {' '}
              · NLHE {icp(view.config.small_blind)}/{icp(view.config.big_blind)} ICP · Hand #{String(view.hand_no)}
            </span>
          </div>
          <div className="row gap">
            {me && (
              <>
                <button className="btn ghost" onClick={() => run(async () => unwrap(await backend.sit_out(id, !me.sitting_out)))}>
                  {me.sitting_out ? "I'm back" : 'Sit out'}
                </button>
                <button
                  className="btn ghost"
                  onClick={async () => {
                    const r = await run(async () => unwrap(await backend.leave_table(id)));
                    if (r !== undefined) {
                      await refreshProfile();
                      await load();
                    }
                  }}
                >
                  Stand up
                </button>
              </>
            )}
          </div>
        </div>

        <div className="felt-wrap">
          <div className="felt">
            <div className="felt-logo">♠ BULLETS ♠</div>
            <div className="board">
              {bytes(view.board).map((c, i) => (
                <Card key={i} c={c} />
              ))}
              {Array.from({ length: 5 - view.board.length }, (_, i) => (
                <div key={`e${i}`} className="card slot" />
              ))}
            </div>
            <div className="pot">{view.pot > 0n ? `Pot ${icp(view.pot, 4)} ICP` : phase === 'Waiting' ? 'Waiting for players…' : ''}</div>
            {result && (
              <div className="result">
                {result.winners.map((w) => (
                  <div key={w.seat}>
                    🏆 {view.seats[w.seat][0]?.username ?? 'Player'} wins {icp(w.amount, 4)} — {w.hand}
                  </div>
                ))}
              </div>
            )}
          </div>

          {view.seats.map((slot, i) => {
            const s = slot[0];
            const pos = seatPos(i);
            if (!s) {
              return (
                <div key={i} className="seat empty" style={pos}>
                  {mySeat === undefined && profile ? (
                    <button className="btn small" onClick={() => setBuyInSeat(i)}>
                      Sit
                    </button>
                  ) : (
                    <span className="muted small">Empty</span>
                  )}
                </div>
              );
            }
            const acting = view.to_act[0] === i;
            const cards = bytes(s.cards);
            return (
              <div
                key={i}
                className={`seat ${acting ? 'acting' : ''} ${s.folded ? 'folded' : ''} ${winnerSeats.has(i) ? 'winner' : ''} ${s.sitting_out ? 'away' : ''}`}
                style={pos}
                onClick={() => s.player.toText() !== profile?.id.toText() && setSelected(s)}
              >
                <div className="hole">
                  {cards.length > 0 ? cards.map((c, k) => <Card key={k} c={c} small={i !== mySeat} />) : s.has_cards ? [0, 1].map((k) => <Card key={k} back={s.card_back} small />) : null}
                </div>
                <div className="avatar">
                  {s.avatar}
                  {s.gift[0] && <span className="gift">{s.gift[0]}</span>}
                </div>
                <div className="nameplate">
                  <div className="name">
                    {s.username} <span className="lvl">Lv{s.level}</span>
                  </div>
                  <div className="stack">{s.all_in ? 'ALL-IN' : `${icp(s.stack, 4)}`}</div>
                  {acting && <div className="timer" style={{ width: `${(remaining / timeout) * 100}%` }} />}
                </div>
                {s.sitting_out && !s.in_hand ? (
                  <div className="action-tag">Sitting out</div>
                ) : (
                  s.last_action[0] && <div className="action-tag">{s.last_action[0]}</div>
                )}
                {view.button === i && phase !== 'Waiting' && <div className="dealer">D</div>}
                <Chips amount={s.bet} />
              </div>
            );
          })}
        </div>

        <div className="action-bar">
          {myTurn ? (
            <>
              <button className="btn fold" disabled={busy} onClick={() => act({ Fold: null })}>
                Fold
              </button>
              {view.to_call === 0n ? (
                <button className="btn check" disabled={busy} onClick={() => act({ Check: null })}>
                  Check
                </button>
              ) : (
                <button className="btn check" disabled={busy} onClick={() => act({ Call: null })}>
                  Call {icp(view.to_call, 4)}
                </button>
              )}
              {view.max_raise_to > view.current_bet && view.max_raise_to > view.to_call && (
                <div className="raise">
                  <div className="row gap">
                    <button className="chip" onClick={() => setRaiseTo(view.min_raise_to)}>
                      Min
                    </button>
                    <button className="chip" onClick={() => potRaise(0.5)}>
                      ½ Pot
                    </button>
                    <button className="chip" onClick={() => potRaise(1)}>
                      Pot
                    </button>
                    <button className="chip" onClick={() => setRaiseTo(view.max_raise_to)}>
                      Max
                    </button>
                  </div>
                  <input
                    type="range"
                    min={Number(view.min_raise_to)}
                    max={Number(view.max_raise_to)}
                    step={Number(view.config.small_blind)}
                    value={Number(raiseTo)}
                    onChange={(e) => setRaiseTo(BigInt(e.target.value))}
                  />
                  <button
                    className="btn raise-btn"
                    disabled={busy}
                    onClick={() => act(raiseTo >= view.max_raise_to ? { AllIn: null } : { Raise: raiseTo })}
                  >
                    {raiseTo >= view.max_raise_to ? 'All-in' : view.current_bet === 0n ? 'Bet' : 'Raise to'} {icp(raiseTo, 4)}
                  </button>
                </div>
              )}
            </>
          ) : (
            <div className="muted">
              {!profile
                ? 'Sign in and create a profile to play.'
                : mySeat === undefined
                  ? 'Pick an empty seat to sit down.'
                  : phase === 'Waiting'
                    ? 'Waiting for another player…'
                    : 'Waiting for your turn…'}
            </div>
          )}
        </div>
      </div>

      <aside className="side">
        <Chat channel={{ Table: id }} title="Table chat" />
      </aside>

      {buyInSeat !== null && <BuyIn view={view} seat={buyInSeat} onClose={() => setBuyInSeat(null)} onDone={load} />}
      {selected && <PlayerCard seat={selected} tableId={id} onClose={() => setSelected(null)} />}
    </div>
  );
}

function BuyIn({ view, seat, onClose, onDone }: { view: TableView; seat: number; onClose: () => void; onDone: () => Promise<void> }) {
  const { backend, profile, run, refreshProfile, go } = useApp();
  const balance = profile?.balance ?? 0n;
  const max = balance < view.config.max_buy_in ? balance : view.config.max_buy_in;
  const [amount, setAmount] = useState<bigint>(max);
  const short = balance < view.config.min_buy_in;

  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>Buy in · Seat {seat + 1}</h3>
        <p className="muted">
          Wallet balance: <b>{icp(balance)} ICP</b>
        </p>
        {short ? (
          <>
            <p>You need at least {icp(view.config.min_buy_in)} ICP to sit here.</p>
            <button className="btn" onClick={() => go({ name: 'wallet' })}>
              Deposit ICP
            </button>
          </>
        ) : (
          <>
            <input
              type="range"
              min={Number(view.config.min_buy_in)}
              max={Number(max)}
              step={Number(view.config.big_blind)}
              value={Number(amount)}
              onChange={(e) => setAmount(BigInt(e.target.value))}
            />
            <div className="big">{icp(amount)} ICP</div>
            <div className="row gap">
              <button className="btn ghost" onClick={onClose}>
                Cancel
              </button>
              <button
                className="btn"
                onClick={async () => {
                  const r = await run(async () => unwrap(await backend.join_table(view.id, seat, amount)), 'Good luck!');
                  if (r !== undefined) {
                    onClose();
                    await Promise.all([refreshProfile(), onDone()]);
                  }
                }}
              >
                Sit down
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function PlayerCard({ seat, tableId, onClose }: { seat: SeatView; tableId: bigint; onClose: () => void }) {
  const { backend, items, profile, run, refreshProfile } = useApp();
  const gifts = [...items.values()].filter((i) => 'Gift' in i.kind);
  const isFriend = profile?.friends.some((f) => f.toText() === seat.player.toText());

  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="row gap">
          <span className="avatar lg">{seat.avatar}</span>
          <div>
            <h3>{seat.username}</h3>
            <div className="muted">Level {seat.level}</div>
          </div>
        </div>
        {profile && !isFriend && (
          <button className="btn" onClick={() => run(async () => unwrap(await backend.send_friend_request(seat.player)), 'Friend request sent')}>
            + Add friend
          </button>
        )}
        {profile && (
          <>
            <h4>Send a gift</h4>
            <div className="gift-grid">
              {gifts.map((g) => (
                <button
                  key={g.id}
                  className="gift-btn"
                  title={g.description}
                  onClick={async () => {
                    const r = await run(async () => unwrap(await backend.send_gift(tableId, seat.player, g.id)), `Sent ${g.name}!`);
                    if (r !== undefined) {
                      await refreshProfile();
                      onClose();
                    }
                  }}
                >
                  <span className="icon">{g.icon}</span>
                  <span>{g.name}</span>
                  <span className="price">{icp(g.price)}</span>
                </button>
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
