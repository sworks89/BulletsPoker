import { useEffect, useState } from 'react';

/**
 * A phone held sideways: wide and short. The table then gets its own layout
 * (felt filling the height, actions in a column on the right). Touch screens
 * only, so a short laptop window keeps the desktop layout. styles.css repeats
 * these queries; phone.test.ts checks the copies match.
 */
export const SIDEWAYS_QUERY = '(orientation: landscape) and (max-height: 500px) and (pointer: coarse)';

/** A phone held upright, which gets the hint to turn it sideways. */
export const UPRIGHT_PHONE_QUERY = '(orientation: portrait) and (max-width: 700px) and (pointer: coarse)';

/** Whether a media query matches, kept up to date as it changes. */
export function useMedia(query: string): boolean {
  const get = () => typeof matchMedia === 'function' && matchMedia(query).matches;
  const [matches, setMatches] = useState(get);
  useEffect(() => {
    if (typeof matchMedia !== 'function') return;
    const mq = matchMedia(query);
    const on = () => setMatches(mq.matches);
    on();
    // Safari before 14 only has the older addListener.
    if (mq.addEventListener) {
      mq.addEventListener('change', on);
      return () => mq.removeEventListener('change', on);
    }
    mq.addListener(on);
    return () => mq.removeListener(on);
  }, [query]);
  return matches;
}

type Storage = { getItem(k: string): string | null; setItem(k: string, v: string): void };

/** localStorage, reached lazily: even touching it throws when site data is blocked. */
export const browserStorage: Storage = {
  getItem: (k) => localStorage.getItem(k),
  setItem: (k, v) => localStorage.setItem(k, v),
};

/** Whether the page is fullscreen, kept up to date. */
export function useFullscreen(): boolean {
  const [on, setOn] = useState(() => typeof document !== 'undefined' && !!document.fullscreenElement);
  useEffect(() => {
    const change = () => setOn(!!document.fullscreenElement);
    document.addEventListener('fullscreenchange', change);
    return () => document.removeEventListener('fullscreenchange', change);
  }, []);
  return on;
}
const HINT_KEY = 'bp.rotateHint';

/** The "turn your phone sideways" hint, once dismissed, stays dismissed on this device. */
export function hintDismissed(storage: Storage): boolean {
  try {
    return storage.getItem(HINT_KEY) === 'off';
  } catch {
    return false;
  }
}

export function dismissHint(storage: Storage): void {
  try {
    storage.setItem(HINT_KEY, 'off');
  } catch {
    // storage unavailable (private mode): the hint just comes back next time
  }
}

/**
 * Fullscreen is offered on touch screens whose browser allows it. That leaves
 * out iPhones, where only videos can go fullscreen.
 */
export function canGoFullscreen(doc: { fullscreenEnabled: boolean }, touch: boolean): boolean {
  return doc.fullscreenEnabled && touch;
}

interface FullscreenDoc {
  fullscreenElement: unknown;
  documentElement: { requestFullscreen(): Promise<void> };
  exitFullscreen(): Promise<void>;
}

/** `screen.orientation`; TypeScript's DOM types leave out `lock`, which only some browsers have. */
export type Orientation = { lock?(o: 'landscape'): Promise<void> };

/**
 * Enters fullscreen and turns the screen sideways, or leaves fullscreen. A
 * refusal is ignored: the table works the same without fullscreen.
 */
export async function toggleFullscreen(doc: FullscreenDoc, orientation?: Orientation): Promise<void> {
  try {
    if (doc.fullscreenElement) {
      await doc.exitFullscreen();
      return;
    }
    await doc.documentElement.requestFullscreen();
    // Locking only works in fullscreen, and not in every browser; fullscreen alone still helps.
    await orientation?.lock?.('landscape');
  } catch {
    // not allowed here (an embedding page, or the lock): stay as we are
  }
}
