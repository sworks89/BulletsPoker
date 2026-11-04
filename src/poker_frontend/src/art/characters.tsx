import { registerArt, type ArtProps, type Look } from './registry';
import { Face, Frame, INK, Radial, ol } from './avatars';

// The second set of store avatars, in the same style as avatars.tsx: more
// characters, each (bar the neutral ones) as a man and a woman, sold together
// (the store item's `pair`). Hats, helmets and crowns sit in .acc so a hit
// knocks them; the rest of the figure stays put.

const SKIN = ['#fde3c8', '#e8a77c'] as const;
const BODY = 'M16 122 q6 -30 44 -32 q38 2 44 32z';

function Stars({ at }: { at: [number, number, number][] }) {
  return (
    <g fill="#fff">
      {at.map(([x, y, r]) => (
        <circle key={`${x},${y}`} cx={x} cy={y} r={r} />
      ))}
    </g>
  );
}

function Blush({ y, l = 41, r = 79 }: { y: number; l?: number; r?: number }) {
  return (
    <>
      <circle cx={l} cy={y} r="5" fill="#fb7185" opacity=".45" />
      <circle cx={r} cy={y} r="5" fill="#fb7185" opacity=".45" />
    </>
  );
}

function Lucky({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#bef264', '#4d7c0f']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-hair`} a="#b45309" b="#78350f" />
          <Radial id={`${uid}-top`} a="#f472b6" b="#be185d" />
        </>
      }
    >
      <path d="M78 34 q32 -4 30 36 q-2 22 -14 30 q6 -24 -6 -44z" fill={`url(#${uid}-hair)`} {...ol} />
      <path d="M14 122 q4 -32 46 -34 q42 2 46 34z" fill={`url(#${uid}-top)`} {...ol} />
      <path d="M52 92 l-2 14 M68 92 l2 14" {...ol} fill="none" />
      <circle cx="33" cy="66" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <circle cx="87" cy="66" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <ellipse cx="60" cy="63" rx="27" ry="30" fill={`url(#${uid}-skin)`} {...ol} />
      <path
        d="M32 64 q-6 -40 28 -40 q34 0 28 40 q-3 -12 -8 -18 q-14 4 -30 -6 q-4 10 -14 12 q-4 2 -4 12z"
        fill={`url(#${uid}-hair)`}
        {...ol}
      />
      <Blush y={76} l={42} r={78} />
      <Face lx={49} rx={71} y={65} mouthY={81} lashes />
      <g className="acc">
        <ellipse cx="84" cy="34" rx="6" ry="7" fill="#facc15" {...ol} strokeWidth={2.5} />
      </g>
    </Frame>
  );
}

