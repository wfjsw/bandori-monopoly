// History API routes, so a page refresh lands on the same screen. The server
// (and the rsbuild dev server) answer every page path with index.html.
//
//   /menu  /gallery  /deck  /lobby  /room/<id>  /play/solo  /play/<roomId>
//   /replay  /replay/view
//
// Phones replace the current History entry instead of stacking one (see
// `navigate`): a swipe-back then has nothing to walk through.

import { useSyncExternalStore } from "react";

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
window.addEventListener("popstate", emit);

/**
 * A phone, in either orientation: the rotate hint's size class without its
 * portrait clause. Tablets and desktops keep a normal History stack.
 */
export const PHONE_QUERY = "(pointer: coarse) and (max-width: 600px)";

export function isPhone(): boolean {
  return window.matchMedia(PHONE_QUERY).matches;
}

export function navigate(r: Route, opts: { replace?: boolean } = {}): void {
  const path = href(r);
  if (location.pathname === path) return;
  // Phones never stack a History entry. `overscroll-behavior-x: none` is
  // ignored by the iOS PWA shell, so the edge-swipe back gesture is the only
  // way out of a screen -- and with a single entry it has nothing to go back
  // to. Desktop keeps pushState so Back walks the screens as usual.
  if (opts.replace || isPhone()) history.replaceState(null, "", path);
  else history.pushState(null, "", path);
  emit();
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
