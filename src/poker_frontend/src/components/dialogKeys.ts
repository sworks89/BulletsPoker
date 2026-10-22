/** Pure keyboard helpers for Dialog, kept DOM-free so they can be unit tested. */

export const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function isActivateKey(key: string): boolean {
  return key === 'Enter' || key === ' ';
}

export function isCloseKey(key: string): boolean {
  return key === 'Escape';
}

/**
 * Where Tab should send focus to keep it inside a dialog, or null to let the
 * browser move it normally. `items` are the focusable elements in order.
 */
export function trapTarget<T>(items: T[], active: T | null, shift: boolean, activeInside: boolean): T | 'container' | null {
  if (items.length === 0) return 'container';
  const first = items[0];
  const last = items[items.length - 1];
  if (!activeInside) return shift ? last : first;
  if (shift && active === first) return last;
  if (!shift && active === last) return first;
  return null;
}

/** A passive dialog (one that opens by itself) leaves focus where the user has it. */
export const movesFocusIn = (passive: boolean, focusInside: boolean): boolean => !passive && !focusInside;
export const restoresFocus = (passive: boolean): boolean => !passive;
