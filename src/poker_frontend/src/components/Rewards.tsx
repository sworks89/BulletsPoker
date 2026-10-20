import { useEffect, useRef, useState } from 'react';
import { useApp, usePoll } from '../context';
import { unwrap, type BoardRow, type BoardView, type RewardsView, type SpinRecord } from '../ic';
import { icp } from '../format';
import { Avatar } from '../art/Avatar';
import { Leaderboard } from './Profile';
import {
  BOARD_NAMES,
  MIN_HANDS,
  WHEEL_X10,
  formatMultiplier,
  handsNeeded,
  isWinningsBoard,
  meterProgress,
  periodCountdown,
  tierProgress,
  wheelAngle,
} from '../rewards';

export type RewardsTab = 'fish' | 'boards';

/** How long the wheel spins (keep in step with `.wheel-disc` in styles.css). */
const SPIN_MS = 4200;

const reducedMotion = () =>
  typeof window !== 'undefined' && !!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;

/** The Rewards page: the Fish Buffet (tiers, the wheel) and the leaderboards. */
export function Rewards({ initial = 'fish' }: { initial?: RewardsTab }) {
  const [tab, setTab] = useState<RewardsTab>(initial);
  return (
    <div className="rewards">
      <div className="row gap rewards-head">
        <h2>Rewards</h2>
        <div className="tabs">
          <button className={`chip ${tab === 'fish' ? 'active' : ''}`} onClick={() => setTab('fish')}>
            🐟 Fish Buffet
          </button>
          <button className={`chip ${tab === 'boards' ? 'active' : ''}`} onClick={() => setTab('boards')}>
            🏆 Leaderboards
          </button>
        </div>
      </div>
      {tab === 'fish' ? <FishBuffet /> : <Leaderboards />}
    </div>
  );
}

const pct = (x: number) => `${Math.round(x * 1000) / 10}%`;
const rate = (bps: bigint) => `${Number(bps) / 100}%`;
const pts = (n: bigint) => n.toLocaleString();
const when = (ns: bigint) =>
  new Date(Number(ns / 1_000_000n)).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });

