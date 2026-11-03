// The site is served at its frontend canister's own address and at custom
// domains. Internet Identity gives each address its own accounts, so on a
// custom domain players sign in as the canister address: the accounts they
// already have, whichever address they use. The canister lists the custom
// domains in public/.well-known/ii-alternative-origins (Internet Identity's
// permission) and public/.well-known/ic-domains (the HTTP gateways'); a test
// keeps both in step with CUSTOM_DOMAINS.

/** The mainnet frontend canister, where every account was made. Never change it: that would hand every player a new, empty account. */
export const CANISTER_ORIGIN = 'https://byggy-dyaaa-aaaad-aha7q-cai.icp0.io';

export const CUSTOM_DOMAINS = ['bulletspoker.com', 'www.bulletspoker.com'];

/** The address to sign in as when the page is served from `hostname`, or undefined to sign in as the page itself. */
export function derivationOriginFor(hostname: string): string | undefined {
  return CUSTOM_DOMAINS.includes(hostname) ? CANISTER_ORIGIN : undefined;
}
