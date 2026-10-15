import { useEffect, useState } from 'react';
import { useApp, usePoll } from '../context';
import {
  Principal,
  unwrap,
  type AdminConfig,
  type AuditEntry,
  type HouseStats,
  type LedgerOp,
  type Metrics,
  type ModAction,
  type ModPlayerView,
  type PauseFlags,
  type Report,
  type Roles,
} from '../ic';
import { countdown, icp, parseIcp, shortPrincipal, timeAgo } from '../format';

type Tab = 'overview' | 'reports' | 'players' | 'money' | 'settings' | 'audit';

const TABS: { key: Tab; label: string; adminOnly: boolean }[] = [
  { key: 'overview', label: '📊 Overview', adminOnly: true },
  { key: 'reports', label: '⚑ Reports', adminOnly: false },
  { key: 'players', label: '👤 Players', adminOnly: false },
  { key: 'money', label: '∞ Money', adminOnly: true },
  { key: 'settings', label: '⚙ Settings', adminOnly: true },
  { key: 'audit', label: '📜 Audit log', adminOnly: true },
];

/** Admin and moderator console. Every action is re-checked by the backend. */
export function Admin({ roles }: { roles: Roles }) {
  const [tab, setTab] = useState<Tab>(roles.admin ? 'overview' : 'reports');
  const [player, setPlayer] = useState<Principal | null>(null);
  if (!roles.admin && !roles.moderator) return <div className="panel">This page is for admins and moderators.</div>;
  const lookUp = (p: Principal) => {
    setPlayer(p);
    setTab('players');
  };

  return (
    <div className="admin">
      <div className="row gap admin-head">
        <h2>{roles.admin ? 'Admin' : 'Moderation'}</h2>
        <div className="tabs">
          {TABS.filter((t) => roles.admin || !t.adminOnly).map((t) => (
            <button key={t.key} className={`chip ${tab === t.key ? 'active' : ''}`} onClick={() => setTab(t.key)}>
              {t.label}
            </button>
          ))}
        </div>
      </div>
      {tab === 'overview' && <Overview />}
      {tab === 'reports' && <Reports onLookUp={lookUp} />}
      {tab === 'players' && <Players roles={roles} initial={player} />}
      {tab === 'money' && <Money />}
      {tab === 'settings' && <Settings />}
      {tab === 'audit' && <AuditLog />}
    </div>
  );
}

// ------------------------------------------------------------------ overview

function Stat({ label, value, warn }: { label: string; value: string; warn?: boolean }) {
  return (
    <div className={`stat-tile ${warn ? 'warn' : ''}`}>
      <div className="stat-value">{value}</div>
      <div className="stat-label">{label}</div>
    </div>
  );
}

