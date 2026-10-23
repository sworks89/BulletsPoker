import { useId, useState } from 'react';
import { fromEvidenceJson } from '../fair/evidence';
import { verifyHand, type VerificationReport } from '../fair/verify';
import { VerificationReportView } from './VerificationReportView';

/**
 * Verifies a hand evidence file (see fair/evidence.ts) entirely in the
 * browser. It makes no canister call and needs no sign-in.
 */
export function VerifyEvidence() {
  const [text, setText] = useState('');
  const [report, setReport] = useState<VerificationReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const fileId = useId();
  const textId = useId();

  const check = async (source: string) => {
    setReport(null);
    setError(null);
    const parsed = fromEvidenceJson(source);
    if ('error' in parsed) {
      setError(parsed.error);
      return;
    }
    setBusy(true);
    try {
      setReport(await verifyHand(parsed.record, parsed.evidence, parsed.canister, { now: BigInt(Date.now()) * 1_000_000n }));
    } catch (e) {
      setError(`Verification failed to run: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  const load = async (file: File | undefined) => {
    if (!file) return;
    const content = await file.text();
    setText(content);
    await check(content);
  };

  return (
    <div className="panel">
      <div className="panel-title">Verify a hand file</div>
      <p className="muted small">
        Check a hand evidence file (<code>bulletspoker-hand-evidence/v1</code>) on your own device. Nothing is sent anywhere and the game canister is not
        contacted.
      </p>
      <label htmlFor={fileId}>Evidence file (.json)</label>
      <input id={fileId} type="file" accept=".json,application/json" onChange={(e) => void load(e.target.files?.[0])} />
      <label htmlFor={textId}>Or paste the evidence JSON</label>
      <textarea id={textId} rows={8} value={text} onChange={(e) => setText(e.target.value)} spellCheck={false} />
      <div>
        <button className="btn" disabled={busy || !text.trim()} onClick={() => void check(text)}>
          {busy ? 'Verifying…' : 'Verify'}
        </button>
      </div>
      {error && <div role="alert">{error}</div>}
      {report && <VerificationReportView report={report} />}
    </div>
  );
}
