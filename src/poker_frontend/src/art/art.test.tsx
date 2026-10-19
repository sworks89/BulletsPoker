import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { artFor } from './registry';
import { Avatar } from './Avatar';

const AVATARS = ['🙂', '😎', '🤖', '🤠', '👽', '🥷', '🦈', '🐉', '🦄', '👑'];

describe('avatar art', () => {
  it('has art for every default avatar', () => {
    for (const e of AVATARS) expect(artFor(e)?.kind, e).toBe('avatar');
  });

  it('falls back to the emoji for an unknown item', () => {
    const html = renderToStaticMarkup(<Avatar icon="🦖" size="sm" />);
    expect(html).toContain('🦖');
    expect(html).not.toContain('<svg');
  });

  it('gives each rendered avatar its own svg ids', () => {
    for (const e of AVATARS) {
      const html = renderToStaticMarkup(
        <div>
          <Avatar icon={e} />
          <Avatar icon={e} />
        </div>,
      );
      const ids = [...html.matchAll(/ id="([^"]+)"/g)].map((m) => m[1]);
      expect(new Set(ids).size, e).toBe(ids.length);
    }
  });

  it('exposes the expression for CSS', () => {
    expect(renderToStaticMarkup(<Avatar icon="😎" expression="hit" />)).toContain('data-expr="hit"');
  });
});
