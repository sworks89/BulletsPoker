import { Frame, INK, ol, Radial } from './avatars';
import { registerArt, type ArtProps } from './registry';

// The seven Fish Buffet tier creatures, in the avatars' outlined-cartoon style.
// Backgrounds get deeper with the tier: shallows for Plankton, the abyss for Shark.
// Eyes sit in .eyes (they blink) and the body in Frame's .figure (it breathes).

/** A googly eye: white, ink pupil, highlight. */
function Eye({ x, y, r = 6, look = 1 }: { x: number; y: number; r?: number; look?: number }) {
  return (
    <>
      <circle cx={x} cy={y} r={r} fill="#fff" {...ol} strokeWidth={2.5} />
      <circle cx={x + look * r * 0.25} cy={y + r * 0.1} r={r * 0.55} fill={INK} />
      <circle cx={x + look * r * 0.05} cy={y - r * 0.2} r={r * 0.2} fill="#fff" />
    </>
  );
}

/** A thick outlined limb (leg, tentacle) along `d`. */
function Limb({ d, color, w = 5 }: { d: string; color: string; w?: number }) {
  return (
    <g fill="none" strokeLinecap="round" strokeLinejoin="round">
      <path d={d} stroke={INK} strokeWidth={w + 5} />
      <path d={d} stroke={color} strokeWidth={w} />
    </g>
  );
}

const smile = { stroke: INK, strokeWidth: 3, fill: 'none', strokeLinecap: 'round' } as const;

function Bubbles({ at }: { at: [number, number, number][] }) {
  return (
    <g fill="none" stroke="#f0f9ff" strokeWidth="2" opacity=".75">
      {at.map(([x, y, r]) => (
        <circle key={`${x},${y}`} cx={x} cy={y} r={r} />
      ))}
    </g>
  );
}

function Plankton({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#99f6e4', '#0d9488']} defs={<Radial id={`${uid}-body`} a="#d9f99d" b="#22c55e" />}>
      <Bubbles at={[[18, 96, 4], [26, 84, 2.5], [100, 92, 3], [96, 22, 2]]} />
      <path d="M50 46 q-10 -18 -26 -24 M70 46 q10 -18 26 -24" {...ol} fill="none" />
      <circle cx="24" cy="22" r="4.5" fill="#fde047" {...ol} strokeWidth={2.5} />
      <circle cx="96" cy="22" r="4.5" fill="#fde047" {...ol} strokeWidth={2.5} />
      <path d="M40 74 l-11 5 M41 83 l-9 9 M80 74 l11 5 M79 83 l9 9" {...ol} fill="none" />
      <path d="M60 92 q-9 8 0 14 q9 6 0 12" {...ol} fill="none" />
      <ellipse cx="60" cy="68" rx="23" ry="26" fill={`url(#${uid}-body)`} {...ol} />
      <ellipse cx="51" cy="56" rx="7" ry="5" fill="#fff" opacity=".55" />
      <circle cx="47" cy="77" r="4" fill="#fb7185" opacity=".5" />
      <circle cx="73" cy="77" r="4" fill="#fb7185" opacity=".5" />
      {/* One big eye, like a copepod. */}
      <g className="eyes">
        <Eye x={60} y={63} r={10} look={0} />
      </g>
      <path d="M54 80 q6 5 12 0" {...smile} />
    </Frame>
  );
}

