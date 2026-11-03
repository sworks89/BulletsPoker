/**
 * How far this device's clock is ahead of the game's, for countdowns. Each poll
 * gives a sample: the local time it arrived minus the view's `now`. A sample
 * can only overstate the offset (the reply took time to arrive, and the
 * replica that answered may be running behind), never understate it, so the
 * smallest recent sample is the best estimate. Using each poll's own sample
 * made the action timer jump back up whenever a lagging replica answered.
 */
export interface ClockSample {
  /** Local time the poll arrived (ms). */
  at: number;
  /** `at` minus the view's `now` (ms). */
  offset: number;
}

/** Samples older than this are dropped, so a change to the device clock is picked up. */
export const CLOCK_WINDOW_MS = 60_000;

export function addClockSample(samples: ClockSample[], sample: ClockSample): ClockSample[] {
  return [...samples.filter((s) => Math.abs(sample.at - s.at) <= CLOCK_WINDOW_MS), sample];
}

export function clockOffset(samples: ClockSample[]): number {
  return samples.length ? Math.min(...samples.map((s) => s.offset)) : 0;
}