function Cowgirl({ uid }: ArtProps) {
  const braid = (x: number) => (
    <g key={x}>
      {[76, 86, 96].map((y) => (
        <ellipse key={y} cx={x} cy={y} rx="6" ry="6" fill={`url(#${uid}-hair)`} {...ol} strokeWidth={2.5} />
      ))}
      <rect x={x - 4} y="101" width="8" height="5" rx="2" fill="#ec4899" {...ol} strokeWidth={2} />
    </g>
  );
  return (
    <Frame
      uid={uid}
      bg={['#a5f3fc', '#0e7490']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-hair`} a="#fde68a" b="#d97706" />
        </>
      }
    >
      <path d={BODY} fill="#dc2626" {...ol} />
      <path d="M30 104 h60 M24 114 h72 M46 92 v30 M74 92 v30" stroke="#fecaca" strokeWidth="2" opacity=".6" />
      <path d="M44 90 L60 104 L76 90z" fill="#facc15" {...ol} />
      {braid(31)}
      {braid(89)}
      <ellipse cx="60" cy="64" rx="27" ry="29" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M33 58 q4 -14 27 -14 q23 0 27 14 q-12 -6 -27 -4 q-15 -2 -27 4z" fill={`url(#${uid}-hair)`} {...ol} />
      <g fill="#c2410c" opacity=".55">
        <circle cx="42" cy="74" r="1.3" />
        <circle cx="46" cy="77" r="1.3" />
        <circle cx="40" cy="78" r="1.3" />
        <circle cx="78" cy="74" r="1.3" />
        <circle cx="74" cy="77" r="1.3" />
        <circle cx="80" cy="78" r="1.3" />
      </g>
      <Face lx={50} rx={70} y={65} mouthY={81} lashes />
      <g className="acc">
        <ellipse cx="60" cy="46" rx="46" ry="9" fill="#db2777" {...ol} />
        <path d="M36 46 q-2 -30 24 -30 q26 0 24 30z" fill="#f472b6" {...ol} />
        <path d="M37 39 h46" stroke="#831843" strokeWidth="6" />
        <circle cx="60" cy="39" r="3" fill="#facc15" stroke={INK} strokeWidth="1.5" />
      </g>
    </Frame>
  );
}

function Pirate({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#fdba74', '#7c2d12']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-hair`} a="#ef4444" b="#7f1d1d" />
        </>
      }
    >
      <path d="M30 50 q-16 40 2 62 q12 2 14 -10 h28 q2 12 14 10 q18 -22 2 -62z" fill={`url(#${uid}-hair)`} {...ol} />
      <path d={BODY} fill="#1e3a8a" {...ol} />
      <path d="M46 90 L60 114 L74 90z" fill="#f8fafc" {...ol} />
      <circle cx="38" cy="106" r="2.5" fill="#facc15" stroke={INK} strokeWidth="1.5" />
      <circle cx="82" cy="106" r="2.5" fill="#facc15" stroke={INK} strokeWidth="1.5" />
      <ellipse cx="60" cy="64" rx="27" ry="29" fill={`url(#${uid}-skin)`} {...ol} />
      <circle cx="34" cy="82" r="4.5" fill="none" stroke="#facc15" strokeWidth="2.5" />
      <path d="M33 58 q10 -12 27 -12 q-14 8 -27 12z M87 58 q-10 -12 -27 -12 q14 8 27 12z" fill={`url(#${uid}-hair)`} {...ol} />
      <Face lx={50} rx={70} y={65} mouthY={81} lashes />
      <path d="M34 54 q24 0 52 18" fill="none" stroke={INK} strokeWidth="2.5" />
      <ellipse cx="70" cy="65" rx="8" ry="7" fill={INK} />
      <g className="acc">
        <path d="M20 46 q40 -46 80 0 q-10 -8 -20 -2 q-20 -10 -40 0 q-10 -6 -20 2z" fill="#111827" {...ol} />
        <path d="M24 42 q8 -4 16 0 q20 -8 40 0 q8 -4 16 0" fill="none" stroke="#facc15" strokeWidth="2" />
        <circle cx="60" cy="28" r="5" fill="#f8fafc" stroke={INK} strokeWidth="1.5" />
        <path d="M58 27 h0.1 M62 27 h0.1" stroke={INK} strokeWidth="2.5" strokeLinecap="round" />
        <path d="M53 36 l14 -4 M53 32 l14 4" stroke="#f8fafc" strokeWidth="2" strokeLinecap="round" />
      </g>
    </Frame>
  );
}

function Astronaut({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#312e81', '#020617']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-hair`} a="#92400e" b="#451a03" />
          <Radial id={`${uid}-suit`} a="#ffffff" b="#cbd5e1" />
          <Radial id={`${uid}-gold`} a="#fde68a" b="#d97706" />
        </>
      }
    >
      <Stars at={[[14, 18, 1.5], [104, 24, 2], [10, 96, 1.6], [108, 90, 1.4], [96, 8, 1.2]]} />
      <path d={BODY} fill={`url(#${uid}-suit)`} {...ol} />
      <circle cx="38" cy="108" r="6" fill="#2563eb" {...ol} strokeWidth={2} />
      <rect x="70" y="102" width="16" height="10" rx="2" fill="#ef4444" {...ol} strokeWidth={2} />
      <circle cx="60" cy="58" r="40" fill={`url(#${uid}-suit)`} {...ol} />
      <ellipse cx="60" cy="63" rx="31" ry="30" fill="#0f172a" {...ol} />
      <path d="M36 64 q-2 -30 24 -30 q26 0 24 30 v16 q-6 4 -10 0 h-28 q-4 4 -10 0z" fill={`url(#${uid}-hair)`} />
      <ellipse cx="60" cy="65" rx="22" ry="25" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M38 60 q0 -22 22 -22 q22 0 22 22 q-10 -12 -22 -10 q-12 -2 -22 10z" fill={`url(#${uid}-hair)`} {...ol} />
      <Blush y={75} l={46} r={74} />
      <Face lx={51} rx={69} y={65} mouthY={79} eye={4.5} lashes />
      <circle cx="21" cy="62" r="3" fill="#22c55e" stroke={INK} strokeWidth="1.5" />
      <circle cx="99" cy="62" r="3" fill="#ef4444" stroke={INK} strokeWidth="1.5" />
      <g className="acc">
        <path d="M26 44 q34 -38 68 0 q-34 -16 -68 0z" fill={`url(#${uid}-gold)`} {...ol} />
        <path d="M40 30 q8 -6 16 -6" stroke="#fff" strokeWidth="3" fill="none" strokeLinecap="round" opacity=".8" />
      </g>
    </Frame>
  );
}

function Kunoichi({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#f9a8d4', '#831843']} defs={<Radial id={`${uid}-cloth`} a="#7e22ce" b="#2e1065" />}>
      <circle cx="60" cy="24" r="12" fill="#1f2937" {...ol} />
      <path d={BODY} fill={`url(#${uid}-cloth)`} {...ol} />
      <circle cx="60" cy="60" r="34" fill={`url(#${uid}-cloth)`} {...ol} />
      <rect x="29" y="48" width="62" height="21" rx="10" fill="#fcd9b8" {...ol} />
      <Face lx={47} rx={73} y={58} mouthY={80} eye={4.5} mouth={false} lashes />
      <g className="acc">
        <path d="M27 36 q33 -10 66 0 v8 q-33 -9 -66 0z" fill="#ec4899" {...ol} />
        <path d="M28 40 q-14 2 -18 12 M28 42 q-10 8 -10 18" {...ol} stroke="#ec4899" strokeWidth={4} fill="none" />
        <path d="M44 14 L80 30" stroke="#fbbf24" strokeWidth="3" strokeLinecap="round" />
        <g fill="#f9a8d4" stroke={INK} strokeWidth="1.5">
          <circle cx="82" cy="27" r="3.2" />
          <circle cx="86" cy="31" r="3.2" />
          <circle cx="81" cy="33" r="3.2" />
        </g>
        <circle cx="83" cy="30" r="1.6" fill="#facc15" />
      </g>
    </Frame>
  );
}

