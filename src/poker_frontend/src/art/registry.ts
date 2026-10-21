import type { ReactElement } from 'react';

// Art for store items, keyed by the item's emoji (the only thing the backend
// sends). Items without art, such as ones an admin adds later, keep their emoji.

export type ArtKind = 'avatar' | 'gift' | 'throwable' | 'tier';
export type Expression = 'normal' | 'hit' | 'sad' | 'happy';

/** Every art component draws in a 120×120 viewBox and prefixes its svg ids with `uid`. */
export interface ArtProps {
  uid: string;
}

export interface ArtEntry {
  kind: ArtKind;
  name: string;
  Svg: (p: ArtProps) => ReactElement;
}

const art = new Map<string, ArtEntry>();

export function registerArt(icon: string, entry: ArtEntry): void {
  art.set(icon, entry);
}

export function artFor(icon: string): ArtEntry | undefined {
  return art.get(icon);
}
