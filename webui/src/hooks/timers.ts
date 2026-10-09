// Timers and animation frames as semantic hooks. Each one owns its schedule
// (start / stop / re-arm) and keeps the callback in a latest-ref, so the
// cadence never depends on how often the component re-renders.

import { useEffect, useReducer, useRef } from "react";
import { useLatestRef } from "./latest.ts";
import { countdownLeft } from "./pure.ts";

export { countdownLeft };

/**
 * Call `fn` every `ms` while mounted (and while `ms` is not null). `null`
 * pauses the clock without tearing the hook down -- the countdown / banner
 * pattern: tick while there is something to count, idle otherwise.
 */
export function useInterval(fn: () => void, ms: number | null): void {
  const latest = useLatestRef(fn);
  useEffect(() => {
    if (ms === null) return;
    const id = window.setInterval(() => latest.current(), ms);
    return () => clearInterval(id);
  }, [ms, latest]);
}

/** Re-render every `ms` (timers, countdowns). Returns a tick counter. */
export function useTick(ms: number): number {
  const [n, bump] = useReducer((x: number) => x + 1, 0);
  useInterval(bump, ms);
  return n;
}

/**
 * Call `fn` once, `ms` from now (re-armed whenever `ms` or the identity of the
 * schedule changes). `null` cancels the pending call. Re-arming is the point:
 * a fade timer that must restart when its subject changes lives here.
 */
export function useTimeout(fn: () => void, ms: number | null): void {
  const latest = useLatestRef(fn);
  useEffect(() => {
    if (ms === null) return;
    const id = window.setTimeout(() => latest.current(), ms);
    return () => clearTimeout(id);
  }, [ms, latest]);
}

/**
 * Drive `fn(dtMs, timeMs)` from a `requestAnimationFrame` loop for as long as
 * the component is mounted. The loop owns the clock: the first frame's `dt` is
 * 0, later ones are the real gap (a throttled tab steps by whatever it got).
 */
export function useAnimationFrame(fn: (dtMs: number, timeMs: number) => void): void {
  const latest = useLatestRef(fn);
  useEffect(() => {
    let raf = 0;
    let last = performance.now();
    const tick = (t: number) => {
      raf = requestAnimationFrame(tick);
      const dt = t - last;
      last = t;
      latest.current(dt, t);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [latest]);
}

/**
 * Milliseconds left on a countdown to `deadline` (0 when there is none).
 * Re-renders about every `intervalMs` while time remains and stops when it
 * hits zero; the value is always read live from the clock at render time, so
 * any other re-render also refreshes it.
 */
export function useCountdown(deadline: number | null, intervalMs = 200): number {
  const [, pulse] = useReducer((x: number) => x + 1, 0);
  const left = deadline === null ? 0 : countdownLeft(deadline, Date.now());
  useInterval(pulse, deadline !== null && left > 0 ? intervalMs : null);
  return left;
}

/** `ms` from `performance.now()` -- the match clocks' epoch. */
export function perfDeadline(ms: number): number {
  return performance.now() + ms;
}

/**
 * Like `useCountdown` but on `performance.now()` (the match / animation
 * clocks). `deadline` is a `performance.now()` stamp; pass null to stop.
 */
export function usePerfCountdown(deadline: number | null, intervalMs = 200): number {
  const [, pulse] = useReducer((x: number) => x + 1, 0);
  const left = deadline === null ? 0 : countdownLeft(deadline, performance.now());
  useInterval(pulse, deadline !== null && left > 0 ? intervalMs : null);
  return left;
}

/** Restart the CSS animation on `key` without remounting the node. */
export function useRestartAnimation(key: unknown, ref: { current: HTMLElement | null }): void {
  const live = useLatestRef(ref);
  const k = useRef(key);
  useEffect(() => {
    if (!key || k.current === key) return;
    k.current = key;
    const el = live.current.current;
    if (!el) return;
    el.style.animation = "none";
    void el.offsetHeight; // force a reflow so the reset takes
    el.style.animation = "";
  }, [key, live]);
}