function Overview() {
  const { backend, run } = useApp();
  const [m, setM] = useState<Metrics | null>(null);
  const load = async () => setM(unwrap(await backend.admin_get_metrics()));
  usePoll(load, 10000, [backend]);
  if (!m) return <div className="panel muted">Loading…</div>;

  const solvencyOk = m.last_solvency_ok[0];
  const cyclesT = Number(m.cycles) / 1e12;
  const mb = (b: bigint) => `${(Number(b) / 1048576).toFixed(1)} MB`;

  return (
    <div className="admin-grid">
      <div className="panel span2">
        <div className="panel-title">Players & tables</div>
        <div className="stat-row">
          <Stat label="Registered" value={m.users.toLocaleString()} />
          <Stat label="Active (5 min)" value={m.active_players_5m.toLocaleString()} />
          <Stat label="Seated" value={m.seated_players.toLocaleString()} />
          <Stat label="Active tables" value={`${m.active_tables} / ${m.tables}`} />
          <Stat label="Tournaments running" value={m.running_tournaments.toLocaleString()} />
          <Stat label="Hands dealt" value={m.hands_dealt.toLocaleString()} />
        </div>
      </div>
      <div className="panel span2">
        <div className="panel-title">Money</div>
        <div className="stat-row">
          <Stat label="Owed to players & pools" value={`${icp(m.liabilities_e8s, 2)} ICP`} />
          <Stat label="Player wallets" value={`${icp(m.wallet_total_e8s, 2)} ICP`} />
          <Stat label="Treasury" value={`${icp(m.treasury_e8s, 4)} ICP`} />
          <Stat label="ICP jackpot" value={`${icp(m.jackpot_icp_e8s, 4)} ICP`} />
          <Stat label="Chip jackpot" value={m.jackpot_chips.toLocaleString()} />
          <Stat label="Unconfirmed ledger ops" value={String(m.unresolved_ledger_ops)} warn={m.unresolved_ledger_ops > 0n} />
        </div>
        <div className="row gap wrap">
          <span className={`tag ${solvencyOk === undefined ? '' : solvencyOk ? 'op-completed' : 'op-failed'}`}>
            {solvencyOk === undefined ? 'No solvency check yet' : solvencyOk ? 'Solvent' : 'SHORTFALL'}
          </span>
          {m.last_solvency_at[0] !== undefined && (
            <span className="muted small">
              checked {timeAgo(m.last_solvency_at[0])} ago · surplus {icp(m.last_solvency_surplus_e8s[0] ?? 0n, 4)} ICP
            </span>
          )}
          <button className="btn small" onClick={() => run(async () => { unwrap(await backend.admin_check_solvency()); await load(); }, 'Solvency checked')}>
            Check now
          </button>
        </div>
      </div>
      <PausePanel />
      <div className="panel">
        <div className="panel-title">Canister</div>
        <div className="stat-row">
          <Stat label="Cycles" value={`${cyclesT.toFixed(2)} T`} warn={cyclesT < 2} />
          <Stat label="Heap" value={mb(m.heap_bytes)} />
          <Stat label="Stable memory" value={mb(m.stable_bytes)} />
        </div>
        <div className="stat-row">
          <Stat label="Open reports" value={String(m.open_reports)} warn={m.open_reports > 0n} />
          <Stat label="Suspensions" value={String(m.bans)} />
          <Stat label="Rate-limited calls" value={m.rate_limited_calls.toLocaleString()} />
          <Stat label="Failed ledger ops" value={String(m.ledger_ops_failed)} />
          <Stat label="Idle stand-ups" value={m.idle_stand_ups.toLocaleString()} />
        </div>
      </div>
    </div>
  );
}

const PAUSE_KEYS: { key: 'deposits' | 'withdrawals' | 'dealing' | 'joins'; label: string }[] = [
  { key: 'deposits', label: 'Deposits' },
  { key: 'withdrawals', label: 'Withdrawals' },
  { key: 'dealing', label: 'Dealing new hands' },
  { key: 'joins', label: 'New seats & tables' },
];

function PausePanel() {
  const { backend, run } = useApp();
  const [flags, setFlags] = useState<PauseFlags | null>(null);
  const [reason, setReason] = useState('');
  useEffect(() => {
    backend.get_status().then((s) => {
      setFlags(s.pause);
      setReason(s.pause.reason[0] ?? '');
    });
  }, [backend]);
  if (!flags) return null;
  const any = PAUSE_KEYS.some((k) => flags[k.key]);

  const save = (next: PauseFlags) =>
    run(async () => {
      unwrap(await backend.admin_set_pause(next));
      setFlags(next);
    }, 'Pause switches updated');

  return (
    <div className={`panel ${any ? 'paused-panel' : ''}`}>
      <div className="panel-title">Emergency switches</div>
      {PAUSE_KEYS.map((k) => (
        <label key={k.key} className="switch-row">
          <input type="checkbox" checked={flags[k.key]} onChange={(e) => setFlags({ ...flags, [k.key]: e.target.checked })} />
          Pause {k.label.toLowerCase()}
        </label>
      ))}
      <input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Reason shown to players (optional)" maxLength={200} />
      <div className="row gap">
        <button className="btn small" onClick={() => save({ ...flags, reason: reason.trim() ? [reason.trim()] : [] })}>
          Apply
        </button>
        {any && (
          <button className="btn ghost small" onClick={() => save({ deposits: false, withdrawals: false, dealing: false, joins: false, reason: [] })}>
            Resume everything
          </button>
        )}
      </div>
      <p className="muted small">Hands in progress always finish and players can always stand up.</p>
    </div>
  );
}

