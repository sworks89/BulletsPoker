import { describe, expect, it } from 'vitest';
import { pauseBanner } from './pause';

const none = { deposits: false, withdrawals: false, dealing: false, joins: false };
const SOLV = 'automatic solvency check failed; the team has been alerted';

describe('pauseBanner', () => {
  it('is empty when nothing is paused', () => expect(pauseBanner(none)).toBe(''));
  it('never promises that nothing is lost', () => {
    const all = { deposits: true, withdrawals: true, dealing: true, joins: true };
    for (const r of [undefined, 'maintenance', SOLV]) expect(pauseBanner(all, r)).not.toMatch(/nothing is lost|safe/i);
  });
  it('says plainly that a solvency check found a shortfall', () => {
    const t = pauseBanner({ ...none, withdrawals: true }, SOLV);
    expect(t).toContain('A balance check found a shortfall');
    expect(t).toContain('Your balance record is unchanged');
    expect(t).not.toContain('Reason:');
  });
  it('keeps admin pauses neutral and shows the reason', () => {
    const t = pauseBanner({ ...none, withdrawals: true }, 'upgrade');
    expect(t).toBe('Withdrawals are paused by the operators. Reason: upgrade');
  });
  it('describes each other flag', () => {
    expect(pauseBanner({ ...none, deposits: true })).toMatch(/Deposits are paused/);
    expect(pauseBanner({ ...none, dealing: true })).toMatch(/No new hands/);
    expect(pauseBanner({ ...none, joins: true })).toMatch(/New tables, seats/);
  });
});
