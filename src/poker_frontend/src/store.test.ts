import { describe, expect, it } from 'vitest';
import type { StoreItem } from './ic';
import { avatarCards, ownsItem } from './store';
import './art/Avatar';

function avatar(id: number, icon: string, pair?: number, price = 10_000_000n): StoreItem {
  return {
    id,
    name: icon,
    kind: { Avatar: null },
    price,
    chip_price: 0n,
    icon,
    description: '',
    available: true,
    pair: pair === undefined ? [] : [pair],
  };
}

describe('owning store items', () => {
  it('owns free items, bought items, and the other version of a bought avatar', () => {
    const free = avatar(1, '🙂', undefined, 0n);
    const viking = avatar(2, '🪓', 3);
    const maiden = avatar(3, '🪖', 2);
    const alien = avatar(4, '👽');
    expect(ownsItem(free, [])).toBe(true);
    expect(ownsItem(maiden, [3])).toBe(true);
    expect(ownsItem(viking, [3])).toBe(true);
    expect(ownsItem(alien, [3])).toBe(false);
  });

  it('does not count chip-only items as free', () => {
    expect(ownsItem({ ...avatar(5, '🤖', undefined, 0n), chip_price: 1_000n }, [])).toBe(false);
  });
});

describe('avatar cards in the store', () => {
  it('puts both versions of a paired avatar on one card, male first', () => {
    const items = [avatar(1, '👽'), avatar(2, '🪖', 3), avatar(3, '🪓', 2), avatar(4, '😊', 5), avatar(5, '🙂', 4)];
    expect(avatarCards(items).map((card) => card.map((i) => i.icon))).toEqual([['👽'], ['🪓', '🪖'], ['🙂', '😊']]);
  });

  it('shows a version on its own when its partner is missing from the list', () => {
    expect(avatarCards([avatar(2, '🪓', 3)]).map((card) => card.map((i) => i.id))).toEqual([[2]]);
  });
});