// ------------------------------------------------------------------- reports

const variant = (v: object) => Object.keys(v)[0];
const REASON_LABEL: Record<string, string> = {
  AbusiveChat: 'Abusive chat',
  Cheating: 'Cheating',
  Collusion: 'Collusion',
  OffensiveName: 'Offensive name',
  Spam: 'Spam',
  Other: 'Other',
};

function actionLabel(a: ModAction): string {
  if ('Dismiss' in a) return 'Dismissed';
  if ('Warn' in a) return 'Warned';
  if ('MuteChat' in a) return `Muted ${a.MuteChat}h`;
  return a.Ban === 0 ? 'Suspended permanently' : `Suspended ${a.Ban}h`;
}

function Reports({ onLookUp }: { onLookUp: (p: Principal) => void }) {
  const { backend } = useApp();
  const [showAll, setShowAll] = useState(false);
  const [reports, setReports] = useState<Report[] | null>(null);
  const load = async () => setReports(unwrap(showAll ? await backend.mod_recent_reports(100) : await backend.mod_open_reports()));
  usePoll(load, 15000, [backend, showAll]);

  return (
    <div className="panel">
      <div className="panel-title row">
        <span>{showAll ? 'Recent reports' : `Open reports (${reports?.length ?? '…'})`}</span>
        <label className="switch-row small">
          <input type="checkbox" checked={showAll} onChange={(e) => setShowAll(e.target.checked)} /> Include resolved
        </label>
      </div>
      {reports?.length === 0 && <p className="muted">Nothing to review. 🎉</p>}
      {reports?.map((r) => <ReportCard key={String(r.id)} report={r} onLookUp={onLookUp} onDone={load} />)}
    </div>
  );
}

