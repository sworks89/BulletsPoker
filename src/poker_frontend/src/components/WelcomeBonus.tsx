import { useState } from 'react';
import { useApp, usePoll } from '../context';
import type { WelcomeView } from '../ic';
import { prizeIcp } from '../rewards';
import { daysLeft, welcomeProgress } from '../welcome';

/** The caller's welcome bonus, refreshed every 30 s; `reload` refreshes it now. */
export function useWelcome() {
  const { backend, profile } = useApp();
  const [view, setView] = useState<WelcomeView | null>(null);
  const reload = async () => setView(await backend.get_welcome());
  usePoll(reload, 30000, [backend, profile?.id.toText()]);
  return { view, reload };
}

/** Shown before the first deposit. */
export function WelcomeBanner() {
  return (
    <div className="panel welcome-banner">
      <div className="panel-title">Welcome bonus</div>
      <p className="small">Your first deposit is matched with up to 100 ICP of bonus, released as you play. Every 1 ICP of rake you pay releases 0.3 ICP of bonus.</p>
    </div>
  );
}

/** Progress of a granted bonus. */
export function WelcomeCard({ view }: { view: WelcomeView }) {
  const pct = Math.round(welcomeProgress(view) * 100);
  const days = daysLeft(view.expires_at, Date.now());
  return (
    <div className="panel welcome-card">
      <div className="panel-title">Welcome bonus</div>
      <div className="row">
        <span>{prizeIcp(view.locked)} ICP locked</span>
        <span className="muted small">{prizeIcp(view.released)} ICP released</span>
      </div>
      <div className="xpbar meter" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct} aria-label="Welcome bonus released">
        <div style={{ width: `${pct}%` }} />
      </div>
      <p className="muted small">
        {days} {days === 1 ? 'day' : 'days'} left. Every 1 ICP of rake you pay releases 0.3 ICP of bonus.
      </p>
    </div>
  );
}
