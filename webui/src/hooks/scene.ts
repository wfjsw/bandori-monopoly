// Scene / screen transitions: the cross-fade the shell and the match screen
// use to swap one scene for another without a hard cut.

import { useEffect, useState } from "react";
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
 * `eq` decides when two keys name the same scene (default `===`).
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

  useEffect(() => {
    if (key === null) return;
    const cur = shownRef.current;
    if (cur !== null && eq.current(cur, key)) return;
    setFading(true);
    let lift = 0;
    const swap = window.setTimeout(() => {
      onSwap.current?.(key);
      setShown(key);
      if (inMs <= 0) setFading(false);
      else lift = window.setTimeout(() => setFading(false), inMs);
    }, cur === null ? 0 : outMs);
    return () => {
      clearTimeout(swap);
      clearTimeout(lift);
    };
  }, [key, outMs, inMs, onSwap, eq, shownRef]);

  return { shown, fading };
}