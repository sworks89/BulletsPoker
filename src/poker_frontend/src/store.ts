import type { StoreItem } from './ic';
import { artFor } from './art/registry';

/** Whether a player with `inventory` may equip `item`: it is free, bought, or the other version of a bought avatar. */
export function ownsItem(item: StoreItem, inventory: readonly number[] | Uint32Array): boolean {
  if (item.price === 0n && item.chip_price === 0n) return true;
  return inventory.includes(item.id) || item.pair.some((other) => inventory.includes(other));
}

/** Avatars grouped into store cards: one card per avatar, holding both versions of a pair, male first. */
export function avatarCards(avatars: StoreItem[]): StoreItem[][] {
  const byId = new Map(avatars.map((a) => [a.id, a]));
  const shown = new Set<number>();
  const cards: StoreItem[][] = [];
  for (const a of avatars) {
    if (shown.has(a.id)) continue;
    const other = a.pair.length ? byId.get(a.pair[0]) : undefined;
    const card = other ? [a, other] : [a];
    card.sort((x, y) => Number(artFor(y.icon)?.look === 'male') - Number(artFor(x.icon)?.look === 'male'));
    for (const i of card) shown.add(i.id);
    cards.push(card);
  }
  return cards;
}

/** The deck styles this app draws; a store deck's icon is `deck-<style>`. */
export const DECKS = ['classic', 'jumbo', 'neon', 'pixel', 'gold'] as const;
export type Deck = (typeof DECKS)[number];

/** The style of the deck with id `id`: the classic deck when there is none, or it is one this app can't draw. */
export function deckOf(id: number | undefined, items: ReadonlyMap<number, StoreItem>): Deck {
  const icon = id === undefined ? undefined : items.get(id)?.icon;
  const style = icon?.startsWith('deck-') ? icon.slice(5) : undefined;
  return (DECKS as readonly string[]).includes(style ?? '') ? (style as Deck) : 'classic';
}
