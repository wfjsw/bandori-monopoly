// Pure helpers behind the semantic hooks: no React, no DOM, no fetch. This is
// the module `node --test` can load directly (see pure.test.ts).
//
//   matchHotkey        -- the `useHotkeys` key matcher
//   countdownLeft      -- the `useCountdown` / `usePerfCountdown` arithmetic
//   scrollFollowStep   -- the `useScrollFollow` tail-tracking state machine
//   canvasBox          -- the Live2D stand's portrait fit (Live2DPortrait.Draw)
//   sameParams         -- the Live2D `params` change detector
//   bannerDeadline     -- the AutoBanner's hide-at stamp

// ---------------------------------------------------------------- hotkeys

export interface Hotkey {
  /** Matches `KeyboardEvent.key` -- " ", "ArrowLeft", "h", "H", … */
  key: string | readonly string[];
  /** Fire even with ctrl / meta / alt held (default: never). */
  mods?: boolean;
  /** Fire even while the user is typing in a field (default: never). */
  typing?: boolean;
  /** Fire when focus is on a button / link (default: yes; pass false so Space
   *  still activates the focused control instead of the shortcut). */
  onControl?: boolean;
  /** Ignore `e.repeat` (held key) (default: no -- arrows want the repeat). */
  once?: boolean;
  run: (e: KeyboardEvent) => void;
}

/** Is the event going to a field the user is typing in (or editing)? */
export function isEditableTarget(t: EventTarget | null | { tagName?: string; isContentEditable?: boolean }): boolean {
  const el = t as { isContentEditable?: boolean; tagName?: string } | null;
  if (!el || typeof el.tagName !== "string") return false;
  return !!el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName);
}

/** Is the event going to an activatable control (a button keeps Space)?
 *  A `role="button"` div counts too -- `CardFace` is one, and Enter on it
 *  selects the card rather than confirming the prompt's choice. */
export function isControlTarget(t: EventTarget | null | { tagName?: string; role?: string }): boolean {
  const el = t as { tagName?: string; role?: string; getAttribute?: (n: string) => string | null } | null;
  if (!el || typeof el.tagName !== "string") return false;
  if (/^(BUTTON|A)$/.test(el.tagName)) return true;
  const role = el.role ?? el.getAttribute?.("role");
  return role === "button";
}

/** The event shape `matchHotkey` needs (a real `KeyboardEvent` fits). */
export interface HotkeyEventLike {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  repeat: boolean;
  /** True while an IME composition is in flight (those keys are not shortcuts). */
  isComposing?: boolean;
  target: EventTarget | { tagName?: string; isContentEditable?: boolean; role?: string } | null;
}

/** Does `e` hit `hk`? */
export function matchHotkey(hk: Hotkey, e: HotkeyEventLike): boolean {
  const keys = typeof hk.key === "string" ? [hk.key] : hk.key;
  if (!keys.includes(e.key)) return false;
  if (e.isComposing) return false;
  if (!hk.mods && (e.ctrlKey || e.metaKey || e.altKey)) return false;
  if (!hk.typing && isEditableTarget(e.target as EventTarget | null)) return false;
  if (hk.onControl === false && isControlTarget(e.target as EventTarget | null)) return false;
  if (hk.once && e.repeat) return false;
  return true;
}

// ---------------------------------------------------------------- timers

/** Milliseconds left until `deadline`, never negative. */
export function countdownLeft(deadline: number, now: number): number {
  return Math.max(0, deadline - now);
}

// ---------------------------------------------------------------- scroll follow

/**
 * How many CSS px below the viewport still count as "at the end" of a scroller
 * (`useScrollFollow`). Not 0: a wheel notch, a trackpad rest, a fractional
 * `scrollTop` rounding, or one log line landing mid-gesture must not read as
 * "the user scrolled up to read" -- that silently kills the follow and it only
 * comes back when they move again. One and a bit log lines (14px / 1.55) is
 * tight enough that scrolling up to re-read the last entry still opts out.
 */
export const END_SLACK = 32;

/** `gap` is the px of content still below the viewport. Is the user at the end? */
export function atScrollEnd(gap: number, slack: number = END_SLACK): boolean {
  return gap <= slack;
}

