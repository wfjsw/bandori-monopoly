// The latest-ref pattern: a ref that always holds the newest value, so a
// long-lived callback (event listener, interval, RAF tick) can read fresh
// props without being re-created -- the one way out of dependency churn.

import { useRef } from "react";

/** A ref kept in sync with `value` on every render (write during render). */
export function useLatestRef<T>(value: T): { current: T } {
  const ref = useRef(value);
  ref.current = value;
  return ref;
}