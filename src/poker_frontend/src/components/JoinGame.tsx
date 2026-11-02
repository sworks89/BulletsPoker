import { useEffect, useId, useState } from 'react';
import { useApp } from '../context';
import { unwrap, type TableConfig } from '../ic';
import { amountWithUnit, isChips } from '../format';
import type { GameRow } from '../lobby';
import { Dialog } from './Dialog';

type Limits = Pick<TableConfig, 'currency' | 'min_buy_in' | 'max_buy_in' | 'big_blind'>;

/**
 * Picks a buy-in within a table's limits and the player's balance, then runs
 * `onConfirm`. Closes when it resolves to true. Shared by joining a game from
 * the lobby and by auto-sit. `returnMin` is the stack you left this game with
 * in the last 30 minutes: you buy back in for at least that, even above the max.
 */
export function BuyInPicker({
  title,
  limits,
  confirm,
  initial,
  returnMin = 0n,
  onConfirm,
  onClose,
}: {
  title: string;
  limits: Limits;
  confirm: string;
  initial?: bigint;
  returnMin?: bigint;
  onConfirm: (buyIn: bigint) => Promise<boolean>;
  onClose: () => void;
}) {
  const { profile, go } = useApp();
  const currency = limits.currency;
  const chips = isChips(currency);
  const balance = (chips ? profile?.chips : profile?.balance) ?? 0n;
  const returning = returnMin > 0n;
  const min = returnMin > limits.min_buy_in ? returnMin : limits.min_buy_in;
  const tableMax = returnMin > limits.max_buy_in ? returnMin : limits.max_buy_in;
  const max = balance < tableMax ? balance : tableMax;
  const start = initial !== undefined && initial >= min && initial <= max ? initial : max;
  const [value, setValue] = useState<bigint>(start);
  const [busy, setBusy] = useState(false);
  const short = balance < min;
  const sliderId = useId();

  return (
    <Dialog title={title} onClose={onClose}>
        <p className="muted">
          {chips ? 'Play chips' : 'Wallet balance'}: <b>{amountWithUnit(balance, currency)}</b>
        </p>
        {short ? (
          <>
            <p>You need at least {amountWithUnit(min, currency)} to sit here{returning ? ' (you left recently with that much)' : ''}.</p>
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
            <label htmlFor={sliderId} className="small muted">
              Buy-in amount
            </label>
            <input
              id={sliderId}
              aria-valuetext={amountWithUnit(value, currency)}
              type="range"
              min={Number(min)}
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
    </Dialog>
  );
}

/** Joins a game from its lobby row: the server picks the table (or opens a copy). */
export function JoinGame({ row, onClose }: { row: GameRow; onClose: () => void }) {
  const { backend, run, refreshProfile, go } = useApp();
  // The picker opens once the return minimum is known. If the check fails, the
  // server still enforces the minimum when you sit down.
  const [returnMin, setReturnMin] = useState<bigint>();
  useEffect(() => {
    backend.get_min_return_buy_in(row.game).then(setReturnMin, () => setReturnMin(0n));
  }, [backend, row.game]);
  if (returnMin === undefined) return null;
  return (
    <BuyInPicker
      title={`Join ${row.first.name}`}
      limits={row.first}
      returnMin={returnMin}
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
