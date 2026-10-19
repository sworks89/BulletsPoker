import type { CSSProperties, ReactNode } from 'react';
import { INK } from '../art/avatars';

// What a throwable leaves on the avatar it hits. Each mark draws in the
// avatar's 120×120 box and is clipped to its circle by .avatar-art.

export type MarkKind = 'tomato' | 'pie' | 'snow' | 'fish' | 'kiss' | 'money' | 'bomb';

const o = { stroke: INK, strokeWidth: 3, strokeLinejoin: 'round', strokeLinecap: 'round' } as const;

function Layer({ kind, children }: { kind: MarkKind; children: ReactNode }) {
  return (
    <svg className={`mark mark-${kind}`} viewBox="0 0 120 120" aria-hidden="true">
      {children}
    </svg>
  );
}

const splat = 'M44 34 q6 -14 18 -6 q10 -12 18 2 q14 0 8 14 q12 8 -2 16 q4 14 -12 10 q-6 12 -18 2 q-14 6 -14 -8 q-14 -4 -4 -16 q-8 -12 6 -14 z';

function Tomato() {
  return (
    <Layer kind="tomato">
      <path d={splat} fill="#dc2626" {...o} />
      <path d="M52 44 q6 -6 12 0 q8 2 4 10 q-4 8 -12 4 q-10 -2 -4 -14z" fill="#991b1b" opacity=".6" />
      <path d="M54 38 q4 -3 8 0" stroke="#fff" strokeWidth="3" fill="none" strokeLinecap="round" opacity=".7" />
      <g fill="#fde68a" stroke={INK} strokeWidth="1">
        <ellipse cx="66" cy="52" rx="2.2" ry="3.2" />
        <ellipse cx="54" cy="58" rx="2.2" ry="3.2" />
        <ellipse cx="72" cy="62" rx="2.2" ry="3.2" />
      </g>
      <rect className="drip" x="48" y="64" width="7" height="26" rx="3.5" fill="#dc2626" stroke={INK} strokeWidth="2" />
      <rect className="drip d2" x="66" y="66" width="6" height="34" rx="3" fill="#dc2626" stroke={INK} strokeWidth="2" />
      <rect className="drip d3" x="80" y="58" width="6" height="20" rx="3" fill="#b91c1c" stroke={INK} strokeWidth="2" />
    </Layer>
  );
}

function Pie() {
  return (
    <Layer kind="pie">
      <g className="slide">
        <path d="M22 44 q4 -16 20 -12 q8 -14 22 -6 q14 -8 22 6 q16 -2 14 14 q10 10 -2 20 q2 14 -14 12 q-8 12 -22 4 q-14 8 -22 -4 q-16 0 -14 -14 q-12 -10 -4 -20z" fill="#fff" {...o} />
        <path d="M40 56 q10 6 20 0 q10 -6 20 0" stroke="#e2e8f0" strokeWidth="4" fill="none" strokeLinecap="round" />
        <circle cx="62" cy="40" r="6" fill="#dc2626" {...o} strokeWidth={2} />
        <path d="M30 80 l10 6 l12 -4 l12 6 l12 -4 l10 4" fill="none" stroke="#d97706" strokeWidth="5" strokeLinecap="round" />
        <rect className="drip" x="36" y="80" width="8" height="22" rx="4" fill="#fff" stroke={INK} strokeWidth="2" />
        <rect className="drip d2" x="74" y="82" width="7" height="28" rx="3.5" fill="#fff" stroke={INK} strokeWidth="2" />
      </g>
    </Layer>
  );
}

function Snow() {
  return (
    <Layer kind="snow">
      <path d="M30 40 q10 -14 26 -6 q14 -12 28 2 q14 2 8 18 q-14 2 -20 -6 q-12 10 -26 2 q-14 4 -16 -10z" fill="#fff" {...o} strokeWidth={2.5} />
      <path d="M36 82 q8 -6 14 2 q-4 8 -14 -2z M74 84 q8 -8 14 0 q-6 8 -14 0z" fill="#fff" {...o} strokeWidth={2} />
      <g stroke="#bae6fd" strokeWidth="2.5" strokeLinecap="round">
        <path d="M16 60 h12 M22 54 v12 M18 56 l8 8 M26 56 l-8 8" />
        <path d="M94 66 h12 M100 60 v12 M96 62 l8 8 M104 62 l-8 8" />
      </g>
      <rect width="120" height="120" fill="#bae6fd" opacity=".22" />
    </Layer>
  );
}

