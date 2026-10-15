import { AppProvider, useApp, type View } from './context';
import { countdown, icp } from './format';
import { unwrap } from './ic';
import { History } from './components/History';
import { Tournaments } from './components/Tournaments';
import { Lobby } from './components/Lobby';
import { TableArea, TableStrip } from './components/MultiTable';
import { TablesProvider } from './tables';
import { Wallet } from './components/Wallet';
import { Store } from './components/Store';
import { Friends } from './components/Friends';
import { Leaderboard, ProfilePage, Register } from './components/Profile';

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
      </main>

      <footer className="footer muted small">
        Real-money play may be restricted where you live. Play responsibly. 18+.
      </footer>

      {principal && !profile && <Register />}
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