function Viking({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#bae6fd', '#0369a1']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-beard`} a="#fb923c" b="#c2410c" />
          <Radial id={`${uid}-steel`} a="#f1f5f9" b="#64748b" />
        </>
      }
    >
      <path d="M14 122 q6 -32 46 -34 q40 2 46 34z" fill="#78350f" {...ol} />
      <path d="M20 112 q8 -16 22 -20 M100 112 q-8 -16 -22 -20" stroke="#d6d3d1" strokeWidth="7" strokeLinecap="round" fill="none" />
      <circle cx="34" cy="108" r="5" fill="#facc15" {...ol} strokeWidth={2} />
      <circle cx="33" cy="66" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <circle cx="87" cy="66" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <ellipse cx="60" cy="64" rx="27" ry="29" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M33 64 q0 40 27 44 q27 -4 27 -44 q-6 14 -14 14 q-13 -6 -26 0 q-8 0 -14 -14z" fill={`url(#${uid}-beard)`} {...ol} />
      <g fill={`url(#${uid}-beard)`} stroke={INK} strokeWidth="2">
        <circle cx="52" cy="110" r="3.5" />
        <circle cx="52" cy="116" r="3.5" />
        <circle cx="68" cy="110" r="3.5" />
        <circle cx="68" cy="116" r="3.5" />
      </g>
      <path d="M46 79 q7 -6 14 -1 q7 -5 14 1 q-7 5 -14 1 q-7 4 -14 -1z" fill="#9a3412" stroke={INK} strokeWidth="2" strokeLinejoin="round" />
      <Face lx={50} rx={70} y={63} mouthY={87} />
      <g className="acc">
        <path d="M34 44 q-18 -2 -22 -26 q12 10 24 14z" fill="#fef3c7" {...ol} />
        <path d="M86 44 q18 -2 22 -26 q-12 10 -24 14z" fill="#fef3c7" {...ol} />
        <path d="M32 50 q0 -32 28 -32 q28 0 28 32z" fill={`url(#${uid}-steel)`} {...ol} />
        <rect x="30" y="44" width="60" height="9" rx="3" fill="#b45309" {...ol} />
        <path d="M60 18 v26" stroke={INK} strokeWidth="2.5" />
        <g fill="#fde68a">
          <circle cx="40" cy="48.5" r="1.6" />
          <circle cx="60" cy="48.5" r="1.6" />
          <circle cx="80" cy="48.5" r="1.6" />
        </g>
      </g>
    </Frame>
  );
}

function Samurai({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#e5e7eb', '#4b5563']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-armor`} a="#ef4444" b="#7f1d1d" />
          <Radial id={`${uid}-gold`} a="#fde68a" b="#ca8a04" />
        </>
      }
    >
      <path d="M14 122 q6 -32 46 -34 q40 2 46 34z" fill={`url(#${uid}-armor)`} {...ol} />
      <path d="M24 106 h72 M20 115 h80" stroke="#fde68a" strokeWidth="2" />
      <path d="M40 100 v20 M60 98 v22 M80 100 v20" stroke="#fde68a" strokeWidth="1.5" strokeDasharray="3 3" />
      <path d="M30 46 q-12 20 -10 42 l14 -4 q-4 -18 2 -34z" fill="#111827" {...ol} />
      <path d="M90 46 q12 20 10 42 l-14 -4 q4 -18 -2 -34z" fill="#111827" {...ol} />
      <ellipse cx="60" cy="64" rx="26" ry="28" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M52 77 q-6 1 -10 7 M68 77 q6 1 10 7" stroke={INK} strokeWidth="2.5" fill="none" strokeLinecap="round" />
      <Face lx={50} rx={70} y={63} mouthY={83} />
      <g className="acc">
        <path d="M30 50 q0 -32 30 -32 q30 0 30 32z" fill="#1f2937" {...ol} />
        <path d="M24 50 q36 -8 72 0 l2 7 q-38 -8 -76 0z" fill="#111827" {...ol} />
        <path d="M28 6 q32 48 64 0 q-32 32 -64 0z" fill={`url(#${uid}-gold)`} {...ol} />
        <circle cx="60" cy="40" r="4.5" fill="#dc2626" {...ol} strokeWidth={2} />
      </g>
    </Frame>
  );
}

