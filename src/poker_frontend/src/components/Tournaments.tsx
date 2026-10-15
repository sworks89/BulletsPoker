import { useState } from 'react';
import { useApp, usePoll } from '../context';
import { unwrap, type TournamentDetail, type TournamentSummary } from '../ic';
import { amountWithUnit, countdown, isChips } from '../format';

const STATUS_LABEL: Record<string, string> = {
  Registering: 'Registering',
  Running: 'Running',
  Finished: 'Finished',
  Cancelled: 'Cancelled',
};

function entryLabel(t: TournamentSummary): string {
  if (t.buy_in === 0n && t.fee === 0n) return 'Free';
  const fee = t.fee > 0n ? ` + ${amountWithUnit(t.fee, t.currency)}` : '';
  return `${amountWithUnit(t.buy_in, t.currency)}${fee}`;
}

/** Late registration / rebuy rules, e.g. "Late reg to L4 · Rebuys ×3 to L4". */
export function formatLabel(t: TournamentSummary): string {
  const parts: string[] = [];
  if (t.late_reg_levels > 0) parts.push(`Late reg to level ${t.late_reg_levels}`);
  if (t.rebuy_levels > 0) parts.push(`Rebuys ×${t.max_rebuys} to level ${t.rebuy_levels}`);
  return parts.join(' · ');
}

function startLabel(t: TournamentSummary): string {
  const status = Object.keys(t.status)[0];
  if (status === 'Running') return `${t.players_left}/${t.registered} left`;
  if (status !== 'Registering') return STATUS_LABEL[status];
  if ('SitAndGo' in t.kind) return `Starts at ${t.max_players} players`;
  return `Starts in ${countdown(t.start_at)}`;
}

