import { useId, useState } from 'react';
import { MAX_EVIDENCE_CHARS, fromEvidenceJson } from '../fair/evidence';
import { verifyHand, type VerificationReport } from '../fair/verify';
import { defaultStore, seenCommitment } from '../fair/seen';
import { backendId } from '../ic';
import { UNSIGNED_LINE, canisterNote } from '../replay';
import { VerificationReportView } from './VerificationReportView';

/**
 * Verifies a hand evidence file (see fair/evidence.ts) entirely in the
 * browser. It makes no canister call and needs no sign-in.
 */
export function VerifyEvidence() {
  const [text, setText] = useState('');
  const [report, setReport] = useState<VerificationReport | null>(null);
  const [claimed, setClaimed] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const fileId = useId();
  const textId = useId();

  const check = async (source: string) => {
    setReport(null);
    setClaimed(null);
    setError(null);
    setBusy(true);
    try {
      const parsed = fromEvidenceJson(source);
      if ('error' in parsed) {
        setError(parsed.error);
        return;
      }
      const observed = seenCommitment(defaultStore(), parsed.canister, parsed.evidence.table_id, parsed.evidence.hand_no);
      setClaimed(parsed.canister);
      setReport(await verifyHand(parsed.record, parsed.evidence, parsed.canister, { now: BigInt(Date.now()) * 1_000_000n, observedCommitment: observed }));
    } catch (e) {
      setError(`Verification failed to run: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  const load = async (file: File | undefined) => {
    if (!file) return;
    setReport(null);
    if (file.size > MAX_EVIDENCE_CHARS) {
      setError(`That file is too large to be a hand evidence file (over ${MAX_EVIDENCE_CHARS.toLocaleString('en-US')} bytes).`);
      return;
    }
    let content: string;
    try {
      content = await file.text();
    } catch (e) {
      setError(`Could not read the file: ${e instanceof Error ? e.message : String(e)}`);
      return;
    }
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
      <textarea id={textId} rows={8} value={text} maxLength={MAX_EVIDENCE_CHARS}
        onChange={(e) => setText(e.target.value)} spellCheck={false} />
      <div>
        <button className="btn" disabled={busy || !text.trim()} onClick={() => void check(text)}>
          {busy ? 'Verifying…' : 'Verify'}
        </button>
      </div>
      {error && <div role="alert">{error}</div>}
      {report && claimed !== null && (
        <>
          {(() => {
            const note = canisterNote(claimed, backendId);
            return note.warn ? <div role="alert">{note.text}</div> : <p className="small">{note.text}</p>;
          })()}
          <p className="small verify-unsigned">
            <strong>{UNSIGNED_LINE}</strong>
          </p>
          <VerificationReportView report={report} />
        </>
      )}
    </div>
  );
}