function Shrimp({ uid }: ArtProps) {
  // A curled body: segments around a circle, tail first so the head sits on top.
  const segs = [5, 4, 3, 2, 1, 0].map((k) => {
    const a = ((-120 + k * 42) * Math.PI) / 180;
    return { k, x: 64 + 24 * Math.cos(a), y: 62 + 24 * Math.sin(a), rot: -120 + k * 42 + 90, r: 13 - k };
  });
  const tail = segs[0];
  return (
    <Frame uid={uid} bg={['#a5f3fc', '#0891b2']} defs={<Radial id={`${uid}-shell`} a="#fed7aa" b="#f97316" />}>
      <Bubbles at={[[100, 24, 3.5], [108, 36, 2], [18, 100, 3]]} />
      <path d="M34 34 q-6 -22 18 -30 M40 32 q4 -20 32 -24" {...ol} fill="none" />
      <path d="M58 64 l-10 4 M62 72 l-8 8 M70 76 l-2 10" {...ol} fill="none" strokeWidth={2.5} />
      <g transform={`translate(${tail.x} ${tail.y + 8})`}>
        <path d="M0 -4 q-16 6 -18 20 q10 -2 18 -8 q8 6 18 8 q-2 -14 -18 -20z" fill="#fb923c" {...ol} />
        <path d="M0 -2 v14" stroke={INK} strokeWidth="2" />
      </g>
      {segs.map((s) => (
        <ellipse
          key={s.k}
          cx={s.x}
          cy={s.y}
          rx={s.r + 2}
          ry={s.r}
          transform={`rotate(${s.rot} ${s.x} ${s.y})`}
          fill={`url(#${uid}-shell)`}
          {...ol}
        />
      ))}
      <ellipse cx="38" cy="46" rx="20" ry="16" fill={`url(#${uid}-shell)`} {...ol} />
      <path d="M22 40 l-6 -3 l8 -2" fill="#fb923c" {...ol} strokeWidth={2.5} />
      <circle cx="31" cy="54" r="3.5" fill="#fb7185" opacity=".55" />
      <g className="eyes">
        <Eye x={36} y={42} r={6} look={-1} />
      </g>
      <path d="M26 54 q5 5 11 1" {...smile} />
    </Frame>
  );
}

/** One claw (left side; the right one is mirrored). */
function Claw({ fill }: { fill: string }) {
  return (
    <>
      <Limb d="M40 70 q-16 -6 -18 -24" color="#ef4444" />
      <path d="M22 50 q-16 0 -16 -16 q0 -14 14 -16 q10 0 14 8 l-10 6 l12 2 q-2 16 -14 16z" fill={fill} {...ol} />
      <path d="M12 32 q2 -6 8 -8" stroke="#fecaca" strokeWidth="3" fill="none" strokeLinecap="round" />
    </>
  );
}

function Crab({ uid }: ArtProps) {
  const shell = `url(#${uid}-shell)`;
  return (
    <Frame uid={uid} bg={['#fef3c7', '#d97706']} defs={<Radial id={`${uid}-shell`} a="#fca5a5" b="#dc2626" />}>
      <path d="M0 108 q30 -8 60 0 q30 8 60 0 v12 h-120z" fill="#fcd34d" opacity=".7" />
      <Limb d="M34 80 q-14 4 -20 16 M36 88 q-10 8 -12 20 M86 80 q14 4 20 16 M84 88 q10 8 12 20" color="#ef4444" w={4} />
      <Claw fill={shell} />
      <g transform="translate(120 0) scale(-1 1)">
          <Claw fill={shell} />
      </g>
      <path d="M50 58 v-16 M70 58 v-16" {...ol} fill="none" />
      <ellipse cx="60" cy="74" rx="32" ry="22" fill={shell} {...ol} />
      <path d="M40 66 q20 -10 40 0" stroke="#fecaca" strokeWidth="3" fill="none" strokeLinecap="round" opacity=".8" />
      <circle cx="44" cy="80" r="4" fill="#fb7185" opacity=".6" />
      <circle cx="76" cy="80" r="4" fill="#fb7185" opacity=".6" />
      <g className="eyes">
        <Eye x={50} y={38} r={7} />
        <Eye x={70} y={38} r={7} />
      </g>
      <path d="M52 82 q8 7 16 0" {...smile} />
    </Frame>
  );
}

