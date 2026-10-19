import { useCallback, useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';
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

/** A little rain cloud over a player who just took a bad beat. */
export function RainCloud() {
  return (
    <span className="rain-cloud" aria-hidden="true">
      <svg viewBox="0 0 60 40">
        <path d="M14 26 q-10 0 -10 -8 q0 -9 10 -8 q2 -9 12 -8 q6 -8 16 -2 q12 -2 12 9 q8 1 6 9 q-1 8 -10 8z" fill="#94a3b8" stroke="#1b1b1b" strokeWidth="2.5" strokeLinejoin="round" />
      </svg>
      {[0, 1, 2, 3].map((k) => (
        <i key={k} style={{ left: `${30 + k * 14}%`, animationDelay: `${k * 0.18}s` }} />
      ))}
    </span>
  );
}

export function BadBeatTag() {
  return <span className="bad-beat-tag">Bad beat</span>;
}

/** Where each chip of a burst flies: out to both sides and slightly down, never up over the hole cards. */
export function chipBurstVectors(big: boolean): { dx: number; dy: number }[] {
  const n = big ? 18 : 8;
  return Array.from({ length: n }, (_, k) => {
    const dir = k % 2 === 0 ? 1 : -1;
    const spread = (k >> 1) / (n / 2); // 0 .. <1
    const dist = (big ? 60 : 40) + (k % 3) * 8;
    const angle = spread * 0.5; // 0 (level) .. ~0.5 rad below level
    return { dx: Math.round(dir * Math.cos(angle) * dist), dy: Math.round(Math.sin(angle) * dist) };
  });
}

/** Chips (and confetti, for a big win) bursting out of a winner. */
export function ChipBurst({ big }: { big: boolean }) {
  const colors = ['#facc15', '#ef4444', '#22c55e', '#3b82f6', '#a855f7', '#f97316'];
  return (
    <span className={`chip-burst ${big ? 'big' : ''}`} aria-hidden="true">
      {chipBurstVectors(big).map(({ dx, dy }, k) => (
        <i
          key={k}
          className={big && k % 4 >= 2 ? 'confetti' : 'chip'}
          style={
            {
              '--dx': `${dx}px`,
              '--dy': `${dy}px`,
              '--c': colors[k % colors.length],
              animationDelay: `${(k % 4) * 0.05}s`,
            } as CSSProperties
          }
        />
      ))}
    </span>
  );
}
