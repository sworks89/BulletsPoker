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
import { ProfilePage, Register } from './components/Profile';
import { Rewards, TierWatch } from './components/Rewards';
import { Referrals } from './components/Referrals';
import { AccountNotice } from './components/Safety';
import { Admin } from './components/Admin';
import { Avatar } from './art/Avatar';
import { shouldOfferRegistration } from './startup';

const NAV: { name: View['name']; label: string }[] = [
  { name: 'lobby', label: 'Lobby' },
  { name: 'tournaments', label: 'Tournaments' },
  { name: 'store', label: 'Store' },
  { name: 'friends', label: 'Friends & Notes' },
  { name: 'history', label: 'History' },
  { name: 'rewards', label: 'Rewards' },
  { name: 'referrals', label: 'Referrals' },
  { name: 'wallet', label: 'Wallet' },
];

function Shell() {
  const { view, go, principal, profile, profileLoad, retryProfile, items, signIn, signOut, run, notify, backend, refreshProfile } = useApp();
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
        <button className="brand btn-reset" onClick={() => go({ name: 'lobby' })} aria-label="Bullets Poker, go to lobby">
          <span className="logo">♠</span> Bullets Poker
          <span className="tag">on ICP</span>
        </button>
        <nav>
          {NAV.map((n) => (
            <button key={n.name} className={`nav ${view.name === n.name || (n.name === 'rewards' && view.name === 'leaderboard') ? 'active' : ''}`} onClick={() => go({ name: n.name } as View)}>
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
                <Avatar icon={items.get(profile.avatar)?.icon ?? ''} size="sm" />
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

      <TierWatch />
      <StatusBanner />
      <AccountNotice />
      <ConnectionBanner />
      <TableStrip />

      {principal && !profile && profileLoad === 'failed' && (
        <div className="panel" role="alert">
          <h3>Couldn't load your profile</h3>
          <p className="muted">Your account is fine; we just couldn't reach it. Retrying automatically.</p>
          <button className="btn" onClick={retryProfile}>
            Retry
          </button>
        </div>
      )}

      <main>
        {view.name === 'lobby' && <Lobby />}
        <TableArea />
        {view.name === 'wallet' && <Wallet />}
        {view.name === 'store' && <Store />}
        {view.name === 'friends' && <Friends />}
        {(view.name === 'rewards' || view.name === 'leaderboard') && (
          <Rewards key={view.name} initial={view.name === 'leaderboard' ? 'boards' : 'fish'} />
        )}
        {view.name === 'referrals' && <Referrals />}
        {view.name === 'history' && <History />}
        {view.name === 'tournaments' && <Tournaments key={String(view.id ?? '')} initial={view.id} />}
        {view.name === 'profile' && <ProfilePage />}
        {view.name === 'admin' && <Admin roles={roles} />}
      </main>

      <footer className="footer muted small">
        Real-money play may be restricted where you live. Play responsibly. 18+.
      </footer>

      {principal && !profile && shouldOfferRegistration(profileLoad) && <Register />}
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

/** One accurate sentence per pause flag that is set. Balances are never described as simply "safe". */
export function pauseNotes(p: { deposits: boolean; withdrawals: boolean; dealing: boolean; joins: boolean }): string[] {
  const notes: string[] = [];
  if (p.deposits) notes.push('Deposits are paused. Please do not send new funds until they reopen.');
  if (p.withdrawals) notes.push('Withdrawals are paused while we check balances. Nothing is lost; withdrawals resume after the check.');
  if (p.dealing) notes.push('No new hands are being dealt. Hands already in progress will finish.');
  if (p.joins) notes.push('New tables, seats and tournament entries are paused. You can keep playing where you are seated.');
  return notes;
}

/** Shows a banner while any part of the service is paused for maintenance. */
function StatusBanner() {
  const { backend } = useApp();
  const [status, setStatus] = useState<ServiceStatus | null>(null);
  usePoll(async () => setStatus(await backend.get_status()), 30000, [backend]);
  const p = status?.pause;
  if (!p) return null;
  const notes = pauseNotes(p);
  if (notes.length === 0) return null;
  return (
    <div className="status-banner">
      ⚠️ {notes.join(' ')}
      {p.reason[0] ? ` Reason: ${p.reason[0]}` : ''}
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
