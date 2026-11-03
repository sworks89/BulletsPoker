import type { CSSProperties } from 'react';
import { CHIP_COLORS, chipColumns } from './chips';

const chipStyle = (tier: number, k: number) =>
  ({ '--c': CHIP_COLORS[tier], '--k': k, ...(tier === 0 ? { '--e': '#1d4ed8' } : {}) }) as CSSProperties;

/** Chips on the felt: stacks by value, with the amount beside them. */
export function ChipPile({
  amount,
  bigBlind,
  label,
  fx,
  className = '',
  style,
}: {
  amount: bigint;
  bigBlind: bigint;
  label?: string;
  fx?: string;
  className?: string;
  style?: CSSProperties;
}) {
  return (
    <div className={`tc-pile ${className}`} data-fx={fx} style={style}>
      <div className="tc-stack">
        {chipColumns(amount, bigBlind).map((c) => (
          <span key={c.tier} className="tc-col" style={{ '--n': c.n } as CSSProperties}>
            {Array.from({ length: c.n }, (_, k) => (
              <i key={k} className="tc-chip" style={chipStyle(c.tier, k)} />
            ))}
          </span>
        ))}
      </div>
      {label && <span className="tc-amt">{label}</span>}
    </div>
  );
}

/** One loose chip, for chips in flight. */
export function chipNode(tier: number): HTMLElement {
  const c = document.createElement('i');
  c.className = 'tc-chip';
  c.style.setProperty('--c', CHIP_COLORS[tier]);
  if (tier === 0) c.style.setProperty('--e', '#1d4ed8');
  return c;
}

/** The same markup as ChipPile, built outside React for chips in flight. */
export function pileNode(amount: bigint, bigBlind: bigint): HTMLElement {
  const pile = document.createElement('div');
  pile.className = 'tc-pile';
  const stack = document.createElement('div');
  stack.className = 'tc-stack';
  for (const col of chipColumns(amount, bigBlind)) {
    const el = document.createElement('span');
    el.className = 'tc-col';
    el.style.setProperty('--n', String(col.n));
    for (let k = 0; k < col.n; k++) {
      const c = chipNode(col.tier);
      c.style.setProperty('--k', String(k));
      el.appendChild(c);
    }
    stack.appendChild(el);
  }
  pile.appendChild(stack);
  return pile;
}
