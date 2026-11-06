import { describe, expect, it } from 'vitest';
import { CANISTER_ORIGIN, CUSTOM_DOMAINS } from './domains';
import { alertForward, PLAY_ORIGIN, readLaunchView, readTournamentParam } from './links';

const CANISTER_HOST = new URL(CANISTER_ORIGIN).hostname;

describe('forwarding an alert to the main site', () => {
  it('sends a tapped alert from the canister address to the same tournament on the main site', () => {
    expect(alertForward(CANISTER_HOST, '?tournament=7')).toBe('https://www.bulletspoker.com/?tournament=7');
  });

  it('forwards the daily chips, friend request and friend message alerts too', () => {
    expect(alertForward(CANISTER_HOST, '?open=lobby')).toBe('https://www.bulletspoker.com/?open=lobby');
    expect(alertForward(CANISTER_HOST, '?open=friends')).toBe('https://www.bulletspoker.com/?open=friends');
    expect(alertForward(CANISTER_HOST, '?open=friends&chat=2vxsx-fae')).toBe('https://www.bulletspoker.com/?open=friends&chat=2vxsx-fae');
  });

  it('forwards to a domain the site is really served at', () => {
    expect(CUSTOM_DOMAINS).toContain(new URL(PLAY_ORIGIN).hostname);
  });

  it('leaves people who came to the canister address directly where they are', () => {
    expect(alertForward(CANISTER_HOST, '')).toBeNull();
    expect(alertForward(CANISTER_HOST, '?ref=abc')).toBeNull();
    expect(alertForward(CANISTER_HOST, '?tournament=abc')).toBeNull();
  });

  it('never forwards from the main site, or from a local copy of the game', () => {
    expect(alertForward('www.bulletspoker.com', '?tournament=7')).toBeNull();
    expect(alertForward('bulletspoker.com', '?tournament=7')).toBeNull();
    expect(alertForward('abcde-cai.localhost', '?tournament=7')).toBeNull();
  });
});

describe('the page an alert opens', () => {
  it('opens the lobby, the friends page or a conversation', () => {
    expect(readLaunchView('?open=lobby')).toEqual({ name: 'lobby' });
    expect(readLaunchView('?open=friends')).toEqual({ name: 'friends' });
    expect(readLaunchView('?open=friends&chat=2vxsx-fae')).toEqual({ name: 'friends', chat: '2vxsx-fae' });
    expect(readLaunchView('?tournament=7')).toEqual({ name: 'tournaments', id: 7n });
  });

  it('ignores a page it does not know, and a conversation that is not a principal', () => {
    expect(readLaunchView('')).toBeNull();
    expect(readLaunchView('?open=wallet')).toBeNull();
    expect(readLaunchView('?open=friends&chat=<script>')).toEqual({ name: 'friends' });
  });
});

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
