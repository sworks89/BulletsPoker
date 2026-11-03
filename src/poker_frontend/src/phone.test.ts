import { describe, expect, it, vi } from 'vitest';
import { canGoFullscreen, hintDismissed, dismissHint, toggleFullscreen } from './phone';

const memory = () => {
  const m = new Map<string, string>();
  return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) };
};

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
