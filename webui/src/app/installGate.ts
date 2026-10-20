// The install / rotate cover decision table, pure: no DOM, no I/O, so the
// gates can be checked with `node --test` (installHint.test.ts). The browser
// side (media queries, `beforeinstallprompt`, the dismiss flag) lives in
// installHint.ts and feeds these.

/**
 * Should the install cover take the screen? Either orientation: a handset
 * already turned sideways is still a browser tab worth installing. It sits
 * in front of the rotate cover rather than beside it, so the user resolves
 * one prompt at a time.
 *
 * `phone` is a handset. The trigger on its own is "not running as an
 * installed app", which is true in every browser tab on every platform --
 * without this gate the prompt would land on desktops and tablets too.
 * `standalone` is an installed PWA; `dismissed` is the user's override.
 */
export function showInstallHint(env: {
  phone: boolean;
  standalone: boolean;
  dismissed: boolean;
}): boolean {
  return env.phone && !env.standalone && !env.dismissed;
}

/**
 * The rotate cover steps aside while the install cover is up, so the user
 * deals with one prompt at a time (install first, rotate after the override).
 */
export function showRotateHint(env: {
  portrait: boolean;
  forced: boolean;
  installCover: boolean;
}): boolean {
  return env.portrait && !env.forced && !env.installCover;
}