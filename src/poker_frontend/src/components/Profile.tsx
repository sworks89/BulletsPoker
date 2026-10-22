import { useState } from 'react';
import { solvePow } from '../pow';
import { useApp, usePoll } from '../context';
import { unwrap, type LeaderboardEntry, type RewardsView, type Settings } from '../ic';
import { icp } from '../format';
import { Avatar } from '../art/Avatar';
import { Dialog } from './Dialog';
import { leaderboardMedal } from '../rewards';
import { clearRememberedRef, peekRememberedRef, rememberRef, takeRememberedRef } from '../referrals';

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
          <Avatar icon={items.get(profile.avatar)?.icon ?? ''} size="lg" />
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
          <TierBadge />
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
          <div>
            <b className={s.chip_net < 0n ? 'neg' : 'pos'}>{s.chip_net.toLocaleString()}</b>
            <span>Net chips</span>
          </div>
        </div>
      </div>

      <MedalsPanel />

      <SettingsPanel />

      <div className="panel">
        <div className="panel-title">My avatars</div>
        <div className="row gap wrap">
          {avatars.map((a) => (
            <button
              key={a.id}
              className="avatar-pick"
              title={a.name}
              onClick={async () => {
                await run(async () => unwrap(await backend.equip(a.id)));
                await refreshProfile();
              }}
            >
              <Avatar icon={a.icon} size="lg" className={`pick ${profile.avatar === a.id ? 'active' : ''}`} />
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

const SETTINGS: { key: keyof Settings; label: string; hint: string }[] = [
  { key: 'four_color_deck', label: 'Four-color deck', hint: 'Clubs green, diamonds blue — no more mixing up flushes.' },
  { key: 'auto_muck', label: 'Auto-muck losing hands', hint: "Don't reveal your cards when you lose at showdown." },
  { key: 'auto_top_up', label: 'Auto top-up', hint: 'Refill your stack to the table max when it drops below half.' },
  { key: 'sound', label: 'Sounds', hint: 'Chime on your turn, deals, wins and throws.' },
  { key: 'show_throwables', label: 'Show throwables', hint: 'Animate tomatoes, pies and friends flying across the table.' },
];

/** The player's Fish Buffet creature; opens the Rewards page. */
function TierBadge() {
  const { backend, go } = useApp();
  const [view, setView] = useState<RewardsView | null>(null);
  usePoll(async () => setView(await backend.get_rewards()), 60000, [backend]);
  if (!view) return null;
  return (
    <button className="tier-badge" onClick={() => go({ name: 'rewards' })} title="Fish Buffet tier: open Rewards">
      <Avatar icon={`tier:${view.tier}`} size="lg" />
      <span className="small">{view.tier_name}</span>
    </button>
  );
}

function MedalsPanel() {
  const { profile, medals } = useApp();
  if (!profile) return null;
  const earned = new Map(profile.medals.map((m) => [m.id, m.earned_at]));
  const staticEarned = [...medals.keys()].filter((id) => earned.has(id)).length;
  // Leaderboard medals aren't in the catalogue: their id names them. Newest first.
  const boardMedals = profile.medals
    .flatMap((m) => {
      const def = leaderboardMedal(m.id);
      return def ? [{ ...def, id: m.id, earned_at: m.earned_at }] : [];
    })
    .sort((a, b) => (b.earned_at > a.earned_at ? 1 : b.earned_at < a.earned_at ? -1 : 0));
  return (
    <div className="panel">
      <div className="panel-title">
        Medals ({staticEarned}/{medals.size})
      </div>
      {boardMedals.length > 0 && (
        <div className="medal-grid">
          {boardMedals.map((m) => (
            <div key={m.id} className="medal earned" title={m.name}>
              <span className="medal-icon">{m.icon}</span>
              <b className="small">{m.name}</b>
            </div>
          ))}
        </div>
      )}
      <div className="medal-grid">
        {[...medals.values()].map((m) => (
          <div key={m.id} className={`medal ${earned.has(m.id) ? 'earned' : ''}`} title={m.description}>
            <span className="medal-icon">{m.icon}</span>
            <b className="small">{m.name}</b>
            <span className="muted small">{m.description}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function SettingsPanel() {
  const { backend, profile, run, refreshProfile } = useApp();
  if (!profile) return null;
  const toggle = async (key: keyof Settings) => {
    const next = { ...profile.settings, [key]: !profile.settings[key] };
    await run(async () => unwrap(await backend.update_settings(next)));
    await refreshProfile();
  };
  return (
    <div className="panel">
      <div className="panel-title">Settings</div>
      {SETTINGS.map((o) => (
        <label key={o.key} className="setting">
          <input type="checkbox" checked={profile.settings[o.key]} onChange={() => toggle(o.key)} />
          <span>
            <b>{o.label}</b>
            <span className="muted small">{o.hint}</span>
          </span>
        </label>
      ))}
    </div>
  );
}

export function Leaderboard() {
  const { backend } = useApp();
  const [rows, setRows] = useState<LeaderboardEntry[]>([]);
  usePoll(async () => setRows(await backend.get_leaderboard()), 15000, [backend]);

  return (
    <div className="panel">
      <div className="panel-title">All time · net winnings</div>
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
                <Avatar icon={r.avatar} size="sm" /> {r.username}
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
  const { backend, run, notify, refreshProfile, principal } = useApp();
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [ref, setRef] = useState<string | null>(() => peekRememberedRef());

  // Sign-ups carry a small proof-of-work against bulk bot accounts. The
  // difficulty can rise while we work, so retry a couple of times.
  const register = async () => {
    for (let attempt = 0; ; attempt++) {
      const { difficulty } = await backend.get_signup_challenge();
      const nonce = await solvePow(principal!.toUint8Array(), difficulty);
      const r = await backend.register(name.trim(), [nonce]);
      if ('Ok' in r || attempt >= 2 || !r.Err.includes('Sign-up check failed')) {
        const done = unwrap(r);
        await attachReferrer();
        return done;
      }
    }
  };

  // A code from a ?ref= link, still present at submit. Signup never fails
  // because of it.
  const attachReferrer = async () => {
    const code = takeRememberedRef();
    if (!code) return;
    try {
      const r = await backend.set_referrer(code);
      if ('Err' in r) {
        notify(`Referral code: ${r.Err} — you can enter one on the Referrals page within 24 hours`, 'err');
      }
    } catch {
      // The call itself failed: keep the code so the Referrals page can retry.
      rememberRef(code);
      notify('Could not add your referral code — you can enter it on the Referrals page within 24 hours', 'err');
    }
  };

  return (
    <Dialog title="Pick your table name">
      <form
        className="dialog-form"
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          const r = await run(register, 'Welcome to the tables!');
          setBusy(false);
          if (r !== undefined) await refreshProfile();
        }}
      >
        <p className="muted small">3–16 letters, digits or underscores. This is how other players see you.</p>
        <input autoFocus aria-label="Username" value={name} onChange={(e) => setName(e.target.value)} placeholder="Username" />
        {ref && (
          <p className="muted small">
            Referred by <strong>{ref}</strong>{' '}
            <button
              type="button"
              className="link"
              onClick={() => {
                clearRememberedRef();
                setRef(null);
              }}
            >
              Remove
            </button>
          </p>
        )}
        <button className="btn" disabled={busy || !principal}>
          {busy ? 'Setting up your account…' : 'Create profile'}
        </button>
      </form>
    </Dialog>
  );
}
