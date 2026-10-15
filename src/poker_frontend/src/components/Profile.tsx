import { useState } from 'react';
import { useApp, usePoll } from '../context';
import { unwrap, type LeaderboardEntry } from '../ic';
import { icp } from '../format';

export function ProfilePage() {
  const { backend, profile, items, run, refreshProfile } = useApp();
  const [status, setStatus] = useState(profile?.status ?? '');
  if (!profile) return <div className="panel">Sign in to see your profile.</div>;

  const s = profile.stats;
  const lvl = profile.level;
  const base = 50 * (lvl - 1) ** 2;
  const next = 50 * lvl ** 2;
  const pct = Math.min(100, ((Number(profile.xp) - base) / (next - base)) * 100);
  const avatars = [...items.values()].filter((i) => 'Avatar' in i.kind && (i.price === 0n || profile.inventory.includes(i.id)));

  return (
    <div className="page-grid">
      <div className="panel">
        <div className="row gap">
          <span className="avatar lg">{items.get(profile.avatar)?.icon}</span>
          <div className="grow">
            <h2>{profile.username}</h2>
            <div className="muted">Level {lvl}</div>
            <div className="xpbar">
              <div style={{ width: `${pct}%` }} />
            </div>
            <div className="muted small">
              {String(profile.xp)} / {next} XP
            </div>
          </div>
        </div>
        <form
          className="row gap"
          onSubmit={async (e) => {
            e.preventDefault();
            await run(async () => unwrap(await backend.set_status(status)), 'Status updated');
            await refreshProfile();
          }}
        >
          <input value={status} maxLength={80} onChange={(e) => setStatus(e.target.value)} placeholder="Status message" />
          <button className="btn">Save</button>
        </form>
      </div>

      <div className="panel">
        <div className="panel-title">Stats</div>
        <div className="stats">
          <div>
            <b>{String(s.hands_played)}</b>
            <span>Hands</span>
          </div>
          <div>
            <b>{String(s.hands_won)}</b>
            <span>Won</span>
          </div>
          <div>
            <b>{s.hands_played > 0n ? Math.round((Number(s.hands_won) / Number(s.hands_played)) * 100) : 0}%</b>
            <span>Win rate</span>
          </div>
          <div>
            <b>{icp(s.biggest_pot_won, 4)}</b>
            <span>Biggest pot</span>
          </div>
          <div>
            <b className={s.net_winnings < 0n ? 'neg' : 'pos'}>{icp(s.net_winnings, 4)}</b>
            <span>Net ICP</span>
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-title">My avatars</div>
        <div className="row gap wrap">
          {avatars.map((a) => (
            <button
              key={a.id}
              className={`avatar lg pick ${profile.avatar === a.id ? 'active' : ''}`}
              title={a.name}
              onClick={async () => {
                await run(async () => unwrap(await backend.equip(a.id)));
                await refreshProfile();
              }}
            >
              {a.icon}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

export function Leaderboard() {
  const { backend } = useApp();
  const [rows, setRows] = useState<LeaderboardEntry[]>([]);
  usePoll(async () => setRows(await backend.get_leaderboard()), 15000, [backend]);

  return (
    <div className="panel">
      <div className="panel-title">Leaderboard · net winnings</div>
      <table className="grid">
        <thead>
          <tr>
            <th>#</th>
            <th>Player</th>
            <th>Level</th>
            <th>Hands</th>
            <th>Won</th>
            <th>Net ICP</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={r.id.toText()}>
              <td>{i + 1}</td>
              <td>
                <span className="avatar sm">{r.avatar}</span> {r.username}
              </td>
              <td>{r.level}</td>
              <td>{String(r.stats.hands_played)}</td>
              <td>{String(r.stats.hands_won)}</td>
              <td className={r.stats.net_winnings < 0n ? 'neg' : 'pos'}>{icp(r.stats.net_winnings, 4)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <p className="muted">No hands played yet.</p>}
    </div>
  );
}

export function Register() {
  const { backend, run, refreshProfile } = useApp();
  const [name, setName] = useState('');
  return (
    <div className="modal-bg">
      <form
        className="modal"
        onSubmit={async (e) => {
          e.preventDefault();
          const r = await run(async () => unwrap(await backend.register(name.trim())), 'Welcome to the tables!');
          if (r !== undefined) await refreshProfile();
        }}
      >
        <h3>Pick your table name</h3>
        <p className="muted small">3–16 letters, digits or underscores. This is how other players see you.</p>
        <input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="Username" />
        <button className="btn">Create profile</button>
      </form>
    </div>
  );
}