function Witch({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#86efac', '#14532d']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-hair`} a="#a855f7" b="#581c87" />
        </>
      }
    >
      <path d="M30 56 q-10 40 -2 56 h64 q8 -16 -2 -56z" fill={`url(#${uid}-hair)`} {...ol} />
      <path d={BODY} fill="#1f2937" {...ol} />
      <path d="M60 90 l3 7 h7 l-5.5 4.5 l2 7 l-6.5 -4 l-6.5 4 l2 -7 l-5.5 -4.5 h7z" fill="#facc15" stroke={INK} strokeWidth="1.5" strokeLinejoin="round" />
      <ellipse cx="60" cy="65" rx="26" ry="28" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M34 62 q2 -14 14 -16 q-4 10 -14 16z M86 62 q-2 -14 -14 -16 q4 10 14 16z" fill={`url(#${uid}-hair)`} {...ol} />
      <Blush y={77} l={43} r={77} />
      <Face lx={50} rx={70} y={65} mouthY={81} lashes />
      <g className="acc">
        <ellipse cx="60" cy="47" rx="42" ry="8" fill="#312e81" {...ol} />
        <path d="M38 47 q8 -20 14 -30 q8 -12 30 -15 q-12 10 -10 20 q2 14 10 25z" fill="#4c1d95" {...ol} />
        <path d="M40 41 q20 4 41 0 l1 5 q-21 4 -43 0z" fill="#22c55e" {...ol} strokeWidth={2} />
        <rect x="55" y="40" width="9" height="8" rx="1.5" fill="none" stroke="#facc15" strokeWidth="2" />
      </g>
    </Frame>
  );
}

function Vampire({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#c084fc', '#3b0764']} defs={<Radial id={`${uid}-skin`} a="#f8fafc" b="#c7d2fe" />}>
      <circle cx="98" cy="22" r="10" fill="#fef9c3" opacity=".9" />
      <path d="M22 34 L36 92 H84 L98 34 Q60 60 22 34z" fill="#b91c1c" {...ol} />
      <path d={BODY} fill="#111827" {...ol} />
      <path d="M48 90 L60 108 L72 90z" fill="#f8fafc" {...ol} />
      <path d="M54 94 l6 4 l6 -4 v8 l-6 -4 l-6 4z" fill="#dc2626" stroke={INK} strokeWidth="1.5" strokeLinejoin="round" />
      <path d="M36 62 l-11 -10 l7 18z M84 62 l11 -10 l-7 18z" fill={`url(#${uid}-skin)`} {...ol} />
      <ellipse cx="60" cy="63" rx="25" ry="29" fill={`url(#${uid}-skin)`} {...ol} />
      <path
        d="M35 56 q-2 -28 25 -28 q27 0 25 28 q-6 -12 -12 -14 q-6 0 -13 12 q-7 -12 -13 -12 q-6 2 -12 14z"
        fill="#111827"
        {...ol}
      />
      <Face lx={49} rx={71} y={64} mouthY={80} fangs />
    </Frame>
  );
}

function Mermaid({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#5eead4', '#0f766e']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-hair`} a="#fb7185" b="#be123c" />
        </>
      }
    >
      <g fill="none" stroke="#ecfeff" strokeWidth="2" opacity=".7">
        <circle cx="16" cy="30" r="4" />
        <circle cx="24" cy="18" r="2.5" />
        <circle cx="104" cy="44" r="3" />
      </g>
      <path d="M28 52 q-14 40 -4 70 h72 q10 -30 -4 -70z" fill={`url(#${uid}-hair)`} {...ol} />
      <path d="M26 122 q4 -28 34 -30 q30 2 34 30z" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M38 112 q8 -14 16 0z M66 112 q8 -14 16 0z" fill="#c084fc" {...ol} strokeWidth={2.5} />
      <path d="M54 112 q6 -4 12 0" stroke={INK} strokeWidth="2" fill="none" />
      <ellipse cx="60" cy="62" rx="26" ry="28" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M34 60 q-2 -30 26 -30 q28 0 26 30 q-10 -20 -30 -14 q-12 4 -22 14z" fill={`url(#${uid}-hair)`} {...ol} />
      <Blush y={74} l={43} r={77} />
      <Face lx={50} rx={70} y={63} mouthY={79} lashes />
      <g className="acc">
        <path
          d="M76 22 l3 7 l7 1 l-5 5 l1.5 7 l-6.5 -3.5 l-6.5 3.5 l1.5 -7 l-5 -5 l7 -1z"
          fill="#facc15"
          stroke={INK}
          strokeWidth="2"
          strokeLinejoin="round"
        />
        <g fill="#fff" stroke={INK} strokeWidth="1.2">
          <circle cx="44" cy="34" r="2.6" />
          <circle cx="50" cy="31" r="2.6" />
          <circle cx="57" cy="30" r="2.6" />
        </g>
      </g>
    </Frame>
  );
}

