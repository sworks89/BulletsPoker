import { describe, expect, it } from 'vitest';
import icDomains from '../public/.well-known/ic-domains?raw';
import alternativeOrigins from '../public/.well-known/ii-alternative-origins?raw';
import headers from '../public/_headers?raw';
import { CANISTER_ORIGIN, CUSTOM_DOMAINS, derivationOriginFor } from './domains';

describe('signing in on the custom domain', () => {
  it('signs in as the canister address, so players keep their accounts', () => {
    expect(derivationOriginFor('bulletspoker.com')).toBe(CANISTER_ORIGIN);
    expect(derivationOriginFor('www.bulletspoker.com')).toBe(CANISTER_ORIGIN);
  });

  it('leaves the canister address itself, and local development, as they are', () => {
    expect(derivationOriginFor('byggy-dyaaa-aaaad-aha7q-cai.icp0.io')).toBeUndefined();
    expect(derivationOriginFor('byggy-dyaaa-aaaad-aha7q-cai.icp.net')).toBeUndefined();
    expect(derivationOriginFor('poker_frontend.local.localhost')).toBeUndefined();
  });

  it('is the mainnet frontend canister', () => {
    expect(CANISTER_ORIGIN).toBe('https://byggy-dyaaa-aaaad-aha7q-cai.icp0.io');
  });
});

describe('the files that make the custom domain work', () => {
  it('lists every custom domain for the HTTP gateways', () => {
    expect(icDomains.trim().split(/\r?\n/)).toEqual(CUSTOM_DOMAINS);
  });

  it('lets Internet Identity accept every custom domain as the canister address', () => {
    expect(JSON.parse(alternativeOrigins)).toEqual({ alternativeOrigins: CUSTOM_DOMAINS.map((d) => `https://${d}`) });
  });

  it('serves the alternative origins as JSON that Internet Identity may read', () => {
    const block = headers.split(/\r?\n(?=\S)/).find((b) => b.startsWith('/.well-known/ii-alternative-origins'));
    expect(block).toMatch(/Content-Type: application\/json/);
    expect(block).toMatch(/Access-Control-Allow-Origin: \*/);
  });
});
