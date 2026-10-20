// The install / rotate cover decision table, pure: no DOM, no I/O, so the
// gates can be checked with `node --test` (installHint.test.ts). The browser
// side (media queries, `beforeinstallprompt`, the dismiss flag) lives in
// installHint.ts and feeds these.

/**
 * Should the install cover take the rotate cover's slot? "Before the rotation
 * prompt" means exactly that: it claims the moment the rotate cover would
 * have claimed, and only there -- a handset already turned sideways is
 * playing and must not be interrupted.
 *
 * `phone` is a handset. It is a separate gate from `portrait` on purpose: the
 * rotate query is a size/orientation test a small tablet can pass, and the
 * prompt must never land on one. A desktop browser tab is just as "not
 * standalone" as a phone's, so without this gate it would land there too.
 * `standalone` is an installed PWA; `dismissed` is the user's override.
 */
export function showInstallHint(env: {
  phone: boolean;
  portrait: boolean;
  standalone: boolean;
  dismissed: boolean;
}): boolean {
  return env.phone && env.portrait && !env.standalone && !env.dismissed;
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