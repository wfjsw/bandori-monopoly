// Scene / screen transitions: the cross-fade the shell and the match screen
// use to swap one scene for another without a hard cut.

import { useEffect, useRef, useState } from "react";
import { useLatestRef } from "./latest.ts";

export interface Crossfade<K> {
  /** The scene to render right now (lags `key` while fading out). */
  shown: K | null;
  /** True while the fader should cover the swap. */
  fading: boolean;
}

/**
 * Cross-fade between successive `key`s. The outgoing scene stays under the
 * fader for `outMs`, then `onSwap` runs and `shown` flips to the new key; the
 * fader lifts `inMs` later. `initial` is the scene already on screen (no fade
 * out of it); without one the first key appears after a single tick and fades
 * in. A key that changes again mid-fade restarts from whatever is still on
 * screen -- the pending swap is dropped.
 *
 * `eq` decides when two keys name the same scene (default `===`). A caller may
 * rebuild its key object every render (a route object, say); `eq` is what
 * keeps that from restarting the fade or -- worse -- clearing the lift timer
 * and leaving the fader up. While a fade is already running toward an `eq`
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
  } = {},
): Crossfade<K> {
  const { outMs = 180, inMs = 0 } = opts;
  const onSwap = useLatestRef(opts.onSwap);
  const eq = useLatestRef(opts.eq ?? ((a: K, b: K) => a === b));
  const [shown, setShown] = useState<K | null>(() => opts.initial ?? null);
  const [fading, setFading] = useState(false);
  // Read at effect time so the swap check does not put `shown` in the deps
  // (which would cancel the in-fade timer the moment it was scheduled).
  const shownRef = useLatestRef(shown);
  // The fade target and its timers live in refs: a re-render that rebuilds the
  // key object of the same scene must not cancel the in-flight fade.
  const pending = useRef<K | null>(null);
  const timers = useRef<{ swap: number; lift: number }>({ swap: 0, lift: 0 });

  useEffect(() => {
    if (key === null) return;
    const cur = shownRef.current;
    // Already on this scene: drop any stale pending fade and make sure the
    // fader is down (heals a swap that raced a re-render).
    if (cur !== null && eq.current(cur, key)) {
      pending.current = null;
      window.clearTimeout(timers.current.swap);
      window.clearTimeout(timers.current.lift);
      setFading(false);
      return;
    }
    // Already fading toward this same scene: leave the timers running.
    if (pending.current !== null && eq.current(pending.current, key)) return;

    // A different scene: cancel whatever was in flight and start over.
    window.clearTimeout(timers.current.swap);
    window.clearTimeout(timers.current.lift);
    pending.current = key;
    setFading(true);
    timers.current.swap = window.setTimeout(() => {
      onSwap.current?.(key);
      setShown(key);
      pending.current = null;
      if (inMs <= 0) setFading(false);
      else timers.current.lift = window.setTimeout(() => setFading(false), inMs);
    }, cur === null ? 0 : outMs);
  }, [key, outMs, inMs, onSwap, eq, shownRef]);

  // Unmount: stop the timers so a lift cannot fire into a dead tree.
  useEffect(() => () => {
    window.clearTimeout(timers.current.swap);
    window.clearTimeout(timers.current.lift);
  }, []);

  return { shown, fading };
}