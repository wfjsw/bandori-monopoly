import { useSyncExternalStore } from "react";
import { isStageRotated, stageScale } from "./stageScale.ts";

// A portrait phone shows the stage turned a quarter clockwise (see Stage.tsx),
// so what a user sees as "up" is the design's "left". Every place that maps a
// screen box / pointer move back into design pixels goes through here.

/** A phone held upright: the stage is rotated to landscape instead of asking for a turn. */
export const ROTATE_QUERY = "(orientation: portrait) and (pointer: coarse) and (max-width: 600px)";

// Opt-in: the user taps the button on the rotate cover (RotateHint).
let forced = false;
const forcedListeners = new Set<() => void>();

export function forceLandscape(): void {
  forced = true;
  forcedListeners.forEach((cb) => cb());
}

/** The user turned the phone to landscape: the choice lapses (portrait again shows the cover). */
export function clearForced(): void {
  if (!forced) return;
  forced = false;
  forcedListeners.forEach((cb) => cb());
}

/** Is this a portrait phone right now? Re-renders when the phone is turned. */
export function usePortraitPhone(): boolean {
  return useSyncExternalStore(
    (cb) => {
      const mq = window.matchMedia(ROTATE_QUERY);
      mq.addEventListener("change", cb);
      return () => mq.removeEventListener("change", cb);
    },
    () => window.matchMedia(ROTATE_QUERY).matches,
  );
}

export function subscribeForced(cb: () => void): () => void {
  forcedListeners.add(cb);
  return () => forcedListeners.delete(cb);
}

export function isForced(): boolean {
  return forced;
}

export interface LocalRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * Design px per screen px (the factor the stage's `zoom` contributes). Recorded
 * by `Stage` rather than measured back out of the DOM: browsers disagree on
 * what `offsetWidth` reports under `zoom`, while the scale is already a known
 * number.
 */
export function screenScale(): number {
  return stageScale();
}

/** `el`'s screen box in `ref`'s own (unrotated, unscaled) pixel space. */
export function localRect(ref: HTMLElement, el: HTMLElement): LocalRect {
  const r = ref.getBoundingClientRect();
  const e = el.getBoundingClientRect();
  const k = screenScale();
  if (!isStageRotated()) return { left: (e.left - r.left) / k, top: (e.top - r.top) / k, width: e.width / k, height: e.height / k };
  return { left: (e.top - r.top) / k, top: (r.right - e.right) / k, width: e.height / k, height: e.width / k };
}

/** A client point in `ref`'s own pixel space. */
export function localPoint(ref: HTMLElement, cx: number, cy: number): { x: number; y: number } {
  const r = ref.getBoundingClientRect();
  const k = screenScale();
  return isStageRotated() ? { x: (cy - r.top) / k, y: (r.right - cx) / k } : { x: (cx - r.left) / k, y: (cy - r.top) / k };
}

/** A client-pixel move as a move in `ref`'s own pixel space. `ref` is kept for
 *  the shared call shape with `localRect` / `localPoint`; the scale is recorded,
 *  not measured off the element. */
export function localDelta(_ref: HTMLElement, dx: number, dy: number): { x: number; y: number } {
  const k = screenScale();
  return isStageRotated() ? { x: dy / k, y: -dx / k } : { x: dx / k, y: dy / k };
}
