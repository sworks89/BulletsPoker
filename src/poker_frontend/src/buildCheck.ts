// Pure logic of the build badge: does the live game canister's module hash
// match a published GitHub Release? See docs/reproducible-builds.md.

export interface Release {
  tag: string;
  published_at: string;
  /** SHA-256 (lowercase hex) of poker_backend.wasm.gz, from the release body. */
  hash: string;
  /** The asset's `digest` from the API (hex without the `sha256:` prefix), if any. */
  assetDigest?: string;
  url: string;
  /** The body hash and the asset digest disagree: never counts as a match. */
  conflict: boolean;
}

export type BadgeState =
  | { kind: 'latest'; tag: string }
  | { kind: 'older'; tag: string; newest: string }
  | { kind: 'none' }
  | { kind: 'unknown'; reason: string }
  | { kind: 'local' };

const ASSET = 'poker_backend.wasm.gz';
/** Releases are built and published by .github/workflows/release.yml as this account. */
const CI_LOGIN = 'github-actions[bot]';
const HASH_LINE = /^poker_backend\.wasm\.gz sha256: ([0-9a-f]{64})\s*$/gm;
const HEX64 = /^[0-9a-f]{64}$/;

export function hexToBytes(hex: string): Uint8Array | null {
  if (hex.length % 2 !== 0 || !/^[0-9a-fA-F]*$/.test(hex)) return null;
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  return out;
}

export function bytesToHex(b: Uint8Array): string {
  return Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
}

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null;
const byCi = (v: unknown, key: string): boolean => isObj(v) && isObj(v[key]) && v[key].login === CI_LOGIN;

/** Exactly one hash line, otherwise null (ambiguity is not trusted). */
function bodyHash(body: unknown): string | null {
  if (typeof body !== 'string') return null;
  const found = [...body.matchAll(HASH_LINE)].map((m) => m[1]);
  return found.length === 1 ? found[0] : null;
}

export function parseReleases(json: unknown): Release[] {
  if (!Array.isArray(json)) return [];
  const out: Release[] = [];
  for (const r of json) {
    if (!isObj(r) || r.draft !== false || r.prerelease !== false) continue;
    if (!byCi(r, 'author')) continue; // a release made by hand is not a CI build
    const hash = bodyHash(r.body);
    if (!hash || typeof r.tag_name !== 'string' || typeof r.published_at !== 'string' || Number.isNaN(Date.parse(r.published_at))) continue;
    let assetDigest: string | undefined;
    let conflict = false;
    const asset = Array.isArray(r.assets) ? r.assets.find((a) => isObj(a) && a.name === ASSET) : undefined;
    if (!isObj(asset) || !byCi(asset, 'uploader')) continue; // the CI-uploaded wasm asset must be part of the release
    if (asset.digest !== undefined && asset.digest !== null) {
      const m = typeof asset.digest === 'string' ? /^sha256:([0-9a-f]{64})$/.exec(asset.digest) : null;
      if (m) {
        assetDigest = m[1];
        conflict = m[1] !== hash;
      } else {
        conflict = true; // a digest we cannot read is not a confirmation
      }
    }
    out.push({
      tag: r.tag_name,
      published_at: r.published_at,
      hash,
      ...(assetDigest ? { assetDigest } : {}),
      url: typeof r.html_url === 'string' ? r.html_url : '',
      conflict,
    });
  }
  return out;
}

export function classify(liveHash: Uint8Array | null, releases: Release[], network: 'ic' | 'local'): BadgeState {
  if (network === 'local') return { kind: 'local' };
  if (releases.length === 0) return { kind: 'unknown', reason: 'no published releases could be read' };
  if (!liveHash || liveHash.length !== 32) return { kind: 'unknown', reason: 'could not read the live module hash' };
  const live = bytesToHex(liveHash);
  const usable = releases
    .filter((r) => !r.conflict && HEX64.test(r.hash))
    .sort((a, b) => Date.parse(b.published_at) - Date.parse(a.published_at));
  const hit = usable.find((r) => r.hash === live);
  if (!hit) return { kind: 'none' };
  const newest = usable[0];
  return hit.tag === newest.tag ? { kind: 'latest', tag: hit.tag } : { kind: 'older', tag: hit.tag, newest: newest.tag };
}

export function shortPrincipal(p: string): string {
  return p.length > 20 ? `${p.slice(0, 8)}…${p.slice(-5)}` : p;
}

export function controllersText(principals: string[]): string {
  return principals.length ? principals.map(shortPrincipal).join(', ') : 'none';
}

export type ControllersView = { kind: 'unreadable' } | { kind: 'none' } | { kind: 'list'; items: string[] };

/** null means the read failed: never present that as "no controllers". */
export function controllersView(controllers: string[] | null): ControllersView {
  if (controllers === null) return { kind: 'unreadable' };
  return controllers.length === 0 ? { kind: 'none' } : { kind: 'list', items: controllers };
}

/** Controllers from the live read; null (unknown) whenever that read failed, whatever the releases did. */
export function controllersFromRead(
  r: PromiseSettledResult<{ controllers: string[] | null }>,
): string[] | null {
  return r.status === 'fulfilled' ? r.value.controllers : null;
}
