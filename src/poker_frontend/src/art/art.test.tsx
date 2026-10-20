import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { artFor } from './registry';
import { Avatar } from './Avatar';
import { ItemArt } from './ItemArt';

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

  it('has art for every default gift and throwable', () => {
    for (const e of ['🍺', '☕', '🌹', '🍸', '🍕', '🚬', '🍾', '🏆']) expect(artFor(e)?.kind, e).toBe('gift');
    // ❄️ includes U+FE0F, exactly as the catalogue writes it.
    for (const e of ['🍅', '🥧', '❄️', '🐟', '💋', '💸', '💣']) expect(artFor(e)?.kind, e).toBe('throwable');
  });

  it('draws an unknown item as its emoji', () => {
    const html = renderToStaticMarkup(<ItemArt icon="🦖" />);
    expect(html).toContain('🦖');
    expect(html).not.toContain('<svg');
  });

  it('has a creature for every Fish Buffet tier', () => {
    for (let i = 0; i < 7; i++) {
      const key = `tier:${i}`;
      expect(artFor(key)?.kind, key).toBe('tier');
      const html = renderToStaticMarkup(
        <div>
          <Avatar icon={key} size="lg" />
          <Avatar icon={key} size="lg" />
        </div>,
      );
      expect(html, key).toContain('<svg');
      const ids = [...html.matchAll(/ id="([^"]+)"/g)].map((m) => m[1]);
      expect(new Set(ids).size, key).toBe(ids.length);
    }
    expect(artFor('tier:7')).toBeUndefined();
  });

  it('exposes the expression for CSS', () => {
    expect(renderToStaticMarkup(<Avatar icon="😎" expression="hit" />)).toContain('data-expr="hit"');
  });
});
