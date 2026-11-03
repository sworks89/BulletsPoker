import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react';
import type { TableView } from '../ic';
import { dealOrder, motionBetween, type MotionSnap } from './motion';

/** How long after a hand ends (as this browser saw it) the deck is shuffled.
 *  The table waits at least 5s between hands and the riffle takes about 1.1s. */
export const SHUFFLE_AFTER_MS = 3000;

const DEAL_MS = 420;
const DEAL_STAGGER_MS = 70;
const BOARD_STAGGER_MS = 80;
const REVEAL_MS = 260;

const reducedMotion = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

export function snapOf(view: TableView): MotionSnap {
  const seats = view.seats.flatMap((s) => (s[0] ? [s[0]] : []));
  return {
    hand: view.hand_no,
    board: view.board.length,
    dealt: seats.filter((s) => s.has_cards || s.cards.length > 0).map((s) => s.seat),
    faceUp: seats.filter((s) => s.cards.length > 0).map((s) => s.seat),
  };
}

const center = (el: Element) => {
  const r = el.getBoundingClientRect();
  return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
};

/** Slides a card in from `from`; face-up cards land face down and turn over. */
function slideIn(card: HTMLElement, from: { x: number; y: number }, delay: number, flip: boolean) {
  const at = center(card);
  const start = { transform: `translate(${from.x - at.x}px, ${from.y - at.y}px) scale(0.7)`, opacity: 0 };
  if (!flip) {
    card.animate([start, { transform: 'none', opacity: 1 }], { duration: DEAL_MS * 0.55, delay, easing: 'cubic-bezier(.2,.7,.3,1)', fill: 'backwards' });
    return;
  }
  card.animate([start, { offset: 0.55, transform: 'none', opacity: 1 }, { offset: 0.78, transform: 'scaleX(0)' }, { transform: 'none' }], {
    duration: DEAL_MS,
    delay,
    easing: 'ease-out',
    fill: 'backwards',
  });
  cover(card, 0.78, DEAL_MS, delay);
}

/** Turns a card face up where it lies. */
function turnOver(card: HTMLElement, delay: number) {
  card.animate([{ transform: 'none' }, { offset: 0.5, transform: 'scaleX(0)' }, { transform: 'none' }], { duration: REVEAL_MS, delay, easing: 'ease-in-out', fill: 'backwards' });
  cover(card, 0.5, REVEAL_MS, delay);
}

/** The card's back shows until the flip's midpoint. */
function cover(card: HTMLElement, at: number, duration: number, delay: number) {
  card.querySelector('.cover')?.animate(
    [{ opacity: 1 }, { offset: at, opacity: 1 }, { offset: at, opacity: 0 }, { opacity: 0 }],
    { duration, delay, fill: 'backwards' },
  );
}

/**
 * Animates cards on the table under `root` as the view changes: hole cards dealt
 * from the deck, board cards sliding out and turning, shown hands turning over.
 * Cards are found by their `data-fx` names (`board`, `board-<i>`, `hole-<seat>-<k>`).
 */
export function useCardMotion(root: RefObject<HTMLElement | null>, view: TableView | null, visible: boolean) {
  const prev = useRef<MotionSnap | null>(null);
  useLayoutEffect(() => {
    if (!view) return;
    const next = snapOf(view);
    const before = prev.current;
    prev.current = next;
    const el = root.current;
    if (!el || !visible || reducedMotion()) return;
    const m = motionBetween(before, next);
    if (!m.deal.length && !m.board.length && !m.reveal.length) return;
    const find = (name: string) => el.querySelector<HTMLElement>(`[data-fx="${name}"]`);
    const board = find('board');
    if (!board) return;
    const deck = center(board);
    dealOrder(m.deal, view.button, view.seats.length).forEach(([seat, k], i) => {
      const card = find(`hole-${seat}-${k}`);
      if (card) slideIn(card, deck, i * DEAL_STAGGER_MS, next.faceUp.includes(seat));
    });
    // A board that arrives with the deal (a missed poll) comes out after it.
    const afterDeal = m.deal.length * 2 * DEAL_STAGGER_MS;
    m.board.forEach((b, i) => {
      const card = find(`board-${b}`);
      if (card) slideIn(card, deck, afterDeal + i * BOARD_STAGGER_MS, true);
    });
    m.reveal.forEach((seat, i) => {
      for (const k of [0, 1]) {
        const card = find(`hole-${seat}-${k}`);
        if (card) turnOver(card, (i * 2 + k) * 60);
      }
    });
    // view drives this; root is a stable ref
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, visible]);
}

/**
 * Whether to show the deck being shuffled: from SHUFFLE_AFTER_MS after a hand
 * ends (one this browser watched end, not one already over when the table
 * opened) until the next hand is dealt.
 */
export function useShuffle(view: TableView | null, visible: boolean): boolean {
  const [on, setOn] = useState(false);
  const seen = useRef<{ hand: bigint; over: boolean } | null>(null);
  const ended = useRef<{ hand: bigint; at: number } | null>(null);
  const hand = view?.hand_no;
  const over = !!view && 'HandOver' in view.phase;
  useEffect(() => {
    if (hand === undefined) return;
    const was = seen.current;
    seen.current = { hand, over };
    if (!over) {
      setOn(false);
      return;
    }
    if (was && !(was.over && was.hand === hand)) ended.current = { hand, at: Date.now() };
    const e = ended.current;
    if (!e || e.hand !== hand || !visible || reducedMotion()) return;
    const t = setTimeout(() => setOn(true), Math.max(0, e.at + SHUFFLE_AFTER_MS - Date.now()));
    return () => clearTimeout(t);
  }, [hand, over, visible]);
  // `on` is cleared after the render that brings the next hand; checking `over`
  // too drops the deck in that same render, so it never overlaps the deal.
  return on && over;
}
