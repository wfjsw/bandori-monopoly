// The one client-side mirror of "the board is still presenting a commit".
//
// The Animator owns the real predicate (`presenting` in `scenes/board/anim`);
// Board publishes it here so surfaces outside the Board tree (the prompt
// sheet's timer) freeze on the same signal instead of each re-deriving it from
// the animator / state pair. The clock display inside the Board reads the
// animator directly -- both ends of the contract resolve to this one boolean,
// which is what keeps the display and the `anim_done` ack gate from disagreeing
// about whether the player may act yet.

import { useSyncExternalStore } from "react";

let presenting = false;
const subs = new Set<() => void>();

/** Publish the current presentation state (Board). */
export function setPresenting(v: boolean): void {
  if (v === presenting) return;
  presenting = v;
  for (const fn of subs) fn();
}

function subscribe(fn: () => void): () => void {
  subs.add(fn);
  return () => subs.delete(fn);
}

/** Reactive read for components outside the board. */
export function usePresenting(): boolean {
  return useSyncExternalStore(subscribe, () => presenting, () => presenting);
}
