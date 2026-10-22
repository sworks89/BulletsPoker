import { describe, expect, it } from 'vitest';
import { agoText } from './format';

const nsAgo = (ms: number) => BigInt(Date.now() - ms) * 1_000_000n;

describe('agoText', () => {
  it('never reads "now ago"', () => {
    expect(agoText(nsAgo(5_000))).toBe('just now');
    expect(agoText(nsAgo(5 * 60_000))).toBe('5m ago');
    expect(agoText(nsAgo(3 * 3_600_000))).toBe('3h ago');
  });
});
