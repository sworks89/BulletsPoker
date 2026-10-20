import type { ReactNode } from 'react';
import { registerArt, type ArtProps } from './registry';

// The ten store avatars, drawn in one outlined-cartoon style: ink outlines,
// gradient shading, a round coloured background. Each draws its four faces
// (.face-normal/.face-hit/.face-sad/.face-happy) and CSS shows the one that
// matches the avatar's data-expr. Accessories sit in .acc so a hit can knock them.

export const INK = '#1b1b1b';
export const ol = { stroke: INK, strokeWidth: 3, strokeLinejoin: 'round', strokeLinecap: 'round' } as const;

export function Radial({ id, a, b, cx = 0.4, cy = 0.35 }: { id: string; a: string; b: string; cx?: number; cy?: number }) {
  return (
    <radialGradient id={id} cx={cx} cy={cy} r={0.8}>
      <stop offset="0" stopColor={a} />
      <stop offset="1" stopColor={b} />
    </radialGradient>
  );
}

export function Frame({ uid, bg, children, defs }: { uid: string; bg: [string, string]; children: ReactNode; defs?: ReactNode }) {
  return (
    <svg className="art avatar-svg" viewBox="0 0 120 120" aria-hidden="true">
      <defs>
        <Radial id={`${uid}-bg`} a={bg[0]} b={bg[1]} cx={0.5} />
        {defs}
      </defs>
      <rect width="120" height="120" fill={`url(#${uid}-bg)`} />
      <g className="figure">{children}</g>
    </svg>
  );
}

/** The four expressions, placed by eye and mouth position. */
function Face({
  lx,
  rx,
  y,
  mouthY,
  eye = 5,
  ink = INK,
  mouth = true,
}: {
  lx: number;
  rx: number;
  y: number;
  mouthY: number;
  eye?: number;
  ink?: string;
  mouth?: boolean;
}) {
  const cx = (lx + rx) / 2;
  const line = { stroke: ink, strokeWidth: 3, fill: 'none', strokeLinecap: 'round', strokeLinejoin: 'round' } as const;
  return (
    <>
      <g className="face-normal">
        <g className="eyes">
          {[lx, rx].map((x) => (
            <g key={x}>
              <ellipse cx={x} cy={y} rx={eye * 0.8} ry={eye} fill={ink} />
              <circle cx={x - eye * 0.25} cy={y - eye * 0.4} r={eye * 0.32} fill="#fff" />
            </g>
          ))}
        </g>
        {mouth && <path d={`M${cx - 8} ${mouthY} q8 7 16 0`} {...line} />}
      </g>
      <g className="face-hit">
        <path d={`M${lx - 5} ${y - 4} l8 4 l-8 4 M${rx + 5} ${y - 4} l-8 4 l8 4`} {...line} strokeWidth={3.5} />
        {mouth && <ellipse cx={cx} cy={mouthY + 2} rx={5.5} ry={6.5} fill="#7f1d1d" stroke={ink} strokeWidth={2.5} />}
      </g>
      <g className="face-sad">
        <path d={`M${lx - 7} ${y - 6} l11 -4 M${rx + 7} ${y - 6} l-11 -4`} {...line} />
        <path d={`M${lx - 5} ${y + 1} q5 4 10 0 M${rx - 5} ${y + 1} q5 4 10 0`} {...line} />
        <path className="tear" d={`M${lx - 1} ${y + 5} q-4 6 0 9 q4 -3 0 -9z`} fill="#7dd3fc" stroke={ink} strokeWidth={1.5} />
        {mouth && <path d={`M${cx - 8} ${mouthY + 4} q8 -7 16 0`} {...line} />}
      </g>
      <g className="face-happy">
        <path d={`M${lx - 5} ${y + 2} q5 -8 10 0 M${rx - 5} ${y + 2} q5 -8 10 0`} {...line} strokeWidth={3.5} />
        {mouth && (
          <>
            <path d={`M${cx - 11} ${mouthY - 2} q11 16 22 0z`} fill="#7f1d1d" stroke={ink} strokeWidth={2.5} strokeLinejoin="round" />
            <ellipse cx={cx} cy={mouthY + 4} rx={5} ry={2.5} fill="#fb7185" />
          </>
        )}
      </g>
    </>
  );
}

