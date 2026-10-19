import type { ReactNode } from 'react';
import { INK, ol, Radial } from './avatars';
import { registerArt, type ArtProps } from './registry';

// Gifts and throwables in the same outlined style as the avatars. Gifts have an
// .idle group (foam, steam, bubbles…) that CSS animates while the gift sits at a seat.

function Item({ children, defs }: { children: ReactNode; defs?: ReactNode }) {
  return (
    <svg className="art item-svg" viewBox="0 0 120 120" aria-hidden="true">
      {defs && <defs>{defs}</defs>}
      {children}
    </svg>
  );
}

const thin = { ...ol, strokeWidth: 2.5 } as const;

function Beer({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-ale`} a="#fde047" b="#d97706" />}>
      <path d="M84 48 h10 q14 0 14 18 q0 18 -14 18 h-10" fill="none" {...ol} strokeWidth={6} stroke={INK} />
      <path d="M84 50 h10 q10 0 10 16 q0 14 -10 14 h-10" fill="none" stroke="#e2e8f0" strokeWidth={4} />
      <rect x="26" y="34" width="60" height="70" rx="8" fill={`url(#${uid}-ale)`} {...ol} />
      <path d="M38 50 v40 M52 50 v40 M66 50 v40" stroke="#fff" strokeWidth="3" opacity=".35" strokeLinecap="round" />
      <g className="idle">
        <path d="M22 40 q0 -14 14 -12 q4 -12 18 -8 q8 -10 20 -2 q14 -4 16 10 q6 2 4 12 q-36 6 -72 0z" fill="#fff" {...ol} />
        <circle cx="48" cy="26" r="2.5" fill="#fff" stroke={INK} strokeWidth="1.5" />
      </g>
    </Item>
  );
}

function Coffee({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-cup`} a="#ffffff" b="#cbd5e1" />}>
      <ellipse cx="60" cy="100" rx="44" ry="10" fill="#e2e8f0" {...ol} />
      <path d="M84 62 q18 0 16 14 q-2 12 -18 10" fill="none" {...ol} strokeWidth={5} />
      <path d="M28 54 h60 l-6 38 q-24 10 -48 0z" fill={`url(#${uid}-cup)`} {...ol} />
      <ellipse cx="58" cy="54" rx="30" ry="7" fill="#78350f" {...ol} />
      <g className="idle" fill="none" stroke="#e2e8f0" strokeWidth="4" strokeLinecap="round" opacity=".85">
        <path d="M46 42 q-6 -8 0 -14 q6 -6 0 -14" />
        <path d="M62 42 q-6 -8 0 -14 q6 -6 0 -14" />
      </g>
    </Item>
  );
}

function Rose({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-petal`} a="#fb7185" b="#be123c" />}>
      <g className="idle">
        <path d="M60 58 q4 30 -2 56" fill="none" {...ol} stroke="#15803d" strokeWidth={5} />
        <path d="M60 84 q16 -14 28 -6 q-12 14 -28 6z" fill="#22c55e" {...thin} />
        <path d="M58 96 q-16 -12 -26 -2 q12 12 26 2z" fill="#22c55e" {...thin} />
        <path d="M36 36 q-2 -22 24 -24 q26 2 24 24 q0 24 -24 26 q-24 -2 -24 -26z" fill={`url(#${uid}-petal)`} {...ol} />
        <path d="M48 32 q12 -10 24 0 q-2 12 -12 12 q-8 0 -8 -8 q4 -4 8 -2" fill="none" stroke={INK} strokeWidth="2.5" strokeLinecap="round" />
      </g>
    </Item>
  );
}

function Cocktail({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-drink`} a="#a5f3fc" b="#0891b2" />}>
      <path d="M60 70 v34 M40 106 h40" fill="none" {...ol} strokeWidth={8} />
      <path d="M60 70 v34 M40 106 h40" fill="none" stroke="#e2e8f0" strokeWidth={3.5} strokeLinecap="round" />
      <path d="M18 24 h84 l-42 46z" fill="#f8fafc" {...ol} />
      <path d="M28 32 h64 l-32 34z" fill={`url(#${uid}-drink)`} />
      <path d="M50 14 l26 30" {...thin} />
      <circle cx="70" cy="36" r="7" fill="#65a30d" {...thin} />
      <circle cx="72" cy="35" r="2.5" fill="#dc2626" />
      <g className="idle">
        <path d="M30 28 l10 10" stroke="#fff" strokeWidth="4" strokeLinecap="round" opacity=".9" />
      </g>
    </Item>
  );
}

function Pizza({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-cheese`} a="#fef08a" b="#f59e0b" />}>
      <path d="M16 36 q44 -22 88 0 L60 110z" fill={`url(#${uid}-cheese)`} {...ol} />
      <path d="M14 34 q46 -26 92 0 l-4 8 q-42 -22 -84 0z" fill="#c2410c" {...ol} />
      <circle cx="46" cy="52" r="7" fill="#dc2626" {...thin} />
      <circle cx="70" cy="56" r="6" fill="#dc2626" {...thin} />
      <circle cx="58" cy="78" r="6" fill="#dc2626" {...thin} />
      <path d="M38 62 q2 10 -2 14" fill="none" stroke="#f59e0b" strokeWidth="4" strokeLinecap="round" />
      <g className="idle" fill="none" stroke="#e2e8f0" strokeWidth="3.5" strokeLinecap="round" opacity=".8">
        <path d="M50 22 q-5 -6 0 -12" />
        <path d="M70 22 q-5 -6 0 -12" />
      </g>
    </Item>
  );
}

function Cigar({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-leaf`} a="#a16207" b="#57300b" />}>
      <g transform="rotate(-20 60 78)">
        <rect x="14" y="70" width="88" height="18" rx="9" fill={`url(#${uid}-leaf)`} {...ol} />
        <rect x="70" y="70" width="12" height="18" fill="#dc2626" {...thin} />
        <rect x="74" y="73" width="4" height="12" fill="#facc15" />
        <path d="M14 79 a9 9 0 0 1 9 -9 v18 a9 9 0 0 1 -9 -9z" fill="#f97316" {...thin} />
        <path d="M16 76 l4 2 l-3 3" stroke="#fde047" strokeWidth="2" fill="none" />
      </g>
      <g className="idle" fill="none" stroke="#cbd5e1" strokeWidth="4" strokeLinecap="round" opacity=".8">
        <path d="M22 76 q-8 -10 0 -20 q8 -10 0 -22" />
      </g>
    </Item>
  );
}

function Champagne({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-glass`} a="#4ade80" b="#14532d" />}>
      <path d="M48 32 h24 v14 q16 10 16 30 v32 q0 6 -6 6 h-44 q-6 0 -6 -6 v-32 q0 -20 16 -30z" fill={`url(#${uid}-glass)`} {...ol} />
      <path d="M46 20 h28 v18 h-28z" fill="#facc15" {...ol} />
      <path d="M34 72 h52 v24 h-52z" fill="#fef3c7" {...thin} />
      <path d="M48 84 h24" stroke={INK} strokeWidth="3" strokeLinecap="round" />
      <path d="M42 54 v16" stroke="#fff" strokeWidth="4" strokeLinecap="round" opacity=".45" />
      <g className="idle" fill="#fef9c3" stroke={INK} strokeWidth="1.5">
        <circle cx="54" cy="12" r="3" />
        <circle cx="66" cy="8" r="2.5" />
        <circle cx="60" cy="2" r="2" />
      </g>
    </Item>
  );
}

function Trophy({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-gold`} a="#fef08a" b="#ca8a04" />}>
      <path d="M30 26 q-20 0 -16 18 q4 14 22 16 M90 26 q20 0 16 18 q-4 14 -22 16" fill="none" {...ol} strokeWidth={5} />
      <path d="M30 18 h60 v18 q0 32 -30 36 q-30 -4 -30 -36z" fill={`url(#${uid}-gold)`} {...ol} />
      <path d="M54 72 h12 v16 h-12z" fill={`url(#${uid}-gold)`} {...ol} />
      <path d="M36 88 h48 v18 h-48z" fill="#78350f" {...ol} />
      <path d="M48 94 h24" stroke="#facc15" strokeWidth="4" />
      <path d="M60 30 l4 8 l9 1 l-7 6 l2 9 l-8 -5 l-8 5 l2 -9 l-7 -6 l9 -1z" fill="#fff" opacity=".85" />
      <g className="idle">
        <path d="M40 24 l8 0 l-12 40 l-6 -2z" fill="#fff" opacity=".55" />
      </g>
    </Item>
  );
}

function Tomato({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-red`} a="#f87171" b="#b91c1c" />}>
      <circle cx="60" cy="66" r="44" fill={`url(#${uid}-red)`} {...ol} />
      <path d="M38 48 q8 -10 18 -10" stroke="#fff" strokeWidth="6" fill="none" strokeLinecap="round" opacity=".7" />
      <path d="M60 22 l-20 12 l20 2 l20 -2z" fill="#22c55e" {...ol} />
      <path d="M60 24 v-14" {...ol} stroke="#166534" strokeWidth={5} />
    </Item>
  );
}

function Pie({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-cream`} a="#ffffff" b="#e2e8f0" />}>
      <path d="M10 64 h100 l-10 32 h-80z" fill="#d97706" {...ol} />
      <path d="M20 72 v18 M36 72 v20 M52 72 v20 M68 72 v20 M84 72 v20" stroke="#92400e" strokeWidth="3" />
      <path d="M8 66 q0 -14 14 -14 q4 -16 22 -12 q8 -14 24 -6 q16 -8 24 8 q20 0 18 24z" fill={`url(#${uid}-cream)`} {...ol} />
      <circle cx="60" cy="36" r="7" fill="#dc2626" {...thin} />
    </Item>
  );
}

function Snowball({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-snow`} a="#ffffff" b="#93c5fd" />}>
      <circle cx="60" cy="60" r="44" fill={`url(#${uid}-snow)`} {...ol} />
      <path d="M32 50 q4 -6 10 -6 M70 82 q8 -2 12 -8 M50 74 l4 2" stroke="#bfdbfe" strokeWidth="4" fill="none" strokeLinecap="round" />
      <path d="M38 36 q8 -8 18 -8" stroke="#fff" strokeWidth="6" fill="none" strokeLinecap="round" />
    </Item>
  );
}

function Fish({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-scale`} a="#7dd3fc" b="#0369a1" />}>
      <path d="M84 60 l26 -22 v44z" fill="#0ea5e9" {...ol} />
      <path d="M10 60 q30 -40 76 0 q-46 40 -76 0z" fill={`url(#${uid}-scale)`} {...ol} />
      <path d="M40 40 q8 20 0 40" fill="none" {...thin} />
      <path d="M52 46 q4 4 0 8 M60 56 q4 4 0 8 M52 66 q4 4 0 8" fill="none" stroke="#bae6fd" strokeWidth="2.5" strokeLinecap="round" />
      <circle cx="26" cy="54" r="5" fill="#fff" {...thin} />
      <circle cx="25" cy="54" r="2.2" fill={INK} />
    </Item>
  );
}

function Kiss({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-lip`} a="#fb7185" b="#be123c" />}>
      <path d="M10 58 q20 -30 36 -22 q14 -8 14 4 q0 -12 14 -4 q16 -8 36 22 q-50 6 -100 0z" fill={`url(#${uid}-lip)`} {...ol} />
      <path d="M10 58 q50 50 100 0 q-50 10 -100 0z" fill={`url(#${uid}-lip)`} {...ol} />
      <path d="M40 70 q10 6 20 6" stroke="#fff" strokeWidth="4" fill="none" strokeLinecap="round" opacity=".6" />
    </Item>
  );
}

function Money({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-bill`} a="#bbf7d0" b="#16a34a" />}>
      <g transform="rotate(-12 60 60)">
        <rect x="12" y="44" width="92" height="46" rx="5" fill="#15803d" {...ol} />
        <rect x="16" y="34" width="92" height="46" rx="5" fill={`url(#${uid}-bill)`} {...ol} />
        <circle cx="62" cy="57" r="14" fill="#dcfce7" {...thin} />
        <text x="62" y="65" textAnchor="middle" fontSize="22" fontWeight="900" fill="#14532d" fontFamily="system-ui, sans-serif">
          $
        </text>
        <path d="M24 42 h10 M90 72 h10" stroke="#14532d" strokeWidth="3" strokeLinecap="round" />
      </g>
    </Item>
  );
}

function Bomb({ uid }: ArtProps) {
  return (
    <Item defs={<Radial id={`${uid}-iron`} a="#64748b" b="#0f172a" />}>
      <circle cx="54" cy="70" r="40" fill={`url(#${uid}-iron)`} {...ol} />
      <rect x="64" y="22" width="18" height="16" rx="3" transform="rotate(35 73 30)" fill="#475569" {...ol} />
      <path d="M80 22 q8 -14 20 -10" fill="none" {...ol} stroke="#a16207" />
      <path d="M100 4 l3 7 l7 -1 l-5 6 l5 6 l-7 -1 l-3 7 l-3 -7 l-7 1 l5 -6 l-5 -6 l7 1z" fill="#facc15" {...thin} />
      <path d="M32 54 q8 -12 20 -14" stroke="#fff" strokeWidth="6" fill="none" strokeLinecap="round" opacity=".45" />
    </Item>
  );
}

const ITEMS: [string, 'gift' | 'throwable', string, (p: ArtProps) => ReturnType<typeof Beer>][] = [
  ['🍺', 'gift', 'Beer', Beer],
  ['☕', 'gift', 'Coffee', Coffee],
  ['🌹', 'gift', 'Rose', Rose],
  ['🍸', 'gift', 'Cocktail', Cocktail],
  ['🍕', 'gift', 'Pizza', Pizza],
  ['🚬', 'gift', 'Cigar', Cigar],
  ['🍾', 'gift', 'Champagne', Champagne],
  ['🏆', 'gift', 'Trophy', Trophy],
  ['🍅', 'throwable', 'Tomato', Tomato],
  ['🥧', 'throwable', 'Pie', Pie],
  ['❄️', 'throwable', 'Snowball', Snowball],
  ['🐟', 'throwable', 'Fish', Fish],
  ['💋', 'throwable', 'Kiss', Kiss],
  ['💸', 'throwable', 'Money', Money],
  ['💣', 'throwable', 'Bomb', Bomb],
];
for (const [icon, kind, name, Svg] of ITEMS) registerArt(icon, { kind, name, Svg });
