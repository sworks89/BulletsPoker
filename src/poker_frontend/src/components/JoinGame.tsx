import { useState } from 'react';
import { useApp } from '../context';
import { unwrap, type TableConfig } from '../ic';
import { amountWithUnit, isChips } from '../format';
import type { GameRow } from '../lobby';

type Limits = Pick<TableConfig, 'currency' | 'min_buy_in' | 'max_buy_in' | 'big_blind'>;

/**
 * Picks a buy-in within a table's limits and the player's balance, then runs
 * `onConfirm`. Closes when it resolves to true. Shared by joining a game from
 * the lobby and by auto-sit.
 */
export function BuyInPicker({
  title,
  limits,
  confirm,
  initial,
  onConfirm,
  onClose,
}: {
  title: string;
  limits: Limits;
  confirm: string;
  initial?: bigint;
  onConfirm: (buyIn: bigint) => Promise<boolean>;
  onClose: () => void;
}) {
  const { profile, go } = useApp();
  const currency = limits.currency;
  const chips = isChips(currency);
  const balance = (chips ? profile?.chips : profile?.balance) ?? 0n;
  const max = balance < limits.max_buy_in ? balance : limits.max_buy_in;
  const start = initial !== undefined && initial >= limits.min_buy_in && initial <= max ? initial : max;
  const [value, setValue] = useState<bigint>(start);
  const [busy, setBusy] = useState(false);
  const short = balance < limits.min_buy_in;

  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>{title}</h3>
        <p className="muted">
          {chips ? 'Play chips' : 'Wallet balance'}: <b>{amountWithUnit(balance, currency)}</b>
        </p>
        {short ? (
          <>
            <p>You need at least {amountWithUnit(limits.min_buy_in, currency)} to sit here.</p>
            {chips ? (
              <p className="muted small">Claim your daily chip bonus from the top bar, or try a smaller table.</p>
            ) : (
              <button className="btn" onClick={() => go({ name: 'wallet' })}>
                Deposit ICP
              </button>
            )}
          </>
        ) : (
          <>
            <input
              type="range"
              min={Number(limits.min_buy_in)}
              max={Number(max)}
              step={Number(limits.big_blind)}
              value={Number(value)}
              onChange={(e) => setValue(BigInt(e.target.value))}
            />
            <div className="big">{amountWithUnit(value, currency)}</div>
            <div className="row gap">
              <button className="btn ghost" onClick={onClose}>
                Cancel
              </button>
              <button
                className="btn"
                disabled={busy}
                onClick={async () => {
                  setBusy(true);
                  const ok = await onConfirm(value);
                  setBusy(false);
                  if (ok) onClose();
                }}
              >
                {confirm}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

/** Joins a game from its lobby row: the server picks the table (or opens a copy). */
export function JoinGame({ row, onClose }: { row: GameRow; onClose: () => void }) {
  const { backend, run, refreshProfile, go } = useApp();
  return (
    <BuyInPicker
      title={`Join ${row.first.name}`}
      limits={row.first}
      confirm="Sit down"
      onClose={onClose}
      onConfirm={async (buyIn) => {
        const id = await run(async () => unwrap(await backend.join_game(row.game, buyIn)), 'Good luck!');
        if (id === undefined) return false;
        await refreshProfile();
        go({ name: 'table', id });
        return true;
      }}
    />
  );
}
