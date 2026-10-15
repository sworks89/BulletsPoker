import { useState } from 'react';
import { useApp } from '../context';
import { unwrap } from '../ic';
import { CHIPS, ICP, amountWithUnit, isChips, parseIcp, type CurrencyV } from '../format';

const CHIP_BLINDS = [2n, 10n, 50n, 100n, 500n, 1_000n, 5_000n];
const ICP_BLINDS = [2_000n, 20_000n, 200_000n, 2_000_000n, 10_000_000n];

export function CreateRoom({ onClose }: { onClose: () => void }) {
  const { backend, run, go } = useApp();
  const [name, setName] = useState('');
  const [currency, setCurrency] = useState<CurrencyV>(CHIPS);
  const [bb, setBb] = useState<bigint>(50n);
  const [seats, setSeats] = useState(6);
  const [timeout, setTimeoutSecs] = useState(25);
  const [password, setPassword] = useState('');
  const blinds = isChips(currency) ? CHIP_BLINDS : ICP_BLINDS;

  const pickCurrency = (c: CurrencyV) => {
    setCurrency(c);
    setBb(isChips(c) ? 50n : 20_000n);
  };

  const create = async () => {
    const id = await run(
      async () =>
        unwrap(
          await backend.create_room({
            name: name.trim(),
            currency,
            big_blind: bb,
            max_seats: seats,
            password: password ? [password] : [],
            action_timeout_secs: BigInt(timeout),
          }),
        ),
      'Your table is open!',
    );
    if (id !== undefined) {
      onClose();
      go({ name: 'table', id });
    }
  };

  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>Host your own table</h3>
        <input value={name} maxLength={40} onChange={(e) => setName(e.target.value)} placeholder="Table name" />
        <div className="row gap">
          <button className={`chip ${isChips(currency) ? 'active' : ''}`} onClick={() => pickCurrency(CHIPS)}>
            🎲 Play chips
          </button>
          <button className={`chip ${!isChips(currency) ? 'active' : ''}`} onClick={() => pickCurrency(ICP)}>
            ∞ ICP
          </button>
        </div>
        <label className="small muted">Big blind</label>
        <div className="row gap wrap">
          {blinds.map((b) => (
            <button key={String(b)} className={`chip ${bb === b ? 'active' : ''}`} onClick={() => setBb(b)}>
              {amountWithUnit(b, currency)}
            </button>
          ))}
          {!isChips(currency) && (
            <input
              style={{ width: 110 }}
              placeholder="custom ICP"
              onChange={(e) => {
                try {
                  setBb(parseIcp(e.target.value));
                } catch {
                  /* keep the last valid value */
                }
              }}
            />
          )}
        </div>
        <p className="muted small">
          Buy-in {amountWithUnit(bb * 20n, currency)} – {amountWithUnit(bb * 100n, currency)}.
          {isChips(currency) ? ' Chip tables are rake free.' : ' Standard 2.5% rake; you earn 20% of it as the host.'}
        </p>
        <div className="row gap">
          <label className="small muted">Seats</label>
          {[2, 6, 9].map((n) => (
            <button key={n} className={`chip ${seats === n ? 'active' : ''}`} onClick={() => setSeats(n)}>
              {n}
            </button>
          ))}
          <label className="small muted">Clock</label>
          {[15, 25, 45].map((n) => (
            <button key={n} className={`chip ${timeout === n ? 'active' : ''}`} onClick={() => setTimeoutSecs(n)}>
              {n}s
            </button>
          ))}
        </div>
        <input value={password} maxLength={32} onChange={(e) => setPassword(e.target.value)} placeholder="Password (optional, for private games)" />
        <div className="row gap">
          <button className="btn ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn" disabled={!name.trim()} onClick={create}>
            Open table
          </button>
        </div>
        <p className="muted small">Empty tables close automatically after 30 minutes.</p>
      </div>
    </div>
  );
}
