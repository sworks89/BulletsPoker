import { useSyncExternalStore } from 'react';

/** What this browser can do to put the game on the home screen. */
export type InstallOffer = 'none' | 'prompt' | 'ios';

/** Chromium's install event, which no TypeScript library describes yet. */
interface InstallPromptEvent extends Event {
  prompt(): Promise<void>;
}

// Not `bp.`: signing out clears those, and this belongs to the device.
const DISMISSED_KEY = 'bullets.install-dismissed';

/** Safari on iPhone and iPad has no install event. An iPad reports itself as a Mac with a touch screen. */
export function isIos(userAgent: string, platform: string, maxTouchPoints: number): boolean {
  return /iPad|iPhone|iPod/.test(userAgent) || (platform === 'MacIntel' && maxTouchPoints > 1);
}

export function installOffer(installed: boolean, canPrompt: boolean, ios: boolean): InstallOffer {
  if (installed) return 'none';
  if (canPrompt) return 'prompt';
  return ios ? 'ios' : 'none';
}

let deferred: InstallPromptEvent | null = null;
let installedNow = false;
const listeners = new Set<() => void>();
const changed = () => listeners.forEach((l) => l());

/**
 * Keeps the browser's install event for the Install button. It fires once, early,
 * so this runs before the app renders.
 */
export function captureInstallPrompt(): void {
  window.addEventListener('beforeinstallprompt', (e) => {
    e.preventDefault();
    deferred = e as InstallPromptEvent;
    changed();
  });
  window.addEventListener('appinstalled', () => {
    deferred = null;
    installedNow = true;
    changed();
  });
}

function runningInstalled(): boolean {
  return window.matchMedia('(display-mode: standalone)').matches || (navigator as Navigator & { standalone?: boolean }).standalone === true;
}

function currentOffer(): InstallOffer {
  return installOffer(installedNow || runningInstalled(), deferred !== null, isIos(navigator.userAgent, navigator.platform, navigator.maxTouchPoints));
}

export function useInstallOffer(): InstallOffer {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    currentOffer,
    () => 'none',
  );
}

/** Shows the browser's install dialog, which it allows once per event. */
export async function promptInstall(): Promise<void> {
  const event = deferred;
  deferred = null;
  changed();
  await event?.prompt();
}

export function wasInstallDismissed(): boolean {
  try {
    return localStorage.getItem(DISMISSED_KEY) !== null;
  } catch {
    return false;
  }
}

export function rememberInstallDismissed(): void {
  try {
    localStorage.setItem(DISMISSED_KEY, '1');
  } catch {
    /* storage unavailable: the banner just comes back next visit */
  }
}
