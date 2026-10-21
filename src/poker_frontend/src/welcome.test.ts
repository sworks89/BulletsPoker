import { describe, expect, it } from 'vitest';
import type { WelcomeView } from './ic';
import { daysLeft, welcomeProgress, welcomeStatus } from './welcome';

const E8S = 100_000_000n;
const view = (o: Partial<WelcomeView> = {}): WelcomeView => ({
  eligible: false,
  active: false,
  amount: 0n,
  released: 0n,
  locked: 0n,
  expires_at: 0n,
  expired: false,
  ...o,
});

describe('welcomeProgress', () => {
  it('is the released share of the bonus', () => {
    expect(welcomeProgress(view({ amount: 10n * E8S, released: 5n * E8S }))).toBe(0.5);
    expect(welcomeProgress(view({ amount: 10n * E8S, released: 10n * E8S }))).toBe(1);
  });
  it('is 0 with no bonus and never above 1', () => {
    expect(welcomeProgress(view())).toBe(0);
    expect(welcomeProgress(view({ amount: 4n, released: 9n }))).toBe(1);
  });
});

describe('daysLeft', () => {
  const day = 86_400_000;
  const at = (ms: number) => BigInt(ms) * 1_000_000n;
  it('rounds down to whole days', () => {
    expect(daysLeft(at(10 * day + 5), 0)).toBe(10);
    expect(daysLeft(at(10 * day), 1)).toBe(9);
  });
  it('is 0 at and after expiry', () => {
    expect(daysLeft(at(day), day)).toBe(0);
    expect(daysLeft(at(day), 5 * day)).toBe(0);
  });
});

describe('welcomeStatus', () => {
  it('names each state', () => {
    expect(welcomeStatus(view({ eligible: true }))).toBe('offer');
    expect(welcomeStatus(view({ active: true, amount: 8n, released: 3n, locked: 5n }))).toBe('active');
    expect(welcomeStatus(view({ amount: 8n, released: 8n }))).toBe('done');
    expect(welcomeStatus(view({ amount: 8n, released: 3n, expired: true }))).toBe('expired');
    expect(welcomeStatus(view())).toBe('none');
  });
});
