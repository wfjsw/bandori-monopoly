// History API routes, so a page refresh lands on the same screen. The server
// (and the rsbuild dev server) answer every page path with index.html.
//
//   /menu  /gallery  /deck  /lobby  /room/<id>  /play/solo  /play/<roomId>
//   /replay  /replay/view

import { useSyncExternalStore } from "react";
import { isNavLockEntry, rearmNavLock } from "./navLock.ts";

export type Route =
  | { name: "menu" | "gallery" | "deck" | "lobby" | "replay" }
  | { name: "replayView" }
  | { name: "room"; id: string }
  | { name: "play"; id: string };

export function parse(path: string): Route | null {
  const parts = path.split("/").filter(Boolean).map(decodeURIComponent);
  switch (parts[0]) {
    case "menu": case "gallery": case "deck": case "lobby":
      return { name: parts[0] };
    case "replay":
      return parts[1] === "view" ? { name: "replayView" } : { name: "replay" };
    case "room":
      return parts[1] ? { name: "room", id: parts[1] } : { name: "lobby" };
    case "play":
      return parts[1] ? { name: "play", id: parts[1] } : null;
    default:
      return null;
  }
}

export function href(r: Route): string {
  if (r.name === "replayView") return "/replay/view";
  return "/" + ("id" in r ? `${r.name}/${encodeURIComponent(r.id)}` : r.name);
}

const listeners = new Set<() => void>();
const emit = () => listeners.forEach((cb) => cb());

/** The path `lockRoute` is holding, or null when nothing is locked. */
let lockPath: string | null = null;
/** Nesting depth, so StrictMode's simulated remount does not double-arm. */
let lockDepth = 0;

window.addEventListener("popstate", () => {
  // Back / edge-swipe popped us. Put the sentinel (and the locked path) back
  // before any subscriber can swap the screen.
  if (lockPath) rearmNavLock(history, location, lockPath);
  emit();
});

export function navigate(r: Route, opts: { replace?: boolean } = {}): void {
  const path = href(r);
  if (location.pathname === path) return;
  // A lock sentinel on top is not a real screen: leaving through it replaces
  // it, so Back after an intentional leave lands on the pre-match screen
  // instead of a stale /play entry.
  if (opts.replace || isNavLockEntry(history.state)) history.replaceState(null, "", path);
  else history.pushState(null, "", path);
  emit();
}

/**
 * Hold the current route against Back and the mobile edge-swipe back gesture
 * (the match screen: a swipe must not drop a loaded game). Arms a same-document
 * sentinel above the real entry -- the swipe lands there and is undone -- and
 * `navigate()` consumes it when the screen leaves through the UI. The disposer
 * only unlocks; it never trims history, so a StrictMode remount is safe.
 */
export function lockRoute(): () => void {
  if (lockDepth++ === 0) lockPath = location.pathname;
  rearmNavLock(history, location, lockPath!);
  return () => {
    if (--lockDepth === 0) lockPath = null;
  };
}

/** The current path (re-renders when it changes). */
export function usePath(): string {
  return useSyncExternalStore(
    (cb) => {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    () => location.pathname,
  );
}
