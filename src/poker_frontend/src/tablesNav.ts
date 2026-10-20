/**
 * Which tables stay open after some close, and where to go if the one on screen
 * was among them: the last table still open, or the lobby. `goTo` is null when
 * the screen doesn't need to change.
 */
export function afterClosing(
  open: bigint[],
  active: bigint | null,
  closing: bigint[],
): { open: bigint[]; goTo: bigint | 'lobby' | null } {
  const left = open.filter((id) => !closing.includes(id));
  if (active === null || !closing.includes(active)) return { open: left, goTo: null };
  return { open: left, goTo: left.length ? left[left.length - 1] : 'lobby' };
}
