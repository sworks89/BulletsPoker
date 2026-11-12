/**
 * The committed `public/_headers` is what mainnet serves: its CSP allows API
 * calls to IC boundary nodes only. A local network serves the page and its API
 * from different localhost origins, so a local build (`BP_LOCAL_NETWORK=1`,
 * set by `scripts/deploy-local.sh`) adds those to `connect-src`.
 */
export const LOCAL_CONNECT_SOURCES = ['http://localhost:8000', 'http://127.0.0.1:8000', 'http://*.localhost:8000'];

export function withLocalConnectSources(headers: string): string {
  const out = headers.replace(/connect-src 'self'/, `connect-src 'self' ${LOCAL_CONNECT_SOURCES.join(' ')}`);
  if (out === headers) throw new Error('_headers has no connect-src to extend');
  return out;
}
