// PC click vs touch double-tap, for the "this whole face toggles a thing" grids
// (the deck-pick pool and working strip). A fine pointer toggles on one click;
// a coarse one previews on the first tap and toggles only on a second tap of
// the same card. Compat mouse events a touch emits are folded into that tap
// (hold.ts's TOUCH_MOUSE_MS) so one tap never runs both paths.

import { useRef } from "react";
import { useMountEffect } from "./mount";

/** Compat mouse events follow a touch within this window (mirrors hold.ts). */
const TOUCH_MOUSE_MS = 1000;
/** A second tap on the same id inside this window is the toggle. */
const DOUBLE_TAP_MS = 300;

/** Touch bookkeeping is module-level: a tap's compat mouse click is global, not per card. */
let lastTouch = 0;

export interface CardTapOpts {
  /** Standing preview: every hover / tap / click feeds it; never a mutation. */
  onPreview: (id: string) => void;
  /** The one mutation (add / remove). Must no-op when input is locked (托管). */
  onToggle: (id: string) => void;
}

/**
 * Card-face click handler factory for the interaction table:
 *
 * |            | preview    | toggle       |
 * | PC (fine)  | hover      | single click |
 * | touch      | single tap | double tap   |
 *
 * Preview is non-destructive. Detect the mode with `matchMedia("(pointer:
 * coarse)")` (the repo's signal) plus the hold.ts touch clock, so a touch on a
 * fine-pointer machine still takes the double-tap path and its compat mouse
 * click never toggles on its own.
 */
export function useCardTap({ onPreview, onToggle }: CardTapOpts): (id: string) => () => void {
  const lastTap = useRef<{ id: string; t: number } | null>(null);
  useMountEffect(() => {
    const onTouch = () => {
      lastTouch = Date.now();
    };
    const onPointer = (e: globalThis.PointerEvent) => {
      if (e.pointerType === "touch") lastTouch = Date.now();
    };
    window.addEventListener("touchstart", onTouch, true);
    window.addEventListener("pointerdown", onPointer, true);
    return () => {
      window.removeEventListener("touchstart", onTouch, true);
      window.removeEventListener("pointerdown", onPointer, true);
    };
  });

  return (id: string) => () => {
    const now = Date.now();
    const fromTouch = now - lastTouch < TOUCH_MOUSE_MS;
    const coarse = fromTouch || window.matchMedia("(pointer: coarse)").matches;
    onPreview(id);
    if (!coarse) {
      lastTap.current = null;
      onToggle(id);
      return;
    }
    // Touch: the first tap only previews (the standing panel). A second tap on
    // the same card inside DOUBLE_TAP_MS toggles. One tap is one `click` -- its
    // compat mouse events are not a second tap, and `fromTouch` above keeps a
    // real mouse click right after a touch from taking the fine-pointer path.
    const t = lastTap.current;
    if (t && t.id === id && now - t.t <= DOUBLE_TAP_MS) {
      lastTap.current = null;
      onToggle(id);
    } else {
      lastTap.current = { id, t: now };
    }
  };
}