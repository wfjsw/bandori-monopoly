// Hover-or-hold: the pointer-hover peeks (seat highlight, markers, graveyard)
// have no hover on a phone, so a touch held on the element past HOLD_MS opens
// the same peek, and lifting the finger closes it. Mice keep plain enter / leave; the compatibility mouse events a
// touch emits are ignored so a tap never leaves a peek stuck open.
//
// State is module-level, not per call: `holdPeek` is called on every render, so
// a closure would lose the open peek the moment the board re-renders.

import type { MouseEvent, PointerEvent } from "react";

const HOLD_MS = 350;
const MOVE_PX = 10;
/** Compat mouse events follow a touch within this window. */
const TOUCH_MOUSE_MS = 1000;

let lastTouch = 0;
/** Clicks are swallowed until this time: Infinity while a hold is open, then a
 *  short grace after release for the click that ends it. */
let swallowUntil = -Infinity;
let timer = 0;
let startX = 0;
let startY = 0;
/** The open (touch-held) peek's way to close it. */
let open: (() => void) | null = null;

function dismiss(): void {
  const o = open;
  open = null;
  if (o) {
    swallowUntil = Date.now() + 500;
    o();
  }
}

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
  const fromTouch = () => Date.now() - lastTouch < TOUCH_MOUSE_MS;
  const cancel = () => {
    clearTimeout(timer);
    timer = 0;
  };
  return {
    onMouseEnter: () => { if (!fromTouch()) enter(); },
    onMouseLeave: () => { if (!fromTouch()) leave(); },
    onPointerDown: (e) => {
      if (e.pointerType !== "touch") return;
      e.stopPropagation();
      swallowUntil = -Infinity;
      lastTouch = Date.now();
      startX = e.clientX;
      startY = e.clientY;
      cancel();
      timer = window.setTimeout(() => {
        timer = 0;
        dismiss();
        open = leave;
        swallowUntil = Infinity;
        enter();
      }, HOLD_MS);
    },
    onPointerMove: (e) => {
      if (e.pointerType === "touch" && timer && Math.hypot(e.clientX - startX, e.clientY - startY) > MOVE_PX) cancel();
    },
    onPointerUp: (e) => {
      if (e.pointerType !== "touch") return;
      lastTouch = Date.now();
      cancel();
      dismiss();
    },
    onPointerCancel: () => {
      cancel();
      dismiss();
    },
    // The long-press callout menu would cover the peek.
    onContextMenu: (e) => { if (Date.now() - lastTouch < TOUCH_MOUSE_MS * 2) e.preventDefault(); },
    // Releasing a hold is not a tap.
    onClickCapture: (e) => {
      if (Date.now() < swallowUntil) {
        e.stopPropagation();
        e.preventDefault();
      }
    },
  };
}
