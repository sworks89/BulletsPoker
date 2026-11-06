import type { View } from './context';
import { CANISTER_ORIGIN } from './domains';

/** Where a tapped alert ends up. */
export const PLAY_ORIGIN = 'https://www.bulletspoker.com';

/**
 * Where to send a tapped alert, or null to stay put. Alerts have to open the
 * canister address to pass Internet Identity's check (src/poker_backend/src/alerts.rs),
 * but players are signed in, and have the game installed, on the main site.
 */
export function alertForward(hostname: string, search: string): string | null {
  if (hostname !== new URL(CANISTER_ORIGIN).hostname) return null;
  const view = readLaunchView(search);
  if (view === null) return null;
  if (view.name === 'tournaments') return `${PLAY_ORIGIN}/?tournament=${view.id}`;
  if (view.name === 'friends' && view.chat) return `${PLAY_ORIGIN}/?open=friends&chat=${view.chat}`;
  return `${PLAY_ORIGIN}/?open=${view.name}`;
}

/** The parameters an alert's link can carry, cleared from the address bar once read. */
export const LAUNCH_PARAMS = ['tournament', 'open', 'chat'];

/** A principal in text form: lower-case groups of letters and digits. */
const PRINCIPAL_TEXT = /^[a-z2-7]{1,5}(-[a-z2-7]{1,5})*$/;

/** The page an alert's link names, or null if it names none. */
export function readLaunchView(search: string): View | null {
  const params = new URLSearchParams(search);
  const id = readTournamentParam(search);
  if (id !== null) return { name: 'tournaments', id };
  const open = params.get('open');
  if (open === 'lobby') return { name: 'lobby' };
  if (open === 'friends') {
    const chat = params.get('chat');
    return chat && PRINCIPAL_TEXT.test(chat) ? { name: 'friends', chat } : { name: 'friends' };
  }
  return null;
}

/** Longest decimal a u64 tournament id can be. */
const MAX_ID_DIGITS = 20;

/** The tournament an alert's link names (`?tournament=7`), or null if it names none. */
export function readTournamentParam(search: string): bigint | null {
  const v = new URLSearchParams(search).get('tournament')?.trim();
  return v && v.length <= MAX_ID_DIGITS && /^\d+$/.test(v) ? BigInt(v) : null;
}
