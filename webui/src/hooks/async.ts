// Async work whose lifetime is a component (or a dep list): fetch a resource,
// build a portable record, resolve a pending open. The hook owns the
// cancellation flag so the caller never writes state after unmount.

import { useEffect, useState } from "react";
import { useLatestRef } from "./latest.ts";

export interface AsyncState<T> {
  /** A run is in flight (or about to be). */
  loading: boolean;
  /** The last successful result; `undefined` while a new run is in flight. */
  value: T | undefined;
  /** The last failure; `undefined` while a new run is in flight. */
  error: unknown;
}

/**
 * Run `fn` after mount and whenever `deps` change. The previous run is
 * abandoned (its result ignored) when `deps` change or the component unmounts.
 * Starting a run clears `value` / `error` and sets `loading`.
 *
 * With `enabled: false` nothing runs and the state is idle -- flipping it back
 * on starts a run. `deps` must have a constant length across renders.
 */
export function useAsync<T>(
  fn: () => Promise<T>,
  deps: readonly unknown[],
  enabled = true,
): AsyncState<T> {
  const latest = useLatestRef(fn);
  const [state, set] = useState<AsyncState<T>>({ loading: false, value: undefined, error: undefined });
  useEffect(() => {
    if (!enabled) {
      set({ loading: false, value: undefined, error: undefined });
      return;
    }
    let alive = true;
    set({ loading: true, value: undefined, error: undefined });
    void latest.current().then(
      (value) => {
        if (alive) set({ loading: false, value, error: undefined });
      },
      (error) => {
        if (alive) set({ loading: false, value: undefined, error });
      },
    );
    return () => {
      alive = false;
    };
    // `deps` is the caller's dep list (constant length); `latest` is stable.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, enabled, latest]);
  return state;
}