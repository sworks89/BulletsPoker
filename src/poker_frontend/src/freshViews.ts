/**
 * The table views to show after a poll: one per table just fetched, but never
 * older than the one already held. On mainnet a query may be answered by a
 * replica a little behind the others; showing its view would briefly undo the
 * hand (cards vanish, then come back). A view's `now` is the time of the state
 * it was read from, so an older `now` means older state.
 */
export function freshest<V extends { id: bigint; now: bigint }>(held: Map<string, V>, fetched: V[]): Map<string, V> {
  return new Map(
    fetched.map((v) => {
      const key = String(v.id);
      const prev = held.get(key);
      return [key, prev && prev.now > v.now ? prev : v];
    }),
  );
}