function Fish() {
  return (
    <Layer kind="fish">
      <path d="M18 30 q40 20 84 64" stroke="#38bdf8" strokeWidth="16" fill="none" strokeLinecap="round" opacity=".45" />
      <g className="stars" fill="#facc15" stroke={INK} strokeWidth="1.5">
        <path d="M30 22 l3 6 l7 1 l-5 5 l1 7 l-6 -3 l-6 3 l1 -7 l-5 -5 l7 -1z" />
        <path d="M84 16 l2 4 l5 1 l-4 3 l1 5 l-4 -2 l-4 2 l1 -5 l-4 -3 l5 -1z" />
      </g>
    </Layer>
  );
}

function Kiss() {
  return (
    <Layer kind="kiss">
      <g transform="rotate(-18 82 74)">
        <path d="M66 74 q8 -12 16 -8 q8 -4 16 8 q-16 4 -32 0z" fill="#e11d48" opacity=".9" />
        <path d="M66 74 q16 14 32 0 q-16 4 -32 0z" fill="#be123c" opacity=".9" />
      </g>
      <ellipse cx="36" cy="76" rx="9" ry="5" fill="#fb7185" opacity=".55" />
      <ellipse cx="86" cy="62" rx="9" ry="5" fill="#fb7185" opacity=".4" />
    </Layer>
  );
}

function Money() {
  const bill = (x: number, r: number, cls: string) => (
    <g className={`bill ${cls}`} transform={`translate(${x} 0) rotate(${r} 14 8)`}>
      <rect width="28" height="16" rx="2" fill="#4ade80" {...o} strokeWidth={2} />
      <circle cx="14" cy="8" r="4" fill="#dcfce7" stroke={INK} strokeWidth="1.2" />
    </g>
  );
  return (
    <Layer kind="money">
      {bill(14, -14, 'b1')}
      {bill(46, 10, 'b2')}
      {bill(78, -6, 'b3')}
      {bill(30, 18, 'b4')}
      {bill(64, -20, 'b5')}
    </Layer>
  );
}

function Bomb() {
  return (
    <Layer kind="bomb">
      <g className="blast">
        <path d="M60 14 l10 22 l24 -10 l-8 24 l24 8 l-24 10 l12 22 l-24 -6 l-6 24 l-12 -20 l-18 16 l2 -24 l-24 -2 l18 -16 l-16 -18 l24 2z" fill="#fb923c" {...o} />
        <path d="M60 34 l6 14 l14 -4 l-6 14 l12 8 l-14 4 l2 14 l-12 -8 l-10 10 l-2 -14 l-14 -2 l10 -10 l-8 -12 l14 2z" fill="#fde047" />
      </g>
      <g className="soot">
        <path d="M26 46 q10 -6 18 2 q-6 10 -18 -2z M74 44 q12 -8 20 2 q-8 10 -20 -2z M48 84 q12 -6 22 2 q-10 8 -22 -2z" fill="#1f2937" opacity=".75" />
        <path d="M30 22 l6 10 l4 -12 l6 12 l6 -14 l6 14 l6 -12 l4 12 l6 -10 l2 12" fill="none" stroke="#1f2937" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" />
        <rect width="120" height="120" fill="#111827" opacity=".18" />
      </g>
    </Layer>
  );
}

const MARKS: Record<MarkKind, () => ReactNode> = { tomato: Tomato, pie: Pie, snow: Snow, fish: Fish, kiss: Kiss, money: Money, bomb: Bomb };

export function Mark({ kind }: { kind: MarkKind }) {
  const M = MARKS[kind];
  return <M />;
}

/** Bits that fly out on impact, around (not clipped to) the avatar. */
export function Burst({ kind }: { kind: MarkKind | 'plain' | 'sparkle' }) {
  const colors: Record<string, string[]> = {
    tomato: ['#dc2626', '#dc2626', '#fde68a'],
    pie: ['#fff', '#fff', '#d97706'],
    snow: ['#fff', '#e0f2fe', '#fff'],
    fish: ['#7dd3fc', '#38bdf8', '#e0f2fe'],
    kiss: ['#fb7185', '#e11d48', '#fda4af'],
    money: ['#4ade80', '#facc15', '#4ade80'],
    bomb: ['#fb923c', '#fde047', '#1f2937'],
    plain: ['#e6ebf3', '#e6ebf3', '#e6ebf3'],
    sparkle: ['#fde047', '#fff', '#facc15'],
  };
  // Out to the sides and down, never up over the hole cards.
  const dirs = [
    [-38, -8],
    [38, -6],
    [-44, 10],
    [44, 12],
    [-30, 24],
    [30, 26],
    [-48, 0],
    [48, 2],
  ];
  return (
    <span className={`burst burst-${kind}`} aria-hidden="true">
      {dirs.map(([dx, dy], i) => (
        <i key={i} style={{ '--dx': `${dx}px`, '--dy': `${dy}px`, background: colors[kind][i % 3] } as CSSProperties}>
          {kind === 'kiss' && i % 2 === 0 ? '♥' : null}
        </i>
      ))}
    </span>
  );
}
