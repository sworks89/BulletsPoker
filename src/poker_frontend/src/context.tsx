import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { authClient, makeActors, type Backend, type Ledger, type Profile, type StoreItem } from './ic';
import type { Principal } from '@icp-sdk/core/principal';
import { errorText } from './format';

export type View =
  | { name: 'lobby' }
  | { name: 'table'; id: bigint }
  | { name: 'wallet' }
  | { name: 'store' }
  | { name: 'friends' }
  | { name: 'leaderboard' }
  | { name: 'history' }
  | { name: 'profile' };

interface Toast {
  id: number;
  text: string;
  kind: 'ok' | 'err';
}

interface AppState {
  backend: Backend;
  ledger: Ledger;
  principal: Principal | null;
  profile: Profile | null;
  items: Map<number, StoreItem>;
  view: View;
  go: (v: View) => void;
  refreshProfile: () => Promise<void>;
  signIn: () => Promise<void>;
  signOut: () => Promise<void>;
  notify: (text: string, kind?: 'ok' | 'err') => void;
  /** Runs an async action, reporting failures as a toast. */
  run: <T>(f: () => Promise<T>, success?: string) => Promise<T | undefined>;
}

const Ctx = createContext<AppState | null>(null);

export function useApp(): AppState {
  const c = useContext(Ctx);
  if (!c) throw new Error('useApp outside provider');
  return c;
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [actors, setActors] = useState<{ backend: Backend; ledger: Ledger } | null>(null);
  const [principal, setPrincipal] = useState<Principal | null>(null);
  const [profile, setProfile] = useState<Profile | null>(null);
  const [items, setItems] = useState<Map<number, StoreItem>>(new Map());
  const [view, setView] = useState<View>({ name: 'lobby' });
  const [toasts, setToasts] = useState<Toast[]>([]);

  const notify = useCallback((text: string, kind: 'ok' | 'err' = 'ok') => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t, { id, text, kind }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 4000);
  }, []);

  const connect = useCallback(async () => {
    const identity = authClient.isAuthenticated() ? await authClient.getIdentity() : undefined;
    const a = await makeActors(identity);
    setActors(a);
    setPrincipal(identity ? identity.getPrincipal() : null);
    const [p, store] = await Promise.all([
      identity ? a.backend.get_my_profile() : Promise.resolve<[] | [Profile]>([]),
      a.backend.list_store(),
    ]);
    setProfile(p[0] ?? null);
    setItems(new Map(store.map((i) => [i.id, i])));
  }, []);

  useEffect(() => {
    connect().catch((e) => notify(errorText(e), 'err'));
  }, [connect, notify]);

  const value = useMemo<AppState | null>(() => {
    if (!actors) return null;
    const refreshProfile = async () => {
      const [p, store] = await Promise.all([actors.backend.get_my_profile(), actors.backend.list_store()]);
      setProfile(p[0] ?? null);
      setItems(new Map(store.map((i) => [i.id, i])));
    };
    return {
      ...actors,
      principal,
      profile,
      items,
      view,
      go: setView,
      refreshProfile,
      signIn: async () => {
        await authClient.signIn({ maxTimeToLive: BigInt(7 * 24 * 3600) * 1_000_000_000n });
        await connect();
      },
      signOut: async () => {
        await authClient.signOut();
        setView({ name: 'lobby' });
        await connect();
      },
      notify,
      run: async (f, success) => {
        try {
          const r = await f();
          if (success) notify(success);
          return r;
        } catch (e) {
          notify(errorText(e), 'err');
          return undefined;
        }
      },
    };
  }, [actors, principal, profile, items, view, connect, notify]);

  if (!value) {
    return <div className="splash">♠ ♥ Shuffling up… ♦ ♣</div>;
  }
  return (
    <Ctx.Provider value={value}>
      {children}
      <div className="toasts">
        {toasts.map((t) => (
          <div key={t.id} className={`toast ${t.kind}`}>
            {t.text}
          </div>
        ))}
      </div>
    </Ctx.Provider>
  );
}

/** Polls `f` every `ms` milliseconds while mounted. */
export function usePoll(f: () => Promise<void> | void, ms: number, deps: unknown[]) {
  useEffect(() => {
    let alive = true;
    let timer: ReturnType<typeof setTimeout>;
    const loop = async () => {
      try {
        await f();
      } catch {
        // transient network errors are retried on the next tick
      }
      if (alive) timer = setTimeout(loop, ms);
    };
    loop();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}
