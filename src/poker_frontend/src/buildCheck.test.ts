import { describe, expect, it } from 'vitest';
import { classify, controllersFromRead, controllersText, controllersView, hexToBytes, parseReleases, type Release } from './buildCheck';

import fixture from './buildCheck.fixture.json';

// buildCheck.fixture.json is a GitHub API release object whose body was
// rendered by the real scripts/release-notes.sh, so this suite breaks if the
// script's output and the badge's parser drift apart. To regenerate it (WSL):
//   D=$(mktemp -d); printf '\0asm\1\0\0\0' | gzip -n -9 > $D/poker_backend.wasm.gz
//   (cd $D && sha256sum poker_backend.wasm.gz > poker_backend.wasm.gz.sha256)
//   COMMIT=0123456789abcdef0123456789abcdef01234567 bash scripts/release-notes.sh v1.0.0 $D
// then put the output in "body" and the hash in the asset "digest".
const BOT = { login: 'github-actions[bot]' };
const H1 = 'a'.repeat(64);
const H2 = 'b'.repeat(64);
const H3 = 'c'.repeat(64);

const rel = (tag: string, hash: string, over: Record<string, unknown> = {}) => ({
  tag_name: tag,
  draft: false,
  prerelease: false,
  published_at: '2026-01-01T00:00:00Z',
  html_url: `https://github.com/sworks89/BulletsPoker/releases/tag/${tag}`,
  body: `Notes\n\npoker_backend.wasm.gz sha256: ${hash}\n`,
  author: { login: 'github-actions[bot]' },
  assets: [{ name: 'poker_backend.wasm.gz', digest: `sha256:${hash}`, uploader: { login: 'github-actions[bot]' } }],
  ...over,
});

const bytes = (hex: string) => hexToBytes(hex)!;

describe('parseReleases', () => {
  it('reads the hash line, tag, date and url', () => {
    const [r] = parseReleases([rel('v1', H1)]);
    expect(r).toMatchObject({ tag: 'v1', hash: H1, published_at: '2026-01-01T00:00:00Z', conflict: false });
    expect(r.url).toContain('/releases/tag/v1');
  });
  it('ignores drafts and pre-releases', () => {
    expect(parseReleases([rel('v1', H1, { draft: true }), rel('v2', H2, { prerelease: true })])).toEqual([]);
  });
  it('skips releases with a missing or malformed hash line', () => {
    const bad = [
      rel('v1', H1, { body: 'no line here' }),
      rel('v2', H1, { body: 'poker_backend.wasm.gz sha256: ' + 'A'.repeat(64) }),
      rel('v3', H1, { body: 'poker_backend.wasm.gz sha256: ' + 'a'.repeat(63) }),
      rel('v4', H1, { body: 'poker_backend.wasm.gz sha256: ' + 'a'.repeat(65) }),
      rel('v5', H1, { body: null }),
      rel('v6', H1, { body: `x poker_backend.wasm.gz sha256: ${H1}` }),
      rel('v7', H1, { body: `poker_backend.wasm.gz sha256: ${H1}\npoker_backend.wasm.gz sha256: ${H2}` }),
    ];
    expect(parseReleases(bad)).toEqual([]);
  });
  it('accepts CRLF bodies', () => {
    expect(parseReleases([rel('v1', H1, { body: `a\r\npoker_backend.wasm.gz sha256: ${H1}\r\n` })])).toHaveLength(1);
  });
  it('marks a body/digest disagreement as a conflict', () => {
    const [r] = parseReleases([rel('v1', H1, { assets: [{ name: 'poker_backend.wasm.gz', digest: `sha256:${H2}`, uploader: BOT }] })]);
    expect(r.conflict).toBe(true);
    expect(r.assetDigest).toBe(H2);
  });
  it('accepts a release with no digest', () => {
    const [r] = parseReleases([rel('v1', H1, { assets: [{ name: 'poker_backend.wasm.gz', uploader: BOT }] })]);
    expect(r.conflict).toBe(false);
  });
  it('tolerates garbage', () => {
    expect(parseReleases(null)).toEqual([]);
    expect(parseReleases({ message: 'rate limited' })).toEqual([]);
    expect(parseReleases([1, null, 'x', {}])).toEqual([]);
  });
});