function Rookie({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#5eead4', '#0f766e']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a="#ffe4c4" b="#f5a97a" />
          <Radial id={`${uid}-hood`} a="#4ade80" b="#15803d" />
        </>
      }
    >
      <path d="M14 122 q4 -32 46 -34 q42 2 46 34z" fill={`url(#${uid}-hood)`} {...ol} />
      <path d="M52 92 l-2 14 M68 92 l2 14" {...ol} fill="none" />
      <circle cx="31" cy="64" r="7" fill={`url(#${uid}-skin)`} {...ol} />
      <circle cx="89" cy="64" r="7" fill={`url(#${uid}-skin)`} {...ol} />
      <ellipse cx="60" cy="62" rx="29" ry="32" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M31 56 q-4 -32 29 -32 q33 0 29 32 q-7 -11 -17 -9 q-6 -9 -14 -2 q-9 -8 -17 1 q-7 -1 -10 10z" fill="#7c2d12" {...ol} />
      <circle cx="42" cy="75" r="5" fill="#fb7185" opacity=".45" />
      <circle cx="78" cy="75" r="5" fill="#fb7185" opacity=".45" />
      <Face lx={49} rx={71} y={63} mouthY={79} />
    </Frame>
  );
}

function CoolCat({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#38bdf8', '#1e3a8a']} defs={<Radial id={`${uid}-fur`} a="#fbbf24" b="#d97706" />}>
      <path d="M18 122 q8 -30 42 -32 q34 2 42 32z" fill="#ef4444" {...ol} />
      <path d="M48 92 q12 8 24 0" fill="none" {...ol} stroke="#fff" />
      <path d="M24 46 L28 8 L52 30 Z" fill={`url(#${uid}-fur)`} {...ol} />
      <path d="M31 38 L32 20 L44 31 Z" fill="#fda4af" />
      <path d="M96 46 L92 8 L68 30 Z" fill={`url(#${uid}-fur)`} {...ol} />
      <path d="M89 38 L88 20 L76 31 Z" fill="#fda4af" />
      <ellipse cx="60" cy="62" rx="40" ry="34" fill={`url(#${uid}-fur)`} {...ol} />
      <path d="M36 40 q6 -6 12 -2 M66 36 q6 -4 12 2" stroke="#b45309" strokeWidth="3" fill="none" strokeLinecap="round" />
      <ellipse cx="60" cy="80" rx="17" ry="12" fill="#fff7ed" stroke={INK} strokeWidth="2.5" />
      <path d="M54 73 h12 l-6 7 z" fill="#db2777" stroke={INK} strokeWidth="2" strokeLinejoin="round" />
      <g stroke={INK} strokeWidth="2" strokeLinecap="round">
        <path d="M42 80 l-17 -4 M42 85 l-17 2 M78 80 l17 -4 M78 85 l17 2" />
      </g>
      <Face lx={42} rx={78} y={57} mouthY={85} />
      <g className="acc">
        <rect x="28" y="48" width="28" height="18" rx="7" fill="#0f172a" {...ol} />
        <rect x="64" y="48" width="28" height="18" rx="7" fill="#0f172a" {...ol} />
        <path d="M56 55 h8" stroke={INK} strokeWidth="4" />
        <path d="M33 53 l9 0 M69 53 l6 0" stroke="#7dd3fc" strokeWidth="3" strokeLinecap="round" />
      </g>
    </Frame>
  );
}

function Robot({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#a78bfa', '#4c1d95']} defs={<Radial id={`${uid}-metal`} a="#e2e8f0" b="#64748b" />}>
      <path d="M16 122 q4 -28 44 -30 q40 2 44 30z" fill={`url(#${uid}-metal)`} {...ol} />
      <circle cx="40" cy="108" r="3" fill={INK} />
      <circle cx="80" cy="108" r="3" fill={INK} />
      <rect x="52" y="84" width="16" height="10" fill="#475569" {...ol} />
      <rect x="18" y="50" width="10" height="20" rx="3" fill="#475569" {...ol} />
      <rect x="92" y="50" width="10" height="20" rx="3" fill="#475569" {...ol} />
      <rect x="26" y="26" width="68" height="62" rx="14" fill={`url(#${uid}-metal)`} {...ol} />
      <rect x="34" y="40" width="52" height="38" rx="9" fill="#0f172a" {...ol} />
      <Face lx={48} rx={72} y={55} mouthY={67} ink="#67e8f9" />
      <g className="acc">
        <path d="M60 26 V12" {...ol} fill="none" />
        <circle cx="60" cy="10" r="5" fill="#ef4444" {...ol} />
      </g>
    </Frame>
  );
}

