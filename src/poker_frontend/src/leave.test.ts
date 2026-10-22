import { describe, expect, it } from 'vitest';
import { LEAVE_CONFIRM, leaveLabel, leavesAfterHand } from './leave';

const me = (in_hand: boolean, folded = false, leaving = false) => ({ in_hand, folded, leaving });

describe('leavesAfterHand', () => {
  it('waits for the hand to end while I am still in it', () => {
    expect(leavesAfterHand('Flop', me(true))).toBe(true);
    expect(leavesAfterHand('PreFlop', me(true))).toBe(true);
  });
  it('leaves at once between hands, after folding, or when not dealt in', () => {
    expect(leavesAfterHand('HandOver', me(true))).toBe(false);
    expect(leavesAfterHand('Waiting', me(false))).toBe(false);
    expect(leavesAfterHand('River', me(true, true))).toBe(false);
    expect(leavesAfterHand('Turn', me(false))).toBe(false);
  });
});

describe('leave wording', () => {
  it('explains that the hand plays on', () => {
    expect(LEAVE_CONFIRM).toBe(
      "You'll leave when this hand ends. Your remaining turns are checked or folded for you; an all-in hand still plays to showdown.",
    );
  });
  it('labels the button by state', () => {
    expect(leaveLabel(me(true))).toBe('Leave table');
    expect(leaveLabel(me(true, false, true))).toBe('Leaving after this hand…');
  });
});
