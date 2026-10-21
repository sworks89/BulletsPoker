import { useRef, useState } from 'react';
import { useApp, usePoll } from '../context';
import { unwrap, type FriendRow, type ReferralView } from '../ic';
import { periodCountdown, prizeIcp } from '../rewards';
import { referralLink } from '../referrals';

const PAGE = 50;
const day = (ns: bigint) => new Date(Number(ns / 1_000_000n)).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });

/** The Referrals page: your link, what your friends have earned you, and code entry. */
export function Referrals() {
  const { backend, profile, run, notify } = useApp();
  const [view, setView] = useState<ReferralView | null>(null);
  const [more, setMore] = useState<FriendRow[]>([]);
  const [busy, setBusy] = useState(false);
  const [code, setCode] = useState('');
  const linkRef = useRef<HTMLInputElement>(null);

  const load = async () => setView(await backend.get_referrals(0));
  usePoll(load, 30000, [backend, profile?.id.toText()]);

  if (!view) return <div className="panel muted">Loading referrals…</div>;
  if (!view.code) return <div className="panel muted">Create a profile to get your referral link.</div>;

  const seen = new Set(view.rows.map((r) => r.username));
  const rows = [...view.rows, ...more.filter((r) => !seen.has(r.username))];
  const link = referralLink(window.location.origin, view.code);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(link);
      notify('Link copied');
    } catch {
      linkRef.current?.select();
      notify('Press Ctrl+C to copy the link');
    }
  };

  const loadMore = async () => {
    setBusy(true);
    try {
      const v = await backend.get_referrals(rows.length);
      setMore((m) => [...m, ...v.rows]);
    } catch {
      notify('Could not load more friends', 'err');
    }
    setBusy(false);
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!code.trim()) return;
    setBusy(true);
    const r = await run(async () => unwrap(await backend.set_referrer(code.trim())), 'Referral code added');
    if (r !== undefined) {
      setCode('');
      await load().catch(() => {});
    }
    setBusy(false);
  };

  return (
    <div className="referrals">
      <h2>Referrals</h2>
      <div className="panel">
        <div className="panel-title">Your link</div>
        <div className="row gap ref-link">
          <input ref={linkRef} readOnly value={link} onFocus={(e) => e.currentTarget.select()} aria-label="Your referral link" />
          <button className="btn" onClick={copy}>Copy</button>
        </div>
        <p className="muted small">Your code: <b>{view.code}</b></p>
        <p className="muted small">
          You get 20% of the house's cut of your friends' rake at ICP tables and ICP tournaments for 12 months. Paid every Monday.
        </p>
      </div>

      <div className="panel ref-totals">
        <div><span className="muted small">Friends</span><b>{view.friends}</b></div>
        <div><span className="muted small">Active (7 days)</span><b>{view.active_7d}</b></div>
        <div><span className="muted small">Owed this week</span><b>{prizeIcp(view.owed_this_week)} ICP</b></div>
        <div><span className="muted small">Paid in total</span><b>{prizeIcp(view.paid_total)} ICP</b></div>
        <div><span className="muted small">Next payout</span><b>{periodCountdown(view.pays_at)}</b></div>
      </div>

      {view.can_enter_code && (
        <form className="panel" onSubmit={submit}>
          <div className="panel-title">Got a code from a friend?</div>
          <div className="row gap">
            <input value={code} onChange={(e) => setCode(e.target.value)} placeholder="Their username" maxLength={32} />
            <button className="btn" disabled={busy || !code.trim()}>Add</button>
          </div>
          <p className="muted small">You can add one within your first 24 hours.</p>
        </form>
      )}
      {view.my_referrer[0] && <p className="muted small">You were referred by <b>{view.my_referrer[0]}</b>.</p>}

      <div className="panel">
        <div className="panel-title">Friends</div>
        {rows.length === 0 ? (
          <p className="muted">No friends yet. Share your link.</p>
        ) : (
          <div className="tables">
            <table className="grid">
              <thead>
                <tr><th>Player</th><th>Joined</th><th>Cut ends</th><th>Earned you</th></tr>
              </thead>
              <tbody>
                {rows.map((r) => (
                  <tr key={r.username}>
                    <td>{r.username}{r.active && <span className="chip" title="Earned you something in the last 7 days"> active</span>}</td>
                    <td>{day(r.joined_at)}</td>
                    <td>{day(r.ends_at)}</td>
                    <td>{prizeIcp(r.earned)} ICP</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {view.friends > rows.length && rows.length >= PAGE && (
          <button className="btn ghost" disabled={busy} onClick={loadMore}>Load more</button>
        )}
      </div>
    </div>
  );
}
