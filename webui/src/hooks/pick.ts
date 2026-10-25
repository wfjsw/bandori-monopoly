// A pick is a lens, not a mode: the field row's coarse first tap presents one
// card, and any pointer-down elsewhere drops it again (the row retracts with
// it). A pick on the picked card's own slot is not "elsewhere" -- that is the
// two-step's second tap, which opens the detail sheet.

import { useEffect } from "react";

/** Marker on the picked element's slot: `data-pick="<key>"`. */
export const PICK_ATTR = "data-pick";

/**
 * Is `target` inside the slot currently carrying `picked`? `null` / a target
 * with no slot is elsewhere. Pure -- the pointerdown filter and its tests read
 * the same rule.
 */
export function isPickedSlot(target: EventTarget | null, picked: string): boolean {
  const el = target as Element | null;
  if (!el?.closest) return false;
  const slot = el.closest(`[${PICK_ATTR}]`) as HTMLElement | null;
  return slot?.dataset.pick === picked;
}

/**
 * Drop `picked` when a pointer goes down anywhere outside that card's slot.
 * Capture phase, so a nested `stopPropagation` (the hold-peek) never hides the
 * gesture; the listener only clears -- it never swallows the click underneath.
 */
export function usePickDeselect(picked: string | null, clear: () => void): void {
  useEffect(() => {
    if (!picked) return;
    const onDown = (e: Event) => {
      if (!isPickedSlot(e.target, picked)) clear();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => document.removeEventListener("pointerdown", onDown, true);
  }, [picked, clear]);
}