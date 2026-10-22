import { useState } from 'react';
import { useApp, usePoll } from '../context';
import type { JackpotHit, JackpotInfo } from '../ic';
import { amountWithUnit, bytes, CHIPS, ICP, timeAgo, type CurrencyV } from '../format';
import { Card } from './Table';
import { Dialog } from './Dialog';

/** Lobby banner: both jackpots, the rules, and recent hits. */
export function JackpotBanner({ currency }: { currency: CurrencyV }) {
  const { backend } = useApp();
  const [info, setInfo] = useState<JackpotInfo | null>(null);
  const [open, setOpen] = useState(false);
  usePoll(async () => setInfo(await backend.get_jackpots()), 5000, [backend]);
  if (!info) return null;
  const amount = 'Chips' in currency ? info.chips : info.icp;
  const pct = (bps: bigint) => `${Number(bps) / 100}%`;

  return (
    <div className="jackpot-banner">
      <div className="row gap">
        <span className="jp-icon">💥</span>
        <div className="grow">
          <div className="jp-label">Bad Beat Jackpot</div>
          <div className="jp-amount">{amountWithUnit(amount, currency)}</div>
        </div>
        <div className="muted small jp-other">
          {'Chips' in currency ? `ICP jackpot: ${amountWithUnit(info.icp, ICP)}` : `Chip jackpot: ${amountWithUnit(info.chips, CHIPS)}`}
        </div>
        <button className="chip" onClick={() => setOpen((o) => !o)}>
          {open ? 'Hide' : 'How it works'}
        </button>
      </div>
      {open && (
        <div className="jp-rules small">
          <p>
            <b>Hits when:</b> a hand of {info.qualifying_hand} loses at showdown, with at least {info.min_players} players dealt in. Cash tables only.
          </p>
          <p>
            <b>Pays:</b> {pct(info.payout_bps)} of the jackpot: {pct(info.loser_bps)} to the player with the losing hand,{' '}
            {pct(info.winner_bps)} to the winner, and the rest shared by everyone else dealt in. The remainder seeds the next jackpot.
          </p>
          <p>
            <b>Funded by:</b> {pct(BigInt(info.drop_bps))} of each cash pot that sees a flop (after rake), capped at one big blind.
          </p>
          {info.recent.length > 0 && (
            <>
              <b>Recent hits</b>
              {info.recent.slice(0, 5).map((h) => (
                <div key={`${h.table_id}-${h.hand_no}`} className="jp-recent">
                  {timeAgo(h.time)} · {h.table_name}: {h.loser_name}'s {h.loser_hand} lost to {h.winner_name}'s {h.winner_hand} ·{' '}
                  <b>{amountWithUnit(h.total, h.currency)}</b>
                </div>
              ))}
            </>
          )}
        </div>
      )}
    </div>
  );
}

/** Full-table celebration for a jackpot hit in the hand on screen. */
export function JackpotCelebration({ hit, onClose }: { hit: JackpotHit; onClose: () => void }) {
  const { profile } = useApp();
  const mine = hit.payouts.find((p) => p.player.toText() === profile?.id.toText());
  return (
    <Dialog title="Bad beat jackpot" onClose={onClose} hideTitle passive bgClass="jp-celebrate" boxClass="jp-card">
        <div className="jp-title">💥 BAD BEAT JACKPOT 💥</div>
        <div className="jp-total">{amountWithUnit(hit.total, hit.currency)}</div>
        <div className="jp-hands">
          <div>
            <div className="small muted">{hit.loser_name} · {hit.loser_hand}</div>
            <div className="row gap">{bytes(hit.loser_cards).map((c, i) => <Card key={i} c={c} small />)}</div>
          </div>
          <div className="jp-vs">lost to</div>
          <div>
            <div className="small muted">{hit.winner_name} · {hit.winner_hand}</div>
            <div className="row gap">{bytes(hit.winner_cards).map((c, i) => <Card key={i} c={c} small />)}</div>
          </div>
        </div>
        <div className="row gap jp-board">{bytes(hit.board).map((c, i) => <Card key={i} c={c} small />)}</div>
        <div className="jp-payouts">
          {hit.payouts.map((p) => (
            <div key={p.player.toText()} className={p.player.toText() === profile?.id.toText() ? 'me' : ''}>
              <span>{p.username}</span>
              <span className="muted small">{p.share}</span>
              <b>{amountWithUnit(p.amount, hit.currency)}</b>
            </div>
          ))}
        </div>
        {mine && <div className="jp-mine">You won {amountWithUnit(mine.amount, hit.currency)}!</div>}
        <button className="btn" onClick={onClose}>
          Nice!
        </button>
    </Dialog>
  );
}