export function Tournaments({ initial }: { initial?: bigint }) {
  const { backend, profile, run, refreshProfile, go } = useApp();
  const [list, setList] = useState<TournamentSummary[]>([]);
  const [tab, setTab] = useState<'sng' | 'scheduled'>('sng');
  const [selected, setSelected] = useState<bigint | null>(initial ?? null);
  const [detail, setDetail] = useState<TournamentDetail | null>(null);

  usePoll(
    async () => {
      const l = await backend.list_tournaments();
      setList(l);
      if (selected !== null) setDetail((await backend.get_tournament(selected))[0] ?? null);
    },
    3000,
    [backend, selected],
  );

  const shown = list.filter((t) => ('SitAndGo' in t.kind) === (tab === 'sng'));

  const register = async (t: TournamentSummary, join: boolean) => {
    const r = await run(
      async () => unwrap(join ? await backend.register_tournament(t.id) : await backend.unregister_tournament(t.id)),
      join ? `Registered for ${t.name}` : 'Unregistered, entry refunded',
    );
    if (r !== undefined) {
      await refreshProfile();
      setList(await backend.list_tournaments());
    }
  };

  return (
    <div className="history">
      <div className="panel">
        <div className="panel-title row">
          <span>Tournaments</span>
          <div className="tabs">
            <button className={`chip ${tab === 'sng' ? 'active' : ''}`} onClick={() => setTab('sng')}>
              Sit & Go
            </button>
            <button className={`chip ${tab === 'scheduled' ? 'active' : ''}`} onClick={() => setTab('scheduled')}>
              Scheduled
            </button>
          </div>
        </div>
        <p className="muted small">
          {tab === 'sng'
            ? 'Single-table events that start the moment they fill up. Blinds rise every few minutes; the top finishers share the prize pool.'
            : 'Multi-table events on a schedule. Tables are balanced and merged as players bust, down to a final table.'}
        </p>
        {shown.map((t) => {
          const status = Object.keys(t.status)[0];
          return (
            <div key={String(t.id)} className={`tourney-row ${selected === t.id ? 'active' : ''}`} onClick={() => setSelected(t.id)}>
              <div className="grow">
                <div>
                  <b>{t.name}</b> <span className={`tag ${isChips(t.currency) ? 'chips' : ''}`}>{isChips(t.currency) ? 'chips' : 'ICP'}</span>
                  {status === 'Running' && <span className="tag live">LIVE</span>}
                </div>
                <div className="muted small">
                  {entryLabel(t)} · Prize pool {amountWithUnit(t.prize_pool, t.currency)} · {startLabel(t)}
                </div>
                {formatLabel(t) && (
                  <div className="small format-line">
                    {formatLabel(t)}
                    {status === 'Running' && t.can_register && <span className="tag live">Late reg open</span>}
                  </div>
                )}
                {status === 'Registering' && (
                  <div className="seatbar wide">
                    {Array.from({ length: Math.min(t.max_players, 20) }, (_, i) => (
                      <span key={i} className={i < (t.registered * Math.min(t.max_players, 20)) / t.max_players ? 'on' : ''} />
                    ))}
                    <span className="muted small">
                      {' '}
                      {t.registered}/{t.max_players}
                    </span>
                  </div>
                )}
              </div>
              {profile && status === 'Registering' && (
                <button
                  className={`btn small ${t.registered_me ? 'ghost' : ''}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    register(t, !t.registered_me);
                  }}
                >
                  {t.registered_me ? 'Unregister' : 'Register'}
                </button>
              )}
              {profile && status === 'Running' && t.can_register && !t.registered_me && (
                <button
                  className="btn small"
                  onClick={(e) => {
                    e.stopPropagation();
                    register(t, true);
                  }}
                >
                  Late register
                </button>
              )}
              {t.my_table[0] !== undefined && (
                <button
                  className="btn small"
                  onClick={(e) => {
                    e.stopPropagation();
                    go({ name: 'table', id: t.my_table[0]! });
                  }}
                >
                  Go to table
                </button>
              )}
            </div>
          );
        })}
        {shown.length === 0 && <p className="muted">Nothing here right now.</p>}
      </div>

      {detail ? <Detail d={detail} /> : <div className="panel muted">Pick a tournament to see its structure and standings.</div>}
    </div>
  );
}

function Detail({ d }: { d: TournamentDetail }) {
  const { go, profile } = useApp();
  const t = d.summary;
  const status = Object.keys(t.status)[0];
  const cur = t.currency;
  return (
    <div className="panel">
      <div className="panel-title row">
        <span>{t.name}</span>
        <span className="muted small">{STATUS_LABEL[status]}</span>
      </div>
      <div className="stats">
        <div>
          <b>{amountWithUnit(t.prize_pool, cur)}</b>
          <span>Prize pool</span>
        </div>
        <div>
          <b>{t.starting_stack.toLocaleString()}</b>
          <span>Starting chips</span>
        </div>
        <div>
          <b>
            {d.small_blind.toLocaleString()}/{d.big_blind.toLocaleString()}
          </b>
          <span>Level {d.level} blinds</span>
        </div>
        <div>
          <b>{status === 'Running' ? countdown(d.next_level_at) : `${Number(t.level_secs) / 60} min`}</b>
          <span>{status === 'Running' ? 'Next level' : 'Levels'}</span>
        </div>
      </div>

      {(t.late_reg_levels > 0 || t.rebuy_levels > 0) && (
        <p className="small">
          {formatLabel(t)}. {t.entries} entries
          {t.entries > t.registered ? ` (${t.entries - t.registered} rebuys)` : ''}.
          {t.rebuy_levels > 0 && ' Rebuy when busted (30s to decide) or when at or below the starting stack.'}
        </p>
      )}
      <h4>Payouts</h4>
      <div className="payouts">
        {Array.from(d.payouts, (p, i) => (
          <div key={i}>
            <span className="muted">#{i + 1}</span> <b>{amountWithUnit(p, cur)}</b>
          </div>
        ))}
      </div>

      <h4>{status === 'Registering' ? `Registered (${d.standings.length})` : 'Standings'}</h4>
      <table className="grid">
        <thead>
          <tr>
            <th>#</th>
            <th>Player</th>
            <th>{status === 'Registering' ? '' : 'Chips'}</th>
            <th>Prize</th>
          </tr>
        </thead>
        <tbody>
          {d.standings.map((s, i) => (
            <tr key={s.player.toText()} className={s.player.toText() === profile?.id.toText() ? 'me-row' : ''}>
              <td>{s.place[0] ?? (status === 'Registering' ? '' : i + 1)}</td>
              <td>
                <span className="avatar sm">{s.avatar}</span> {s.username}
              </td>
              <td>
                {s.place[0] !== undefined ? <span className="muted">out</span> : status !== 'Registering' && s.chips.toLocaleString()}
                {s.table[0] !== undefined && (
                  <button className="chip" onClick={() => go({ name: 'table', id: s.table[0]! })}>
                    watch
                  </button>
                )}
              </td>
              <td>{s.prize > 0n ? amountWithUnit(s.prize, cur) : ''}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
