import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { authClient, makeActors, type Backend, type Ledger, type MedalDef, type Profile, type StoreItem } from './ic';
import type { Principal } from '@icp-sdk/core/principal';
import { errorText } from './format';
import { clearRememberedRef } from './referrals';
import { nextRetryMs, type ProfileLoad, type Startup } from './startup';

/** Internet Identity delegations last 12 hours: sign in again after that. */
const SESSION_TTL_NS = BigInt(12 * 3600) * 1_000_000_000n;
/** Sign out after this long without any input. */
const IDLE_SIGN_OUT_MS = 60 * 60_000;

export type View =
  | { name: 'lobby' }
  | { name: 'table'; id: bigint }
  | { name: 'wallet' }
  | { name: 'store' }
  | { name: 'friends' }
  | { name: 'rewards' }
  | { name: 'referrals' }
  /** The Rewards page, opened on its Leaderboards tab. */
  | { name: 'leaderboard' }
  | { name: 'history' }
  | { name: 'tournaments'; id?: bigint }
  | { name: 'profile' }
  | { name: 'admin' };

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
  /** Whether the profile lookup succeeded, found nothing, or failed. */
  profileLoad: ProfileLoad;
  retryProfile: () => void;
  items: Map<number, StoreItem>;
  medals: Map<string, MedalDef>;
  view: View;
  go: (v: View) => void;
  refreshProfile: () => Promise<void>;
  signIn: () => Promise<void>;
  signOut: (idle?: boolean) => Promise<void>;
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
  const [medals, setMedals] = useState<Map<string, MedalDef>>(new Map());
  const [view, setView] = useState<View>({ name: 'lobby' });
  const [toasts, setToasts] = useState<Toast[]>([]);

  const notify = useCallback((text: string, kind: 'ok' | 'err' = 'ok') => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t, { id, text, kind }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 4000);
  }, []);

  const [startup, setStartup] = useState<Startup>('connecting');
  const [startupError, setStartupError] = useState('');
  const [attempt, setAttempt] = useState(0);
  const [profileLoad, setProfileLoad] = useState<ProfileLoad>('unknown');
  const [profileAttempt, setProfileAttempt] = useState(0);
  // Only the newest connect/lookup may write state.
  const connectId = useRef(0);

  /** Looks the profile up. Only a successful empty answer means "no profile". */
  const loadProfile = useCallback(async (b: Backend, id: number) => {
    setProfileLoad('loading');
    try {
      const p = await b.get_my_profile();
      if (id !== connectId.current) return;
      setProfile(p[0] ?? null);
      setProfileLoad(p[0] ? 'found' : 'none');
      setProfileAttempt(0);
    } catch {
      if (id !== connectId.current) return;
      setProfileLoad('failed');
    }
  }, []);

  const connect = useCallback(async () => {
    const id = ++connectId.current;
    setStartup((s) => (s === 'ready' ? s : 'connecting'));
    try {
      const identity = authClient.isAuthenticated() ? await authClient.getIdentity() : undefined;
      const a = await makeActors(identity);
      const [store, defs] = await Promise.all([a.backend.list_store(), a.backend.list_medals()]);
      if (id !== connectId.current) return;
      setActors(a);
      setPrincipal(identity ? identity.getPrincipal() : null);
      setItems(new Map(store.map((i) => [i.id, i])));
      setMedals(new Map(defs.map((m) => [m.id, m])));
      setStartup('ready');
      setStartupError('');
      setAttempt(0);
      if (identity) {
        void loadProfile(a.backend, id);
      } else {
        setProfile(null);
        setProfileLoad('unknown');
      }
    } catch (e) {
      if (id !== connectId.current) return;
      setStartupError(errorText(e));
      setStartup('failed');
      setAttempt((n) => n + 1);
    }
  }, [loadProfile]);

  // Retry a failed connection on the backoff; the Retry button just does it sooner.
  useEffect(() => {
    if (startup !== 'failed') return;
    const t = setTimeout(() => void connect(), nextRetryMs(attempt - 1));
    return () => clearTimeout(t);
  }, [startup, attempt, connect]);

  const retryProfile = useCallback(() => {
    if (actors) void loadProfile(actors.backend, connectId.current);
  }, [actors, loadProfile]);

  useEffect(() => {
    if (profileLoad !== 'failed' || !actors) return;
    const t = setTimeout(() => {
      setProfileAttempt((n) => n + 1);
      void loadProfile(actors.backend, connectId.current);
    }, nextRetryMs(profileAttempt));
    return () => clearTimeout(t);
  }, [profileLoad, profileAttempt, actors, loadProfile]);

  // A visitor who already has a profile can't use a referral code: forget it.
  useEffect(() => {
    if (profile) clearRememberedRef();
  }, [profile]);

  useEffect(() => {
    void connect();
  }, [connect]);

  // Signing out forgets this browser's per-player state (open tables, layout)
  // and reloads, so nothing from the last session lingers in memory.
  const signOut = useCallback(async (idle = false) => {
    // Seats stay live on the server while you're away: say so before leaving them.
    if (!idle && actors) {
      const seats = await actors.backend.get_my_seats().catch(() => null);
      if (seats === null) {
        if (!confirm("Couldn't check your seats — sign out anyway?")) return;
      }
      const n = seats?.length ?? 0;
      if (
        n > 0 &&
        !confirm(
          `You still have ${n === 1 ? 'a seat' : `${n} seats`} at the table${n === 1 ? '' : 's'}. The blinds keep running and your hands are folded for you while you're signed out. Sign in again to get back to ${n === 1 ? 'it' : 'them'}. Sign out anyway?`,
        )
      )
        return;
    }
    await authClient.signOut();
    try {
      Object.keys(localStorage)
        .filter((k) => k.startsWith('bp.'))
        .forEach((k) => localStorage.removeItem(k));
    } catch {
      // storage unavailable: nothing to clear
    }
    window.location.reload();
  }, [actors]);

  // Idle sign-out: a signed-in session that sees no input for a while ends,
  // so an unattended or shared computer doesn't stay logged in to a wallet.
  useEffect(() => {
    if (!principal) return;
    let last = Date.now();
    const touch = () => {
      last = Date.now();
    };
    const events = ['pointerdown', 'keydown', 'wheel', 'touchstart'] as const;
    events.forEach((e) => window.addEventListener(e, touch, { passive: true }));
    const timer = setInterval(() => {
      if (Date.now() - last > IDLE_SIGN_OUT_MS) signOut(true);
    }, 30_000);
    return () => {
      events.forEach((e) => window.removeEventListener(e, touch));
      clearInterval(timer);
    };
  }, [principal, signOut]);

  const value = useMemo<AppState | null>(() => {
    if (!actors) return null;
    const refreshProfile = async () => {
      const [p, store] = await Promise.all([actors.backend.get_my_profile(), actors.backend.list_store()]);
      setProfile(p[0] ?? null);
      setProfileLoad(p[0] ? 'found' : 'none');
      setItems(new Map(store.map((i) => [i.id, i])));
    };
    return {
      ...actors,
      principal,
      profile,
      profileLoad,
      retryProfile,
      items,
      medals,
      view,
      go: setView,
      refreshProfile,
      signIn: async () => {
        await authClient.signIn({ maxTimeToLive: SESSION_TTL_NS });
        await connect();
      },
      signOut,
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
  }, [actors, principal, profile, profileLoad, retryProfile, items, medals, view, connect, notify, signOut]);

  const toastBox = (
    <div className="toasts" aria-live="polite" aria-relevant="additions text">
      {toasts.map((t) => (
        <div key={t.id} className={`toast ${t.kind}`} role={t.kind === 'err' ? 'alert' : 'status'}>
          {t.text}
        </div>
      ))}
    </div>
  );
  if (startup === 'failed' || !value) {
    return (
      <>
        {startup === 'failed' ? (
          <div className="splash failed" role="alert">
            <div>Couldn't connect to Bullets Poker</div>
            <p className="muted small">{startupError || 'Unknown error'}</p>
            <p className="muted small">Retrying automatically. You can also try now.</p>
            <button className="btn" onClick={() => void connect()}>
              Retry
            </button>
          </div>
        ) : (
          <div className="splash">♠ ♥ Shuffling up… ♦ ♣</div>
        )}
        {toastBox}
      </>
    );
  }
  return (
    <Ctx.Provider value={value}>
      {children}
      {toastBox}
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
