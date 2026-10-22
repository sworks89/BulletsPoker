import { Principal } from '@icp-sdk/core/principal';

/**
 * Ledger ops stay "unknown" and are retried automatically for this long.
 * Mirrors RETRY_WINDOW_NS in src/poker_backend/src/wallet.rs (23 h, inside the ledger's 24 h dedup window).
 */
export const RETRY_WINDOW_MS = 23 * 60 * 60 * 1000;

/** ICRC-2 deposit: the wallet pays an approve fee, then the pull (transfer_from) fee on top of the amount. */
export function depositCost(amount: bigint, fee: bigint): { approveFee: bigint; pullFee: bigint; total: bigint } {
  return { approveFee: fee, pullFee: fee, total: amount + 2n * fee };
}

/** What an external deposit credits: the swept balance minus one ledger fee (never negative). */
export function sweepNet(amount: bigint, fee: bigint): bigint {
  return amount > fee ? amount - fee : 0n;
}

/** Checks a withdrawal before the review step: non-zero, and amount + fee within the balance. */
export function withdrawReview(amount: bigint, fee: bigint, balance: bigint): { ok: true; total: bigint } | { ok: false; error: string } {
  if (amount <= 0n) return { ok: false, error: 'Enter an amount greater than zero' };
  const total = amount + fee;
  if (total > balance) return { ok: false, error: 'Amount plus the ledger fee is more than your poker balance' };
  return { ok: true, total };
}

export function unknownOpState(op: { created_at_time: bigint }, nowMs: number): 'confirming' | 'needs_support' {
  const ageMs = nowMs - Number(op.created_at_time / 1_000_000n);
  return ageMs >= RETRY_WINDOW_MS ? 'needs_support' : 'confirming';
}

// ---- Destination parsing -------------------------------------------------

const B32 = 'abcdefghijklmnopqrstuvwxyz234567';

function crc32(data: Uint8Array): number {
  let crc = 0xffffffff;
  for (const b of data) {
    crc ^= b;
    for (let i = 0; i < 8; i++) crc = crc & 1 ? (crc >>> 1) ^ 0xedb88320 : crc >>> 1;
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function base32NoPad(data: Uint8Array): string {
  let out = '';
  let bits = 0;
  let acc = 0;
  for (const b of data) {
    acc = (acc << 8) | b;
    bits += 8;
    while (bits >= 5) {
      out += B32[(acc >>> (bits - 5)) & 31];
      bits -= 5;
    }
    acc &= (1 << bits) - 1;
  }
  if (bits > 0) out += B32[(acc << (5 - bits)) & 31];
  return out;
}

function checksum(owner: Principal, sub: Uint8Array): string {
  const o = owner.toUint8Array();
  const buf = new Uint8Array(o.length + 32);
  buf.set(o, 0);
  buf.set(sub, o.length);
  const c = crc32(buf);
  return base32NoPad(new Uint8Array([c >>> 24, (c >>> 16) & 255, (c >>> 8) & 255, c & 255]));
}

const isZero = (b: Uint8Array) => b.every((x) => x === 0);

function hexToBytes(hex: string): Uint8Array {
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(2 * i, 2 * i + 2), 16);
  return out;
}

/** All 32 subaccount bytes as 64 lowercase hex characters. */
export function subaccountHex(sub: Uint8Array | number[]): string {
  return Array.from(sub, (b) => b.toString(16).padStart(2, '0')).join('');
}

/** ICRC-1 textual account: `<principal>` when the subaccount is default, else `<principal>-<checksum>.<hex without leading zeros>`. */
export function icrcAccountText(owner: Principal, subaccount: Uint8Array | null): string {
  if (!subaccount || isZero(subaccount)) return owner.toText();
  const hex = Array.from(subaccount, (b) => b.toString(16).padStart(2, '0')).join('').replace(/^0+/, '');
  return `${owner.toText()}-${checksum(owner, subaccount)}.${hex}`;
}

export type Destination = { owner: Principal; subaccount: Uint8Array | null } | { error: string };

function normalized(owner: Principal, sub: Uint8Array | null): Destination {
  if (owner.isAnonymous()) return { error: 'The anonymous principal cannot receive funds' };
  return { owner, subaccount: sub && !isZero(sub) ? sub : null };
}

/**
 * Accepts a principal, a principal plus "." or a space plus 64 hex characters (the subaccount),
 * or the ICRC-1 textual account format. Legacy 64-hex ICP account ids are rejected.
 */
export function parseDestination(text: string): Destination {
  const t = text.trim();
  if (!t) return { error: 'Enter a destination principal' };
  if (/^[0-9a-fA-F]{64}$/.test(t)) return { error: "Legacy ICP account IDs aren't supported — use a principal" };
  const bad = { error: 'That is not a valid principal or ICRC-1 account' };

  const tryPrincipal = (s: string): Principal | null => {
    try {
      return Principal.fromText(s);
    } catch {
      return null;
    }
  };

  const split = t.match(/^(\S+)[\s.]\s*([0-9a-fA-F]{64})$/);
  if (split) {
    const owner = tryPrincipal(split[1]);
    if (owner) return normalized(owner, hexToBytes(split[2]));
  }

  const dot = t.indexOf('.');
  if (dot >= 0) {
    // ICRC-1: <principal>-<7 char checksum>.<hex subaccount, no leading zeros>
    const left = t.slice(0, dot);
    const hex = t.slice(dot + 1);
    const m = left.match(/^(.+)-([a-z2-7]{7})$/);
    if (!m || !/^[0-9a-fA-F]{1,64}$/.test(hex) || hex.startsWith('0')) return bad;
    const owner = tryPrincipal(m[1]);
    if (!owner) return bad;
    const sub = hexToBytes(hex.padStart(64, '0'));
    if (checksum(owner, sub) !== m[2]) return { error: 'The account checksum does not match — check for typos' };
    return normalized(owner, sub);
  }

  const owner = tryPrincipal(t);
  return owner ? normalized(owner, null) : bad;
}