/** The follow state `useScrollFollow` keeps between renders. */
export interface ScrollFollow {
  /** The user is at the end, so new content should re-pin them to the bottom. */
  atEnd: boolean;
  /** The `scrollTop` of our last programmatic pin (null when the user is
   *  steering). Its asynchronous `scroll` echo is not a user scroll. */
  pinned: number | null;
}

/**
 * Fold one `scroll` event into the follow state.
 *
 * The echo of our own pin must not be read as the user scrolling away. That
 * echo is dispatched asynchronously, so it routinely lands after more content
 * has already been appended -- and then measures the gap of the *new* lines,
 * not the pin. Left unchecked that gap flips `atEnd` off while the user is
 * still sitting on the bottom, and the follow dies until they scroll again.
 */
export function scrollFollowStep(
  s: ScrollFollow,
  ev: { scrollTop: number; gap: number; visible: boolean },
): ScrollFollow {
  // A hidden pane (display: none) has no height; its events say nothing.
  if (!ev.visible) return s;
  if (s.pinned != null && ev.scrollTop === s.pinned) return { atEnd: true, pinned: s.pinned };
  return { atEnd: atScrollEnd(ev.gap), pinned: null };
}

/**
 * Should new content re-pin the scroller to its bottom? Yes while the user is
 * at the end -- and also when they are only just short of it: the tail may
 * have landed mid-gesture, after the last `scroll` already measured the gap.
 */
export function shouldPinToBottom(s: ScrollFollow, gap: number, enabled: boolean): boolean {
  return enabled && (s.atEnd || atScrollEnd(gap));
}

// ---------------------------------------------------------------- banner

/**
 * When the AutoBanner drops its strip: through the start-up cooldown plus the
 * hold, or 0 (never) while the mode is off.
 */
export function bannerDeadline(mode: string, nowMs: number, cooldownLeftMs: number, holdMs: number): number {
  return mode === "off" ? 0 : nowMs + cooldownLeftMs + holdMs;
}

// ---------------------------------------------------------------- live2d

export interface Framing {
  top: number;
  scale: number;
}

/** Canvas rect inside the 3:4 window, in px relative to the window. */
export interface StandBox {
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * Map the canvas into the 3:4 window (Live2DPortrait.Draw).
 *
 * When `eyeY` is known the figure is anchored by its **eyes**: every character
 * lands its eye line on `eyeFrac` of the window height, so casts of different
 * heights line up. Anchoring on the head top instead makes the eye line wander
 * with each model's height. The anchor is then clamped so the figure's top
 * keeps `headroom` of the window below the window's top edge -- without that a
 * tall hairdo (004 needs 21% of the window above its eyes) is sliced off by
 * the portrait rect. `top`/`headroom` from framing.json remain a fallback for
 * models without eye params.
 */
export function canvasBox(
  winW: number,
  canvas: [number, number],
  f: Framing | undefined,
  zoom: number,
  focusTop: number,
  headroom: number,
  eyeY: number | null,
  eyeFrac: number,
  figTop: number | null,
): StandBox {
  const [cw, ch] = canvas;
  const z = zoom * (f && f.scale > 0 ? f.scale : 1);
  const num = Math.max(cw, ch * 0.75);
  const num2 = num / 0.75;
  const viewW = num / z;
  const viewH = viewW / 0.75;
  const left = (cw - num) / 2 + (num - viewW) / 2;
  const k = winW / viewW; // canvas unit -> px
  let top: number;
  if (eyeY != null) {
    // Put the eye line at `eyeFrac` of the window height, then drop the figure
    // (shrink `top`) as far as needed to keep `headroom` clear above its head.
    top = eyeY - eyeFrac * viewH;
    if (figTop != null && headroom > 0) {
      top = Math.min(top, figTop - headroom * viewH);
    }
  } else {
    const focus = f && f.top >= 0 && headroom > 0 ? Math.min(focusTop, f.top - headroom / z) : focusTop;
    top = (ch - num2) / 2 + num2 * focus;
  }
  return { left: -left * k, top: -top * k, width: cw * k, height: ch * k };
}

/** Do two Live2D `params` maps hold the same values? */
export function sameParams(a: Record<string, number> | undefined, b: Record<string, number> | undefined): boolean {
  if (a === b) return true;
  const ka = a ?? {};
  const kb = b ?? {};
  const keys = Object.keys(ka);
  if (keys.length !== Object.keys(kb).length) return false;
  return keys.every((k) => ka[k] === kb[k]);
}