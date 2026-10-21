const KEY = 'bp.ref';
const MAX_CODE = 32;

/** The `ref` code in a URL query string, trimmed; null when absent, empty or too long. */
export function readRefParam(search: string): string | null {
  const v = new URLSearchParams(search).get('ref')?.trim();
  return v && v.length <= MAX_CODE ? v : null;
}

/** A remembered code is ignored once it is this old. */
export const REF_MAX_AGE_MS = 30 * 24 * 3_600 * 1000;

/** Keeps a referral code (and when it was seen) until the visitor signs up. */
export function rememberRef(code: string, nowMs: number = Date.now()): void {
  try {
    localStorage.setItem(KEY, JSON.stringify({ code, at: nowMs }));
  } catch {
    /* storage unavailable: the code can still be entered by hand */
  }
}

/** Forgets the remembered code. */
export function clearRememberedRef(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    /* nothing to clear */
  }
}

/**
 * The remembered code without consuming it. A code older than `REF_MAX_AGE_MS`
 * is dropped, and so is one in the old plain-string format (no timestamp, so
 * its age is unknown: treated as expired).
 */
export function peekRememberedRef(nowMs: number = Date.now()): string | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw === null) return null;
    let code: string | null = null;
    try {
      const v = JSON.parse(raw);
      if (v && typeof v.code === 'string' && typeof v.at === 'number' && nowMs - v.at <= REF_MAX_AGE_MS) {
        code = v.code;
      }
    } catch {
      /* legacy plain string */
    }
    if (code === null) localStorage.removeItem(KEY);
    return code;
  } catch {
    return null;
  }
}

/** The remembered code, once: it is cleared as it is read. */
export function takeRememberedRef(nowMs: number = Date.now()): string | null {
  const code = peekRememberedRef(nowMs);
  clearRememberedRef();
  return code;
}

export function referralLink(origin: string, code: string): string {
  return `${origin}/?ref=${encodeURIComponent(code)}`;
}
