import { describe, expect, it } from 'vitest';
import { isMyTurn } from './turn';

const view = (sittingOut: boolean, toAct: number[] = [0]) => ({
  my_seat: [0] as [] | [number],
  to_act: toAct as [] | [number],
  seats: [[{ sitting_out: sittingOut }], [{ sitting_out: false }]] as ([] | [{ sitting_out: boolean }])[],
});

describe('isMyTurn', () => {
  it('is my turn when I am to act', () => {
    expect(isMyTurn(view(false))).toBe(true);
  });
  it('is not my turn while I am sitting out, even if the server has not acted for me yet', () => {
    expect(isMyTurn(view(true))).toBe(false);
  });
  it('is not my turn when someone else is to act', () => {
    expect(isMyTurn(view(false, [1]))).toBe(false);
  });
  it('is not my turn without a view', () => {
    expect(isMyTurn(undefined)).toBe(false);
  });
});
