import { beforeEach, describe, expect, it, vi } from 'vitest';
import { readRefParam, referralLink, rememberRef, takeRememberedRef } from './referrals';

describe('readRefParam', () => {
  it('reads and trims the code', () => {
    expect(readRefParam('?ref=Mark')).toBe('Mark');
    expect(readRefParam('?x=1&ref=%20Bob%20')).toBe('Bob');
  });
  it('returns null when missing, empty or too long', () => {
    expect(readRefParam('?x=1')).toBeNull();
    expect(readRefParam('?ref=')).toBeNull();
    expect(readRefParam('?ref=' + 'a'.repeat(33))).toBeNull();
    expect(readRefParam('?ref=' + 'a'.repeat(32))).toBe('a'.repeat(32));
  });
});

describe('referralLink', () => {
  it('encodes the code', () => {
    expect(referralLink('https://a.b', 'Mark Al')).toBe('https://a.b/?ref=Mark%20Al');
  });
});

describe('remembered ref', () => {
  beforeEach(() => {
    const store = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, v),
      removeItem: (k: string) => void store.delete(k),
    });
  });
  it('is taken once', () => {
    rememberRef('Mark');
    expect(takeRememberedRef()).toBe('Mark');
    expect(takeRememberedRef()).toBeNull();
  });
  it('survives a broken localStorage', () => {
    vi.stubGlobal('localStorage', {
      getItem: () => { throw new Error('no'); },
      setItem: () => { throw new Error('no'); },
      removeItem: () => { throw new Error('no'); },
    });
    expect(() => rememberRef('x')).not.toThrow();
    expect(takeRememberedRef()).toBeNull();
  });
});