function Princess({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#f5d0fe', '#a21caf']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-hair`} a="#fde68a" b="#ca8a04" />
          <Radial id={`${uid}-gown`} a="#f472b6" b="#be185d" />
        </>
      }
    >
      <path d="M28 52 q-10 40 0 62 h64 q10 -22 0 -62z" fill={`url(#${uid}-hair)`} {...ol} />
      <path d="M18 122 q6 -30 42 -32 q36 2 42 32z" fill={`url(#${uid}-gown)`} {...ol} />
      <circle cx="26" cy="106" r="10" fill={`url(#${uid}-gown)`} {...ol} />
      <circle cx="94" cy="106" r="10" fill={`url(#${uid}-gown)`} {...ol} />
      <path d="M40 94 q20 12 40 0" stroke="#fff" strokeWidth="3" fill="none" strokeLinecap="round" />
      <circle cx="60" cy="101" r="3" fill="#fff" stroke={INK} strokeWidth="1.5" />
      <ellipse cx="60" cy="63" rx="26" ry="28" fill={`url(#${uid}-skin)`} {...ol} />
      <path
        d="M34 62 q-4 -32 26 -32 q30 0 26 32 q-6 -14 -16 -18 q-6 6 -10 -2 q-4 8 -10 2 q-10 4 -16 18z"
        fill={`url(#${uid}-hair)`}
        {...ol}
      />
      <Blush y={75} l={43} r={77} />
      <Face lx={50} rx={70} y={64} mouthY={80} lashes />
      <g className="acc">
        <path d="M40 36 L44 24 L52 32 L60 18 L68 32 L76 24 L80 36 q-20 -6 -40 0z" fill="#e2e8f0" {...ol} strokeWidth={2.5} />
        <circle cx="60" cy="29" r="3.5" fill="#ec4899" stroke={INK} strokeWidth="1.5" />
      </g>
    </Frame>
  );
}

function Wizard({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#818cf8', '#1e1b4b']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-robe`} a="#3b82f6" b="#1e3a8a" />
          <Radial id={`${uid}-beard`} a="#ffffff" b="#cbd5e1" />
        </>
      }
    >
      <Stars at={[[12, 20, 1.5], [104, 58, 1.8], [14, 84, 1.4]]} />
      <path d={BODY} fill={`url(#${uid}-robe)`} {...ol} />
      <circle cx="34" cy="64" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <circle cx="86" cy="64" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <ellipse cx="60" cy="61" rx="25" ry="26" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M36 66 q-2 44 24 54 q26 -10 24 -54 q-6 12 -14 12 q-10 -6 -20 0 q-8 0 -14 -12z" fill={`url(#${uid}-beard)`} {...ol} />
      <path d="M44 77 q8 -6 16 -1 q8 -5 16 1 q-6 8 -16 2 q-10 6 -16 -2z" fill={`url(#${uid}-beard)`} {...ol} strokeWidth={2} />
      <Face lx={50} rx={70} y={60} mouthY={87} />
      <g className="acc">
        <ellipse cx="60" cy="42" rx="40" ry="8" fill={`url(#${uid}-robe)`} {...ol} />
        <path d="M36 42 q10 -24 20 -36 q6 -8 14 -2 q-6 4 -4 12 q4 14 18 26z" fill={`url(#${uid}-robe)`} {...ol} />
        <path d="M62 22 l2 4.5 l5 0.5 l-3.8 3.2 l1.2 4.8 l-4.4 -2.6 l-4.4 2.6 l1.2 -4.8 l-3.8 -3.2 l5 -0.5z" fill="#facc15" />
        <path d="M48 34 a5 5 0 1 0 5 -6 a4 4 0 1 1 -5 6z" fill="#fde68a" />
      </g>
    </Frame>
  );
}

function Queen({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#fca5a5', '#7f1d1d']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-hair`} a="#92400e" b="#451a03" />
          <Radial id={`${uid}-gold`} a="#fde68a" b="#ca8a04" />
        </>
      }
    >
      <ellipse cx="32" cy="80" rx="8" ry="14" fill={`url(#${uid}-hair)`} {...ol} />
      <ellipse cx="88" cy="80" rx="8" ry="14" fill={`url(#${uid}-hair)`} {...ol} />
      <path d="M14 122 q6 -32 46 -34 q40 2 46 34z" fill="#7c3aed" {...ol} />
      <path d="M30 98 q30 12 60 0 l2 10 q-32 12 -64 0z" fill="#fff" {...ol} />
      <g fill={INK}>
        <circle cx="42" cy="104" r="1.6" />
        <circle cx="60" cy="107" r="1.6" />
        <circle cx="78" cy="104" r="1.6" />
      </g>
      <ellipse cx="60" cy="64" rx="25" ry="28" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M33 66 q-6 -38 27 -38 q33 0 27 38 q-4 -18 -27 -22 q-23 4 -27 22z" fill={`url(#${uid}-hair)`} {...ol} />
      <path d="M44 92 q16 6 32 0" stroke="#facc15" strokeWidth="2.5" fill="none" />
      <circle cx="60" cy="95" r="3.5" fill="#ef4444" stroke={INK} strokeWidth="1.5" />
      <circle cx="36" cy="82" r="3" fill="#22d3ee" stroke={INK} strokeWidth="1.5" />
      <circle cx="84" cy="82" r="3" fill="#22d3ee" stroke={INK} strokeWidth="1.5" />
      <Blush y={76} l={44} r={76} />
      <Face lx={50} rx={70} y={65} mouthY={81} lashes />
      <g className="acc">
        <path d="M36 40 L36 16 L46 28 L54 10 L60 24 L66 10 L74 28 L84 16 L84 40z" fill={`url(#${uid}-gold)`} {...ol} />
        <circle cx="60" cy="33" r="4" fill="#ef4444" {...ol} strokeWidth={2} />
        <circle cx="46" cy="35" r="2.6" fill="#22d3ee" {...ol} strokeWidth={1.5} />
        <circle cx="74" cy="35" r="2.6" fill="#22d3ee" {...ol} strokeWidth={1.5} />
      </g>
    </Frame>
  );
}

