import { useEffect, useId, useRef, type KeyboardEvent, type ReactNode } from 'react';
import { FOCUSABLE, isCloseKey, movesFocusIn, restoresFocus, trapTarget } from './dialogKeys';

/**
 * Accessible modal: dialog role, labelled by its title, focus moves in on
 * open and is contained while open, Escape closes, and focus goes back to the
 * opener on close. Omit `onClose` for a dialog that must be completed.
 */
export function Dialog({
  title,
  onClose,
  children,
  hideTitle,
  bgClass = 'modal-bg',
  boxClass = 'modal',
  passive = false,
}: {
  title: string;
  onClose?: () => void;
  children: ReactNode;
  hideTitle?: boolean;
  bgClass?: string;
  boxClass?: string;
  /** For dialogs that open on their own: don't move focus in on open or back on close. */
  passive?: boolean;
}) {
  const titleId = useId();
  const box = useRef<HTMLDivElement>(null);
  // Read during the first render, before any child autofocus moves focus.
  const opener = useRef<Element | null>(typeof document === 'undefined' ? null : document.activeElement);

  useEffect(() => {
    const el = box.current;
    if (el && movesFocusIn(passive, el.contains(document.activeElement))) el.focus();
    const from = opener.current;
    return () => {
      if (restoresFocus(passive) && from instanceof HTMLElement && from.isConnected) from.focus();
    };
  }, []);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (isCloseKey(e.key) && onClose) {
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key !== 'Tab' || !box.current) return;
    const items = Array.from(box.current.querySelectorAll<HTMLElement>(FOCUSABLE));
    const active = document.activeElement as HTMLElement | null;
    const t = trapTarget(items, active, e.shiftKey, !!active && box.current.contains(active) && active !== box.current);
    if (t === null) return;
    e.preventDefault();
    (t === 'container' ? box.current : t).focus();
  };

  return (
    <div className={bgClass} onClick={onClose}>
      <div
        className={boxClass}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        ref={box}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={onKeyDown}
      >
        <h3 id={titleId} className={hideTitle ? 'sr-only' : undefined}>
          {title}
        </h3>
        {children}
      </div>
    </div>
  );
}
