// Spotting gifts that just arrived. A gift is identified by when it was sent,
// so a second beer after the first still counts as new.

export interface GiftSeat {
  seat: number;
  username: string;
  gift: [] | [string];
  gift_from: [] | [string];
  gift_sent_at: [] | [bigint];
}

export interface GiftArrival {
  seat: number;
  icon: string;
  /** The sender's seat at this table, or null when they aren't seated here. */
  fromSeat: number | null;
}

/** Compares this poll's seats with the last one. `prev` is null on first load, which shows gifts without arrivals. */
export function giftArrivals(prev: Map<number, bigint> | null, seats: GiftSeat[]): { next: Map<number, bigint>; arrivals: GiftArrival[] } {
  const next = new Map<number, bigint>();
  const arrivals: GiftArrival[] = [];
  for (const s of seats) {
    const sentAt = s.gift_sent_at[0];
    const icon = s.gift[0];
    if (sentAt === undefined || icon === undefined) continue;
    next.set(s.seat, sentAt);
    if (prev && prev.get(s.seat) !== sentAt) {
      const from = s.gift_from[0];
      const sender = seats.find((x) => x.username === from && x.seat !== s.seat);
      arrivals.push({ seat: s.seat, icon, fromSeat: sender ? sender.seat : null });
    }
  }
  return { next, arrivals };
}
