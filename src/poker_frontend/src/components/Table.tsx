import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { useApp } from '../context';
import { isMyTurn, useTables } from '../tables';
import { unwrap, type PublicProfile, type SeatView, type StoreItem, type TableView, type ThrowEvent } from '../ic';
import { amount, amountWithUnit, bytes, cardRank, cardSuit, CHIPS, countdown, ICP, icp, isChips, suitClass, type CurrencyV } from '../format';
import { sounds } from '../sound';
import { Chat } from './Chat';
import { NoteEditor } from './Notes';
import { SafetyActions } from './Safety';
import { JackpotCelebration } from './Jackpot';
import { Avatar } from '../art/Avatar';
import { ItemArt } from '../art/ItemArt';
import { THROW_FX, useSeatFx } from '../fx/SeatFx';
import { giftArrivals } from '../fx/gifts';
import { Burst, Mark } from '../fx/marks';

/** How long a thrown item flies, and how long its mark stays (spec: ~0.7s, ~4s plus the wipe). */
const THROW_FLIGHT_MS = 700;
const THROW_MARK_MS = 4700;
/** Items without art keep the old flight. */
const LEGACY_FLIGHT_MS = 1100;

/** How long a gift takes to fly to its seat. */
const GIFT_FLIGHT_MS = 700;

interface GiftFlight {
  id: number;
  icon: string;
  from: { left: string; top: string };
  to: { left: string; top: string };
  /** The sender isn't seated here: it drops in from above. */
  drop: boolean;
}

/** Most a time bank can hold (matches the backend). */
const TIME_BANK_MS = 30_000;

export function Card({ c, back, small }: { c?: number; back?: string; small?: boolean }) {
  const { profile } = useApp();
  if (c === undefined) {
    return <div className={`card back ${small ? 'sm' : ''}`} style={{ background: back || '#b91c1c' }} />;
  }
  return (
    <div className={`card ${suitClass(c, profile?.settings.four_color_deck ?? false)} ${small ? 'sm' : ''}`}>
      <span className="rank">{cardRank(c)}</span>
      <span className="suit">{cardSuit(c)}</span>
    </div>
  );
}

function Chips({ value, currency }: { value: bigint; currency: CurrencyV }) {
  if (value === 0n) return null;
  return (
    <div className="bet">
      <span className="chip-stack" />
      {amount(value, currency)}
    </div>
  );
}

/** Which currency to pay for an item with at a table: the table's own if the item supports it. */
export function payCurrency(item: StoreItem, tableCurrency: CurrencyV): CurrencyV {
  if (isChips(tableCurrency) && item.chip_price > 0n) return CHIPS;
  return item.price > 0n ? ICP : CHIPS;
}

export function priceLabel(item: StoreItem, currency: CurrencyV): string {
  return isChips(currency) ? `${item.chip_price.toLocaleString()} chips` : `${icp(item.price)} ICP`;
}

interface Flying {
  id: bigint;
  icon: string;
  /** No art or mark for this item: the old spin-and-fade. */
  legacy: boolean;
  from: { left: string; top: string };
  to: { left: string; top: string };
}

/**
 * One poker table. Several can be mounted at once (multi-tabling): `visible`
 * hides background tabs, and `compact` is the smaller tiled layout.
 */