describe('classify', () => {
  const releases = (): Release[] =>
    parseReleases([
      rel('v1', H1, { published_at: '2026-01-01T00:00:00Z' }),
      rel('v2', H2, { published_at: '2026-02-01T00:00:00Z' }),
    ]);

  it('latest', () => {
    expect(classify(bytes(H2), releases(), 'ic')).toEqual({ kind: 'latest', tag: 'v2' });
  });
  it('older, naming the newest', () => {
    expect(classify(bytes(H1), releases(), 'ic')).toEqual({ kind: 'older', tag: 'v1', newest: 'v2' });
  });
  it('none', () => {
    expect(classify(bytes(H3), releases(), 'ic')).toEqual({ kind: 'none' });
  });
  it('a conflicted release never matches', () => {
    const rs = parseReleases([rel('v1', H1, { assets: [{ name: 'poker_backend.wasm.gz', digest: `sha256:${H2}`, uploader: BOT }] })]);
    expect(classify(bytes(H1), rs, 'ic')).toEqual({ kind: 'none' });
    expect(classify(bytes(H2), rs, 'ic')).toEqual({ kind: 'none' });
  });
  it('unknown without a live hash', () => {
    expect(classify(null, releases(), 'ic').kind).toBe('unknown');
  });
  it('local is not checked', () => {
    expect(classify(bytes(H1), releases(), 'local')).toEqual({ kind: 'local' });
    expect(classify(null, [], 'local')).toEqual({ kind: 'local' });
  });
});

describe('controllersText', () => {
  it('shortens long principals and joins them', () => {
    const p = 'aaaaa-aaaaa-aaaaa-aaaaa-aaaaa-aaaaa-aaaaa-aaaaa-aaaaa-aaaaa-aae';
    expect(controllersText([p, 'ryjl3-tyaaa-aaaaa-aaaba-cai'])).toBe('aaaaa-aa…a-aae, ryjl3-ty…a-cai');
  });
  it('handles none', () => {
    expect(controllersText([])).toBe('none');
  });
});

describe('controllersView', () => {
  it('an unreadable result is not "none"', () => {
    expect(controllersView(null)).toEqual({ kind: 'unreadable' });
  });
  it('a genuinely empty list is its own case', () => {
    expect(controllersView([])).toEqual({ kind: 'none' });
  });
  it('lists controllers', () => {
    expect(controllersView(['x'])).toEqual({ kind: 'list', items: ['x'] });
  });
});

describe('stricter release parsing', () => {
  it('requires the wasm asset', () => {
    expect(parseReleases([rel('v1', H1, { assets: [] }), rel('v2', H2, { assets: [{ name: 'other.did' }] }), rel('v3', H3, { assets: undefined })])).toEqual([]);
  });
  it('skips a malformed published_at', () => {
    expect(parseReleases([rel('v1', H1, { published_at: 'not a date' })])).toEqual([]);
  });
  it('no parsed releases is unknown, not none', () => {
    expect(classify(bytes(H1), [], 'ic').kind).toBe('unknown');
  });
});

describe('only releases built by CI count', () => {
  it('ignores a release authored by a person', () => {
    expect(parseReleases([rel('v1', H1, { author: { login: 'someone' } })])).toEqual([]);
  });
  it('ignores a release with no author', () => {
    expect(parseReleases([rel('v1', H1, { author: undefined }), rel('v2', H2, { author: null })])).toEqual([]);
  });
  it('ignores a release whose wasm asset was uploaded by a person', () => {
    const asset = { name: 'poker_backend.wasm.gz', digest: `sha256:${H1}`, uploader: { login: 'someone' } };
    expect(parseReleases([rel('v1', H1, { assets: [asset] })])).toEqual([]);
  });
  it('ignores a wasm asset with no uploader', () => {
    expect(parseReleases([rel('v1', H1, { assets: [{ name: 'poker_backend.wasm.gz', digest: `sha256:${H1}` }] })])).toEqual([]);
  });
  it('accepts the CI bot', () => {
    expect(parseReleases([rel('v1', H1)])).toHaveLength(1);
  });
});

describe('controllersFromRead', () => {
  it('uses the controllers when the live read worked', () => {
    expect(controllersFromRead({ status: 'fulfilled', value: { controllers: ['x'] } })).toEqual(['x']);
  });
  it('is unknown when the live read failed', () => {
    expect(controllersFromRead({ status: 'rejected', reason: new Error('x') })).toBeNull();
  });
  it('is unknown when the read had no readable list', () => {
    expect(controllersFromRead({ status: 'fulfilled', value: { controllers: null } })).toBeNull();
  });
});

describe('release body rendered by scripts/release-notes.sh', () => {
  it('is accepted and its hash extracted', () => {
    const [r, ...rest] = parseReleases(fixture);
    expect(rest).toEqual([]);
    expect(r).toMatchObject({ tag: 'v1.0.0', hash: fixture[0].assets[0].digest.slice('sha256:'.length), conflict: false });
    expect(r.hash).toMatch(/^[0-9a-f]{64}$/);
  });
});
