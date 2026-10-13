// The seat whose Players panel the pointer is over. The board highlights that
// seat's tiles, field cards and map token. A tiny external store, so only the
// pieces whose answer flips re-render (the memoized tiles stay put otherwise).

import { useSyncExternalStore } from "react";

let seat = -1;
const subs = new Set<() => void>();

export function setHoverSeat(i: number): void {
  if (seat === i) return;
  seat = i;
  subs.forEach((f) => f());
}

const subscribe = (f: () => void) => {
  subs.add(f);
  return () => void subs.delete(f);
};

/** The hovered seat, or -1. */
export function useHoverSeat(): number {
  return useSyncExternalStore(subscribe, () => seat);
}

/** Is `i` the hovered seat? Re-renders only when that answer changes. */
export function useSeatHovered(i: number): boolean {
  return useSyncExternalStore(subscribe, () => i >= 0 && seat === i);
}