function FishBuffet() {
  const { backend, profile, run, refreshProfile, notify } = useApp();
  const [view, setView] = useState<RewardsView | null>(null);
  const [rotation, setRotation] = useState(0);
  const [spinning, setSpinning] = useState(false);
  const [result, setResult] = useState<SpinRecord | null>(null);
  // The history as it was when the wheel started, so the list doesn't give the result away.
  const [held, setHeld] = useState<SpinRecord[] | null>(null);
  const lastTier = useRef<number | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  const load = async () => {
    const v = await backend.get_rewards();
    if (lastTier.current !== null && v.tier > lastTier.current) notify(`🎉 Tier up! Welcome to ${v.tier_name}.`);
    lastTier.current = v.tier;
    setView(v);
  };
  usePoll(load, 15000, [backend, profile?.id.toText()]);
  // Leaving mid-spin skips the reveal, but the balance must still catch up.
  const refreshRef = useRef(refreshProfile);
  refreshRef.current = refreshProfile;
  useEffect(
    () => () => {
      if (timer.current === undefined) return;
      clearTimeout(timer.current);
      refreshRef.current().catch(() => {});
    },
    [],
  );

  const spin = async () => {
    if (!view || spinning) return;
    setSpinning(true);
    setResult(null);
    setHeld(view.history);
    const rec = await run(async () => unwrap(await backend.spin_wheel()));
    if (rec === undefined) {
      setSpinning(false);
      setHeld(null);
      return;
    }
    setRotation((r) => Math.ceil(r / 360) * 360 + wheelAngle(rec.segment, WHEEL_X10.length));
    timer.current = setTimeout(
      async () => {
        timer.current = undefined;
        setResult(rec);
        setSpinning(false);
        setHeld(null);
        await Promise.all([refreshProfile(), load()]).catch(() => {});
      },
      reducedMotion() ? 0 : SPIN_MS,
    );
  };

  if (!view) return <div className="panel muted">Loading the buffet…</div>;
  const next = view.tiers[view.tier + 1];
  const decay = view.decay_starts_in_days[0];
  const history = held ?? view.history;

  return (
    <div className="page-grid">
      {profile ? (
        <div className="panel tier-panel">
          <div className="row gap">
            <Avatar icon={`tier:${view.tier}`} size="lg" className="tier-creature" title={view.tier_name} />
            <div className="grow">
              <div className="muted small">Your Fish Buffet tier</div>
              <h2 className="tier-name">{view.tier_name}</h2>
              <div className="muted small">
                {pts(view.tier_points)} tier points · {rate(view.tiers[view.tier]?.rate_bps ?? 0n)} cashback
              </div>
            </div>
          </div>
          <div className="xpbar" title={pct(tierProgress(view))}>
            <div style={{ width: pct(tierProgress(view)) }} />
          </div>
          <div className="muted small">
            {next && view.next_tier_at[0] !== undefined
              ? `${pts(view.next_tier_at[0] - view.tier_points)} points to ${next.name}`
              : 'Top of the food chain! 🦈'}
          </div>
          {decay !== undefined && decay <= 2n && (
            <div className="decay-hint">
              {decay === 0n
                ? `⏳ Your tier points are shrinking 5% a day: you last earned points ${view.days_since_earned} days ago. Play a hand at an ICP table to stop it.`
                : `⏳ Your tier points start shrinking in ${decay} day${decay === 1n ? '' : 's'} (last earned ${view.days_since_earned} days ago). Play a hand at an ICP table to keep them.`}
            </div>
          )}
          <div className="muted small">{pts(view.lifetime_points)} lifetime points</div>
        </div>
      ) : (
        <div className="panel">
          <div className="panel-title">Fish Buffet</div>
          <p className="muted">Sign in to collect points, climb the tiers and spin the cashback wheel.</p>
        </div>
      )}

      {profile && (
        <div className="panel wheel-panel">
          <div className="panel-title">Cashback wheel</div>
          <div className="muted small">
            Spin meter: {pts(view.meter)} / {pts(view.step)} points to your next spin
          </div>
          <div className="xpbar meter">
            <div style={{ width: pct(meterProgress(view)) }} />
          </div>
          <Wheel rotation={rotation} spinning={spinning} />
          <button className="btn spin-btn" disabled={spinning || view.spins_banked === 0} onClick={spin}>
            {spinning ? 'Spinning…' : `Spin (${view.spins_banked})`}
          </button>
          <div className="spin-result" aria-live="polite">
            {result && (
              <>
                <b className="pos">
                  +{icp(result.paid, 6)} ICP · {formatMultiplier(result.multiplier_x10)}
                </b>
                {result.capped && <div className="muted small">The pool was short this time.</div>}
              </>
            )}
            {!result && !spinning && view.spins_banked === 0 && (
              <span className="muted small">Fill the meter at ICP tables to bank a spin.</span>
            )}
          </div>
          <div className="muted small">Fish Buffet pool: {icp(view.fish_pool, 4)} ICP</div>
        </div>
      )}

      {profile && (
        <div className="panel">
          <div className="panel-title">Recent spins</div>
          {history.length === 0 ? (
            <p className="muted">No spins yet.</p>
          ) : (
            <table className="grid">
              <thead>
                <tr>
                  <th>When</th>
                  <th>Tier</th>
                  <th>Wheel</th>
                  <th>Paid</th>
                </tr>
              </thead>
              <tbody>
                {history.map((s) => (
                  <tr key={String(s.time)}>
                    <td className="small">{when(s.time)}</td>
                    <td>{s.tier}</td>
                    <td>{formatMultiplier(s.multiplier_x10)}</td>
                    <td className="pos">
                      {icp(s.paid, 6)}
                      {s.capped && (
                        <span className="muted small" title="The pool was short">
                          {' '}
                          (pool short)
                        </span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      <div className="panel">
        <div className="panel-title">Tiers</div>
        <table className="grid tiers">
          <thead>
            <tr>
              <th>Tier</th>
              <th>From</th>
              <th>Cashback</th>
            </tr>
          </thead>
          <tbody>
            {view.tiers.map((t, i) => (
              <tr key={t.name} className={profile && i === view.tier ? 'current' : ''}>
                <td>
                  <Avatar icon={`tier:${i}`} size="sm" /> {t.name}
                </td>
                <td>{pts(t.from)} pts</td>
                <td>{rate(t.rate_bps)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <p className="muted small">Points come from rake paid at ICP tables and ICP tournament fees.</p>
      </div>
    </div>
  );
}

const SEGMENT_COLORS = ['#38bdf8', '#22c55e', '#a855f7', '#f97316', '#ef4444', '#facc15'];

/** The wheel: segments in the backend's order, clockwise from the top, under a fixed pointer. */
function Wheel({ rotation, spinning }: { rotation: number; spinning: boolean }) {
  const n = WHEEL_X10.length;
  const r = 92;
  // Degrees clockwise from the top to svg coordinates.
  const xy = (deg: number, radius: number): [number, number] => {
    const a = (deg * Math.PI) / 180;
    return [+(radius * Math.sin(a)).toFixed(2), +(-radius * Math.cos(a)).toFixed(2)];
  };
  const at = (deg: number, radius: number) => xy(deg, radius).join(' ');
  return (
    <div className={`wheel ${spinning ? 'spinning' : ''}`}>
      <svg className="wheel-disc" viewBox="-100 -100 200 200" style={{ transform: `rotate(${rotation}deg)` }} aria-hidden="true">
        <circle r="98" fill="#1b1b1b" />
        {WHEEL_X10.map((x, i) => {
          const a0 = (i * 360) / n;
          const a1 = ((i + 1) * 360) / n;
          const mid = (a0 + a1) / 2;
          return (
            <g key={i}>
              <path d={`M0 0 L${at(a0, r)} A${r} ${r} 0 0 1 ${at(a1, r)} Z`} fill={SEGMENT_COLORS[i]} stroke="#1b1b1b" strokeWidth="3" />
              <text
                transform={`rotate(${mid}) translate(0 -58)`}
                textAnchor="middle"
                dominantBaseline="middle"
                className={`wheel-label ${x >= 50n ? 'big' : ''}`}
              >
                {formatMultiplier(x)}
              </text>
            </g>
          );
        })}
        {Array.from({ length: n }, (_, i) => {
          const [cx, cy] = xy((i * 360) / n, 86);
          return <circle key={i} cx={cx} cy={cy} r="3.5" fill="#fff7d6" stroke="#1b1b1b" strokeWidth="1.5" />;
        })}
        <circle r="16" fill="#facc15" stroke="#1b1b1b" strokeWidth="3" />
        <text textAnchor="middle" dominantBaseline="central" fontSize="16">
          🐟
        </text>
      </svg>
      <svg className="wheel-pointer" viewBox="0 0 30 34" aria-hidden="true">
        <path d="M3 3 h24 l-12 28z" fill="#ef4444" stroke="#1b1b1b" strokeWidth="3" strokeLinejoin="round" />
      </svg>
    </div>
  );
}

function Leaderboards() {
  const { backend, profile } = useApp();
  const [boards, setBoards] = useState<BoardView[] | null>(null);
  usePoll(async () => setBoards(await backend.get_leaderboards()), 15000, [backend, profile?.id.toText()]);
  const now = Date.now();
  return (
    <div className="page-grid boards">
      {boards === null && <div className="panel muted">Loading the boards…</div>}
      {boards?.map((b) => <Board key={b.board} b={b} now={now} me={profile?.id.toText()} />)}
      <Leaderboard />
    </div>
  );
}

const PLACE = ['', '🥇', '🥈', '🥉'];

function Board({ b, now, me }: { b: BoardView; now: number; me?: string }) {
  const winnings = isWinningsBoard(b.board);
  const value = (r: BoardRow) => (winnings ? `${icp(r.value, 4)} ICP` : `${r.value.toLocaleString()} pts`);
  const mine = b.me[0];
  const showMe = mine && (mine.rank === 0 || mine.rank > b.rows.length);
  const row = (r: BoardRow, you: boolean) => {
    const needs = winnings ? handsNeeded(b.board, r.hands) : 0;
    return (
      <tr key={`${r.player.toText()}${you ? '-me' : ''}`} className={`${you ? 'you' : ''} ${r.qualified ? '' : 'unqualified'}`}>
        <td>{r.rank > 0 ? r.rank : '—'}</td>
        <td>
          <Avatar icon={r.avatar} size="sm" /> {r.username}
          {you && <span className="tag">you</span>}
          {!r.qualified && needs > 0 && <div className="muted small">needs {needs} more hands</div>}
        </td>
        <td className={winnings ? (r.value < 0n ? 'neg' : 'pos') : ''}>{value(r)}</td>
        <td>{r.hands}</td>
        <td className="prize">{r.qualified && r.prize > 0n ? icp(r.prize, 4) : '—'}</td>
      </tr>
    );
  };
  return (
    <div className="panel board">
      <div className="row">
        <div className="panel-title">
          {BOARD_NAMES[b.board] ?? b.board} <span className="tag">{b.label}</span>
        </div>
        <span className="muted small" title="Time until this board resets">
          ⏱ {periodCountdown(b.ends_at, now)}
        </span>
      </div>
      <div className="board-pool">
        Prize pool <b>{icp(b.pool, 4)} ICP</b>
        {winnings && <span className="muted small"> · {MIN_HANDS[b.board]} hands to qualify</span>}
      </div>
      {b.rows.length === 0 && !showMe ? (
        <p className="muted">No one on the board yet. Be the first!</p>
      ) : (
        <table className="grid">
          <thead>
            <tr>
              <th>#</th>
              <th>Player</th>
              <th>{winnings ? 'Net ICP' : 'Points'}</th>
              <th>Hands</th>
              <th>Prize</th>
            </tr>
          </thead>
          <tbody>
            {b.rows.map((r) => row(r, r.player.toText() === me))}
            {showMe && (
              <>
                <tr className="gap-row">
                  <td colSpan={5}>⋯</td>
                </tr>
                {row(mine, true)}
              </>
            )}
          </tbody>
        </table>
      )}
      {b.last_winners.length > 0 && (
        <div className="last-winners">
          <div className="muted small">Last period's winners</div>
          {b.last_winners.map((w) => (
            <div key={`${w.rank}-${w.player.toText()}`} className="winner small">
              <span>{PLACE[w.rank] ?? `#${w.rank}`}</span>
              <Avatar icon={w.avatar} size="sm" />
              <span className="grow">{w.username}</span>
              <b className="pos">{icp(w.prize, 4)} ICP</b>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
