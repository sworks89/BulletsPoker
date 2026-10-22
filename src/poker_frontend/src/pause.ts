/** Matches the reason the backend records when its automatic solvency check pauses withdrawals. */
export const SOLVENCY_REASON_PREFIX = 'automatic solvency check';

type Pause = { deposits: boolean; withdrawals: boolean; dealing: boolean; joins: boolean };

/**
 * The banner text for the pause flags that are set, or '' when nothing is paused.
 * Makes no blanket promises about balances: an automatic solvency pause says so plainly.
 */
export function pauseBanner(p: Pause, reason?: string | null): string {
  const notes: string[] = [];
  const solvency = !!reason && reason.toLowerCase().startsWith(SOLVENCY_REASON_PREFIX);
  if (p.deposits) notes.push('Deposits are paused. Please do not send new funds until they reopen.');
  if (p.withdrawals) {
    notes.push(
      solvency
        ? 'A balance check found a shortfall. Withdrawals are paused and the team has been alerted. Your balance record is unchanged.'
        : 'Withdrawals are paused by the operators.',
    );
  }
  if (p.dealing) notes.push('No new hands are being dealt. Hands already in progress will finish.');
  if (p.joins) notes.push('New tables, seats and tournament entries are paused. You can keep playing where you are seated.');
  if (notes.length === 0) return '';
  const text = notes.join(' ');
  return reason && !solvency ? `${text} Reason: ${reason}` : text;
}
