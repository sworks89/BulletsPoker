import { artFor } from './registry';
import { useSvgUid } from './Avatar';
import './items';

/** A gift or throwable: its illustration, or its emoji when there is no art. Sized by the parent. */
export function ItemArt({ icon, className = '' }: { icon: string; className?: string }) {
  const uid = useSvgUid();
  const entry = artFor(icon);
  return <span className={`item-art ${entry ? 'has-art' : ''} ${className}`}>{entry ? <entry.Svg uid={uid} /> : icon}</span>;
}
