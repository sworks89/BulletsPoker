import { useEffect, useLayoutEffect, useRef, type RefObject } from 'react';
import type { TableView } from '../ic';
import { betSpot, chipColumns, chipMotionBetween, type ChipSnap } from './chips';
import { chipNode, pileNode } from './ChipPile';

const TOSS_MS = 320;
const TOSS_STAGGER_MS = 50;
/** Between seats tossing at once (the blinds, or a missed poll). */
const TOSS_SEAT_MS = 150;
const TOSS_MAX_CHIPS = 6;
const SWEEP_MS = 420;
const SWEEP_STAGGER_MS = 70;
const PUSH_MS = 650;

const reducedMotion = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

export function chipSnapOf(view: TableView): ChipSnap {
  const seats = view.seats.flatMap((s) => (s[0] ? [s[0]] : []));
  const bets = new Map(seats.filter((s) => s.bet > 0n).map((s) => [s.seat, s.bet]));
  const result = 'HandOver' in view.phase ? view.last_result[0] : undefined;
  const betting = [...bets.values()].reduce((a, b) => a + b, 0n);
  return {
    hand: view.hand_no,
    bets,
    // While betting the view's pot includes this street's bets.
    collected: view.pot > betting ? view.pot - betting : 0n,
    over: !!result,
    winners: result?.winners.map((w) => ({ seat: w.seat, amount: w.amount })) ?? [],
  };
}

type Spot = { left: number; top: number };

/**
 * Moves chips on the table under `root` as the view changes: bets tossed from
 * the seat, swept into the pot when a street ends, and the pot pushed to the
 * winners. The piles themselves are rendered by the table (bets as `bet-<seat>`,
 * the pot as `pot`); this adds chips in flight over them.
 */
export function useChipMotion(
  root: RefObject<HTMLElement | null>,
  view: TableView | null,
  visible: boolean,
  seatSpot: (seat: number) => Spot,
  narrow: boolean,
) {
  const prev = useRef<ChipSnap | null>(null);
  const flying = useRef(new Set<HTMLElement>());

  useEffect(() => {
    const live = flying.current;
    return () => live.forEach((el) => el.remove());
  }, []);

  useLayoutEffect(() => {
    if (!view) return;
    const next = chipSnapOf(view);
    const before = prev.current;
    prev.current = next;
    const el = root.current;
    if (!el || !visible || reducedMotion()) return;
    const m = chipMotionBetween(before, next);
    if (!m.toss.length && !m.sweep.length && !m.push.length) return;

    const bb = view.config.big_blind;
    const box = el.getBoundingClientRect();
    const px = (s: Spot & { dy?: number }) => ({ x: (box.width * s.left) / 100, y: (box.height * s.top) / 100 + (s.dy ?? 0) });
    const betAt = (seat: number) => betSpot(seatSpot(seat), narrow);
    const potEl = el.querySelector<HTMLElement>('[data-fx="pot"]');
    const pot = potEl
      ? (() => {
          const r = potEl.getBoundingClientRect();
          return { x: r.left - box.left + r.width / 2, y: r.top - box.top + r.height / 2 };
        })()
      : px({ left: 50, top: 58 });
    const fly = (node: HTMLElement, at: { x: number; y: number }) => {
      node.classList.add('tc-flying');
      node.style.left = `${at.x}px`;
      node.style.top = `${at.y}px`;
      el.appendChild(node);
      flying.current.add(node);
      return node;
    };
    const land = (node: HTMLElement) => (a: Animation) =>
      a.finished.then(
        () => {
          node.remove();
          flying.current.delete(node);
        },
        () => {},
      );

    // Bets go into the pot first; new bets and the payout follow.
    const sweepMs = m.sweep.length ? SWEEP_MS + (m.sweep.length - 1) * SWEEP_STAGGER_MS : 0;
    m.sweep.forEach(({ seat, amount }, k) => {
      const from = px(betAt(seat));
      const node = fly(pileNode(amount, bb), from);
      land(node)(
        node.animate([{ transform: 'none' }, { transform: `translate(${pot.x - from.x}px, ${pot.y - from.y}px) scale(0.85)` }], {
          duration: SWEEP_MS,
          delay: k * SWEEP_STAGGER_MS,
          easing: 'cubic-bezier(.4,0,.2,1)',
          fill: 'forwards',
        }),
      );
    });
    if (m.sweep.length && potEl) {
      potEl.animate([{ transform: 'scale(1.12)' }, { transform: 'none' }], { duration: 180, delay: sweepMs, easing: 'ease-out' });
    }

    m.toss.forEach(({ seat, amount, onto }, j) => {
      const from = px(seatSpot(seat));
      const to = px(betAt(seat));
      const chips = chipColumns(amount, bb)
        .flatMap((c) => Array.from({ length: c.n }, () => c.tier))
        .slice(0, TOSS_MAX_CHIPS);
      const start = sweepMs + j * TOSS_SEAT_MS;
      chips.forEach((tier, k) => {
        const node = fly(document.createElement('div'), to);
        node.appendChild(chipNode(tier));
        const spin = (k % 2 ? 1 : -1) * 40;
        land(node)(
          node.animate(
            [{ transform: `translate(${from.x - to.x}px, ${from.y - to.y}px) rotate(${spin}deg) scale(0.9)`, opacity: 1 }, { transform: 'none', opacity: 1 }],
            { duration: TOSS_MS, delay: start + k * TOSS_STAGGER_MS, easing: 'cubic-bezier(.2,.7,.3,1)', fill: 'both' },
          ),
        );
      });
      // A new bet appears as its last chip lands.
      const pile = el.querySelector<HTMLElement>(`[data-fx="bet-${seat}"]`);
      if (pile && !onto) {
        pile.animate([{ opacity: 0 }, { opacity: 0 }], { duration: start + (chips.length - 1) * TOSS_STAGGER_MS + TOSS_MS });
      }
    });

    m.push.forEach(({ seat, amount }) => {
      const to = px(seatSpot(seat));
      const node = fly(pileNode(amount, bb), pot);
      const end = `translate(${to.x - pot.x}px, ${to.y - pot.y}px) scale(0.75)`;
      land(node)(
        node.animate([{ transform: 'none', opacity: 1 }, { transform: end, opacity: 1, offset: 0.85 }, { transform: end, opacity: 0 }], {
          duration: PUSH_MS,
          delay: sweepMs,
          easing: 'cubic-bezier(.4,0,.2,1)',
          fill: 'both',
        }),
      );
    });
    // view drives this; root, seatSpot and narrow are read at the change
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, visible]);
}