function PirateMan({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#fdba74', '#7c2d12']} defs={<Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />}>
      <path d={BODY} fill="#1e3a8a" {...ol} />
      <path d="M46 90 L60 114 L74 90z" fill="#f8fafc" {...ol} />
      <circle cx="38" cy="106" r="2.5" fill="#facc15" stroke={INK} strokeWidth="1.5" />
      <circle cx="82" cy="106" r="2.5" fill="#facc15" stroke={INK} strokeWidth="1.5" />
      <circle cx="33" cy="66" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <circle cx="87" cy="66" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <ellipse cx="60" cy="64" rx="27" ry="29" fill={`url(#${uid}-skin)`} {...ol} />
      <circle cx="31" cy="76" r="4" fill="none" stroke="#facc15" strokeWidth="2.5" />
      <path d="M34 72 q2 22 26 22 q24 0 26 -22 q-6 8 -12 8 q-14 -4 -28 0 q-6 0 -12 -8z" fill="#292524" {...ol} />
      <path d="M46 78 q7 -6 14 -1 q7 -5 14 1 q-7 5 -14 1 q-7 4 -14 -1z" fill="#292524" stroke={INK} strokeWidth="2" strokeLinejoin="round" />
      <Face lx={50} rx={70} y={63} mouthY={85} ink={INK} />
      <path d="M34 54 q24 0 52 18" fill="none" stroke={INK} strokeWidth="2.5" />
      <ellipse cx="70" cy="63" rx="8" ry="7" fill={INK} />
      <g className="acc">
        <path d="M20 46 q40 -46 80 0 q-10 -8 -20 -2 q-20 -10 -40 0 q-10 -6 -20 2z" fill="#111827" {...ol} />
        <path d="M24 42 q8 -4 16 0 q20 -8 40 0 q8 -4 16 0" fill="none" stroke="#facc15" strokeWidth="2" />
        <circle cx="60" cy="28" r="5" fill="#f8fafc" stroke={INK} strokeWidth="1.5" />
        <path d="M58 27 h0.1 M62 27 h0.1" stroke={INK} strokeWidth="2.5" strokeLinecap="round" />
        <path d="M53 36 l14 -4 M53 32 l14 4" stroke="#f8fafc" strokeWidth="2" strokeLinecap="round" />
      </g>
    </Frame>
  );
}

function AstronautMan({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#312e81', '#020617']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-suit`} a="#ffffff" b="#cbd5e1" />
          <Radial id={`${uid}-gold`} a="#fde68a" b="#d97706" />
        </>
      }
    >
      <Stars at={[[14, 18, 1.5], [104, 24, 2], [10, 96, 1.6], [108, 90, 1.4], [96, 8, 1.2]]} />
      <path d={BODY} fill={`url(#${uid}-suit)`} {...ol} />
      <circle cx="38" cy="108" r="6" fill="#2563eb" {...ol} strokeWidth={2} />
      <rect x="70" y="102" width="16" height="10" rx="2" fill="#ef4444" {...ol} strokeWidth={2} />
      <circle cx="60" cy="58" r="40" fill={`url(#${uid}-suit)`} {...ol} />
      <ellipse cx="60" cy="63" rx="31" ry="30" fill="#0f172a" {...ol} />
      <ellipse cx="60" cy="65" rx="22" ry="25" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M38 60 q0 -22 22 -22 q22 0 22 22 q-4 -8 -10 -10 q-6 6 -14 2 q-8 4 -12 -2 q-6 2 -8 10z" fill="#1c1917" {...ol} />
      <Face lx={51} rx={69} y={65} mouthY={79} eye={4.5} />
      <circle cx="21" cy="62" r="3" fill="#22c55e" stroke={INK} strokeWidth="1.5" />
      <circle cx="99" cy="62" r="3" fill="#ef4444" stroke={INK} strokeWidth="1.5" />
      <g className="acc">
        <path d="M26 44 q34 -38 68 0 q-34 -16 -68 0z" fill={`url(#${uid}-gold)`} {...ol} />
        <path d="M40 30 q8 -6 16 -6" stroke="#fff" strokeWidth="3" fill="none" strokeLinecap="round" opacity=".8" />
      </g>
    </Frame>
  );
}

