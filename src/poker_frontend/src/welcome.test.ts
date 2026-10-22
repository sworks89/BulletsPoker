import { afterEach, describe, expect, it, vi } from 'vitest';
import type { WelcomeView } from './ic';
import { daysLeft, lastAnnounced, markAnnounced, toastFor, welcomeProgress, welcomeStatus } from './welcome';

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
  it('rounds up, so the last day never reads 0', () => {
    expect(daysLeft(at(10 * day + 5), 0)).toBe(11);
    expect(daysLeft(at(10 * day), 0)).toBe(10);
    expect(daysLeft(at(10 * day), 1)).toBe(10);
    expect(daysLeft(at(day), day - 3_600_000)).toBe(1);
    expect(daysLeft(at(day + 1), 0)).toBe(2);
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

function stubStorage(store: Record<string, string> = {}) {
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => store[k] ?? null,
    setItem: (k: string, v: string) => void (store[k] = v),
  });
  return store;
}

describe('lastAnnounced / markAnnounced', () => {
  afterEach(() => vi.unstubAllGlobals());
  it('remembers one status per principal', () => {
    stubStorage();
    expect(lastAnnounced('a')).toBeNull();
    markAnnounced('a', 'active');
    markAnnounced('b', 'done');
    expect(lastAnnounced('a')).toBe('active');
    expect(lastAnnounced('b')).toBe('done');
    markAnnounced('a', 'expired');
    expect(lastAnnounced('a')).toBe('expired');
    expect(lastAnnounced('b')).toBe('done');
  });
  it('recovers from corrupt storage', () => {
    stubStorage({ 'bp.welcome': '{nope' });
    expect(lastAnnounced('a')).toBeNull();
    markAnnounced('a', 'active');
    expect(lastAnnounced('a')).toBe('active');
  });
  it('does not throw when storage throws', () => {
    const boom = () => {
      throw new Error('denied');
    };
    vi.stubGlobal('localStorage', { getItem: boom, setItem: boom });
    expect(lastAnnounced('a')).toBeNull();
    expect(() => markAnnounced('a', 'active')).not.toThrow();
  });
});

describe('toastFor', () => {
  it('announces each status once', () => {
    expect(toastFor('active', 'active')).toBeNull();
    expect(toastFor(null, 'active')).toBe('active');
    expect(toastFor('active', 'done')).toBe('done');
    expect(toastFor('active', 'expired')).toBe('expired');
  });
  it('stays quiet about done/expired when nothing was announced here (new device)', () => {
    expect(toastFor(null, 'done')).toBeNull();
    expect(toastFor(null, 'expired')).toBeNull();
  });
  it('never toasts the offer or none', () => {
    expect(toastFor(null, 'offer')).toBeNull();
    expect(toastFor('active', 'none')).toBeNull();
  });
});
