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

/** How a piece owned by seat `i` (-1: nobody) reads while a panel is hovered:
 *  "hot" for the hovered seat's, "dim" for everything else, "" when no panel
 *  is hovered. Re-renders only when that answer changes. */
export function useSeatFocus(i: number): "" | "hot" | "dim" {
  return useSyncExternalStore(subscribe, () => (seat < 0 ? "" : i === seat ? "hot" : "dim"));
}

// The tiles a hovered on-field card designates. While set, those tiles light up
// and the rest of the board fades, like a hovered Players panel.
let cardTiles: ReadonlySet<number> | null = null;

export function setHoverCardTiles(t: ReadonlySet<number> | null): void {
  if (cardTiles === t) return;
  cardTiles = t;
  subs.forEach((f) => f());
}

/** How tile `i` reads while a field card with designated tiles is hovered. */
export function useCardTileFocus(i: number): "" | "hot" | "dim" {
  return useSyncExternalStore(subscribe, () => (!cardTiles ? "" : cardTiles.has(i) ? "hot" : "dim"));
}
