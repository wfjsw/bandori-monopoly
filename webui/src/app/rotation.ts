import { useSyncExternalStore } from "react";

// A portrait phone shows the stage turned a quarter clockwise (see Stage.tsx),
// so what a user sees as "up" is the design's "left". Every place that maps a
// screen box / pointer move back into design pixels goes through here.

/** A phone held upright: the stage is rotated to landscape instead of asking for a turn. */
export const ROTATE_QUERY = "(orientation: portrait) and (pointer: coarse) and (max-width: 600px)";

let rotated = false;

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

/** Set by `Stage` each render. */
export function setStageRotated(on: boolean): void {
  rotated = on;
}

export function isStageRotated(): boolean {
  return rotated;
}

export interface LocalRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Design px per screen px for `ref` (the factor the stage's scale contributes). */
export function screenScale(ref: HTMLElement): number {
  const r = ref.getBoundingClientRect();
  return (rotated ? r.height : r.width) / (ref.offsetWidth || 1) || 1;
}

/** `el`'s screen box in `ref`'s own (unrotated, unscaled) pixel space. */
export function localRect(ref: HTMLElement, el: HTMLElement): LocalRect {
  const r = ref.getBoundingClientRect();
  const e = el.getBoundingClientRect();
  const k = screenScale(ref);
  if (!rotated) return { left: (e.left - r.left) / k, top: (e.top - r.top) / k, width: e.width / k, height: e.height / k };
  return { left: (e.top - r.top) / k, top: (r.right - e.right) / k, width: e.height / k, height: e.width / k };
}

/** A client point in `ref`'s own pixel space. */
export function localPoint(ref: HTMLElement, cx: number, cy: number): { x: number; y: number } {
  const r = ref.getBoundingClientRect();
  const k = screenScale(ref);
  return rotated ? { x: (cy - r.top) / k, y: (r.right - cx) / k } : { x: (cx - r.left) / k, y: (cy - r.top) / k };
}

/** A client-pixel move as a move in `ref`'s own pixel space. */
export function localDelta(ref: HTMLElement, dx: number, dy: number): { x: number; y: number } {
  const k = screenScale(ref);
  return rotated ? { x: dy / k, y: -dx / k } : { x: dx / k, y: dy / k };
}
