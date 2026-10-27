// Scene / screen transitions: the cross-fade the shell and the match screen
// use to swap one scene for another without a hard cut.

import { useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { useLatestRef } from "./latest.ts";

export interface Crossfade<K> {
  /** The scene to render right now (lags `key` while fading out). */
  shown: K | null;
  /** True while a scene change is in flight. */
  fading: boolean;
  /**
   * True while the outgoing scene is dissolving away on the no-View-Transition
   * fallback. Put this on the scene layer's `opacity`; the blurred backdrop
   * shows through, so there is never a flat plate over the screen.
   */
  dimmed: boolean;
}

/** Browsers that can dissolve between the two scenes as composited snapshots. */
const supportsViewTransition = typeof document !== "undefined" && "startViewTransition" in document;

// Is a scene transition in flight? Expensive work that would jank the fade
// (Live2D's GL setup in particular) waits for this to go idle.
let idle = true;
const idleListeners = new Set<() => void>();

function setIdle(v: boolean): void {
  if (idle === v) return;
  idle = v;
  idleListeners.forEach((cb) => cb());
}

/** True when no scene cross-fade is running. */
export function isTransitionIdle(): boolean {
  return idle;
}

/** Resolves once no cross-fade is running, or after `maxWaitMs` regardless. */
export function whenTransitionIdle(maxWaitMs = 600): Promise<void> {
  if (idle) return Promise.resolve();
  return new Promise((res) => {
    const done = () => {
      clearTimeout(timer);
      idleListeners.delete(listener);
      res();
    };
    const listener = () => {
      if (idle) done();
    };
    const timer = setTimeout(done, maxWaitMs);
    idleListeners.add(listener);
  });
}

/**
 * Dissolve between successive `key`s. On a browser with the View Transitions
 * API the old scene stays live while `prepare` loads the new one, then both
 * are snapshotted and composited into each other -- no plate ever covers the
 * screen, so there is no blank step between the two. Elsewhere the scene layer
 * fades out to the backdrop, the swap runs under it, and it fades back in.
 *
 * `initial` is the scene already on screen (no dissolve out of it); without
 * one the first key appears after a single tick. A key that changes again
 * mid-transition restarts from whatever is still on screen -- the pending
 * swap is dropped.
 *
 * `eq` decides when two keys name the same scene (default `===`). A caller may
 * rebuild its key object every render (a route object, say); `eq` is what
 * keeps that from restarting the dissolve or -- worse -- clearing its timers
 * and leaving it half-done. While a dissolve is already running toward an `eq`
 * key, this effect leaves the timers alone.
 */
export function useCrossfade<K>(
  key: K | null,
  opts: {
    outMs?: number;
    inMs?: number;
    initial?: K | null;
    eq?: (a: K, b: K) => boolean;
    onSwap?: (key: K) => void;
    /** Awaited before the swap (preloading). The old scene stays up meanwhile. */
    prepare?: (key: K) => Promise<unknown>;
  } = {},
): Crossfade<K> {
  const { outMs = 180, inMs = 0 } = opts;
  const onSwap = useLatestRef(opts.onSwap);
  const prepare = useLatestRef(opts.prepare);
  const eq = useLatestRef(opts.eq ?? ((a: K, b: K) => a === b));
  const [shown, setShown] = useState<K | null>(() => opts.initial ?? null);
  const [fading, setFading] = useState(false);
  const [dimmed, setDimmed] = useState(false);
  // Read at effect time so the swap check does not put `shown` in the deps
  // (which would cancel the in-fade timer the moment it was scheduled).
  const shownRef = useLatestRef(shown);
  // The fade target and its timers live in refs: a re-render that rebuilds the
  // key object of the same scene must not cancel the in-flight fade.
  const pending = useRef<K | null>(null);
  const timers = useRef<{ swap: number; lift: number }>({ swap: 0, lift: 0 });

  // Publish "a transition is running" for `whenTransitionIdle`.
  useEffect(() => {
    setIdle(!fading);
  }, [fading]);

  useEffect(() => {
    if (key === null) return;
    const cur = shownRef.current;
    // Already on this scene: drop any stale pending dissolve and make sure it
    // is not left half-done (heals a swap that raced a re-render).
    if (cur !== null && eq.current(cur, key)) {
      pending.current = null;
      window.clearTimeout(timers.current.swap);
      window.clearTimeout(timers.current.lift);
      setFading(false);
      setDimmed(false);
      return;
    }
    // Already fading toward this same scene: leave the timers running.
    if (pending.current !== null && eq.current(pending.current, key)) return;

    // A different scene: cancel whatever was in flight and start over.
    window.clearTimeout(timers.current.swap);
    window.clearTimeout(timers.current.lift);
    pending.current = key;
    setFading(true);
    // The preload starts now and overlaps the dissolve; the swap waits for it.
    const ready = Promise.resolve(prepare.current?.(key)).catch(() => undefined);
    const swap = () => {
      onSwap.current?.(key);
      setShown(key);
      pending.current = null;
    };
    const settle = () => {
      setFading(false);
      setDimmed(false);
    };

    if (supportsViewTransition) {
      // The old scene stays on screen while the art loads (live, not frozen);
      // only the swap is snapshotted and dissolved into. No plate goes over the
      // screen at any point, so there is nothing blank in between.
      void ready.then(() => {
        if (pending.current !== key) return;
        const vt = document.startViewTransition(async () => {
          flushSync(swap);
          // Let the new scene paint before the browser snapshots it: a snapshot
          // taken on the commit alone can land on a half-built frame and the
          // dissolve would fade into that instead of into the scene.
          await Promise.race([
            new Promise<void>((res) => requestAnimationFrame(() => requestAnimationFrame(() => res()))),
            new Promise<void>((res) => setTimeout(res, 80)),
          ]);
        });
        // A transition started while another is running is skipped: `ready`
        // then rejects. Awaiting only `finished` leaves that rejection
        // unhandled (a source switch can remount the scene mid-fade).
        void vt.ready.catch(() => undefined);
        void vt.finished.then(settle, settle);
      });
    } else {
      // No snapshot dissolve: fade the scene layer out to the backdrop, swap
      // under it, fade back in. The backdrop is continuous, so the worst case
      // is a soft dissolve rather than a flat flash.
      setDimmed(true);
      timers.current.swap = window.setTimeout(() => {
        void ready.then(() => {
          if (pending.current !== key) return;
          swap();
          setDimmed(false);
          if (inMs <= 0) setFading(false);
          else timers.current.lift = window.setTimeout(settle, inMs);
        });
      }, cur === null ? 0 : outMs);
    }
  }, [key, outMs, inMs, onSwap, prepare, eq, shownRef]);

  // Unmount: stop the timers so a lift cannot fire into a dead tree.
  useEffect(() => () => {
    window.clearTimeout(timers.current.swap);
    window.clearTimeout(timers.current.lift);
  }, []);

  return { shown, fading, dimmed };
}