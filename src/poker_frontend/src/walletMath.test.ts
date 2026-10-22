import { describe, expect, it } from 'vitest';
import { Principal } from '@icp-sdk/core/principal';
import { depositCost, parseDestination, sweepNet, unknownOpState, RETRY_WINDOW_MS, withdrawReview, icrcAccountText, subaccountHex } from './walletMath';
import { prizeText, CHIPS, ICP } from './format';

const P = 'rrkah-fqaaa-aaaaa-aaaaq-cai';
const HEX = 'ab'.repeat(32);

describe('depositCost', () => {
  it('adds the approve fee and the pull fee on top of the amount', () => {
    expect(depositCost(100_000_000n, 10_000n)).toEqual({ approveFee: 10_000n, pullFee: 10_000n, total: 100_020_000n });
  });
});

describe('sweepNet', () => {
  it('is the amount minus one fee', () => {
    expect(sweepNet(100_010_000n, 10_000n)).toBe(100_000_000n);
  });
  it('is zero when the amount does not cover the fee', () => {
    expect(sweepNet(10_000n, 10_000n)).toBe(0n);
    expect(sweepNet(5n, 10_000n)).toBe(0n);
  });
});

describe('parseDestination', () => {
  it('accepts a plain principal', () => {
    const r = parseDestination(` ${P} `);
    expect('error' in r).toBe(false);
    if ('owner' in r) {
      expect(r.owner.toText()).toBe(P);
      expect(r.subaccount).toBeNull();
    }
  });
  it('accepts principal plus space or dot plus 64 hex', () => {
    for (const sep of [' ', '.']) {
      const r = parseDestination(`${P}${sep}${HEX}`);
      if (!('owner' in r)) throw new Error(r.error);
      expect(r.owner.toText()).toBe(P);
      expect(Array.from(r.subaccount!)).toEqual(Array(32).fill(0xab));
    }
  });
  it('rejects legacy 64-hex account ids', () => {
    expect(parseDestination(HEX)).toEqual({ error: "Legacy ICP account IDs aren't supported — use a principal" });
  });
  it('round-trips the ICRC-1 textual format', () => {
    const sub = new Uint8Array(32);
    sub[31] = 1;
    const text = icrcAccountText(Principal.fromText(P), sub);
    expect(text).toMatch(/^rrkah-fqaaa-aaaaa-aaaaq-cai-[a-z2-7]{7}\.1$/);
    const r = parseDestination(text);
    if (!('owner' in r)) throw new Error(r.error);
    expect(r.owner.toText()).toBe(P);
    expect(Array.from(r.subaccount!)).toEqual(Array.from(sub));
  });
  it('matches the ICRC-1 specification example', () => {
    const o = 'k2t6j-2nvnp-4zjm3-25dtz-6xhaa-c7boj-5gayf-oj3xs-i43lp-teztq-6ae';
    const sub = Uint8Array.from({ length: 32 }, (_, i) => i + 1);
    const text = `${o}-dfxgiyy.102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20`;
    expect(icrcAccountText(Principal.fromText(o), sub)).toBe(text);
    expect('owner' in parseDestination(text)).toBe(true);
  });
  it('rejects a bad ICRC-1 checksum', () => {
    const sub = new Uint8Array(32);
    sub[31] = 1;
    const text = icrcAccountText(Principal.fromText(P), sub).replace(/-[a-z2-7]{7}\./, '-aaaaaaa.');
    expect('error' in parseDestination(text)).toBe(true);
  });
  it('rejects garbage, empty text and the anonymous principal', () => {
    expect('error' in parseDestination('')).toBe(true);
    expect('error' in parseDestination('not a principal')).toBe(true);
    expect('error' in parseDestination('2vxsx-fae')).toBe(true);
  });
  it('treats an all-zero subaccount as none', () => {
    const r = parseDestination(`${P} ${'00'.repeat(32)}`);
    if (!('owner' in r)) throw new Error(r.error);
    expect(r.subaccount).toBeNull();
  });
});

describe('withdrawReview', () => {
  it('totals the amount and the fee, and checks the balance', () => {
    expect(withdrawReview(100n, 10n, 1000n)).toEqual({ ok: true, total: 110n });
    expect(withdrawReview(0n, 10n, 1000n)).toMatchObject({ ok: false });
    expect(withdrawReview(995n, 10n, 1000n)).toMatchObject({ ok: false });
    expect(withdrawReview(990n, 10n, 1000n)).toEqual({ ok: true, total: 1000n });
  });
});

describe('unknownOpState', () => {
  const created = 1_000_000n * 1_000_000_000n; // ns
  const createdMs = Number(created / 1_000_000n);
  it('is confirming inside the retry window', () => {
    expect(unknownOpState({ created_at_time: created }, createdMs + RETRY_WINDOW_MS - 1)).toBe('confirming');
  });
  it('needs support once the window has passed', () => {
    expect(unknownOpState({ created_at_time: created }, createdMs + RETRY_WINDOW_MS)).toBe('needs_support');
  });
  it('uses 23 hours like wallet.rs', () => {
    expect(RETRY_WINDOW_MS).toBe(23 * 3600 * 1000);
  });
});

describe('prizeText', () => {
  it('uses the tournament currency, not chips', () => {
    expect(prizeText(250_000_000n, ICP)).toBe('2.5 ICP');
    expect(prizeText(1500n, CHIPS)).toBe(`${(1500).toLocaleString()} chips`);
  });
});

describe('subaccountHex', () => {
  it('prints all 32 bytes as 64 lowercase hex characters', () => {
    const sub = new Uint8Array(32);
    sub[31] = 1;
    sub[0] = 0xab;
    expect(subaccountHex(sub)).toBe('ab' + '00'.repeat(30) + '01');
    expect(subaccountHex(Array.from(sub))).toHaveLength(64);
  });
  it('gives a deposit subaccount supplied as a plain number array a full ICRC-1 text', () => {
    const arr = Array.from({ length: 32 }, (_, i) => i + 1);
    const text = icrcAccountText(Principal.fromText(P), new Uint8Array(arr));
    expect(text).toMatch(/^rrkah-fqaaa-aaaaa-aaaaq-cai-[a-z2-7]{7}\.102030405/);
  });
});