function Cowboy({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#fdba74', '#c2410c']} defs={<Radial id={`${uid}-skin`} a="#fde3c8" b="#e8a77c" />}>
      <path d="M16 122 q6 -30 44 -32 q38 2 44 32z" fill="#2563eb" {...ol} />
      <path d="M42 90 L60 108 L78 90z" fill="#dc2626" {...ol} />
      <circle cx="33" cy="68" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <circle cx="87" cy="68" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <ellipse cx="60" cy="67" rx="27" ry="28" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M46 78 q7 -6 14 -1 q7 -5 14 1 q-7 5 -14 1 q-7 4 -14 -1z" fill="#78350f" stroke={INK} strokeWidth="2" strokeLinejoin="round" />
      <Face lx={50} rx={70} y={62} mouthY={85} />
      <g className="acc">
        <ellipse cx="60" cy="45" rx="46" ry="9" fill="#92400e" {...ol} />
        <path d="M36 45 q-2 -30 24 -30 q26 0 24 30z" fill="#b45309" {...ol} />
        <path d="M37 38 h46" stroke="#451a03" strokeWidth="6" />
      </g>
    </Frame>
  );
}

function Alien({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#4338ca', '#0b0f17']} defs={<Radial id={`${uid}-skin`} a="#bbf7d0" b="#16a34a" />}>
      <g fill="#fff">
        <circle cx="16" cy="20" r="1.5" />
        <circle cx="102" cy="30" r="2" />
        <circle cx="96" cy="96" r="1.5" />
        <circle cx="20" cy="88" r="1.8" />
      </g>
      <path d="M22 122 q4 -26 38 -28 q34 2 38 28z" fill="#cbd5e1" {...ol} />
      <path d="M60 16 q38 0 38 38 q0 30 -38 46 q-38 -16 -38 -46 q0 -38 38 -38z" fill={`url(#${uid}-skin)`} {...ol} />
      <Face lx={45} rx={75} y={56} mouthY={82} eye={8} />
      <g className="acc">
        <path d="M44 22 l-8 -14 M76 22 l8 -14" {...ol} fill="none" />
        <circle cx="35" cy="7" r="4" fill="#facc15" {...ol} />
        <circle cx="85" cy="7" r="4" fill="#facc15" {...ol} />
      </g>
    </Frame>
  );
}

function Ninja({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#f87171', '#7f1d1d']} defs={<Radial id={`${uid}-cloth`} a="#475569" b="#111827" />}>
      <path d="M16 122 q6 -30 44 -32 q38 2 44 32z" fill={`url(#${uid}-cloth)`} {...ol} />
      <circle cx="60" cy="60" r="34" fill={`url(#${uid}-cloth)`} {...ol} />
      <rect x="29" y="48" width="62" height="21" rx="10" fill="#fcd9b8" {...ol} />
      <Face lx={47} rx={73} y={58} mouthY={80} eye={4.5} mouth={false} />
      <g className="acc">
        <path d="M27 36 q33 -10 66 0 v8 q-33 -9 -66 0z" fill="#dc2626" {...ol} />
        <path d="M92 40 q14 2 18 12 M92 42 q10 8 10 18" {...ol} stroke="#dc2626" strokeWidth={4} fill="none" />
      </g>
    </Frame>
  );
}

function Shark({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#38bdf8', '#1e3a8a']} defs={<Radial id={`${uid}-skin`} a="#94a3b8" b="#334155" />}>
      <g fill="none" stroke="#e0f2fe" strokeWidth="2" opacity=".7">
        <circle cx="18" cy="30" r="4" />
        <circle cx="26" cy="18" r="2.5" />
        <circle cx="100" cy="40" r="3" />
      </g>
      <path d="M16 122 q-6 -64 44 -88 q50 24 44 88z" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M30 84 q30 18 60 0 q-2 26 -30 30 q-28 -4 -30 -30z" fill="#f1f5f9" {...ol} />
      <path d="M24 66 q-3 6 0 12 M96 66 q3 6 0 12" {...ol} fill="none" />
      <Face lx={44} rx={76} y={58} mouthY={88} />
      <g className="acc">
        <path d="M52 36 L60 6 L72 38z" fill={`url(#${uid}-skin)`} {...ol} />
      </g>
    </Frame>
  );
}

