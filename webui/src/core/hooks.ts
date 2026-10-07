// React bindings for the plain stores (profile, settings, game session).

import { useEffect, useReducer, useRef, useState, useSyncExternalStore } from "react";
import { langVersion, onLangChange } from "../i18n";
import { getProfile, hasProfile, onProfile, settings } from "./store";
import type { MatchEvent, MatchView, PlayerProfile } from "./types";
import type { GameSession } from "../game/session";
import { isAuto, type AutoMode } from "../game/autopilot";

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

/**
 * Keep the screen on while `active` (a live match, from the ban / pick phase on) and
 * the 「保持屏幕常亮」 setting is on -- the Screen Wake Lock API. The browser
 * drops the lock whenever the page is hidden (another app, another tab, the
 * phone locked), so it is taken again each time the page becomes visible; it
 * never holds the screen on in the background. It needs a secure context
 * (HTTPS or localhost) and is a no-op where unsupported or refused (battery
 * saver), so the game behaves the same either way.
 */
export function useWakeLock(active: boolean): void {
  const on = active && settings().keepAwake;
  useSettings(); // re-run when the setting flips
  useEffect(() => {
    if (!on || !("wakeLock" in navigator)) return;
    let lock: WakeLockSentinel | null = null;
    let disposed = false;
    const take = async () => {
      if (disposed || lock || document.visibilityState !== "visible") return;
      try {
        const l = await navigator.wakeLock.request("screen");
        if (disposed) return void l.release();
        lock = l;
        l.addEventListener("release", () => {
          if (lock === l) lock = null;
        });
      } catch {
        // Refused (battery saver, policy, insecure context) -- carry on.
      }
    };
    const onVisible = () => {
      if (document.visibilityState === "visible") void take();
    };
    void take();
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      disposed = true;
      document.removeEventListener("visibilitychange", onVisible);
      void lock?.release();
      lock = null;
    };
  }, [on]);
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

/** 托管 / 混沌 / off for this session. */
export function useAutoMode(s: GameSession | null): AutoMode {
  const [mode, setMode] = useState<AutoMode>(s?.autoMode ?? "off");
  useEffect(() => {
    if (!s) return;
    setMode(s.autoMode);
    return s.subscribeAutoMode(() => setMode(s.autoMode));
  }, [s]);
  return mode;
}

/** True while either auto mode owns the seat -- or the session itself is
 *  read-only (a replay) -- the one input-lock every button reads. */
export function useAutoplay(s: GameSession | null): boolean {
  return isAuto(useAutoMode(s)) || !!s?.readOnly;
}

/** Re-render on language change (the shell keys its tree with this). */
export function useLangVersion(): number {
  return useSyncExternalStore(onLangChange, langVersion);
}
