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
  compact: boolean;
  rotated: boolean;
}

export function stageBox(input: StageBoxInput): StageBox {
  const { vw: winW, vh: winH, inset, rotated, tall, coarse, compactForced } = input;
  // The phantom top dead zone: floor the top inset on touch devices.
  const top = coarse ? Math.max(inset.top, TOUCH_TOP_MIN) : inset.top;
  // Safe rectangle in SCREEN space.
  const availW = winW - inset.left - inset.right;
  const availH = winH - top - inset.bottom;
  // When the stage is rotated 90deg its *width* lies along the screen's
  // vertical axis, so it must fit availH, and its height must fit availW.
  // (Old code skipped the insets when rotated -- wrong: they just swap axes.)
  const stageW = rotated ? availH : availW;
  const stageH = rotated ? availW : availH;
  const compact = tall && isCompactWindow(stageW, stageH, coarse, compactForced);
  const fit = compact
    ? Math.min(stageW / COMPACT_MIN_W, stageH / COMPACT_H)
    : Math.min(stageW / STAGE_W, stageH / STAGE_H);
  // A pathological window (insets larger than the viewport) can drive the
  // scale to 0 or below; keep it positive so the transform never produces NaN.
  const scale = Math.max(fit, 0.01);
  return {
    scale,
    w: stageW / scale,
    h: stageH / scale,
    shiftX: (inset.left - inset.right) / 2,
    shiftY: (top - inset.bottom) / 2,
    compact,
    rotated,
  };
}