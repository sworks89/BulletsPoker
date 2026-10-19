import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import type { Expression } from '../art/registry';
import type { MarkKind } from './marks';

export type { MarkKind } from './marks';

/** What each default throwable does to the avatar it hits. */
export const THROW_FX: Record<string, { mark: MarkKind; expression: Expression; motion: string }> = {
  '🍅': { mark: 'tomato', expression: 'hit', motion: 'fx-shake' },
  '🥧': { mark: 'pie', expression: 'hit', motion: 'fx-shake' },
  '❄️': { mark: 'snow', expression: 'hit', motion: 'fx-shiver' },
  '🐟': { mark: 'fish', expression: 'hit', motion: 'fx-knock' },
  '💋': { mark: 'kiss', expression: 'hit', motion: 'fx-wobble' },
  '💸': { mark: 'money', expression: 'happy', motion: 'fx-bounce' },
  '💣': { mark: 'bomb', expression: 'hit', motion: 'fx-stun' },
};

/** A running effect on one seat's avatar. */
export interface SeatEffect {
  expression?: Expression;
  /** Class on the avatar for a whole-avatar motion (shake, bounce…). */
  motion?: string;
  /** Drawn inside the avatar circle (marks). */
  overlay?: ReactNode;
  /** Drawn around the avatar, unclipped (bursts, cloud, tags). */
  around?: ReactNode;
  /** Changes on every play, so the avatar remounts and its animations restart. */
  key: number;
  /** Set for the last 700ms, while the mark wipes off. */
  cleaning?: boolean;
}

const CLEAN_MS = 700;

/** Per-seat effects: `play` replaces whatever is running on that seat. */
export function useSeatFx(): { fx: Map<number, SeatEffect>; play(seat: number, effect: Omit<SeatEffect, 'key' | 'cleaning'>, ms: number): void } {
  const [fx, setFx] = useState<Map<number, SeatEffect>>(new Map());
  const timers = useRef(new Map<number, number[]>());
  const counter = useRef(0);

  useEffect(() => {
    const all = timers.current;
    return () => all.forEach((ts) => ts.forEach(clearTimeout));
  }, []);

  const play = useCallback((seat: number, effect: Omit<SeatEffect, 'key' | 'cleaning'>, ms: number) => {
    timers.current.get(seat)?.forEach(clearTimeout);
    const key = ++counter.current;
    const update = (f: (m: Map<number, SeatEffect>) => void) =>
      setFx((prev) => {
        const next = new Map(prev);
        f(next);
        return next;
      });
    update((m) => m.set(seat, { ...effect, key }));
    const clean = window.setTimeout(
      () => update((m) => m.get(seat)?.key === key && m.set(seat, { ...m.get(seat)!, cleaning: true })),
      Math.max(0, ms - CLEAN_MS),
    );
    const end = window.setTimeout(() => update((m) => m.get(seat)?.key === key && m.delete(seat)), ms);
    timers.current.set(seat, [clean, end]);
  }, []);

  return { fx, play };
}
