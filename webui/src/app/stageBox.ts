// The stage's screen geometry, extracted from Stage.tsx so the axis mapping
// and the touch dead-zone floor can be unit-tested without a DOM. The stage's
// placer is `position: fixed; left: 50%; top: 50%` with
// `transform: translate(calc(-50% + shiftX), calc(-50% + shiftY))
// [rotate(90deg)]`, sized in screen px, and the design-space child scales by
// `zoom: scale`. Because `translate` is the *outermost* transform, `shiftX` /
// `shiftY` are screen pixels and the stage centre lands at viewport-centre +
// (shiftX, shiftY) regardless of rotation.

import { COMPACT_H, COMPACT_MIN_W, STAGE_H, STAGE_W, TOUCH_TOP_MIN, isCompactWindow } from "../styles/layout.ts";

export interface Insets {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface StageBoxInput {
  /** Visual viewport size in CSS px. */
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
  /** Stage size in DESIGN px (w is always the full available width at `scale`; h is the fill height). */
  w: number;
  h: number;
  /** Screen-space offset of the stage centre from the viewport centre. */
  shiftX: number;
  shiftY: number;
  /**
   * Hardware sitting on top of each *stage* edge -- the safe-area inset plus
   * the top touch dead zone -- in DESIGN px. The stage's edges are the real
   * screen edges along the retractable axis (see `stageBox`), so this is how
   * much of a strip at that edge is under the notch / home indicator / dead
   * zone. Controls pad by it; the retractable strips deliberately do not.
   */
  safe: Insets;
  compact: boolean;
  rotated: boolean;
}

export function stageBox(input: StageBoxInput): StageBox {
  const { vw: winW, vh: winH, inset, rotated, tall, coarse, compactForced } = input;
  // The phantom top dead zone: floor the top inset on touch devices.
  const top = coarse ? Math.max(inset.top, TOUCH_TOP_MIN) : inset.top;
  // Two axes, two jobs:
  //
  //  * the stage's *width* is where the side columns ride, so it is inset to
  //    clear the cutout (which is on the sides in landscape and, under
  //    `rotate(90deg)`, on the top / bottom);
  //  * its *height* is where the retractable strips clip -- the field row
  //    behind the top edge, the hand off the bottom -- so it runs to the REAL
  //    screen edge. Inset that axis and those strips stop short of the border
  //    and leave a gap that reads as a bug.
  const cutout = rotated ? top + inset.bottom : inset.left + inset.right;
  const spanW = (rotated ? winH : winW) - cutout;
  const spanH = rotated ? winW : winH;
  const compact = tall && isCompactWindow(spanW, spanH, coarse, compactForced);
  const fit = compact
    ? Math.min(spanW / COMPACT_MIN_W, spanH / COMPACT_H)
    : Math.min(spanW / STAGE_W, spanH / STAGE_H);
  // A pathological window (insets larger than the viewport) can drive the
  // scale to 0 or below; keep it positive so the transform never produces NaN.
  const scale = Math.max(fit, 0.01);
  // Centre the stage in the inset band on the cutout axis only; the retractable
  // axis is already flush with both screen edges. `shiftX` / `shiftY` are
  // screen pixels, so they are not rotation-swapped -- the cutout axis is
  // screen X normally and screen Y once rotated.
  const shiftX = rotated ? 0 : (inset.left - inset.right) / 2;
  const shiftY = rotated ? (top - inset.bottom) / 2 : 0;
  // Hardware per *stage* edge. `rotate(90deg)` maps the stage's top edge onto
  // the screen's right edge and its left edge onto the screen's top edge (the
  // transform is `translate rotate scale`, so a point goes local -> scale ->
  // rotate -> screen).
  const screen: Insets = rotated
    ? { top: inset.right, right: inset.bottom, bottom: inset.left, left: top }
    : { top, right: inset.right, bottom: inset.bottom, left: inset.left };
  return {
    scale,
    w: spanW / scale,
    h: spanH / scale,
    shiftX,
    shiftY,
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