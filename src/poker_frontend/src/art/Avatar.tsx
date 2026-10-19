import { useId, type CSSProperties, type ReactNode } from 'react';
import { artFor, type Expression } from './registry';
import './avatars';

/** A `useId()` value made safe for svg ids and `url(#…)` references. */
export function useSvgUid(): string {
  return 'u' + useId().replace(/[^a-zA-Z0-9]/g, '');
}

function hash(s: string): number {
  let h = 0;
  for (const ch of s) h = (h * 31 + ch.charCodeAt(0)) | 0;
  return Math.abs(h);
}

/**
 * A player's avatar: the item's illustration, or its emoji when there is no art.
 * `overlay` is drawn inside the clipped circle (marks); `children` outside it (gift, note mark).
 */
export function Avatar({
  icon,
  size = 'md',
  expression,
  className = '',
  title,
  overlay,
  children,
}: {
  icon: string;
  size?: 'sm' | 'md' | 'lg';
  expression?: Expression;
  className?: string;
  title?: string;
  overlay?: ReactNode;
  children?: ReactNode;
}) {
  const uid = useSvgUid();
  const entry = artFor(icon);
  // Blink at a different moment on every avatar.
  const style = { '--blink-delay': `${(hash(uid) % 4000) / 1000}s` } as CSSProperties;
  return (
    <span className={`avatar ${size === 'md' ? '' : size} ${entry ? 'has-art' : ''} ${className}`} data-expr={expression} title={title}>
      {entry ? (
        <span className="avatar-art" style={style}>
          <entry.Svg uid={uid} />
          {overlay}
        </span>
      ) : (
        <>
          {icon}
          {overlay}
        </>
      )}
      {children}
    </span>
  );
}
