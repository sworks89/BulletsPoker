import { describe, expect, it } from 'vitest';
import icDomains from '../public/.well-known/ic-domains?raw';
import alternativeOrigins from '../public/.well-known/ii-alternative-origins?raw';
import notificationSenders from '../public/.well-known/ii-notification-senders?raw';
import appMetadata from '../public/.well-known/ii-app-metadata?raw';
import icpYaml from '../../../icp.yaml?raw';
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

  it('sends alerts for the address accounts belong to, through mainnet Internet Identity', () => {
    expect(icpYaml).toContain(`notification_origin: "${CANISTER_ORIGIN}"`);
    expect(icpYaml).toContain('notification_sender: "rdmx6-jaaaa-aaaaa-aaadq-cai"');
  });

  it('names the game canister, and only it, as allowed to send alerts', () => {
    expect(JSON.parse(notificationSenders)).toEqual({ senders: ['b7ham-oaaaa-aaaad-aha7a-cai'] });
    const block = headers.split(/\r?\n(?=\S)/).find((b) => b.startsWith('/.well-known/ii-notification-senders'));
    expect(block).toMatch(/Content-Type: application\/json/);
  });

  it('tells Internet Identity the name and logo to show on alerts and at sign-in', () => {
    const metadata = JSON.parse(appMetadata);
    expect(metadata.name).toBe('Bullets Poker');
    expect(Object.keys(import.meta.glob('../public/*', { query: '?url' }))).toContain(`../public${metadata.logo}`);
    const block = headers.split(/\r?\n(?=\S)/).find((b) => b.startsWith('/.well-known/ii-app-metadata'));
    expect(block).toMatch(/Content-Type: application\/json/);
    expect(block).toMatch(/Access-Control-Allow-Origin: \*/);
  });

  it('serves the alternative origins as JSON that Internet Identity may read', () => {
    const block = headers.split(/\r?\n(?=\S)/).find((b) => b.startsWith('/.well-known/ii-alternative-origins'));
    expect(block).toMatch(/Content-Type: application\/json/);
    expect(block).toMatch(/Access-Control-Allow-Origin: \*/);
  });
});
