import { describe, expect, it } from 'vitest';
import { nextRetryMs, shouldOfferRegistration } from './startup';

describe('nextRetryMs', () => {
  it('backs off exponentially from one second', () => {
    expect([0, 1, 2, 3, 4].map(nextRetryMs)).toEqual([1000, 2000, 4000, 8000, 16000]);
  });
  it('caps at thirty seconds', () => {
    expect(nextRetryMs(5)).toBe(30000);
    expect(nextRetryMs(50)).toBe(30000);
    expect(nextRetryMs(5000)).toBe(30000);
  });
  it('treats bad attempts as the first', () => {
    expect(nextRetryMs(-3)).toBe(1000);
    expect(nextRetryMs(NaN)).toBe(1000);
  });
});

describe('shouldOfferRegistration', () => {
  it('only when the lookup established that no profile exists', () => {
    expect(shouldOfferRegistration('none')).toBe(true);
    for (const l of ['unknown', 'loading', 'found', 'failed'] as const) {
      expect(shouldOfferRegistration(l)).toBe(false);
    }
  });
});
