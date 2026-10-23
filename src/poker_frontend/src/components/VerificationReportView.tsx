import type { VerificationReport } from '../fair/verify';

const CHECKS = ['commitment', 'shuffle', 'dealing', 'replay', 'settlement'] as const;
const MARK = { pass: 'Passed', fail: 'Failed', skipped: 'Skipped' } as const;

/** A plain list of a verification report: overall result, checks, errors, warnings, what is trusted. */
export function VerificationReportView({ report }: { report: VerificationReport }) {
  return (
    <section className="verify-report" aria-label="Verification report">
      <h3>
        Result: <span className={`verify-overall ${report.overall.toLowerCase()}`}>{report.overall}</span>
      </h3>
      <ul aria-label="Checks">
        {CHECKS.map((k) => (
          <li key={k}>
            <strong>{k}</strong>: {MARK[report.checks[k].status]} ({report.checks[k].basis}). {report.checks[k].detail}
          </li>
        ))}
      </ul>
      {report.errors.length > 0 && (
        <div role="alert">
          <strong>Errors</strong>
          <ul>
            {report.errors.map((e, i) => (
              <li key={i}>{e}</li>
            ))}
          </ul>
        </div>
      )}
      {report.warnings.length > 0 && (
        <div>
          <strong>Warnings</strong>
          <ul>
            {report.warnings.map((w, i) => (
              <li key={i}>{w}</li>
            ))}
          </ul>
        </div>
      )}
      {report.trusted.length > 0 && (
        <div>
          <strong>Taken on trust</strong>
          <ul>
            {report.trusted.map((t, i) => (
              <li key={i}>
                {t.what}: {t.detail}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
