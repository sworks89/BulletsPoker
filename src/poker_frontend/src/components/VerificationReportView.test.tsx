import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { VerificationReportView } from './VerificationReportView';
import { CANISTER, caseNamed, recordOf } from '../fair/fixtures';
import { verifyHand } from '../fair/verify';

describe('VerificationReportView', () => {
  it('lists the overall result, every check and the errors', async () => {
    const record = recordOf(caseNamed('showdown_default_rates'));
    const bad = { ...record, pot: record.pot + 1n };
    const report = await verifyHand(bad, record.fair[0]!, CANISTER, { now: 2n ** 62n });
    const html = renderToStaticMarkup(<VerificationReportView report={report} />);
    expect(html).toContain('FAILED');
    for (const k of ['commitment', 'shuffle', 'dealing', 'replay', 'settlement']) expect(html).toContain(k);
    expect(html).toContain('role="alert"');
    expect(html).toContain('pot');
  });
});
