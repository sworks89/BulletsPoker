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

/**
 * The open tables after checking them against the seats the server says I
 * hold. Every seat is open (cash or tournament, whatever the watch limit);
 * only watched tables count toward `watchLimit`, and the oldest watched ones
 * make room. `added` are the seats that were not open before.
 */
export function reconcileSeats(
  open: bigint[],
  seats: bigint[],
  watchLimit: number,
): { open: bigint[]; added: bigint[] } {
  const added = seats.filter((id) => !open.includes(id));
  const all = [...open, ...added];
  const watched = all.filter((id) => !seats.includes(id));
  let evict = watched.length - watchLimit;
  const kept = all.filter((id) => {
    if (seats.includes(id) || evict <= 0) return true;
    evict -= 1;
    return false;
  });
  return { open: kept, added };
}
