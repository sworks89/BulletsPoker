const KEY = 'bp.ref';
const MAX_CODE = 32;

/** The `ref` code in a URL query string, trimmed; null when absent, empty or too long. */
export function readRefParam(search: string): string | null {
  const v = new URLSearchParams(search).get('ref')?.trim();
  return v && v.length <= MAX_CODE ? v : null;
}

/** Keeps a referral code until the visitor signs up. */
export function rememberRef(code: string): void {
  try {
    localStorage.setItem(KEY, code);
  } catch {
    /* storage unavailable: the code can still be entered by hand */
  }
}

/** The remembered code, once: it is cleared as it is read. */
export function takeRememberedRef(): string | null {
  try {
    const v = localStorage.getItem(KEY);
    localStorage.removeItem(KEY);
    return v;
  } catch {
    return null;
  }
}

export function referralLink(origin: string, code: string): string {
  return `${origin}/?ref=${encodeURIComponent(code)}`;
}