function Octopus({ uid }: ArtProps) {
  const arm = '#a855f7';
  return (
    <Frame uid={uid} bg={['#c4b5fd', '#4c1d95']} defs={<Radial id={`${uid}-skin`} a="#f0abfc" b="#9333ea" />}>
      <Bubbles at={[[16, 22, 3.5], [24, 12, 2], [104, 30, 3]]} />
      <Limb d="M36 78 q-22 8 -18 26 q4 8 12 1" color={arm} w={8} />
      <Limb d="M84 78 q22 8 18 26 q-4 8 -12 1" color={arm} w={8} />
      <Limb d="M46 84 q-10 16 -4 26 q6 5 9 -3" color={arm} w={8} />
      <Limb d="M74 84 q10 16 4 26 q-6 5 -9 -3" color={arm} w={8} />
      <Limb d="M58 86 q-2 14 -6 24" color={arm} w={8} />
      <Limb d="M62 86 q2 14 6 24" color={arm} w={8} />
      <g fill="#fbcfe8" stroke={INK} strokeWidth="1.2">
        <circle cx="22" cy="98" r="2" />
        <circle cx="98" cy="98" r="2" />
        <circle cx="43" cy="102" r="1.8" />
        <circle cx="77" cy="102" r="1.8" />
      </g>
      <path d="M28 82 q-6 -60 32 -62 q38 2 32 62 q-32 10 -64 0z" fill={`url(#${uid}-skin)`} {...ol} />
      <g fill="#7e22ce" opacity=".45">
        <circle cx="44" cy="36" r="4" />
        <circle cx="76" cy="40" r="3" />
        <circle cx="66" cy="30" r="2.5" />
      </g>
      <ellipse cx="48" cy="34" rx="8" ry="5" fill="#fff" opacity=".35" />
      <circle cx="40" cy="72" r="4.5" fill="#fb7185" opacity=".55" />
      <circle cx="80" cy="72" r="4.5" fill="#fb7185" opacity=".55" />
      <g className="eyes">
        <Eye x={48} y={60} r={8} />
        <Eye x={72} y={60} r={8} />
      </g>
      <path d="M54 75 q6 5 12 0" {...smile} />
    </Frame>
  );
}

function Dolphin({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#7dd3fc', '#1d4ed8']} defs={<Radial id={`${uid}-skin`} a="#93c5fd" b="#3b6fb6" />}>
      <path d="M0 102 q15 -8 30 0 t30 0 t30 0 t30 0 v18 h-120z" fill="#e0f2fe" opacity=".55" />
      <g transform="translate(9 3) scale(0.88)">
        <path d="M30 94 q-12 -10 -24 -8 q8 6 6 18 q12 0 18 -10z" fill={`url(#${uid}-skin)`} {...ol} />
        <path d="M56 44 q2 -20 16 -24 q-4 10 -2 20z" fill={`url(#${uid}-skin)`} {...ol} />
        <path
          d="M26 94 C32 62 56 38 84 38 C96 38 102 44 104 50 L114 54 Q117 58 112 61 L98 63 C80 72 60 84 36 98 Z"
          fill={`url(#${uid}-skin)`}
          {...ol}
        />
        <path d="M98 63 C80 72 60 84 36 98 q22 -4 36 -16 q14 -10 26 -19z" fill="#e0f2fe" stroke={INK} strokeWidth="2" strokeLinejoin="round" />
        <path d="M68 68 q-4 14 -16 18 q14 2 22 -12z" fill="#3b6fb6" {...ol} />
        <path d="M70 44 q10 -4 18 0" stroke="#dbeafe" strokeWidth="3" fill="none" strokeLinecap="round" opacity=".7" />
        <circle cx="96" cy="56" r="3" fill="#fb7185" opacity=".6" />
        <g className="eyes">
          <Eye x={90} y={48} r={5} />
        </g>
        <path d="M111 59 q-6 4 -13 3" {...smile} strokeWidth={2.5} />
      </g>
      <Bubbles at={[[106, 30, 3], [112, 20, 2], [20, 30, 2.5]]} />
    </Frame>
  );
}

function Orca({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#93c5fd', '#1e3a8a']} defs={<Radial id={`${uid}-skin`} a="#475569" b="#0f172a" cx={0.5} cy={0.3} />}>
      <Bubbles at={[[16, 22, 3], [26, 14, 2], [106, 20, 3.5]]} />
      <g transform="translate(9 4) scale(0.88)">
        <path d="M24 82 q-8 -16 -20 -18 q6 10 3 24 q12 0 17 -6z" fill={`url(#${uid}-skin)`} {...ol} />
        <path d="M56 42 q0 -28 8 -36 q6 20 14 36z" fill={`url(#${uid}-skin)`} {...ol} />
        <path
          d="M18 84 C22 54 52 38 82 38 C104 40 114 56 110 72 C106 84 90 90 70 90 C50 90 32 88 18 84 Z"
          fill={`url(#${uid}-skin)`}
          {...ol}
        />
        <path d="M110 72 C106 84 90 90 70 90 C54 90 40 88 30 86 q28 -2 46 -10 q18 -8 34 -4z" fill="#f8fafc" stroke={INK} strokeWidth="2" strokeLinejoin="round" />
        <ellipse cx="54" cy="45" rx="11" ry="3.5" fill="#94a3b8" opacity=".75" />
        <ellipse cx="86" cy="54" rx="10" ry="5.5" transform="rotate(-12 86 54)" fill="#f8fafc" stroke={INK} strokeWidth="2" />
        <path d="M74 84 q-2 16 -14 22 q16 0 22 -18z" fill="#0f172a" {...ol} />
        <circle cx="99" cy="70" r="3.5" fill="#fb7185" opacity=".55" />
        <g className="eyes">
          <Eye x={96} y={58} r={4.5} />
        </g>
        <path d="M110 70 q-6 6 -16 6" {...smile} strokeWidth={2.5} />
      </g>
    </Frame>
  );
}

