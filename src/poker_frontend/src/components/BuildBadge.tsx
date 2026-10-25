import { useEffect, useState } from 'react';
import { CanisterStatus, HttpAgent } from '@icp-sdk/core/agent';
import { safeGetCanisterEnv } from '@icp-sdk/core/agent/canister-env';
import { Principal } from '@icp-sdk/core/principal';
import { backendId } from '../ic';
import { classify, controllersView, hexToBytes, parseReleases, shortPrincipal, type BadgeState, type Release } from '../buildCheck';

const RELEASES_API = 'https://api.github.com/repos/sworks89/BulletsPoker/releases';
const DOC_URL = 'https://github.com/sworks89/BulletsPoker/blob/main/docs/reproducible-builds.md';
const CACHE_KEY = 'bp.releases.v1';
const CACHE_MS = 2 * 60 * 1000;

/** Same decision as ic.ts: a page served from localhost is a local network. */
function network(): 'ic' | 'local' {
  const h = location.hostname;
  return h === 'localhost' || h === '127.0.0.1' || h.endsWith('.localhost') ? 'local' : 'ic';
}

async function fetchReleases(): Promise<Release[]> {
  try {
    const raw = sessionStorage.getItem(CACHE_KEY);
    if (raw) {
      const c = JSON.parse(raw) as { at: number; json: unknown };
      if (Date.now() - c.at < CACHE_MS) return parseReleases(c.json);
    }
  } catch { /* storage unavailable or corrupt: fetch */ }
  const res = await fetch(RELEASES_API, { headers: { Accept: 'application/vnd.github+json' } });
  if (!res.ok) throw new Error(`GitHub ${res.status}`);
  const json: unknown = await res.json();
  if (!Array.isArray(json)) throw new Error('unexpected releases response');
  try {
    sessionStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), json }));
  } catch { /* ignore */ }
  return parseReleases(json);
}

/** Module hash and controllers from the IC's certified read_state, verified by the agent. */
async function readLive(net: 'ic' | 'local'): Promise<{ hash: Uint8Array | null; controllers: string[] | null }> {
  // On mainnet pin the agent's built-in IC root key rather than trusting the cookie.
  const rootKey = net === 'ic' ? undefined : safeGetCanisterEnv()?.IC_ROOT_KEY;
  const agent = await HttpAgent.create({ rootKey });
  const status = await CanisterStatus.request({
    canisterId: Principal.fromText(backendId),
    agent,
    paths: ['module_hash', 'controllers'],
  });
  const mh = status.get('module_hash');
  const ctl = status.get('controllers');
  return {
    hash: typeof mh === 'string' ? hexToBytes(mh) : null,
    controllers: Array.isArray(ctl) ? (ctl as Principal[]).map((p) => p.toText()) : null,
  };
}

function describe(s: BadgeState, releaseUrl: (tag: string) => string | undefined) {
  const link = (tag: string) => {
    const u = releaseUrl(tag);
    return u ? <a href={u} target="_blank" rel="noopener noreferrer">{tag}</a> : tag;
  };
  switch (s.kind) {
    case 'latest':
      return <>The game canister matches the latest published release {link(s.tag)}.</>;
    case 'older':
      return <>The game canister matches the published release {link(s.tag)}, but a newer one exists ({link(s.newest)}).</>;
    case 'none':
      return <>The game canister does not match any published release.</>;
    case 'unknown':
      return <>Could not check the game canister against the published releases right now.</>;
    case 'local':
      return <>Not checked on a local network.</>;
  }
}

/**
 * Footer badge: is the live game canister's code a published, reproducible
 * release? It is shown by this website and covers the game canister only.
 */
export function BuildBadge() {
  const [state, setState] = useState<BadgeState | null>(null);
  const [controllers, setControllers] = useState<string[] | null | undefined>(undefined);
  const [releases, setReleases] = useState<Release[]>([]);

  useEffect(() => {
    let alive = true;
    (async () => {
      const net = network();
      if (net === 'local') {
        if (alive) setState({ kind: 'local' });
        return;
      }
      try {
        if (!backendId) throw new Error('no backend id');
        const [live, rs] = await Promise.all([readLive(net), fetchReleases()]);
        if (!alive) return;
        setReleases(rs);
        setControllers(live.controllers);
        setState(classify(live.hash, rs, net));
      } catch (e) {
        if (alive) setState({ kind: 'unknown', reason: e instanceof Error ? e.message : 'error' });
      }
    })();
    return () => { alive = false; };
  }, []);

  if (!state) return null;
  const urlOf = (tag: string) => releases.find((r) => r.tag === tag)?.url || undefined;
  return (
    <div className={`build-badge build-${state.kind}`} data-testid="build-badge">
      <div>{describe(state, urlOf)}</div>
      {controllers !== undefined && state.kind !== 'local' && <ControllersLine controllers={controllers} />}
      <div>
        Shown by this website; covers the game canister only.{' '}
        <a href={DOC_URL} target="_blank" rel="noopener noreferrer">Check it yourself</a>
      </div>
    </div>
  );
}

function ControllersLine({ controllers }: { controllers: string[] | null }) {
  const v = controllersView(controllers);
  if (v.kind === 'unreadable') return <div>Couldn't read who can upgrade this code.</div>;
  if (v.kind === 'none') return <div>No controllers: this code can't be upgraded.</div>;
  return (
    <div>
      The owner can still upgrade this code (controllers:{' '}
      {v.items.map((p, i) => (
        <span key={p}>
          {i > 0 && ', '}
          <span title={p}>{shortPrincipal(p)}</span>
        </span>
      ))}
      ).
    </div>
  );
}
