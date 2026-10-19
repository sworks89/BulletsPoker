/** The parts of a table view that decide whose turn it is. */
interface TurnView {
  my_seat: [] | [number];
  to_act: [] | [number];
  seats: ([] | [{ sitting_out: boolean }])[];
}

/**
 * Whether the viewer should act now. Not while sitting out: the server checks or
 * folds for a player who is away, so there is nothing to click until "I'm back".
 */
export const isMyTurn = (v: TurnView | undefined): boolean => {
  const seat = v?.my_seat[0];
  if (!v || seat === undefined || v.to_act[0] !== seat) return false;
  return !v.seats[seat]?.[0]?.sitting_out;
};