function Shieldmaiden({ uid }: ArtProps) {
  const braid = (x: number) => (
    <g key={x} fill={`url(#${uid}-hair)`} stroke={INK} strokeWidth="2.5">
      {[76, 86, 96, 106].map((y) => (
        <ellipse key={y} cx={x} cy={y} rx="5.5" ry="6" />
      ))}
    </g>
  );
  return (
    <Frame
      uid={uid}
      bg={['#bae6fd', '#0369a1']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-hair`} a="#fef08a" b="#ca8a04" />
          <Radial id={`${uid}-steel`} a="#f1f5f9" b="#64748b" />
        </>
      }
    >
      <path d="M14 122 q6 -32 46 -34 q40 2 46 34z" fill="#78350f" {...ol} />
      <path d="M20 112 q8 -16 22 -20 M100 112 q-8 -16 -22 -20" stroke="#d6d3d1" strokeWidth="7" strokeLinecap="round" fill="none" />
      {braid(32)}
      {braid(88)}
      <ellipse cx="60" cy="64" rx="26" ry="28" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M38 76 l6 -2 M76 74 l6 2" stroke="#1d4ed8" strokeWidth="3" strokeLinecap="round" />
      <Face lx={50} rx={70} y={65} mouthY={82} lashes />
      <g className="acc">
        <path d="M34 46 q-20 -4 -22 -26 q12 10 24 14z" fill="#fef3c7" {...ol} />
        <path d="M86 46 q20 -4 22 -26 q-12 10 -24 14z" fill="#fef3c7" {...ol} />
        <path d="M33 52 q0 -32 27 -32 q27 0 27 32z" fill={`url(#${uid}-steel)`} {...ol} />
        <rect x="31" y="46" width="58" height="9" rx="3" fill="#b45309" {...ol} />
        <path d="M60 20 v26" stroke={INK} strokeWidth="2.5" />
        <g fill="#fde68a">
          <circle cx="41" cy="50.5" r="1.6" />
          <circle cx="60" cy="50.5" r="1.6" />
          <circle cx="79" cy="50.5" r="1.6" />
        </g>
      </g>
    </Frame>
  );
}

function OnnaMusha({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#e5e7eb', '#4b5563']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-armor`} a="#ef4444" b="#7f1d1d" />
        </>
      }
    >
      <path d="M72 30 q36 -10 34 30 q-2 26 -14 36 q6 -30 -8 -50z" fill="#111827" {...ol} />
      <path d="M14 122 q6 -32 46 -34 q40 2 46 34z" fill={`url(#${uid}-armor)`} {...ol} />
      <path d="M24 106 h72 M20 115 h80" stroke="#fde68a" strokeWidth="2" />
      <path d="M40 100 v20 M60 98 v22 M80 100 v20" stroke="#fde68a" strokeWidth="1.5" strokeDasharray="3 3" />
      <ellipse cx="60" cy="64" rx="26" ry="28" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M33 62 q-4 -34 27 -34 q31 0 27 34 q-4 -14 -12 -16 q-14 2 -30 -2 q-8 4 -12 18z" fill="#111827" {...ol} />
      <Blush y={76} l={44} r={76} />
      <Face lx={50} rx={70} y={64} mouthY={80} lashes />
      <g className="acc">
        <path d="M32 44 q28 -8 56 0 v8 q-28 -8 -56 0z" fill="#f8fafc" {...ol} />
        <circle cx="60" cy="45" r="3.5" fill="#dc2626" />
        <path d="M88 46 q12 0 18 8 M88 50 q8 6 8 14" {...ol} stroke="#f8fafc" strokeWidth={4} fill="none" />
        <circle cx="80" cy="30" r="4" fill="#dc2626" {...ol} strokeWidth={2} />
      </g>
    </Frame>
  );
}

function Vampira({ uid }: ArtProps) {
  return (
    <Frame uid={uid} bg={['#c084fc', '#3b0764']} defs={<Radial id={`${uid}-skin`} a="#f8fafc" b="#c7d2fe" />}>
      <circle cx="98" cy="22" r="10" fill="#fef9c3" opacity=".9" />
      <path d="M22 34 L36 92 H84 L98 34 Q60 60 22 34z" fill="#b91c1c" {...ol} />
      <path d="M30 54 q-10 40 -2 60 h64 q8 -20 -2 -60z" fill="#111827" {...ol} />
      <path d={BODY} fill="#111827" {...ol} />
      <path d="M44 92 q16 10 32 0" stroke="#dc2626" strokeWidth="3" fill="none" />
      <circle cx="60" cy="98" r="3.5" fill="#dc2626" stroke={INK} strokeWidth="1.5" />
      <ellipse cx="60" cy="63" rx="25" ry="29" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M34 64 q-4 -36 26 -36 q30 0 26 36 q-4 -14 -10 -18 q-16 6 -32 0 q-6 4 -10 18z" fill="#111827" {...ol} />
      <Blush y={76} l={44} r={76} />
      <Face lx={49} rx={71} y={64} mouthY={80} lashes fangs />
    </Frame>
  );
}

function Merman({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#5eead4', '#0f766e']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-hair`} a="#22d3ee" b="#0e7490" />
          <Radial id={`${uid}-gold`} a="#fde68a" b="#ca8a04" />
        </>
      }
    >
      <g fill="none" stroke="#ecfeff" strokeWidth="2" opacity=".7">
        <circle cx="16" cy="30" r="4" />
        <circle cx="24" cy="18" r="2.5" />
        <circle cx="104" cy="44" r="3" />
      </g>
      <path d="M98 30 v92" stroke="#ca8a04" strokeWidth="4" />
      <path d="M90 34 v-12 M98 30 v-16 M106 34 v-12 M90 34 q8 6 16 0" fill="none" stroke="#ca8a04" strokeWidth="3.5" strokeLinecap="round" />
      <path d="M22 122 q4 -30 38 -32 q34 2 38 32z" fill={`url(#${uid}-skin)`} {...ol} />
      <path d="M44 104 q4 4 8 0 M68 104 q4 4 8 0" stroke={INK} strokeWidth="2" fill="none" strokeLinecap="round" />
      <circle cx="33" cy="64" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <circle cx="87" cy="64" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <ellipse cx="60" cy="62" rx="27" ry="29" fill={`url(#${uid}-skin)`} {...ol} />
      <path
        d="M32 60 q-4 -32 28 -32 q32 0 28 32 q-6 -12 -14 -14 q-8 6 -14 -2 q-6 8 -14 2 q-10 2 -14 14z"
        fill={`url(#${uid}-hair)`}
        {...ol}
      />
      <Face lx={50} rx={70} y={63} mouthY={79} />
      <g className="acc">
        <path d="M38 36 L42 22 L50 32 L60 16 L70 32 L78 22 L82 36 q-22 -6 -44 0z" fill={`url(#${uid}-gold)`} {...ol} strokeWidth={2.5} />
        <circle cx="60" cy="29" r="3" fill="#f472b6" stroke={INK} strokeWidth="1.5" />
      </g>
    </Frame>
  );
}

