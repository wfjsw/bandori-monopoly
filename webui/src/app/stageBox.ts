// The stage's screen geometry, extracted from Stage.tsx so the forced-rotation
// axis swap, the safe-area mapping and the compact-canvas predicate can be
// unit-tested without a DOM. The stage's placer is `position: fixed; left: 50%;
// top: 50%` with `transform: translate(-50%, -50%) [rotate(90deg)]`, sized in
// screen px, and the design-space child scales by `zoom: scale`.
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
 * The window the stage lays out on.
 *
 * A handset's `window.screen` is the whole device in CSS px and swaps with
 * orientation -- the one measurement that is the same rectangle in portrait
 * and in landscape. The layout and visual viewports are both short of it in
 * portrait (Safari's address bar, the iOS PWA's status-bar strip) and full in
 * landscape. Sizing to those left the stage short on the long axis: under
 * forced rotation that axis is the phone's height, so the leftover strip sat
 * under the cutout at the top (invisible behind the island) and showed as a
 * blank bar at the **bottom** of the phone, with the app not filling the
 * screen width.
 *
 * `screen` only counts on a handset-sized coarse device -- a desktop window on
 * a touch monitor must not be blown up to the monitor. Everywhere else the
 * window itself is the right answer. Either way take the largest candidate: a
 * short stage letterboxes, and letterbox is the bug.
 */
export function windowBox(input: {
  /** `window.screen`, CSS px. */
  screenW: number;
  screenH: number;
  /** `window.innerWidth` / `innerHeight`. */
  innerW: number;
  innerH: number;
  /** The `position: fixed; inset: 0` body's box -- the real drawing area. */
  layoutW: number;
  layoutH: number;
  /** `100lvw` / `100lvh` probe, or 0 where unavailable. */
  probeW: number;
  probeH: number;
  /** `orientation: portrait`. */
  portrait: boolean;
  /** `pointer: coarse`. */
  coarse: boolean;
}): Box {
  const candidates: Box[] = [{ vw: input.innerW, vh: input.innerH }];
  if (input.layoutW > 0 && input.layoutH > 0) candidates.push({ vw: input.layoutW, vh: input.layoutH });
  if (input.probeW > 0 && input.probeH > 0) candidates.push({ vw: input.probeW, vh: input.probeH });
  const lo = Math.min(input.screenW, input.screenH);
  const hi = Math.max(input.screenW, input.screenH);
  if (input.coarse && lo > 0 && lo <= 600) {
    candidates.push(input.portrait ? { vw: lo, vh: hi } : { vw: hi, vh: lo });
  }
  return candidates.reduce((a, b) => ({ vw: Math.max(a.vw, b.vw), vh: Math.max(a.vh, b.vh) }));
}

export interface StageBoxInput {
  /** The window's size in CSS px (see `windowBox`). */
  vw: number;
  vh: number;
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
  compact: boolean;
  rotated: boolean;
}

export function stageBox(input: StageBoxInput): StageBox {
  const { vw: winW, vh: winH, inset, rotated, tall, coarse, compactForced } = input;
  // The phantom top dead zone: floor the top inset on touch devices. iOS
  // landscape silently eats touches in the top ~20 CSS px even though
  // `safe-area-inset-top` reports 0.
  const top = coarse ? Math.max(inset.top, TOUCH_TOP_MIN) : inset.top;
  // Under `rotate(90deg)` the stage's *width* lies along the screen's vertical
  // axis, so it is laid out on the swapped window. The window is the large
  // viewport in both orientations, so forced rotation and a real turn of the
  // phone land on the same canvas -- that is the whole point.
  const spanW = rotated ? winH : winW;
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
  // puts it on once the phone is really turned.
  const screen: Insets = rotated
    ? { top: inset.right, right: inset.bottom, bottom: inset.left, left: top }
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
    compact,
    rotated,
  };
}