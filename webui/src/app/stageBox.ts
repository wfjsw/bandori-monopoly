// The stage's screen geometry, extracted from Stage.tsx so the forced-rotation
// axis swap and the compact-canvas predicate can be unit-tested without a DOM.
// The stage's placer is `position: fixed; left: 50%; top: 50%` with
// `transform: translate(-50%, -50%) [rotate(90deg)]`, sized in screen px, and
// the design-space child scales by `zoom: scale`.
//
// The stage fills the window on both axes: the game runs edge to edge, and
// insetting it for the safe area left the retractable strips (the field row,
// the hand) hanging short of the screen border.

import { COMPACT_H, COMPACT_MIN_W, STAGE_H, STAGE_W, isCompactWindow } from "../styles/layout.ts";

export interface StageBoxInput {
  /** Visual viewport size in CSS px. */
  vw: number;
  vh: number;
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
  compact: boolean;
  rotated: boolean;
}

export function stageBox(input: StageBoxInput): StageBox {
  const { vw: winW, vh: winH, rotated, tall, coarse, compactForced } = input;
  // Under `rotate(90deg)` the stage's *width* lies along the screen's vertical
  // axis, so it is laid out on the swapped window.
  const spanW = rotated ? winH : winW;
  const spanH = rotated ? winW : winH;
  const compact = tall && isCompactWindow(spanW, spanH, coarse, compactForced);
  const fit = compact
    ? Math.min(spanW / COMPACT_MIN_W, spanH / COMPACT_H)
    : Math.min(spanW / STAGE_W, spanH / STAGE_H);
  // A pathological window can drive the scale to 0 or below; keep it positive
  // so the transform never produces NaN.
  const scale = Math.max(fit, 0.01);
  return {
    scale,
    w: spanW / scale,
    h: spanH / scale,
    compact,
    rotated,
  };
}