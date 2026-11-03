import { describe, expect, it, vi } from 'vitest';
import css from './styles.css?raw';
import { PHONE_QUERY, SIDEWAYS_QUERY, UPRIGHT_PHONE_QUERY, canGoFullscreen, layoutFor, hintDismissed, dismissHint, toggleFullscreen } from './phone';

const memory = () => {
  const m = new Map<string, string>();
  return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) };
};

describe('phone queries', () => {
  it('apply to touch screens only, so a short laptop window keeps the desktop layout', () => {
    expect(SIDEWAYS_QUERY).toContain('(pointer: coarse)');
    expect(UPRIGHT_PHONE_QUERY).toContain('(pointer: coarse)');
  });

  it('match the copies in styles.css', () => {
    expect(css).toContain(`@media ${SIDEWAYS_QUERY}`);
    expect(css).toContain(`@media ${UPRIGHT_PHONE_QUERY}`);
  });
});

describe('table layout on phones', () => {
  it('is always tabs on a phone, whatever was saved', () => {
    expect(layoutFor('tile', true)).toBe('tabs');
    expect(layoutFor('tabs', true)).toBe('tabs');
  });

  it('follows the saved choice elsewhere', () => {
    expect(layoutFor('tile', false)).toBe('tile');
    expect(layoutFor('tabs', false)).toBe('tabs');
  });

  it('counts a phone either way up', () => {
    expect(PHONE_QUERY).toBe(`${SIDEWAYS_QUERY}, ${UPRIGHT_PHONE_QUERY}`);
  });
});

describe('rotate hint', () => {
  it('shows until dismissed, then stays dismissed', () => {
    const s = memory();
    expect(hintDismissed(s)).toBe(false);
    dismissHint(s);
    expect(hintDismissed(s)).toBe(true);
  });

  it('shows when storage is unavailable (private mode)', () => {
    const broken = {
      getItem: () => {
        throw new Error('blocked');
      },
      setItem: () => {
        throw new Error('blocked');
      },
    };
    expect(hintDismissed(broken)).toBe(false);
    expect(() => dismissHint(broken)).not.toThrow();
  });
});

describe('fullscreen', () => {
  const page = (inFullscreen: boolean, lock = vi.fn(() => Promise.resolve())) => {
    const doc = {
      fullscreenEnabled: true,
      fullscreenElement: inFullscreen ? {} : null,
      documentElement: { requestFullscreen: vi.fn(() => Promise.resolve()) },
      exitFullscreen: vi.fn(() => Promise.resolve()),
    };
    return { doc, orientation: { lock } };
  };

  it('goes fullscreen and turns the screen sideways', async () => {
    const { doc, orientation } = page(false);
    await toggleFullscreen(doc, orientation);
    expect(doc.documentElement.requestFullscreen).toHaveBeenCalled();
    expect(orientation.lock).toHaveBeenCalledWith('landscape');
  });

  it('stays fullscreen when the screen cannot be locked sideways', async () => {
    const { doc, orientation } = page(false, vi.fn(() => Promise.reject(new Error('not supported'))));
    await expect(toggleFullscreen(doc, orientation)).resolves.toBeUndefined();
    expect(doc.documentElement.requestFullscreen).toHaveBeenCalled();
  });

  it('goes fullscreen where the screen has no orientation lock (iPad, Firefox)', async () => {
    const { doc } = page(false);
    await toggleFullscreen(doc);
    await toggleFullscreen(doc, {});
    expect(doc.documentElement.requestFullscreen).toHaveBeenCalledTimes(2);
  });

  it('fails quietly when the browser refuses fullscreen', async () => {
    const { doc, orientation } = page(false);
    doc.documentElement.requestFullscreen = vi.fn(() => Promise.reject(new Error('denied')));
    await expect(toggleFullscreen(doc, orientation)).resolves.toBeUndefined();
    expect(orientation.lock).not.toHaveBeenCalled();
  });

  it('leaves fullscreen when already in it', async () => {
    const { doc, orientation } = page(true);
    await toggleFullscreen(doc, orientation);
    expect(doc.exitFullscreen).toHaveBeenCalled();
    expect(doc.documentElement.requestFullscreen).not.toHaveBeenCalled();
  });

  it('is offered only on touch screens whose browser allows it (not iPhones)', () => {
    expect(canGoFullscreen({ fullscreenEnabled: true }, true)).toBe(true);
    expect(canGoFullscreen({ fullscreenEnabled: false }, true)).toBe(false);
    expect(canGoFullscreen({ fullscreenEnabled: true }, false)).toBe(false);
  });
});
