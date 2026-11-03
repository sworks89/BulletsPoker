import { describe, expect, it } from 'vitest';
import { CLOCK_WINDOW_MS, addClockSample, clockOffset } from './clock';

describe('clock offset', () => {
  it('ignores a poll answered by a replica running behind, so the timer never grows back', () => {
    // True offset 1000ms; the second poll's replica is 2s behind.
    let samples = addClockSample([], { at: 10_000, offset: 1_050 });
    samples = addClockSample(samples, { at: 11_000, offset: 3_050 });
    expect(clockOffset(samples)).toBe(1_050);
  });

  it('settles on the best sample seen', () => {
    let samples = addClockSample([], { at: 10_000, offset: 1_400 });
    samples = addClockSample(samples, { at: 11_000, offset: 1_020 });
    expect(clockOffset(samples)).toBe(1_020);
  });

  it('forgets samples older than the window, so a changed device clock is picked up', () => {
    let samples = addClockSample([], { at: 10_000, offset: 1_000 });
    samples = addClockSample(samples, { at: 10_000 + CLOCK_WINDOW_MS + 1, offset: 61_000 });
    expect(clockOffset(samples)).toBe(61_000);
  });

  it('is zero before any poll', () => {
    expect(clockOffset([])).toBe(0);
  });
});
