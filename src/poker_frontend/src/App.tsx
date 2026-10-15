import { AppProvider, useApp, type View } from './context';
import { countdown, icp } from './format';
import { unwrap, type Roles, type ServiceStatus } from './ic';
import { useEffect, useState } from 'react';
import { usePoll } from './context';
import { History } from './components/History';
import { Tournaments } from './components/Tournaments';
import { Lobby } from './components/Lobby';
import { TableArea, TableStrip } from './components/MultiTable';
import { TablesProvider, useTables } from './tables';
import { Wallet } from './components/Wallet';
import { Store } from './components/Store';
import { Friends } from './components/Friends';
import { Leaderboard, ProfilePage, Register } from './components/Profile';
import { AccountNotice } from './components/Safety';
import { Admin } from './components/Admin';

const NAV: { name: View['name']; label: string }[] = [
  { name: 'lobby', label: 'Lobby' },
  { name: 'tournaments', label: 'Tournaments' },
  { name: 'store', label: 'Store' },
  { name: 'friends', label: 'Friends & Notes' },
  { name: 'history', label: 'History' },
  { name: 'leaderboard', label: 'Leaderboard' },
  { name: 'wallet', label: 'Wallet' },
];

function Shell() {
  const { view, go, principal, profile, items, signIn, signOut, run, notify, backend, refreshProfile } = useApp();
  const dailyReady = !!profile && Number(profile.next_daily_at / 1_000_000n) <= Date.now();
  const claimDaily = async () => {
    const got = await run(async () => unwrap(await backend.claim_daily_chips()));
    if (got !== undefined) {
      await refreshProfile();
      notify(`+${got.toLocaleString()} chips! Come back tomorrow to grow your streak.`);
    }
  };
  const requests = profile?.friend_requests.length ?? 0;
  const [roles, setRoles] = useState<Roles>({ admin: false, moderator: false });
  useEffect(() => {
    if (!principal) {
      setRoles({ admin: false, moderator: false });
      return;
    }
    backend.get_my_roles().then(setRoles).catch(() => {});
  }, [backend, principal]);
  const staff = roles.admin || roles.moderator;

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand" onClick={() => go({ name: 'lobby' })}>
          <span className="logo">♠</span> Bullets Poker
          <span className="tag">on ICP</span>
        </div>
        <nav>
          {NAV.map((n) => (
            <button key={n.name} className={`nav ${view.name === n.name ? 'active' : ''}`} onClick={() => go({ name: n.name } as View)}>
              {n.label}
              {n.name === 'friends' && requests > 0 && <span className="badge">{requests}</span>}
            </button>
          ))}
        </nav>
        <div className="account">
          {staff && (
            <button
              className={`admin-btn ${view.name === 'admin' ? 'active' : ''}`}
              onClick={() => go({ name: 'admin' })}
              title={roles.admin ? 'Admin' : 'Moderation'}
              aria-label={roles.admin ? 'Admin' : 'Moderation'}
            >
              🛡️
            </button>
          )}
          {profile ? (
            <>
              <button
                className={`daily ${dailyReady ? 'ready' : ''}`}
                onClick={claimDaily}
                title={dailyReady ? `Claim ${profile.next_daily_amount.toLocaleString()} free chips` : `Next bonus in ${countdown(profile.next_daily_at)}`}
              >
                🎁 {dailyReady ? `+${profile.next_daily_amount.toLocaleString()}` : countdown(profile.next_daily_at)}
              </button>
              <span className="chips-balance" title="Play chips">
                🎲 {profile.chips.toLocaleString()}
              </span>
              <button className="balance" onClick={() => go({ name: 'wallet' })}>
                {icp(profile.balance, 4)} ICP
              </button>
              <button className="me" onClick={() => go({ name: 'profile' })}>
                <span className="avatar sm">{items.get(profile.avatar)?.icon}</span>
                <span className="me-name">{profile.username}</span>
              </button>
              <button className="btn ghost small" onClick={() => run(signOut)}>
                Sign out
              </button>
            </>
          ) : principal ? (
            <button className="btn ghost small" onClick={() => run(signOut)}>
              Sign out
            </button>
          ) : (
            <button className="btn" onClick={() => run(signIn)}>
              Sign in with Internet Identity
            </button>
          )}
        </div>
      </header>

      <StatusBanner />
      <AccountNotice />
      <ConnectionBanner />
      <TableStrip />

      <main>
        {view.name === 'lobby' && <Lobby />}
        <TableArea />
        {view.name === 'wallet' && <Wallet />}
        {view.name === 'store' && <Store />}
        {view.name === 'friends' && <Friends />}
        {view.name === 'leaderboard' && <Leaderboard />}
        {view.name === 'history' && <History />}
        {view.name === 'tournaments' && <Tournaments key={String(view.id ?? '')} initial={view.id} />}
        {view.name === 'profile' && <ProfilePage />}
        {view.name === 'admin' && <Admin roles={roles} />}
      </main>

      <footer className="footer muted small">
        Real-money play may be restricted where you live. Play responsibly. 18+.
      </footer>

      {principal && !profile && <Register />}
    </div>
  );
}

/** Shown while table updates are failing (dropped connection). */
function ConnectionBanner() {
  const { reconnecting } = useTables();
  if (!reconnecting) return null;
  return (
    <div className="status-banner reconnecting">
      <span className="spinner" /> Connection lost, reconnecting… If it's your turn, your time bank starts automatically when your clock runs out.
    </div>
  );
}

/** Shows a banner while any part of the service is paused for maintenance. */
function StatusBanner() {
  const { backend } = useApp();
  const [status, setStatus] = useState<ServiceStatus | null>(null);
  usePoll(async () => setStatus(await backend.get_status()), 30000, [backend]);
  const p = status?.pause;
  if (!p) return null;
  const paused = [p.deposits && 'deposits', p.withdrawals && 'withdrawals', p.dealing && 'new hands', p.joins && 'new seats'].filter(Boolean);
  if (paused.length === 0) return null;
  return (
    <div className="status-banner">
      ⚠️ Temporarily paused: {paused.join(', ')}.{p.reason[0] ? ` ${p.reason[0]}` : ''} Your balance is safe.
    </div>
  );
}

export default function App() {
  return (
    <AppProvider>
      <TablesProvider>
        <Shell />
      </TablesProvider>
    </AppProvider>
  );
}