function Prince({ uid }: ArtProps) {
  return (
    <Frame
      uid={uid}
      bg={['#f5d0fe', '#a21caf']}
      defs={
        <>
          <Radial id={`${uid}-skin`} a={SKIN[0]} b={SKIN[1]} />
          <Radial id={`${uid}-hair`} a="#fde68a" b="#ca8a04" />
          <Radial id={`${uid}-coat`} a="#60a5fa" b="#1d4ed8" />
        </>
      }
    >
      <path d={BODY} fill={`url(#${uid}-coat)`} {...ol} />
      <path d="M22 104 q6 -10 16 -12 M98 104 q-6 -10 -16 -12" stroke="#facc15" strokeWidth="6" strokeLinecap="round" fill="none" />
      <path d="M52 92 L60 120 L68 92z" fill="#f8fafc" {...ol} />
      <path d="M44 92 q4 16 6 28 M76 92 q-4 16 -6 28" stroke="#dc2626" strokeWidth="3" fill="none" />
      <circle cx="33" cy="64" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <circle cx="87" cy="64" r="6" fill={`url(#${uid}-skin)`} {...ol} />
      <ellipse cx="60" cy="63" rx="27" ry="29" fill={`url(#${uid}-skin)`} {...ol} />
      <path
        d="M32 60 q-4 -32 28 -32 q32 0 28 32 q-4 -10 -10 -14 q-10 8 -26 2 q-8 2 -12 8 q-4 2 -8 4z"
        fill={`url(#${uid}-hair)`}
        {...ol}
      />
      <Face lx={50} rx={70} y={64} mouthY={80} />
      <g className="acc">
        <path d="M42 34 L44 24 L52 30 L60 20 L68 30 L76 24 L78 34 q-18 -4 -36 0z" fill="#facc15" {...ol} strokeWidth={2.5} />
        <circle cx="60" cy="29" r="3" fill="#3b82f6" stroke={INK} strokeWidth="1.5" />
      </g>
    </Frame>
  );
}

/** [icon, name, art, look], in store order. The icon is the store item's key: one emoji, one code point. */
export const CHARACTERS: [string, string, (p: ArtProps) => ReturnType<typeof Lucky>, Look][] = [
  ['😊', 'Lucky', Lucky, 'female'],
  ['🐴', 'Cowgirl', Cowgirl, 'female'],
  ['🌸', 'Kunoichi', Kunoichi, 'female'],
  ['💍', 'Queen', Queen, 'female'],
  ['⚓', 'Pirate', PirateMan, 'male'],
  ['🦜', 'Pirate', Pirate, 'female'],
  ['🪐', 'Astronaut', AstronautMan, 'male'],
  ['🚀', 'Astronaut', Astronaut, 'female'],
  ['🪓', 'Viking', Viking, 'male'],
  ['🪖', 'Shieldmaiden', Shieldmaiden, 'female'],
  ['🏯', 'Samurai', Samurai, 'male'],
  ['🎎', 'Samurai', OnnaMusha, 'female'],
  ['🧙', 'Wizard', Wizard, 'male'],
  ['🧹', 'Witch', Witch, 'female'],
  ['🧛', 'Vampire', Vampire, 'male'],
  ['🩸', 'Vampire', Vampira, 'female'],
  ['🔱', 'Merman', Merman, 'male'],
  ['🧜', 'Mermaid', Mermaid, 'female'],
  ['🤴', 'Prince', Prince, 'male'],
  ['👸', 'Princess', Princess, 'female'],
];
for (const [icon, name, Svg, look] of CHARACTERS) registerArt(icon, { kind: 'avatar', name, Svg, look });