export function TablePage({ id, visible = true, compact = false }: { id: bigint; visible?: boolean; compact?: boolean }) {
  const { backend, profile, run, refreshProfile, go } = useApp();
  const { open, views, clockOffset, refresh, closeTable, focus } = useTables();
  const view = views.get(String(id)) ?? null;
  const [, setTick] = useState(0);
  const [buyInSeat, setBuyInSeat] = useState<number | null>(null);
  const [selected, setSelected] = useState<SeatView | null>(null);
  const [raiseTo, setRaiseTo] = useState<bigint>(0n);
  const [busy, setBusy] = useState(false);
  const [flying, setFlying] = useState<Flying[]>([]);
  const { fx, play } = useSeatFx();
  const [giftFlights, setGiftFlights] = useState<GiftFlight[]>([]);
  // Seats whose gift is still in the air (badge hidden), just landed (pop), or just expired (fading out).
  const [giftLanding, setGiftLanding] = useState<Map<number, 'flying' | 'landed'>>(new Map());
  const [giftFading, setGiftFading] = useState<Map<number, string>>(new Map());
  const seenGifts = useRef<Map<number, bigint> | null>(null);
  const lastGiftIcons = useRef<Map<number, string>>(new Map());
  const giftFlightId = useRef(0);
  const [dismissedHit, setDismissedHit] = useState<string>('');
  const seenThrows = useRef<Set<bigint> | null>(null);
  const settings = profile?.settings;

  const load = refresh;

  useEffect(() => {
    if (!visible) return;
    const t = setInterval(() => setTick((x) => x + 1), 250);
    return () => clearInterval(t);
  }, [visible]);

  const myTurn = isMyTurn(view ?? undefined);
  const lastTurnKey = useRef('');
  useEffect(() => {
    if (!view || !myTurn) return;
    const key = `${view.hand_no}-${Object.keys(view.phase)[0]}-${view.current_bet}`;
    if (key !== lastTurnKey.current) {
      lastTurnKey.current = key;
      setRaiseTo(view.min_raise_to);
      if (settings?.sound) sounds.yourTurn();
    }
  }, [view, myTurn, settings?.sound]);

  // Sounds for new hands and wins.
  const lastHand = useRef<bigint>(0n);
  const lastWin = useRef<bigint>(0n);
  useEffect(() => {
    if (!view || !settings?.sound) return;
    if (view.hand_no !== lastHand.current) {
      if (lastHand.current !== 0n && view.my_seat.length) sounds.deal();
      lastHand.current = view.hand_no;
    }
    const hit = view.jackpot_hit[0];
    if (hit && hit.hand_no !== lastWin.current) {
      lastWin.current = hit.hand_no;
      sounds.win();
      setTimeout(sounds.win, 450);
    }
    const r = view.last_result[0];
    if (r && r.hand_no !== lastWin.current && r.winners.some((w) => w.player.toText() === profile?.id.toText())) {
      lastWin.current = r.hand_no;
      sounds.win();
    }
  }, [view, settings?.sound, profile?.id]);

  // Animate throws we haven't seen yet. Throws already present on first load are skipped.
  useEffect(() => {
    if (!view) return;
    if (seenThrows.current === null) {
      seenThrows.current = new Set(view.throws.map((t) => t.id));
      return;
    }
    const fresh = view.throws.filter((t) => !seenThrows.current!.has(t.id));
    if (fresh.length === 0) return;
    fresh.forEach((t) => seenThrows.current!.add(t.id));
    if ((settings && !settings.show_throwables) || !visible) return;
    if (settings?.sound) sounds.splat();
    const add = fresh.map((t: ThrowEvent) => ({
      id: t.id,
      icon: t.icon,
      legacy: !THROW_FX[t.icon],
      from: seatPos(view, t.from_seat),
      to: seatPos(view, t.to_seat),
    }));
    setFlying((f) => [...f, ...add]);
    for (const [k, t] of fresh.entries()) {
      const hit = THROW_FX[t.icon];
      // The item lands, then its mark sits on the face until it wipes off.
      setTimeout(() => {
        setFlying((f) => f.filter((x) => x.id !== add[k].id));
        play(
          t.to_seat,
          hit
            ? { expression: hit.expression, motion: hit.motion, overlay: <Mark kind={hit.mark} />, around: <Burst kind={hit.mark} /> }
            : { expression: 'hit', motion: 'fx-shake', around: <Burst kind="plain" /> },
          hit ? THROW_MARK_MS : 1500,
        );
      }, hit ? THROW_FLIGHT_MS : LEGACY_FLIGHT_MS);
    }
    // seatPos is a pure function of the view
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, settings?.show_throwables]);

  // Gifts fly in from the sender's seat (or drop in from above) and pop into place.
  useEffect(() => {
    if (!view) return;
    const seats = view.seats.flatMap((s) => (s[0] ? [s[0]] : []));
    const { next, arrivals } = giftArrivals(seenGifts.current, seats);
    seenGifts.current = next;
    const icons = new Map(seats.flatMap((s) => (s.gift[0] ? [[s.seat, s.gift[0]] as const] : [])));
    const gone = [...lastGiftIcons.current].filter(([seat]) => !icons.has(seat));
    lastGiftIcons.current = icons;
    if (!visible) return;
    for (const [seat, icon] of gone) {
      setGiftFading((m) => withSeat(m, seat, icon));
      setTimeout(() => setGiftFading((m) => withSeat(m, seat)), 500);
    }
    for (const a of arrivals) {
      const to = giftSpot(view, a.seat);
      const from = a.fromSeat !== null ? giftSpot(view, a.fromSeat) : { left: to.left, top: `calc(${to.top} - 90px)` };
      const flight = { id: ++giftFlightId.current, icon: a.icon, from, to, drop: a.fromSeat === null };
      setGiftFlights((f) => [...f, flight]);
      setGiftLanding((m) => withSeat(m, a.seat, 'flying'));
      setTimeout(() => {
        setGiftFlights((f) => f.filter((x) => x.id !== flight.id));
        setGiftLanding((m) => withSeat(m, a.seat, 'landed'));
      }, GIFT_FLIGHT_MS);
      setTimeout(() => setGiftLanding((m) => withSeat(m, a.seat)), GIFT_FLIGHT_MS + 900);
    }
    // giftSpot is a pure function of the view
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, visible]);

  if (!view) return visible ? <div className="splash">Loading table…</div> : null;

  const currency = view.config.currency;
  const tourney = view.tournament[0];
  const isHost = !!profile && view.room_owner[0]?.toText() === profile.id.toText();
  const mySeat = view.my_seat[0];
  const phase = Object.keys(view.phase)[0];
  const remaining = Math.max(0, Number(view.action_deadline / 1_000_000n) + clockOffset - Date.now());
  const timeout = Number(view.config.action_timeout_secs) * 1000;
  const me = mySeat !== undefined ? view.seats[mySeat][0] : undefined;

  const act = async (action: Parameters<typeof backend.act>[1]) => {
    setBusy(true);
    if (settings?.sound && !('Fold' in action) && !('Check' in action)) sounds.chips();
    await run(async () => unwrap(await backend.act(id, action)));
    await load();
    setBusy(false);
  };

  const potRaise = (fraction: number) => {
    const pot = view.pot + view.to_call;
    let to = view.current_bet + BigInt(Math.floor(Number(pot) * fraction));
    if (to < view.min_raise_to) to = view.min_raise_to;
    if (to > view.max_raise_to) to = view.max_raise_to;
    setRaiseTo(to);
  };

  const result = phase === 'HandOver' ? view.last_result[0] : undefined;
  // In tab mode, point to another open table that is waiting on you.
  const elsewhere = compact ? undefined : open.filter((o) => o !== id).map((o) => views.get(String(o))).find((v) => isMyTurn(v));
  const winnerSeats = new Set(result?.winners.map((w) => w.seat) ?? []);
  // Won without a showdown: the winner may show their cards until the next deal.
  const canShow =
    !!result && result.shown.length === 0 && result.winners.length === 1 && mySeat !== undefined && result.winners[0].seat === mySeat;
  const showCards = async () => {
    setBusy(true);
    await run(async () => unwrap(await backend.show_cards(id)));
    await load();
    setBusy(false);
  };
  const step = isChips(currency) ? 1 : Number(view.config.small_blind);

  return (
    <div
      className={`table-page ${compact ? 'compact' : ''} ${myTurn ? 'my-turn' : ''}`}
      style={visible ? undefined : { display: 'none' }}
      onMouseDown={compact ? () => focus(id) : undefined}
    >
      <div className="table-main">
        <div className="table-header">
          {compact ? (
            <button className="btn ghost small" title="Close table" onClick={() => closeTable(id)}>
              ✕
            </button>
          ) : (
            <button className="btn ghost" onClick={() => go({ name: 'lobby' })}>
              ← Lobby
            </button>
          )}
          <div>
            <b>{view.config.name}</b>
            {tourney ? (
              <span className="tag live">Tournament</span>
            ) : (
              <span className={`tag ${isChips(currency) ? 'chips' : ''}`}>{isChips(currency) ? 'Play chips' : 'ICP'}</span>
            )}
            {view.locked && <span className="tag">🔒</span>}
            {view.jackpot > 0n && (
              <span className="tag jp-tag" title="Bad Beat Jackpot: quads or better losing at showdown">
                💥 {amount(view.jackpot, currency)}
              </span>
            )}
            <span className="muted">
              {' '}
              · NLHE {amount(view.config.small_blind, currency)}/{amount(view.config.big_blind, currency)} · Hand #{String(view.hand_no)}
            </span>
          </div>
          <div className="row gap">
            {me && (
              <>
                <button
                  className="btn ghost"
                  title={tourney ? undefined : me.sitting_out ? "You'll post a big blind to come back in" : 'Missed hands mean posting a big blind when you come back'}
                  onClick={() => run(async () => unwrap(await backend.sit_out(id, !me.sitting_out)))}
                >
                  {me.sitting_out ? "I'm back" : 'Sit out'}
                </button>
                {!tourney && (
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
                )}
              </>
            )}
            {isHost && view.seats.every((s) => !s[0]) && (
              <button
                className="btn ghost"
                onClick={async () => {
                  const r = await run(async () => unwrap(await backend.close_room(id)), 'Table closed');
                  if (r !== undefined) go({ name: 'lobby' });
                }}
              >
                Close table
              </button>
            )}
          </div>
        </div>

        {tourney && (
          <div className="tourney-bar">
            <button className="chip" onClick={() => go({ name: 'tournaments', id: tourney.id })}>
              {tourney.name}
            </button>
            <span>
              Level {tourney.level} · <b>{tourney.small_blind.toLocaleString()}/{tourney.big_blind.toLocaleString()}</b>
            </span>
            {!tourney.finished && <span className="muted">Next level in {countdown(tourney.next_level_at)}</span>}
            <span className="muted">
              {tourney.players_left}/{tourney.entrants} players left
            </span>
            {tourney.can_rebuy && (
              <button
                className={`btn small ${tourney.rebuy_deadline > 0n ? 'elsewhere' : 'chips-btn'}`}
                onClick={async () => {
                  const r = await run(async () => unwrap(await backend.rebuy_tournament(tourney.id)), 'Rebuy complete. Good luck!');
                  if (r !== undefined) await Promise.all([refreshProfile(), load()]);
                }}
              >
                {tourney.rebuy_deadline > 0n
                  ? `Busted! Rebuy (${Math.max(0, Math.ceil((Number(tourney.rebuy_deadline / 1_000_000n) + clockOffset - Date.now()) / 1000))}s)`
                  : 'Rebuy'}{' '}
                · {amountWithUnit(tourney.rebuy_cost, tourney.currency)} · {tourney.rebuys_left} left
              </button>
            )}
            {tourney.can_addon && (
              <button
                className="btn small elsewhere"
                title="One-time purchase at the end of the rebuy period"
                onClick={async () => {
                  const r = await run(async () => unwrap(await backend.addon_tournament(tourney.id)), `Add-on taken: +${tourney.addon_chips.toLocaleString()} chips`);
                  if (r !== undefined) await Promise.all([refreshProfile(), load()]);
                }}
              >
                Add-on +{tourney.addon_chips.toLocaleString()} chips · {amountWithUnit(tourney.addon_cost, tourney.currency)}
              </button>
            )}
            {tourney.my_place[0] !== undefined && (
              <span className="tag live">
                You finished #{tourney.my_place[0]}
                {tourney.my_prize > 0n ? ` · won ${amountWithUnit(tourney.my_prize, CHIPS)}` : ''}
              </span>
            )}
          </div>
        )}
        {view.room_owner[0] && (
          <div className="muted small host-line">
            Hosted by {isHost ? 'you' : 'a player'}
            {isHost && !isChips(currency) ? ' · you earn 20% of the rake here' : ''}
          </div>
        )}

        {view.restricted && <UnlockRoom id={id} onDone={load} />}

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
            <div className="pot">
              {view.pot > 0n ? `Pot ${amountWithUnit(view.pot, currency)}` : phase === 'Waiting' ? 'Waiting for players…' : ''}
            </div>
            {result && (
              <div className="result">
                {result.winners.map((w) => (
                  <div key={w.seat}>
                    🏆 {view.seats[w.seat][0]?.username ?? 'Player'} wins {amount(w.amount, currency)} — {w.hand}
                  </div>
                ))}
                {canShow && (
                  <button className="btn small" disabled={busy} onClick={showCards}>
                    Show cards
                  </button>
                )}
              </div>
            )}
          </div>

          {view.seats.map((slot, i) => {
            const s = slot[0];
            const pos = seatPos(view, i);
            if (!s) {
              return (
                <div key={i} className="seat empty" style={pos}>
                  {mySeat === undefined && profile && !tourney ? (
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
            const note = s.note[0];
            const noteColor = note?.color[0] ? Object.keys(note.color[0])[0].toLowerCase() : note ? 'plain' : '';
            return (
              <div
                key={i}
                className={`seat ${acting ? 'acting' : ''} ${s.folded ? 'folded' : ''} ${winnerSeats.has(i) ? 'winner' : ''} ${s.sitting_out ? 'away' : ''}`}
                style={pos}
                onClick={() => s.player.toText() !== profile?.id.toText() && setSelected(s)}
              >
                <div className="hole">
                  {cards.length > 0
                    ? cards.map((c, k) => <Card key={k} c={c} small={i !== mySeat} />)
                    : s.has_cards
                      ? [0, 1].map((k) => <Card key={k} back={s.card_back} small />)
                      : null}
                </div>
                <Avatar
                  key={fx.get(i)?.key ?? 0}
                  icon={s.avatar}
                  expression={fx.get(i)?.expression}
                  className={`${noteColor ? `note-${noteColor}` : ''} ${fx.get(i)?.motion ?? ''} ${fx.get(i)?.cleaning ? 'cleaning' : ''}`}
                  title={note ? note.text || 'Has a note' : undefined}
                  overlay={fx.get(i)?.overlay}
                >
                  {s.gift[0] ? (
                    <span className={`gift ${giftLanding.get(i) ?? ''}`}>
                      <ItemArt icon={s.gift[0]} className="idle" />
                      {giftLanding.get(i) === 'landed' && <Burst kind="sparkle" />}
                    </span>
                  ) : (
                    giftFading.has(i) && (
                      <span className="gift fading">
                        <ItemArt icon={giftFading.get(i)!} />
                      </span>
                    )
                  )}
                  {note && <span className="note-mark">📝</span>}
                  {fx.get(i)?.around}
                </Avatar>
                <div className="nameplate">
                  <div className="name">
                    {s.username} <span className="lvl">Lv{s.level}</span>
                  </div>
                  <div className="stack">{s.all_in ? 'ALL-IN' : amount(s.stack, currency)}</div>
                  {acting && (
                    <div
                      className={`timer ${view.time_bank_active ? 'bank' : ''}`}
                      style={{ width: `${Math.min(1, remaining / (view.time_bank_active ? TIME_BANK_MS : timeout)) * 100}%` }}
                    />
                  )}
                </div>
                {acting && view.time_bank_active ? (
                  <div className="action-tag bank-tag">⏳ Time bank {Math.ceil(remaining / 1000)}s</div>
                ) : s.sitting_out && !s.in_hand ? (
                  <div className="action-tag">Sitting out</div>
                ) : (
                  s.last_action[0] && <div className="action-tag">{s.last_action[0]}</div>
                )}
                {view.button === i && phase !== 'Waiting' && <div className="dealer">D</div>}
                <Chips value={s.bet} currency={currency} />
              </div>
            );
          })}

          {giftFlights.map((g) => (
            <div
              key={`gift-${g.id}`}
              className={`gift-flight ${g.drop ? 'drop' : ''}`}
              style={{ '--x0': g.from.left, '--y0': g.from.top, '--x1': g.to.left, '--y1': g.to.top } as CSSProperties}
            >
              <ItemArt icon={g.icon} />
            </div>
          ))}
          {flying.map((f) => (
            <div
              key={String(f.id)}
              className={`flying ${f.legacy ? 'legacy' : ''}`}
              style={{ '--x0': f.from.left, '--y0': f.from.top, '--x1': f.to.left, '--y1': f.to.top } as CSSProperties}
            >
              <ItemArt icon={f.icon} />
            </div>
          ))}
        </div>

        <div className="action-bar">
          {myTurn ? (
            <>
              {me && (
                <span className={`bank-chip ${view.time_bank_active ? 'on' : ''}`} title="Extra time used automatically when your clock runs out. Refills 2s every hand.">
                  ⏳ {view.time_bank_active ? `Time bank ${Math.ceil(remaining / 1000)}s` : `${me.time_bank_secs}s bank`}
                </span>
              )}
              <button className="btn fold" disabled={busy} onClick={() => act({ Fold: null })}>
                Fold
              </button>
              {view.to_call === 0n ? (
                <button className="btn check" disabled={busy} onClick={() => act({ Check: null })}>
                  Check
                </button>
              ) : (
                <button className="btn check" disabled={busy} onClick={() => act({ Call: null })}>
                  Call {amount(view.to_call, currency)}
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
                    step={step}
                    value={Number(raiseTo)}
                    onChange={(e) => setRaiseTo(BigInt(e.target.value))}
                  />
                  <button
                    className="btn raise-btn"
                    disabled={busy}
                    onClick={() => act(raiseTo >= view.max_raise_to ? { AllIn: null } : { Raise: raiseTo })}
                  >
                    {raiseTo >= view.max_raise_to ? 'All-in' : view.current_bet === 0n ? 'Bet' : 'Raise to'} {amount(raiseTo, currency)}
                  </button>
                </div>
              )}
            </>
          ) : elsewhere ? (
            <button className="btn elsewhere" onClick={() => focus(elsewhere.id)}>
              Your turn at {elsewhere.config.name} →
            </button>
          ) : (
            <div className="muted">
              {!profile
                ? 'Sign in and create a profile to play.'
                : mySeat === undefined
                  ? 'Pick an empty seat to sit down. Click a player to send a gift or throw something.'
                  : phase === 'Waiting'
                    ? 'Waiting for another player…'
                    : 'Waiting for your turn…'}
            </div>
          )}
        </div>
      </div>

      {!compact && visible && (
        <aside className="side">
          <Chat channel={{ Table: id }} title="Table chat" />
        </aside>
      )}

      {view.jackpot_hit[0] && visible && dismissedHit !== `${id}-${view.jackpot_hit[0].hand_no}` && (
        <JackpotCelebration hit={view.jackpot_hit[0]} onClose={() => setDismissedHit(`${id}-${view.jackpot_hit[0]!.hand_no}`)} />
      )}
      {buyInSeat !== null && <BuyIn view={view} seat={buyInSeat} onClose={() => setBuyInSeat(null)} onDone={load} />}
      {selected && <PlayerCard seat={selected} view={view} canThrow={mySeat !== undefined} onClose={() => setSelected(null)} />}
    </div>
  );
}

/** A copy of `m` with `seat` set to `value`, or removed when there is no value. */
function withSeat<T>(m: Map<number, T>, seat: number, value?: T): Map<number, T> {
  const n = new Map(m);
  if (value === undefined) n.delete(seat);
  else n.set(seat, value);
  return n;
}

/** Where a seat's gift badge sits: just below and right of the seat centre. */
function giftSpot(view: TableView, i: number) {
  const p = seatPos(view, i);
  return { left: `calc(${p.left} + 24px)`, top: `calc(${p.top} + 10px)` };
}

/** Seat position around the oval, rotated so the viewer sits at the bottom. */
function seatPos(view: TableView, i: number) {
  const n = view.seats.length;
  const rotation = view.my_seat[0] ?? 0;
  const k = (i - rotation + n) % n;
  const angle = Math.PI / 2 + (k * 2 * Math.PI) / n;
  const rx = window.innerWidth < 700 ? 36 : 43;
  return { left: `${50 + rx * Math.cos(angle)}%`, top: `${50 + 40 * Math.sin(angle)}%` };
}

function BuyIn({ view, seat, onClose, onDone }: { view: TableView; seat: number; onClose: () => void; onDone: () => Promise<void> }) {
  const { backend, profile, run, refreshProfile, go } = useApp();
  const currency = view.config.currency;
  const chips = isChips(currency);
  const balance = (chips ? profile?.chips : profile?.balance) ?? 0n;
  // Coming back soon after leaving: at least the stack you left with, even above the table max.
  const returning = view.my_min_buy_in > 0n;
  const minBuyIn = view.my_min_buy_in > view.config.min_buy_in ? view.my_min_buy_in : view.config.min_buy_in;
  const tableMax = view.my_min_buy_in > view.config.max_buy_in ? view.my_min_buy_in : view.config.max_buy_in;
  const max = balance < tableMax ? balance : tableMax;
  const [value, setValue] = useState<bigint>(max);
  const [password, setPassword] = useState('');
  // Hosts, seated players and anyone who already entered the password aren't asked again.
  const needsPassword = view.restricted;
  const short = balance < minBuyIn;

  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>Buy in · Seat {seat + 1}</h3>
        <p className="muted">
          {chips ? 'Play chips' : 'Wallet balance'}: <b>{amountWithUnit(balance, currency)}</b>
        </p>
        {short ? (
          <>
            <p>You need at least {amountWithUnit(minBuyIn, currency)} to sit here{returning ? ' (you left recently with that much)' : ''}.</p>
            {chips ? (
              <p className="muted small">Claim your daily chip bonus from the top bar, or try a smaller table.</p>
            ) : (
              <button className="btn" onClick={() => go({ name: 'wallet' })}>
                Deposit ICP
              </button>
            )}
          </>
        ) : (
          <>
            {returning && (
              <p className="muted small">You left this table less than 30 minutes ago, so you buy back in with at least the {amountWithUnit(view.my_min_buy_in, currency)} you left with.</p>
            )}
            <input
              type="range"
              min={Number(minBuyIn)}
              max={Number(max)}
              step={Number(view.config.big_blind)}
              value={Number(value)}
              onChange={(e) => setValue(BigInt(e.target.value))}
            />
            <div className="big">{amountWithUnit(value, currency)}</div>
            {needsPassword && <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="Table password" />}
            <div className="row gap">
              <button className="btn ghost" onClick={onClose}>
                Cancel
              </button>
              <button
                className="btn"
                onClick={async () => {
                  const r = await run(async () => unwrap(await backend.join_table(view.id, seat, value, needsPassword ? [password] : [])), 'Good luck!');
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

function PlayerCard({ seat, view, canThrow, onClose }: { seat: SeatView; view: TableView; canThrow: boolean; onClose: () => void }) {
  const { backend, items, medals, profile, run, refreshProfile } = useApp();
  const { refresh: load } = useTables();
  const [pub, setPub] = useState<PublicProfile | null>(null);
  useEffect(() => {
    backend.get_profile(seat.player).then((p) => setPub(p[0] ?? null));
  }, [backend, seat.player]);
  const all = [...items.values()];
  const gifts = all.filter((i) => 'Gift' in i.kind);
  const throwables = all.filter((i) => 'Throwable' in i.kind);
  const isFriend = profile?.friends.some((f) => f.toText() === seat.player.toText());
  const blocked = profile?.blocked.some((b) => b.toText() === seat.player.toText());
  const tableCurrency = view.config.currency;

  const use = async (item: StoreItem, kind: 'gift' | 'throw') => {
    const cur = payCurrency(item, tableCurrency);
    const r = await run(
      async () =>
        unwrap(
          kind === 'gift'
            ? await backend.send_gift(view.id, seat.player, item.id, cur)
            : await backend.throw_item(view.id, seat.player, item.id, cur),
        ),
      kind === 'gift' ? `Sent ${item.name}!` : undefined,
    );
    if (r !== undefined) {
      await refreshProfile();
      onClose();
    }
  };

  const grid = (list: StoreItem[], kind: 'gift' | 'throw') => (
    <div className="gift-grid">
      {list.map((g) => (
        <button key={g.id} className="gift-btn" title={g.description} onClick={() => use(g, kind)}>
          <ItemArt icon={g.icon} className="icon" />
          <span>{g.name}</span>
          <span className="price">{priceLabel(g, payCurrency(g, tableCurrency))}</span>
        </button>
      ))}
    </div>
  );

  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="row gap">
          <Avatar icon={seat.avatar} size="lg" />
          <div>
            <h3>{seat.username}</h3>
            <div className="muted">Level {seat.level}</div>
            {pub && (
              <div className="muted small">
                {String(pub.stats.hands_played)} hands · {String(pub.stats.hands_won)} won
                {pub.status && <> · “{pub.status}”</>}
              </div>
            )}
          </div>
        </div>
        {profile && <NoteEditor player={seat.player} note={seat.note[0]} onSaved={load} />}
        {pub && pub.medals.length > 0 && (
          <div className="medal-strip">
            {pub.medals.map((m) => (
              <span key={m} title={medals.get(m)?.name}>
                {medals.get(m)?.icon}
              </span>
            ))}
          </div>
        )}
        {profile && !isFriend && !blocked && (
          <button className="btn" onClick={() => run(async () => unwrap(await backend.send_friend_request(seat.player)), 'Friend request sent')}>
            + Add friend
          </button>
        )}
        <SafetyActions player={seat.player} username={seat.username} channel={{ Table: view.id }} onBlocked={onClose} />
        {profile && (
          <>
            {canThrow && (
              <>
                <h4>Throw something</h4>
                {grid(throwables, 'throw')}
              </>
            )}
            <h4>Send a gift</h4>
            {grid(gifts, 'gift')}
          </>
        )}
      </div>
    </div>
  );
}

/** Password prompt for a private room the viewer hasn't unlocked yet. */
function UnlockRoom({ id, onDone }: { id: bigint; onDone: () => Promise<void> }) {
  const { backend, run, profile } = useApp();
  const [password, setPassword] = useState('');
  if (!profile) return <div className="status-banner">🔒 Private table. Sign in and enter the password to watch.</div>;
  return (
    <form
      className="status-banner row gap unlock"
      onSubmit={async (e) => {
        e.preventDefault();
        const ok = await run(async () => unwrap(await backend.unlock_room(id, password)));
        if (ok !== undefined) await onDone();
      }}
    >
      <span>🔒 Private table. Enter the password to watch and chat, or pick a seat to join.</span>
      <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} maxLength={32} placeholder="Password" />
      <button className="btn small" disabled={!password}>
        Unlock
      </button>
    </form>
  );
}
