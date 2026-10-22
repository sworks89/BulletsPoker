import { useEffect, useState } from 'react';
import { useApp } from '../context';
import { unwrap, type Channel, type Principal, type PublicProfile, type ReportReason } from '../ic';
import { countdown } from '../format';
import { Dialog } from './Dialog';

const REASONS: { key: string; label: string }[] = [
  { key: 'AbusiveChat', label: 'Abusive chat' },
  { key: 'Cheating', label: 'Cheating' },
  { key: 'Collusion', label: 'Collusion / chip dumping' },
  { key: 'OffensiveName', label: 'Offensive name or status' },
  { key: 'Spam', label: 'Spam' },
  { key: 'Other', label: 'Something else' },
];

/** Block and Report buttons for another player. */
export function SafetyActions({ player, username, channel, onBlocked }: { player: Principal; username: string; channel?: Channel; onBlocked?: () => void }) {
  const { backend, profile, run, refreshProfile } = useApp();
  const [reporting, setReporting] = useState(false);
  if (!profile || profile.id.toText() === player.toText()) return null;
  const blocked = profile.blocked.some((b) => b.toText() === player.toText());

  const toggleBlock = async () => {
    if (!blocked && !confirm(`Block ${username}? You won't see their chat, and they can't message, befriend, gift or throw things at you.`)) return;
    const r = await run(async () => unwrap(await backend.block_player(player, !blocked)), blocked ? `Unblocked ${username}` : `Blocked ${username}`);
    if (r !== undefined) {
      await refreshProfile();
      if (!blocked) onBlocked?.();
    }
  };

  return (
    <div className="row gap safety">
      <button className="btn ghost small" onClick={toggleBlock}>
        {blocked ? 'Unblock' : '🚫 Block'}
      </button>
      <button className="btn ghost small" onClick={() => setReporting(true)}>
        ⚑ Report
      </button>
      {reporting && <ReportModal player={player} username={username} channel={channel} onClose={() => setReporting(false)} />}
    </div>
  );
}

function ReportModal({ player, username, channel, onClose }: { player: Principal; username: string; channel?: Channel; onClose: () => void }) {
  const { backend, run } = useApp();
  const [reason, setReason] = useState('AbusiveChat');
  const [details, setDetails] = useState('');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    const r = await run(
      async () => unwrap(await backend.report_player(player, { [reason]: null } as ReportReason, details, channel ? [channel] : [])),
      'Thanks. A moderator will review your report.',
    );
    setBusy(false);
    if (r !== undefined) onClose();
  };

  return (
    <Dialog title={`Report ${username}`} onClose={onClose}>
        <div className="report-reasons">
          {REASONS.map((r) => (
            <label key={r.key} className="small">
              <input type="radio" name="reason" checked={reason === r.key} onChange={() => setReason(r.key)} />
              {r.label}
            </label>
          ))}
        </div>
        <textarea aria-label="Details (optional)" value={details} onChange={(e) => setDetails(e.target.value)} maxLength={1000} rows={4} placeholder="What happened? (optional)" />
        {channel && <p className="muted small">Their recent messages in this chat are attached to the report.</p>}
        <div className="row gap">
          <button className="btn" disabled={busy} onClick={submit}>
            Send report
          </button>
          <button className="btn ghost" onClick={onClose}>
            Cancel
          </button>
        </div>
    </Dialog>
  );
}

/** Blocked players, with unblock (Friends & Notes page). */
export function BlockedPanel() {
  const { backend, profile, run, refreshProfile } = useApp();
  const [players, setPlayers] = useState<PublicProfile[]>([]);
  const key = profile?.blocked.map((b) => b.toText()).join(',') ?? '';
  useEffect(() => {
    if (!profile) return;
    Promise.all(profile.blocked.map((p) => backend.get_profile(p))).then((r) => setPlayers(r.flatMap((x): PublicProfile[] => x)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backend, key]);
  if (!profile || profile.blocked.length === 0) return null;

  const unblock = async (p: PublicProfile) => {
    await run(async () => unwrap(await backend.block_player(p.id, false)), `Unblocked ${p.username}`);
    await refreshProfile();
  };

  return (
    <div className="panel">
      <div className="panel-title">Blocked players</div>
      {players.map((p) => (
        <div key={p.id.toText()} className="friend-row">
          <span className="grow">{p.username}</span>
          <button className="btn ghost small" onClick={() => unblock(p)}>
            Unblock
          </button>
        </div>
      ))}
    </div>
  );
}

/** Tells a suspended or muted player what's going on. */
export function AccountNotice() {
  const { profile } = useApp();
  if (!profile) return null;
  const nowNs = BigInt(Date.now()) * 1_000_000n;
  const ban = profile.ban[0];
  if (ban && (ban.until === 0n || ban.until > nowNs)) {
    return (
      <div className="status-banner danger">
        ⛔ Your account is suspended{ban.until === 0n ? '' : ` for another ${countdown(ban.until)}`}: {ban.reason}. You can still withdraw your balance from the Wallet page.
      </div>
    );
  }
  if (profile.chat_muted_until > nowNs) {
    return <div className="status-banner">🔇 You are muted in chat for another {countdown(profile.chat_muted_until)}.</div>;
  }
  return null;
}
