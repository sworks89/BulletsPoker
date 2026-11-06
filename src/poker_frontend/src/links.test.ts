import { describe, expect, it } from 'vitest';
import { readTournamentParam } from './links';

describe('the link in a tournament alert', () => {
  it('reads the tournament id', () => {
    expect(readTournamentParam('?tournament=7')).toBe(7n);
    expect(readTournamentParam('?ref=abc&tournament=18446744073709551615')).toBe(18446744073709551615n);
  });

  it('ignores anything that is not an id', () => {
    for (const search of ['', '?tournament=', '?tournament=abc', '?tournament=-1', '?tournament=1e3', '?tournament=123456789012345678901']) {
      expect(readTournamentParam(search)).toBeNull();
    }
  });
});