function Dragon({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#fb923c', '#7f1d1d']} defs={<Radial id={`${uid}-scales`} a="#86efac" b="#15803d" />}>
      <path d="M16 122 q6 -30 44 -32 q38 2 44 32z" fill={`url(#${uid}-scales)`} {...ol} />
      <path d="M40 102 q4 -6 8 0 M56 98 q4 -6 8 0 M72 102 q4 -6 8 0" {...ol} fill="none" strokeWidth={2} />
      <path d="M44 30 l6 -12 l6 10 l4 -12 l4 12 l6 -10 l6 12z" fill="#fde047" {...ol} />
      <ellipse cx="60" cy="56" rx="32" ry="29" fill={`url(#${uid}-scales)`} {...ol} />
      <ellipse cx="60" cy="80" rx="21" ry="14" fill="#bbf7d0" {...ol} />
      <circle cx="53" cy="75" r="2" fill={INK} />
      <circle cx="67" cy="75" r="2" fill={INK} />
      <Face lx={46} rx={74} y={52} mouthY={86} />
      <g className="acc">
        <path d="M36 36 q-14 -10 -10 -28 q8 12 18 18z" fill="#fef3c7" {...ol} />
        <path d="M84 36 q14 -10 10 -28 q-8 12 -18 18z" fill="#fef3c7" {...ol} />
      </g>
    </Frame>
  );
}

function Unicorn({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#f9a8d4', '#9d174d']} defs={<Radial id={`${uid}-coat`} a="#ffffff" b="#e9d5ff" />}>
      <path d="M24 122 q4 -28 36 -30 q32 2 36 30z" fill={`url(#${uid}-coat)`} {...ol} />
      <path d="M84 30 q22 16 14 60 q-8 -8 -12 -2 q-2 -30 -10 -50z" fill="#a78bfa" {...ol} />
      <path d="M86 40 q14 14 8 40" stroke="#38bdf8" strokeWidth="5" fill="none" />
      <path d="M84 52 q8 10 4 26" stroke="#facc15" strokeWidth="5" fill="none" />
      <path d="M38 34 L40 16 L52 30z M82 34 L80 16 L68 30z" fill={`url(#${uid}-coat)`} {...ol} />
      <ellipse cx="60" cy="62" rx="28" ry="32" fill={`url(#${uid}-coat)`} {...ol} />
      <ellipse cx="60" cy="83" rx="17" ry="11" fill="#fbcfe8" {...ol} />
      <circle cx="54" cy="81" r="1.8" fill={INK} />
      <circle cx="66" cy="81" r="1.8" fill={INK} />
      <path d="M36 30 q12 -6 20 4 q-14 0 -20 10z" fill="#f472b6" {...ol} />
      <Face lx={48} rx={72} y={60} mouthY={88} />
      <g className="acc">
        <path d="M54 34 L60 4 L66 34z" fill="#fde047" {...ol} />
        <path d="M56 26 l8 -3 M57 18 l6 -2" stroke="#ca8a04" strokeWidth="2" />
      </g>
    </Frame>
  );
}

function Crown({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#fde68a', '#b45309']} defs={<Radial id={`${uid}-skin`} a="#fde3c8" b="#e8a77c" />}>
      <path d="M14 122 q6 -32 46 -34 q40 2 46 34z" fill="#7c3aed" {...ol} />
      <path d="M30 98 q30 12 60 0 l2 10 q-32 12 -64 0z" fill="#fff" {...ol} />
      <g fill={INK}>
        <circle cx="42" cy="104" r="1.6" />
        <circle cx="60" cy="107" r="1.6" />
        <circle cx="78" cy="104" r="1.6" />
      </g>
      <ellipse cx="60" cy="62" rx="27" ry="30" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M34 66 q2 30 26 30 q24 0 26 -30 q-6 10 -14 12 q-12 -6 -24 0 q-8 -2 -14 -12z" fill="#92400e" {...ol} />
      <Face lx={50} rx={70} y={60} mouthY={80} />
      <g className="acc">
        <path d="M32 40 L34 14 L46 28 L60 10 L74 28 L86 14 L88 40z" fill="#facc15" {...ol} />
        <circle cx="60" cy="30" r="4" fill="#ef4444" {...ol} strokeWidth={2} />
        <circle cx="44" cy="34" r="3" fill="#3b82f6" {...ol} strokeWidth={2} />
        <circle cx="76" cy="34" r="3" fill="#22c55e" {...ol} strokeWidth={2} />
      </g>
    </Frame>
  );
}

const AVATARS: [string, string, (p: ArtProps) => ReturnType<typeof Rookie>][] = [
  ['🙂', 'Rookie', Rookie],
  ['😎', 'Cool Cat', CoolCat],
  ['🤖', 'Robot', Robot],
  ['🤠', 'Cowboy', Cowboy],
  ['👽', 'Alien', Alien],
  ['🥷', 'Ninja', Ninja],
  ['🦈', 'Shark', Shark],
  ['🐉', 'Dragon', Dragon],
  ['🦄', 'Unicorn', Unicorn],
  ['👑', 'Crown', Crown],
];
for (const [icon, name, Svg] of AVATARS) registerArt(icon, { kind: 'avatar', name, Svg });
