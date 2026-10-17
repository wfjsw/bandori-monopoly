// The navigation lock behind "don't leave a loaded game": Back and the mobile
// edge-swipe back gesture pop the History stack, and without a same-document
// entry sitting above the match they walk straight out of the screen (or off
// the site entirely). A sentinel entry answers every pop instead.
//
// Split from `router.ts` so the arm / re-arm rules are testable without a DOM.

/** The slice of `History` the lock writes to. */
export type NavLockHistory = {
  pushState(data: unknown, unused: string, url?: string | null): void;
  readonly state: unknown;
};

/** The slice of `Location` the lock reads. */
export type NavLockLocation = { pathname: string };

/** The sentinel's `history.state`. `path` is the route it is holding. */
export type NavLockEntry = { __navLock: 1; path: string };

export function isNavLockEntry(state: unknown): boolean {
  return (state as Partial<NavLockEntry> | null | undefined)?.__navLock === 1;
}

/** True when `state` is *this* route's sentinel, not another screen's. */
export function isNavLockFor(state: unknown, path: string): boolean {
  const e = state as Partial<NavLockEntry> | null | undefined;
  return e?.__navLock === 1 && e.path === path;
}

/**
 * Put the lock sentinel on top of `path`, unless one already sits there.
 * Called when the lock arms and after every pop: the entry above the real
 * screen is always the sentinel, so Back / edge-swipe lands on it (same
 * document, same path) instead of the previous screen or the previous site.
 */
export function rearmNavLock(h: NavLockHistory, loc: NavLockLocation, path: string): void {
  if (loc.pathname === path && isNavLockFor(h.state, path)) return;
  h.pushState({ __navLock: 1, path } satisfies NavLockEntry, "", path);
}