function Shark({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#818cf8', '#1e1b4b']} defs={<Radial id={`${uid}-skin`} a="#cbd5e1" b="#475569" />}>
      <g fill="#fde047">
        <path d="M18 20 l2 -6 l2 6 l6 2 l-6 2 l-2 6 l-2 -6 l-6 -2z" />
        <path d="M104 96 l1.5 -4 l1.5 4 l4 1.5 l-4 1.5 l-1.5 4 l-1.5 -4 l-4 -1.5z" />
        <circle cx="30" cy="100" r="1.8" />
        <circle cx="100" cy="14" r="1.6" />
      </g>
      <g transform="translate(8 6) scale(0.88)">
        <path d="M22 70 L6 44 Q12 60 10 70 Q12 82 6 96 Z" fill={`url(#${uid}-skin)`} {...ol} />
        <path d="M52 44 q4 -28 18 -34 q-4 16 -2 32z" fill={`url(#${uid}-skin)`} {...ol} />
        <path
          d="M16 70 C30 46 70 36 96 46 C106 50 114 56 114 62 C112 70 100 76 86 78 C60 82 34 80 16 70 Z"
          fill={`url(#${uid}-skin)`}
          {...ol}
        />
        <path d="M114 62 C112 70 100 76 86 78 C60 82 36 80 22 74 q40 0 60 -6 q18 -6 32 -6z" fill="#f1f5f9" stroke={INK} strokeWidth="2" strokeLinejoin="round" />
        <path d="M66 74 q-2 14 -14 20 q14 0 22 -16z" fill="#64748b" {...ol} />
        <path d="M72 54 q-3 6 0 12 M78 53 q-3 6 0 12 M84 53 q-3 6 0 12" {...ol} fill="none" strokeWidth={2} />
        {/* The toothy grin. */}
        <path d="M113 62 Q104 76 82 71 Q100 70 113 62 Z" fill="#7f1d1d" stroke={INK} strokeWidth="2" strokeLinejoin="round" />
        <path d="M88 71 l2.5 -4 l2.5 4 M94 71 l2.5 -4.5 l2.5 4 M100 69 l2.5 -4.5 l2.5 3.5 M106 66 l2 -4 l2 3" fill="#fff" stroke={INK} strokeWidth="1.2" strokeLinejoin="round" />
        <g className="eyes">
          <Eye x={96} y={53} r={5} />
        </g>
        <path d="M90 45 l11 3" {...smile} strokeWidth={2.5} />
        {/* Top of the food chain. */}
        <g className="acc" transform="rotate(12 92 30)">
          <path d="M80 40 L81 23 L87 31 L93 19 L99 31 L105 23 L106 40z" fill="#facc15" {...ol} />
          <circle cx="93" cy="34" r="2.6" fill="#ef4444" stroke={INK} strokeWidth="1.5" />
        </g>
      </g>
    </Frame>
  );
}

const CREATURES: [string, (p: ArtProps) => ReturnType<typeof Plankton>][] = [
  ['Plankton', Plankton],
  ['Shrimp', Shrimp],
  ['Crab', Crab],
  ['Octopus', Octopus],
  ['Dolphin', Dolphin],
  ['Orca', Orca],
  ['Shark', Shark],
];
CREATURES.forEach(([name, Svg], i) => registerArt(`tier:${i}`, { kind: 'tier', name, Svg }));
