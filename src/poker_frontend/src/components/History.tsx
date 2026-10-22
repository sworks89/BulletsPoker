import { useEffect, useState } from 'react';
import { useApp, usePoll } from '../context';
import type { HandRecord, HandSummary, LogEntry } from '../ic';
import { amount, amountWithUnit, bytes, timeAgo } from '../format';
import { Card } from './Table';

const BOARD_AT: Record<string, number> = { Waiting: 0, PreFlop: 0, Flop: 3, Turn: 4, River: 5, HandOver: 5 };
const STREETS = new Set(['Flop', 'Turn', 'River']);

/** Pot size after the first `step` log entries. */
function potAt(log: LogEntry[], step: number): bigint {
  let banked = 0n;
  const street = new Map<number, bigint>();
  for (const e of log.slice(0, step)) {
    const seat = e.seat[0];
    if (seat === undefined) {
      if (STREETS.has(e.text)) {
        for (const v of street.values()) banked += v;
        street.clear();
      }
    } else if (e.text === 'Uncalled bet returned') {
      street.set(seat, (street.get(seat) ?? 0n) - e.amount);
    } else if (!e.text.startsWith('Wins') && !e.text.startsWith('Shows') && e.amount > 0n) {
      street.set(seat, e.amount);
    }
  }
  for (const v of street.values()) banked += v;
  return banked;
}

export function History() {
  const { backend, profile } = useApp();
  const [hands, setHands] = useState<HandSummary[]>([]);
  const [selected, setSelected] = useState<bigint | null>(null);

  usePoll(
    async () => {
      const h = await backend.get_hand_history(50);
      setHands(h);
      setSelected((cur) => cur ?? h[0]?.id ?? null);
    },
    10000,
    [backend],
  );

  if (!profile) return <div className="panel">Sign in to see your hand history.</div>;

  return (
    <div className="history">
      <div className="panel hand-list">
        <div className="panel-title">Your last {hands.length} hands</div>
        {hands.length === 0 && <p className="muted">Play a hand and it will show up here.</p>}
        {hands.map((h) => (
          <button key={String(h.id)} className={`hand-row ${selected === h.id ? 'active' : ''}`} onClick={() => setSelected(h.id)}>
            <div className="hand-cards">
              {bytes(h.my_cards).map((c, i) => (
                <Card key={i} c={c} small />
              ))}
            </div>
            <div className="grow">
              <div className="small">
                <b>{h.table_name}</b> <span className="muted">· {timeAgo(h.time)}</span>
              </div>
              <div className="muted small">Pot {amountWithUnit(h.pot, h.currency)}</div>
            </div>
            <div className={h.net > 0n ? 'pos' : h.net < 0n ? 'neg' : 'muted'}>
              {h.net > 0n ? '+' : ''}
              {amount(h.net, h.currency)}
            </div>
          </button>
        ))}
      </div>
      {selected !== null && <Replay id={selected} />}
    </div>
  );
}

function Replay({ id }: { id: bigint }) {
  const { backend, profile } = useApp();
  const [hand, setHand] = useState<HandRecord | null>(null);
  const [step, setStep] = useState(0);
  const [playing, setPlaying] = useState(false);

  useEffect(() => {
    setHand(null);
    backend.get_hand(id).then((h) => {
      setHand(h[0] ?? null);
      setStep(h[0]?.log.length ?? 0);
    });
  }, [backend, id]);

  useEffect(() => {
    if (!playing || !hand) return;
    if (step >= hand.log.length) {
      setPlaying(false);
      return;
    }
    const t = setTimeout(() => setStep((s) => s + 1), 900);
    return () => clearTimeout(t);
  }, [playing, step, hand]);

  if (!hand) return <div className="panel replay muted">Loading hand…</div>;

  const log = hand.log;
  const last = step > 0 ? log[step - 1] : undefined;
  const boardCount = last ? BOARD_AT[Object.keys(last.phase)[0]] : 0;
  const board = bytes(hand.board).slice(0, boardCount);
  const done = step >= log.length;
  const seatName = (seat: number | undefined) => hand.players.find((p) => p.seat === seat)?.username ?? 'Dealer';

  return (
    <div className="panel replay">
      <div className="panel-title row">
        <span>
          {hand.table_name} · Hand #{String(hand.hand_no)}
        </span>
        <span className="muted small">
          {amount(hand.small_blind, hand.currency)}/{amount(hand.big_blind, hand.currency)}
        </span>
      </div>

      <div className="replay-felt">
        <div className="board">
          {board.map((c, i) => (
            <Card key={i} c={c} />
          ))}
          {Array.from({ length: 5 - board.length }, (_, i) => (
            <div key={`e${i}`} className="card slot" />
          ))}
        </div>
        <div className="pot">Pot {amountWithUnit(done ? hand.pot : potAt(log, step), hand.currency)}</div>
      </div>

      <div className="replay-players">
        {hand.players.map((p) => {
          const acting = last?.seat[0] === p.seat;
          const mine = p.player.toText() === profile?.id.toText();
          return (
            <div key={p.seat} className={`replay-player ${acting ? 'acting' : ''} ${done && p.won > 0n ? 'winner' : ''}`}>
              <div className="hole">
                {bytes(p.cards).length > 0 && (mine || done) ? bytes(p.cards).map((c, i) => <Card key={i} c={c} small />) : <span className="muted small">{done ? 'cards hidden' : ''}</span>}
              </div>
              <div className="small">
                <b>{p.username}</b>
                {p.seat === hand.button && <span className="tag">D</span>}
              </div>
              {done && (
                <div className={`small ${p.won > p.committed ? 'pos' : 'neg'}`}>
                  {p.won > p.committed ? '+' : ''}
                  {amount(p.won - p.committed, hand.currency)}
                </div>
              )}
            </div>
          );
        })}
      </div>

      <div className="row gap replay-controls">
        <button className="chip" onClick={() => { setPlaying(false); setStep(0); }}>⏮</button>
        <button className="chip" onClick={() => { setPlaying(false); setStep((s) => Math.max(0, s - 1)); }}>◀</button>
        <button className="btn small" onClick={() => { if (done) setStep(0); setPlaying((p) => !p); }}>
          {playing ? 'Pause' : done ? 'Replay' : 'Play'}
        </button>
        <button className="chip" onClick={() => { setPlaying(false); setStep((s) => Math.min(log.length, s + 1)); }}>▶</button>
        <button className="chip" onClick={() => { setPlaying(false); setStep(log.length); }}>⏭</button>
        <span className="muted small">
          {step}/{log.length}
        </span>
      </div>

      <ul className="hand-log">
        {log.map((e, i) => (
          <li key={i} className={`${i < step ? '' : 'future'} ${i === step - 1 ? 'current' : ''} ${e.seat.length ? '' : 'street'}`} >
            <button className="btn-reset log-btn" aria-current={i === step - 1 ? 'step' : undefined} onClick={() => { setPlaying(false); setStep(i + 1); }}>
            {e.seat.length ? (
              <>
                <b>{seatName(e.seat[0])}</b> {e.text.toLowerCase()}
                {e.amount > 0n && <span className="muted"> {amount(e.amount, hand.currency)}</span>}
              </>
            ) : (
              <b>— {e.text} —</b>
            )}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
