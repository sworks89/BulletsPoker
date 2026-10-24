import { useEffect, useState } from 'react';
import { useApp } from '../context';
import { backendId, type HandRecord } from '../ic';
import { toEvidenceJson } from '../fair/evidence';
import { verifyHand, type VerificationReport } from '../fair/verify';
import { defaultStore, seenCommitment } from '../fair/seen';
import { D1_LINE, DISCLAIMER, verificationRows, verifyState, windowText } from '../replay';

const nowNs = () => BigInt(Date.now()) * 1_000_000n;
const hex = (b: Uint8Array | number[]) => Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');

/** Copies text to the clipboard; if that is blocked, shows the text so it can be selected by hand. */
function CopyButton({ text, what }: { text: string; what: string }) {
  const { notify } = useApp();
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      notify(`${what} copied`);
    } catch {
      notify(`${what}: ${text}`);
    }
  };
  return (
    <button className="btn ghost small" onClick={copy} aria-label={`Copy ${what.toLowerCase()}`}>
      Copy
    </button>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="verify-field">
      <span className="muted small">{label}</span>
      <code className="verify-value">{value}</code>
      <CopyButton text={value} what={label} />
    </div>
  );
}

/** Verify Hand panel for one replayed hand. Runs the verifier in the browser. */
export function HandVerification({ hand }: { hand: HandRecord }) {
  const { notify } = useApp();
  const [now, setNow] = useState(nowNs);
  const [running, setRunning] = useState(false);
  const [report, setReport] = useState<VerificationReport | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    setReport(null);
    setFailure(null);
  }, [hand.id]);
  useEffect(() => {
    const t = setInterval(() => setNow(nowNs()), 30_000);
    return () => clearInterval(t);
  }, []);

  const state = verifyState(hand, now);
  if (state.kind === 'none') return null;
  const ev = hand.fair[0];
  const seed = ev?.revealed_seed[0];
  const observed = ev ? seenCommitment(defaultStore(), backendId, ev.table_id, ev.hand_no) : undefined;

  const run = async () => {
    if (!ev) return;
    setRunning(true);
    setReport(null);
    setFailure(null);
    try {
      setReport(await verifyHand(hand, ev, backendId, { now: nowNs(), observedCommitment: observed }));
    } catch (e) {
      setFailure(`Verification failed to run: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setRunning(false);
    }
  };

  const download = () => {
    if (!ev) return;
    const blob = new Blob([toEvidenceJson(hand, ev)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `hand-${String(hand.id)}-evidence.json`;
    a.click();
    URL.revokeObjectURL(url);
    notify('Evidence exported');
  };

  return (
    <section className="verify-panel" aria-label="Verify hand">
      <div className="panel-title">Fair shuffle</div>

      {state.kind === 'legacy' && (
        <p>
          <span className="tag">Legacy / Not verifiable</span>{' '}
          <span className="muted small">Dealt before shuffles were committed. This is not a failure.</span>
        </p>
      )}

      {ev && (
        <>
          <Field label="Commitment" value={hex(ev.commitment)} />
          <p className="small">
            Commitment seen at the table: <b>{observed ? 'yes' : 'no'}</b>
            {!observed && <span className="muted"> (this device has no copy from the hand, so its timing relies on the canister's log)</span>}
          </p>
          {seed && <Field label="Revealed seed" value={hex(seed)} />}
          <Field label="Shuffle version" value={String(ev.version)} />
          <Field label="Scheme" value={ev.scheme} />
        </>
      )}

      {'voidedLabel' in state && state.voidedLabel && <p className="verify-voided">{state.voidedLabel}</p>}
      {state.kind === 'pending' && <p>{state.message}</p>}
      {(state.kind === 'ended' || state.kind === 'unavailable') && <p>{state.message}</p>}
      {ev && state.kind !== 'pending' && <p className="muted small">{windowText(ev.reveal_at)}</p>}

      {ev && (
        <div className="row gap">
          {(state.kind === 'ready' || (state.kind === 'ended' && state.canVerify)) && (
            <button className="btn small" onClick={run} disabled={running}>
              {running ? 'Verifying…' : 'Verify hand'}
            </button>
          )}
          <button className="btn ghost small" onClick={download}>
            {seed ? 'Export evidence' : 'Export evidence (seed not revealed yet)'}
          </button>
        </div>
      )}

      {running && <p role="status">Rebuilding the shuffle and replaying the hand…</p>}
      {failure && <div role="alert">{failure}</div>}
      {report && (
        <>
          <table className="verify-rows" aria-label="Verification result">
            <tbody>
              {verificationRows(report).map((r) => (
                <tr key={r.key} className={`verify-${r.tone}`}>
                  <th scope="row">{r.label}</th>
                  <td>{r.statusText}</td>
                  <td className="muted small">{r.basisText}</td>
                  <td className="small">{r.detail}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {report.errors.map((e, i) => (
            <div key={i} role="alert" className="small">
              {e}
            </div>
          ))}
          {report.warnings.map((w, i) => (
            <div key={i} className="muted small">
              {w}
            </div>
          ))}
          {report.trusted.length > 0 && (
            <ul className="muted small">
              {report.trusted.map((t, i) => (
                <li key={i}>
                  Taken on trust, {t.what}: {t.detail}
                </li>
              ))}
            </ul>
          )}
        </>
      )}

      <p className="muted small verify-disclaimer">{DISCLAIMER}</p>
      <p className="muted small">{D1_LINE}</p>
    </section>
  );
}