function ReportCard({ report: r, onLookUp, onDone }: { report: Report; onLookUp: (p: Principal) => void; onDone: () => void }) {
  const { backend, run } = useApp();
  const [note, setNote] = useState('');
  const open = 'Open' in r.status;
  const resolve = async (action: ModAction) => {
    if (('Ban' in action) && !confirm(`Suspend ${r.target_name}${action.Ban === 0 ? ' permanently' : ` for ${action.Ban} hours`}?`)) return;
    const ok = await run(async () => unwrap(await backend.mod_resolve_report(r.id, action, note.trim())), 'Report resolved');
    if (ok !== undefined) onDone();
  };

  return (
    <div className={`report-card ${open ? '' : 'resolved'}`}>
      <div className="row gap wrap">
        <b>#{String(r.id)}</b>
        <span className="tag">{REASON_LABEL[variant(r.reason)]}</span>
        <button className="link" onClick={() => onLookUp(r.target)}>
          {r.target_name}
        </button>
        <span className="muted small">
          reported by {r.reporter_name} · {timeAgo(r.created_at)} ago{r.channel[0] ? ` · ${'Lobby' in r.channel[0] ? 'lobby chat' : `table #${r.channel[0].Table}`}` : ''}
        </span>
      </div>
      {r.details && <p className="report-details">“{r.details}”</p>}
      {r.evidence.length > 0 && (
        <div className="evidence">
          {r.evidence.map((m) => (
            <div key={String(m.id)} className="small">
              <span className="muted">{timeAgo(m.time)}</span> <b>{m.username}:</b> {m.text}
            </div>
          ))}
        </div>
      )}
      {open ? (
        <>
          <input value={note} onChange={(e) => setNote(e.target.value)} placeholder="Note (shown in the audit log; used as the suspension reason)" maxLength={300} />
          <div className="row gap wrap">
            <button className="btn ghost small" onClick={() => resolve({ Dismiss: null })}>
              Dismiss
            </button>
            <button className="btn ghost small" onClick={() => resolve({ Warn: null })}>
              Warn
            </button>
            <button className="btn ghost small" onClick={() => resolve({ MuteChat: 1 })}>
              Mute 1h
            </button>
            <button className="btn ghost small" onClick={() => resolve({ MuteChat: 24 })}>
              Mute 24h
            </button>
            <button className="btn danger small" onClick={() => resolve({ Ban: 24 })}>
              Suspend 24h
            </button>
            <button className="btn danger small" onClick={() => resolve({ Ban: 24 * 7 })}>
              Suspend 7d
            </button>
            <button className="btn danger small" onClick={() => resolve({ Ban: 0 })}>
              Suspend permanently
            </button>
          </div>
        </>
      ) : (
        'Resolved' in r.status && (
          <div className="muted small">
            {actionLabel(r.status.Resolved.action)} {timeAgo(r.status.Resolved.at)} ago by {shortPrincipal(r.status.Resolved.by)}
            {r.status.Resolved.note && ` · “${r.status.Resolved.note}”`}
          </div>
        )
      )}
    </div>
  );
}

// ------------------------------------------------------------------- players

function parsePlayer(s: string): Principal | null {
  try {
    return Principal.fromText(s.trim());
  } catch {
    return null;
  }
}

function Players({ roles, initial }: { roles: Roles; initial: Principal | null }) {
  const { backend, run } = useApp();
  const [query, setQuery] = useState('');
  const [player, setPlayer] = useState<ModPlayerView | null>(null);
  const [hours, setHours] = useState('24');
  const [reason, setReason] = useState('');

  const load = async (p: Principal) => {
    const v = await run(async () => unwrap(await backend.mod_get_player(p)));
    setPlayer(v ?? null);
  };
  useEffect(() => {
    if (initial) load(initial);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initial?.toText()]);

  const search = async () => {
    const asPrincipal = query.includes('-') ? parsePlayer(query) : null;
    const id = asPrincipal ?? (await backend.find_user(query.trim()))[0]?.id;
    if (!id) {
      setPlayer(null);
      run(async () => {
        throw new Error('No player with that name');
      });
      return;
    }
    await load(id);
  };

  const act = async (f: () => Promise<{ Ok: null } | { Err: string }>, msg: string) => {
    const ok = await run(async () => unwrap(await f()), msg);
    if (ok !== undefined && player) await load(player.id);
  };

  const nowNs = BigInt(Date.now()) * 1_000_000n;
  const banned = player?.ban[0] && (player.ban[0].until === 0n || player.ban[0].until > nowNs) ? player.ban[0] : null;
  const muted = player && player.chat_muted_until > nowNs;
  const h = Math.max(0, Math.floor(Number(hours) || 0));

  return (
    <div className="admin-grid">
      <div className="panel span2">
        <form
          className="row gap"
          onSubmit={(e) => {
            e.preventDefault();
            search();
          }}
        >
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Username or principal" />
          <button className="btn">Look up</button>
        </form>
      </div>
      {player && (
        <>
          <div className="panel">
            <div className="panel-title row">
              <span>{player.username}</span>
              {player.is_moderator && <span className="tag">Moderator</span>}
            </div>
            <code className="small">{player.id.toText()}</code>
            <div className="kv">
              <span>Level</span> <b>{player.level}</b>
              <span>Joined</span> <b>{new Date(Number(player.created_at / 1_000_000n)).toLocaleDateString()}</b>
              <span>Last seen</span> <b>{timeAgo(player.last_seen)} ago</b>
              <span>ICP balance</span> <b>{icp(player.balance, 4)}</b>
              <span>Play chips</span> <b>{player.chips.toLocaleString()}</b>
              <span>Hands</span> <b>{player.hands_played.toLocaleString()}</b>
              <span>Net (ICP)</span> <b>{icp(player.net_winnings < 0n ? -player.net_winnings : player.net_winnings, 4)}{player.net_winnings < 0n ? ' lost' : ' won'}</b>
              <span>Seated at</span> <b>{player.tables.length ? Array.from(player.tables, (t) => `#${t}`).join(', ') : '—'}</b>
            </div>
            {player.status && <p className="muted small">Status: “{player.status}”</p>}
          </div>
          <div className="panel">
            <div className="panel-title">Actions</div>
            {banned && (
              <div className="status-banner danger">
                Suspended {banned.until === 0n ? 'permanently' : `for ${countdown(banned.until)}`}: {banned.reason}
              </div>
            )}
            {muted && <div className="status-banner">Muted for {countdown(player.chat_muted_until)}</div>}
            <div className="row gap">
              <input className="narrow" value={hours} onChange={(e) => setHours(e.target.value)} inputMode="numeric" />
              <span className="muted small">hours (0 = permanent suspension)</span>
            </div>
            <input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Suspension reason (shown to the player)" maxLength={200} />
            <div className="row gap wrap">
              <button className="btn ghost small" disabled={h === 0} onClick={() => act(() => backend.mod_mute(player.id, h), `Muted for ${h}h`)}>
                Mute {h}h
              </button>
              {muted && (
                <button className="btn ghost small" onClick={() => act(() => backend.mod_mute(player.id, 0), 'Mute lifted')}>
                  Unmute
                </button>
              )}
              <button
                className="btn danger small"
                onClick={() =>
                  confirm(`Suspend ${player.username}${h === 0 ? ' permanently' : ` for ${h} hours`}? They'll be stood up from cash tables.`) &&
                  act(() => backend.mod_ban(player.id, h, reason.trim()), 'Player suspended')
                }
              >
                Suspend{h === 0 ? ' permanently' : ` ${h}h`}
              </button>
              {banned && (
                <button className="btn ghost small" onClick={() => act(() => backend.mod_unban(player.id), 'Suspension lifted')}>
                  Lift suspension
                </button>
              )}
              {roles.admin && (
                <button
                  className="btn ghost small"
                  onClick={() =>
                    act(() => backend.admin_set_moderator(player.id, !player.is_moderator), player.is_moderator ? 'Moderator removed' : 'Moderator added')
                  }
                >
                  {player.is_moderator ? 'Remove moderator' : 'Make moderator'}
                </button>
              )}
            </div>
          </div>
          <div className="panel span2">
            <div className="panel-title">Reports against {player.username} ({player.reports.length})</div>
            {player.reports.length === 0 && <p className="muted small">None.</p>}
            {player.reports.map((r) => (
              <ReportCard key={String(r.id)} report={r} onLookUp={() => {}} onDone={() => load(player.id)} />
            ))}
          </div>
        </>
      )}
    </div>
  );
}

// --------------------------------------------------------------------- money

function Money() {
  const { backend, run, principal } = useApp();
  const [stats, setStats] = useState<HouseStats | null>(null);
  const [ops, setOps] = useState<LedgerOp[]>([]);
  const [to, setTo] = useState('');
  const [amt, setAmt] = useState('');
  const [seed, setSeed] = useState('');
  const [recount, setRecount] = useState<[bigint, bigint] | null>(null);
  const load = async () => {
    setStats(unwrap(await backend.admin_house_stats()));
    setOps(unwrap(await backend.admin_unresolved_ops()));
  };
  usePoll(load, 15000, [backend]);

  const withdraw = () =>
    run(async () => {
      const owner = Principal.fromText(to.trim());
      const amount = parseIcp(amt);
      if (!confirm(`Send ${icp(amount)} ICP from the treasury to ${owner.toText()}?`)) return;
      unwrap(await backend.admin_withdraw_treasury({ owner, subaccount: [] }, amount));
      setAmt('');
      await load();
    }, 'Treasury withdrawal sent');

  const seedJackpot = () =>
    run(async () => {
      unwrap(await backend.admin_seed_jackpot(parseIcp(seed)));
      setSeed('');
      await load();
    }, 'Jackpot topped up');

  const resolveOp = (op: LedgerOp, executed: boolean) =>
    run(async () => {
      let block: [] | [bigint] = [];
      if (executed) {
        const b = prompt('Ledger block index of the transfer (from the ledger explorer):');
        if (b === null) return;
        block = b.trim() ? [BigInt(b.trim())] : [];
      } else if (!confirm('Mark as NOT executed? Any reserved balance is refunded to the player.')) return;
      unwrap(await backend.admin_resolve_op(op.id, executed, block));
      await load();
    }, 'Operation settled');

  return (
    <div className="admin-grid">
      <div className="panel">
        <div className="panel-title">House</div>
        {stats && (
          <div className="kv">
            <span>Players</span> <b>{stats.users.toLocaleString()}</b>
            <span>Player wallets</span> <b>{icp(stats.wallet_balances, 4)} ICP</b>
            <span>On ICP tables</span> <b>{icp(stats.chips_on_tables, 4)} ICP</b>
            <span>ICP jackpot</span> <b>{icp(stats.jackpot, 4)} ICP</b>
            <span>Treasury</span> <b>{icp(stats.treasury, 4)} ICP</b>
            <span>Total owed</span> <b>{icp(stats.liabilities_total, 4)} ICP</b>
          </div>
        )}
        <button
          className="btn ghost small"
          onClick={() => run(async () => setRecount(unwrap(await backend.admin_audit_wallet_total())))}
        >
          Recount wallet total
        </button>
        {recount && (
          <p className={`small ${recount[0] === recount[1] ? 'pos' : 'neg'}`}>
            Running total {icp(recount[0], 8)} · full recount {icp(recount[1], 8)} {recount[0] === recount[1] ? '✓ match' : '✗ MISMATCH'}
          </p>
        )}
      </div>
      <div className="panel">
        <div className="panel-title">Withdraw from treasury</div>
        <input value={to} onChange={(e) => setTo(e.target.value)} placeholder="Destination principal" />
        <div className="row gap">
          <input value={amt} onChange={(e) => setAmt(e.target.value)} placeholder="Amount in ICP" />
          {principal && (
            <button className="btn ghost small" onClick={() => setTo(principal.toText())}>
              Me
            </button>
          )}
          <button className="btn small" disabled={!to || !amt} onClick={withdraw}>
            Withdraw
          </button>
        </div>
        <div className="panel-title">Top up the ICP jackpot</div>
        <div className="row gap">
          <input value={seed} onChange={(e) => setSeed(e.target.value)} placeholder="Amount in ICP (from the treasury)" />
          <button className="btn small" disabled={!seed} onClick={seedJackpot}>
            Add
          </button>
        </div>
      </div>
      <div className="panel span2">
        <div className="panel-title">Unconfirmed ledger operations ({ops.length})</div>
        {ops.length === 0 && <p className="muted small">All deposits and withdrawals are settled.</p>}
        {ops.map((op) => (
          <div key={String(op.id)} className="op-row">
            <div className="grow small">
              <b>#{String(op.id)} {variant(op.kind)}</b> · {icp(op.amount)} ICP · {shortPrincipal(op.user)} → {shortPrincipal(op.to.owner)} ·{' '}
              <span className="muted">{timeAgo(op.created_at_time)} ago</span>
            </div>
            <button className="btn ghost small" onClick={() => run(async () => { unwrap(await backend.admin_retry_op(op.id)); await load(); }, 'Re-sent')}>
              Retry now
            </button>
            <button className="btn ghost small" onClick={() => resolveOp(op, true)}>
              It went through
            </button>
            <button className="btn ghost small" onClick={() => resolveOp(op, false)}>
              It failed
            </button>
          </div>
        ))}
        <p className="muted small">
          These retry automatically every minute for 24 hours. Only settle one by hand after checking the ledger: the memo is <code>bp-op:&lt;id&gt;</code>.
        </p>
      </div>
    </div>
  );
}

// ------------------------------------------------------------------ settings

function Settings() {
  const { backend, run } = useApp();
  const [cfg, setCfg] = useState<AdminConfig | null>(null);
  const [words, setWords] = useState('');
  const [limit, setLimit] = useState('');
  const [newMod, setNewMod] = useState('');
  const load = async () => {
    const c = unwrap(await backend.admin_get_config());
    setCfg(c);
    setWords(c.banned_words.join(', '));
    setLimit(String(c.signups_per_hour));
  };
  useEffect(() => {
    load().catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backend]);
  if (!cfg) return <div className="panel muted">Loading…</div>;

  const setMod = (p: Principal, on: boolean) =>
    run(async () => {
      unwrap(await backend.admin_set_moderator(p, on));
      await load();
    }, on ? 'Moderator added' : 'Moderator removed');

  const addMod = async () => {
    const p = newMod.includes('-') ? parsePlayer(newMod) : (await backend.find_user(newMod.trim()))[0]?.id;
    if (!p) {
      run(async () => {
        throw new Error('No player with that name');
      });
      return;
    }
    await setMod(p, true);
    setNewMod('');
  };

  return (
    <div className="admin-grid">
      <div className="panel">
        <div className="panel-title">Moderators</div>
        {cfg.moderators.length === 0 && <p className="muted small">No moderators yet. Controllers can always moderate.</p>}
        {cfg.moderators.map(([p, name]) => (
          <div key={p.toText()} className="friend-row">
            <span className="grow">
              {name || <i className="muted">no profile</i>} <code className="small muted">{shortPrincipal(p)}</code>
            </span>
            <button className="btn ghost small" onClick={() => setMod(p, false)}>
              Remove
            </button>
          </div>
        ))}
        <div className="row gap">
          <input value={newMod} onChange={(e) => setNewMod(e.target.value)} placeholder="Username or principal" />
          <button className="btn small" disabled={!newMod.trim()} onClick={addMod}>
            Add
          </button>
        </div>
      </div>
      <div className="panel">
        <div className="panel-title">Sign-up limit</div>
        <div className="row gap">
          <input className="narrow" value={limit} onChange={(e) => setLimit(e.target.value)} inputMode="numeric" />
          <span className="muted small">new accounts per hour</span>
          <button
            className="btn small"
            onClick={() => run(async () => { unwrap(await backend.admin_set_signup_limit(Number(limit) || 0)); await load(); }, 'Sign-up limit saved')}
          >
            Save
          </button>
        </div>
        <p className="muted small">Lower it if you see a wave of bot accounts. 0 closes sign-ups.</p>
      </div>
      <div className="panel span2">
        <div className="panel-title">Word filter ({cfg.banned_words.length} words)</div>
        <textarea rows={4} value={words} onChange={(e) => setWords(e.target.value)} />
        <div className="row gap">
          <button
            className="btn small"
            onClick={() =>
              run(async () => {
                unwrap(await backend.admin_set_banned_words(words.split(/[\s,]+/).filter(Boolean)));
                await load();
              }, 'Word filter saved')
            }
          >
            Save
          </button>
          <span className="muted small">Whole words are masked in chat; usernames containing any of them are refused.</span>
        </div>
      </div>
    </div>
  );
}

// ----------------------------------------------------------------- audit log

function AuditLog() {
  const { backend } = useApp();
  const [log, setLog] = useState<AuditEntry[]>([]);
  const [filter, setFilter] = useState('');
  usePoll(async () => setLog(unwrap(await backend.admin_audit_log(500))), 15000, [backend]);
  const f = filter.trim().toLowerCase();
  const rows = f ? log.filter((e) => e.action.toLowerCase().includes(f) || e.actor.toText().includes(f)) : log;

  return (
    <div className="panel">
      <div className="panel-title row">
        <span>Audit log</span>
        <input className="narrow-search" value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Filter" />
      </div>
      <table className="grid">
        <thead>
          <tr>
            <th>When</th>
            <th>Who</th>
            <th>Action</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((e, i) => (
            <tr key={i}>
              <td className="nowrap" title={new Date(Number(e.time / 1_000_000n)).toLocaleString()}>
                {new Date(Number(e.time / 1_000_000n)).toLocaleString()}
              </td>
              <td>
                <code className="small">{shortPrincipal(e.actor)}</code>
              </td>
              <td>{e.action}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <p className="muted small">No entries.</p>}
    </div>
  );
}
