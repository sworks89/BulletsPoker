/** The parts of my seat that decide how leaving the table works. */
interface LeaveSeat {
  in_hand: boolean;
  folded: boolean;
  leaving: boolean;
}

const BETTING = new Set(['PreFlop', 'Flop', 'Turn', 'River']);

export const LEAVE_CONFIRM =
  "You'll leave when this hand ends. Your remaining turns are checked or folded for you; an all-in hand still plays to showdown.";

/**
 * Whether leaving now waits for the hand to end: I'm still in it (all-in
 * included). Otherwise the server stands me up at once.
 */
export const leavesAfterHand = (phase: string, me: Pick<LeaveSeat, 'in_hand' | 'folded'>): boolean =>
  BETTING.has(phase) && me.in_hand && !me.folded;

export const leaveLabel = (me: Pick<LeaveSeat, 'leaving'>): string => (me.leaving ? 'Leaving after this hand…' : 'Leave table');

/** True when the server already has me leaving after this hand, so asking again would be pointless. */
export const alreadyLeaving = (ref: { leaving: boolean } | undefined | null, me: { leaving: boolean } | undefined | null): boolean =>
  !!ref?.leaving || !!me?.leaving;
