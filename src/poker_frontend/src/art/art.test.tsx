import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { artFor } from './registry';
import { Avatar } from './Avatar';
import { ItemArt } from './ItemArt';

const AVATARS = ['🙂', '😎', '🤖', '🤠', '👽', '🥷', '🦈', '🐉', '🦄', '👑'];
// The icons of the second set, exactly as the backend's catalogue writes them.
const CHARACTERS = ['😊', '🐴', '🌸', '💍', '⚓', '🦜', '🪐', '🚀', '🪓', '🪖', '🏯', '🎎', '🧙', '🧹', '🧛', '🩸', '🔱', '🧜', '🤴', '👸'];

describe('avatar art', () => {
  it('has art for every default avatar', () => {
    for (const e of [...AVATARS, ...CHARACTERS]) expect(artFor(e)?.kind, e).toBe('avatar');
  });

  it('keys every new avatar by a single code point, so the admin call cannot get it subtly wrong', () => {
    for (const e of CHARACTERS) expect([...e], e).toHaveLength(1);
  });

  it('says which version each avatar of a male and female pair is', () => {
    for (const e of ['🙂', '🤠', '🥷', '👑', '⚓', '🪐', '🪓', '🏯', '🧙', '🧛', '🔱', '🤴']) expect(artFor(e)?.look, e).toBe('male');
    for (const e of ['😊', '🐴', '🌸', '💍', '🦜', '🚀', '🪖', '🎎', '🧹', '🩸', '🧜', '👸']) expect(artFor(e)?.look, e).toBe('female');
    for (const e of ['😎', '🤖', '👽', '🦈', '🐉', '🦄']) expect(artFor(e)?.look, e).toBeUndefined();
  });

  it('falls back to the emoji for an unknown item', () => {
    const html = renderToStaticMarkup(<Avatar icon="🦖" size="sm" />);
    expect(html).toContain('🦖');
    expect(html).not.toContain('<svg');
  });

  it('gives each rendered avatar its own svg ids', () => {
    for (const e of [...AVATARS, ...CHARACTERS]) {
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
