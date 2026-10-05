// React bindings for the plain stores (profile, settings, game session).

import { useEffect, useReducer, useRef, useState, useSyncExternalStore } from "react";
import { langVersion, onLangChange } from "../i18n";
import { getProfile, hasProfile, onProfile, settings } from "./store";
import type { MatchEvent, MatchView, PlayerProfile } from "./types";
import type { GameSession } from "../game/session";

const subscribeProfile = (cb: () => void) => onProfile(cb);
let profileVersion = 0;
onProfile(() => profileVersion++);

/** The saved profile (re-renders on every change), or null before one exists. */
export function useProfile(): PlayerProfile | null {
  useSyncExternalStore(subscribeProfile, () => profileVersion);
  return hasProfile() ? getProfile() : null;
}

export function useSettings() {
  useSyncExternalStore(subscribeProfile, () => profileVersion);
  return settings();
}

/** Re-render every `ms` (timers, countdowns). */
export function useTick(ms: number): number {
  const [n, bump] = useReducer((x: number) => x + 1, 0);
  useEffect(() => {
    const t = window.setInterval(bump, ms);
    return () => clearInterval(t);
  }, [ms]);
  return n;
}

/** The session's current match view plus when it arrived (for local countdowns). */
export function useMatchView(s: GameSession | null, onEvent?: (e: MatchEvent) => void): { view: MatchView | null; at: number } {
  const [state, set] = useState<{ view: MatchView | null; at: number }>(() => ({ view: s?.view ?? null, at: performance.now() }));
  const evRef = useRef(onEvent);
  evRef.current = onEvent;
  useEffect(() => {
    if (!s) return;
    return s.subscribe((v) => set({ view: v, at: performance.now() }), (e) => evRef.current?.(e));
  }, [s]);
  return state;
}

/** Re-render on room / connection changes of an online session. */
export function useSessionOther(s: GameSession | null): number {
  const [n, bump] = useReducer((x: number) => x + 1, 0);
  useEffect(() => (s ? s.subscribe(() => undefined, undefined, bump) : undefined), [s]);
  return n;
}

/** Re-render on language change (the shell keys its tree with this). */
export function useLangVersion(): number {
  return useSyncExternalStore(onLangChange, langVersion);
}
