// Hover-or-hold: the pointer-hover peeks (seat highlight, markers, graveyard)
// have no hover on a phone, so a touch held on the element past HOLD_MS opens
// the same peek and releasing closes it. Mice keep plain enter / leave; the
// compatibility mouse events a tap emits are ignored so a tap never leaves a
// peek stuck open.

import type { MouseEvent, PointerEvent } from "react";

const HOLD_MS = 350;
const MOVE_PX = 10;
/** Compat mouse events follow a touch within this window. */
const TOUCH_MOUSE_MS = 1000;

let lastTouch = 0;
/** When the last hold fired: the click that ends it must not act. */
let heldAt = -Infinity;

export interface HoldProps {
  onMouseEnter: () => void;
  onMouseLeave: () => void;
  onPointerDown: (e: PointerEvent) => void;
  onPointerMove: (e: PointerEvent) => void;
  onPointerUp: (e: PointerEvent) => void;
  onPointerCancel: (e: PointerEvent) => void;
  onContextMenu: (e: MouseEvent) => void;
  onClickCapture: (e: MouseEvent) => void;
}

/** Handlers giving `enter` / `leave` on mouse hover and on touch hold. The
 *  innermost holder wins (pointerdown stops at it), so nested ones do not stack. */
export function holdPeek(enter: () => void, leave: () => void): HoldProps {
  let timer = 0;
  let on = false;
  let x = 0;
  let y = 0;
  const stop = () => {
    clearTimeout(timer);
    timer = 0;
    if (on) {
      on = false;
      leave();
    }
  };
  const fromTouch = () => Date.now() - lastTouch < TOUCH_MOUSE_MS;
  return {
    onMouseEnter: () => { if (!fromTouch()) enter(); },
    onMouseLeave: () => { if (!fromTouch()) leave(); },
    onPointerDown: (e) => {
      if (e.pointerType !== "touch") return;
      e.stopPropagation();
      lastTouch = Date.now();
      x = e.clientX;
      y = e.clientY;
      clearTimeout(timer);
      timer = window.setTimeout(() => {
        timer = 0;
        on = true;
        heldAt = Date.now();
        enter();
      }, HOLD_MS);
    },
    onPointerMove: (e) => {
      if (e.pointerType === "touch" && timer && Math.hypot(e.clientX - x, e.clientY - y) > MOVE_PX) stop();
    },
    onPointerUp: (e) => {
      if (e.pointerType !== "touch") return;
      lastTouch = Date.now();
      stop();
    },
    onPointerCancel: stop,
    // The long-press callout menu would cover the peek.
    onContextMenu: (e) => { if (Date.now() - lastTouch < TOUCH_MOUSE_MS * 2) e.preventDefault(); },
    // Releasing a hold is not a tap.
    onClickCapture: (e) => {
      if (Date.now() - heldAt < 600) {
        e.stopPropagation();
        e.preventDefault();
      }
    },
  };
}
