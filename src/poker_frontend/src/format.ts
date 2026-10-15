export const E8S = 100_000_000n;

/** Formats e8s as ICP, trimming trailing zeros. */
export function icp(e8s: bigint | number, maxDecimals = 8): string {
  const v = BigInt(e8s);
  const neg = v < 0n;
  const abs = neg ? -v : v;
  const whole = abs / E8S;
  let frac = (abs % E8S).toString().padStart(8, '0').slice(0, maxDecimals).replace(/0+$/, '');
  return `${neg ? '-' : ''}${whole.toLocaleString()}${frac ? '.' + frac : ''}`;
}

/** Parses a decimal ICP amount into e8s. */
export function parseIcp(s: string): bigint {
  const m = s.trim().match(/^(\d*)(?:\.(\d{0,8}))?$/);
  if (!m || (m[1] === '' && !m[2])) throw new Error('Enter a valid ICP amount');
  return BigInt(m[1] || '0') * E8S + BigInt((m[2] ?? '').padEnd(8, '0'));
}

const RANKS = '23456789TJQKA';
const SUITS = ['♣', '♦', '♥', '♠'];

export function cardRank(c: number): string {
  const r = RANKS[Math.floor(c / 4)];
  return r === 'T' ? '10' : r;
}

export function cardSuit(c: number): string {
  return SUITS[c % 4];
}

export function isRed(c: number): boolean {
  return c % 4 === 1 || c % 4 === 2;
}

export const bytes = (b: Uint8Array | number[]): number[] => Array.from(b);

export function shortPrincipal(p: { toText(): string }): string {
  const t = p.toText();
  return t.length > 16 ? `${t.slice(0, 5)}…${t.slice(-3)}` : t;
}

export function timeAgo(ns: bigint): string {
  const secs = Math.max(0, Math.floor((Date.now() - Number(ns / 1_000_000n)) / 1000));
  if (secs < 60) return 'now';
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  return `${Math.floor(secs / 3600)}h`;
}

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export type CurrencyV = { Icp: null } | { Chips: null };
export const ICP: CurrencyV = { Icp: null };
export const CHIPS: CurrencyV = { Chips: null };
export const isChips = (c: CurrencyV): boolean => 'Chips' in c;

/** Formats a table amount: e8s for ICP tables, whole chips for play-chip tables. */
export function amount(v: bigint | number, currency: CurrencyV, maxDecimals = 4): string {
  return isChips(currency) ? BigInt(v).toLocaleString() : icp(v, maxDecimals);
}

export function amountWithUnit(v: bigint | number, currency: CurrencyV): string {
  return isChips(currency) ? `${BigInt(v).toLocaleString()} chips` : `${icp(v)} ICP`;
}

/** Suit colors: classic red/black, or four-color (clubs green, diamonds blue). */
export function suitClass(c: number, fourColor: boolean): string {
  const s = c % 4;
  if (fourColor) return ['clubs4', 'diamonds4', 'red', ''][s];
  return isRed(c) ? 'red' : '';
}

export function countdown(targetNs: bigint): string {
  const ms = Number(targetNs / 1_000_000n) - Date.now();
  if (ms <= 0) return 'now';
  const h = Math.floor(ms / 3_600_000);
  const m = Math.floor((ms % 3_600_000) / 60_000);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}
