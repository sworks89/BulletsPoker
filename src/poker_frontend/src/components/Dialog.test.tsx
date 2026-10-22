import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { Dialog } from './Dialog';
import { trapTarget, isCloseKey, movesFocusIn, restoresFocus } from './dialogKeys';

describe('Dialog markup', () => {
  const html = renderToStaticMarkup(
    <Dialog title="Join Table" onClose={() => {}}>
      <button>OK</button>
    </Dialog>,
  );
  it('has dialog role, aria-modal and a label pointing at the title', () => {
    expect(html).toContain('role="dialog"');
    expect(html).toContain('aria-modal="true"');
    const id = /aria-labelledby="([^"]+)"/.exec(html)?.[1];
    expect(id).toBeTruthy();
    expect(html).toMatch(new RegExp(`<h3 id="${id!.replace(/[:]/g, '\$&')}"[^>]*>Join Table</h3>`));
  });
  it('is focusable itself so focus can move in', () => {
    expect(html).toContain('tabindex="-1"');
  });
});

describe('dialog keys', () => {
  it('closes on Escape only', () => {
    expect(isCloseKey('Escape')).toBe(true);
    expect(isCloseKey('Enter')).toBe(false);
  });
  const a = {}, b = {}, c = {};
  const items = [a, b, c];
  it('wraps Tab from last to first and Shift+Tab from first to last', () => {
    expect(trapTarget(items, c, false, true)).toBe(a);
    expect(trapTarget(items, a, true, true)).toBe(c);
  });
  it('lets Tab move normally in the middle', () => {
    expect(trapTarget(items, b, false, true)).toBeNull();
    expect(trapTarget(items, b, true, true)).toBeNull();
  });
  it('pulls focus back in when it is outside the dialog', () => {
    expect(trapTarget(items, {}, false, false)).toBe(a);
    expect(trapTarget(items, {}, true, false)).toBe(c);
  });
  it('keeps focus on the container when nothing is focusable', () => {
    expect(trapTarget([], {}, false, false)).toBe('container');
  });
});

describe('activate key', () => {
  it('activates on Enter and Space only', async () => {
    const { isActivateKey } = await import('./dialogKeys');
    expect(isActivateKey('Enter')).toBe(true);
    expect(isActivateKey(' ')).toBe(true);
    expect(isActivateKey('a')).toBe(false);
  });
});

describe('focus on open', () => {
  it('moves focus in unless the dialog is passive or focus is already inside', () => {
    expect(movesFocusIn(false, false)).toBe(true);
    expect(movesFocusIn(false, true)).toBe(false);
    expect(movesFocusIn(true, false)).toBe(false);
  });
  it('restores focus on close only for active dialogs', () => {
    expect(restoresFocus(false)).toBe(true);
    expect(restoresFocus(true)).toBe(false);
  });
});
