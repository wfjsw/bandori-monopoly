// The stage's screen geometry, extracted from Stage.tsx so the forced-rotation
// axis swap, the safe-area mapping and the compact-canvas predicate can be
// unit-tested without a DOM. The stage's placer is `position: fixed` with its
// centre at (`cx`, `cy`) and `transform: translate(-50%, -50%) [rotate(90deg)]`,
// sized in screen px, and the design-space child scales by `zoom: scale`.
//
// The stage fills the window on both axes: the game runs edge to edge, and
// insetting it for the safe area left the retractable strips (the field row,
// the hand) hanging short of the screen border. What clears the cutout is the
// *controls*, padding by `safe` (the hardware on each stage edge, in design
// px) -- see docs/CSS.md.

import { COMPACT_H, COMPACT_MIN_W, STAGE_H, STAGE_W, TOUCH_TOP_MIN, isCompactWindow } from "../styles/layout.ts";

export interface Insets {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface Box {
  vw: number;
  vh: number;
}

/**
 * The window the stage lays out on: the box the placer is centered in, and
 * nothing larger.
 *
 * The placer is `position: fixed; left: 50%; top: 50%` -- it is centered in
 * the **layout viewport**. Sizing it from anything bigger overflows that box:
 * the stage grows past the cutout on one side and leaves a blank border on
 * the other (and, in landscape, past both long edges). `window.screen` and
 * `100lvh` are both bigger than the layout viewport whenever browser chrome
 * is showing (Safari's address bar, the iOS PWA's status-bar strip), so they
 * must not drive the size.
 *
 * What is *outside* the layout viewport is the browser's own chrome; it is
 * not ours to fill, and trying is what produced the overflow. When the chrome
 * hides, the layout viewport grows and the stage grows with it.
 */
export function windowBox(input: {
  /** The `position: fixed; inset: 0` box -- the layout viewport. */
  layoutW: number;
  layoutH: number;
  /** `window.innerWidth` / `innerHeight`, a fallback. */
  innerW: number;
  innerH: number;
}): Box {
  if (input.layoutW > 0 && input.layoutH > 0) return { vw: input.layoutW, vh: input.layoutH };
  return { vw: input.innerW, vh: input.innerH };
}

export interface StageBoxInput {
  /** The window's size in CSS px (see `windowBox`). */
  vw: number;
  vh: number;
  /**
   * The device screen's long side in CSS px, where the page is known to draw
   * from the physical screen top down to the physical screen bottom (iPhone
   * WebKit: the fixed box starts under the island and Safari paints the page
   * behind its own bottom toolbar). 0 elsewhere. Only forced rotation uses it
   * -- see `stageBox`.
   */
  screenLong: number;
  /** Device safe-area insets in CSS px (already probed). */
  inset: Insets;
  /** Forced-landscape on a portrait phone (the stage is rotated 90deg). */
  rotated: boolean;
  /** The scene filled the stage height (the board); other scenes letterbox. */
  tall: boolean;
  /** pointer: coarse */
  coarse: boolean;
  /** `?compact=1|0` override, or null for auto. */
  compactForced: boolean | null;
}

export interface StageBox {
  /** Uniform design-px -> screen-px scale. */
  scale: number;
  /** Stage size in DESIGN px (w is always the full window width at `scale`; h is the fill height). */
  w: number;
  h: number;
  /**
   * Hardware sitting on each *stage* edge -- the safe-area inset plus the
   * landscape top touch dead zone -- in DESIGN px. The stage's edges are the
   * real screen edges, so this is how much of a strip at that edge is under
   * the notch / home indicator / dead zone. Controls pad by it; the
   * retractable strips deliberately do not.
   */
  safe: Insets;
  /**
   * Where the placer's centre goes, in screen px from the fixed box's
   * top-left. The middle of the window, except under forced rotation on an
   * iPhone, where it is the middle of the whole screen (see `stageBox`).
   */
  cx: number;
  cy: number;
  compact: boolean;
  rotated: boolean;
}

/**
 * How far past the fixed box the screen may run before `screenLong` is taken
 * as a bad reading rather than browser chrome: Safari's expanded bottom
 * toolbar takes ~190 of 852, under a quarter.
 */
const MAX_REACH = 1.35;

export function stageBox(input: StageBoxInput): StageBox {
  const { vw: winW, vh: winH, screenLong, inset, rotated, tall, coarse, compactForced } = input;
  // The phantom top dead zone: floor the top inset on touch devices. iOS
  // landscape silently eats touches in the top ~20 CSS px even though
  // `safe-area-inset-top` reports 0.
  const top = coarse ? Math.max(inset.top, TOUCH_TOP_MIN) : inset.top;
  // Forced rotation on an iPhone held upright: the fixed box runs from the
  // screen top (under the island) but stops above Safari's bottom toolbar /
  // the home-indicator strip, which Safari paints with the bare page
  // background. Centring a box-sized stage there left that strip blank;
  // centring a screen-sized one pushed it up past the island and *still* left
  // the strip. So take the screen's full long side AND anchor the stage at the
  // box's top: it then spans the physical screen from the island to the home
  // indicator. A real landscape turn has no such strip and keeps the box.
  const reach =
    rotated && screenLong > winH && screenLong <= winH * MAX_REACH ? screenLong : winH;
  // Under `rotate(90deg)` the stage's *width* lies along the screen's vertical
  // axis, so it is laid out on the swapped window.
  const spanW = rotated ? reach : winW;
  const spanH = rotated ? winW : winH;
  const compact = tall && isCompactWindow(spanW, spanH, coarse, compactForced);
  const fit = compact
    ? Math.min(spanW / COMPACT_MIN_W, spanH / COMPACT_H)
    : Math.min(spanW / STAGE_W, spanH / STAGE_H);
  // A pathological window can drive the scale to 0 or below; keep it positive
  // so the transform never produces NaN.
  const scale = Math.max(fit, 0.01);
  // Hardware per *stage* edge. `rotate(90deg)` is clockwise: the stage's top
  // edge lands on the screen's right, its left edge on the screen's top (and
  // so on around). The cutout that was the portrait notch at the screen's top
  // is therefore the stage's left edge -- the same edge a landscape handset
  // puts it on once the phone is really turned. The stretch past the fixed
  // box (`reach - winH`) is under Safari's toolbar: the surfaces run there,
  // but it counts as hardware for the controls on that edge.
  const bottom = Math.max(inset.bottom, reach - winH);
  const screen: Insets = rotated
    ? { top: inset.right, right: bottom, bottom: inset.left, left: top }
    : { top, right: inset.right, bottom: inset.bottom, left: inset.left };
  return {
    scale,
    w: spanW / scale,
    h: spanH / scale,
    safe: {
      top: screen.top / scale,
      right: screen.right / scale,
      bottom: screen.bottom / scale,
      left: screen.left / scale,
    },
    // Anchored at the box's top-left: with `reach === winH` this is the
    // middle of the box, exactly the old `left: 50%; top: 50%`.
    cx: winW / 2,
    cy: reach / 2,
    compact,
    rotated,
  };
}