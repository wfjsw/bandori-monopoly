// Pan + zoom for the tile board. Only the ring (and its interior) is
// transformed -- prompts, panels, the banner and the card flash sit outside
// the zoom layer so they keep their size and screen position at any zoom.
//
// Coordinates are the board's own pixels (856 x 713.33, the TTS 6:5 box). The
// board window (the wrap) is that box scaled to fit the middle column
// (`fit`), and the zoom layer inside it carries `translate(tx px, ty px)
// scale(z)` with `transform-origin: 0 0`: a board point (x, y) lands at
// (x * z + tx, y * z + ty) in board pixels. `z = 1` is "fit": the whole board
// fills the window, so the pan pins to zero.

import {
  useCallback, useEffect, useRef, useState,
  type CSSProperties, type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent, type RefObject,
} from "react";

/** The generated board's size in its own pixels: 12 x 10 square cells. */
export const BOARD_W = 856;
export const BOARD_H = BOARD_W * 10 / 12;

export interface Viewport {
  /** User zoom, 1 = fit. */
  z: number;
  /** Pan in board pixels. */
  tx: number;
  ty: number;
}

export interface Point {
  x: number;
  y: number;
}

/** Fit view (the pre-zoom default: the whole ring, no pan). */
export const MIN_ZOOM = 1;
export const MAX_ZOOM = 3;
/** Screen-pixel movement before a press turns into a pan. Clicks stay under
 *  this so tiles, field cards and tokens remain tappable. */
export const DRAG_THRESHOLD = 6;
/** Wheel pixels (at deltaMode 0) per e-fold of zoom. */
const WHEEL_K = 0.0022;
/** Each button / key step multiplies the zoom by this. */
const STEP = 1.25;

export const fitViewport = (): Viewport => ({ z: MIN_ZOOM, tx: 0, ty: 0 });

export function clampZoom(z: number): number {
  if (!Number.isFinite(z)) return MIN_ZOOM;
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));
}

/** Keep an `w x h` board scaled by `z` covering the `w x h` window: never a
 *  gap at the edges (so the board can't be dragged out of view). At `z = 1`
 *  the board exactly covers the window, which pins the pan to zero. */
export function clampPan(v: Viewport, w: number, h: number): Viewport {
  const z = clampZoom(v.z);
  const loX = w * (1 - z);
  const loY = h * (1 - z);
  return {
    z,
    tx: Math.min(0, Math.max(loX, v.tx)),
    ty: Math.min(0, Math.max(loY, v.ty)),
  };
}

/** Scale to `z`, keeping the window point (`px`, `py`) fixed on the board
 *  (wheel / pinch zooming around the cursor). */
export function zoomAround(v: Viewport, px: number, py: number, z: number, w: number, h: number): Viewport {
  const z2 = clampZoom(z);
  const k = z2 / clampZoom(v.z);
  return clampPan({ z: z2, tx: px - (px - v.tx) * k, ty: py - (py - v.ty) * k }, w, h);
}

/** Mouse wheel / trackpad pinch step. `dy` is the normalized wheel delta
 *  (positive = wheel down = zoom out). */
export function wheelZoom(v: Viewport, px: number, py: number, dy: number, w: number, h: number): Viewport {
  return zoomAround(v, px, py, clampZoom(v.z) * Math.exp(-dy * WHEEL_K), w, h);
}

/** Button / keyboard step (dir > 0 zooms in), around (`px`, `py`). */
export function stepZoom(v: Viewport, dir: 1 | -1, px: number, py: number, w: number, h: number): Viewport {
  return zoomAround(v, px, py, clampZoom(v.z) * (dir > 0 ? STEP : 1 / STEP), w, h);
}

/** Two-finger pinch, recomputed from the gesture start so it cannot drift:
 *  scale `k` (current span / starting span) around the moving midpoint. */
export function pinchAround(v0: Viewport, mid0: Point, mid: Point, k: number, w: number, h: number): Viewport {
  const z = clampZoom(clampZoom(v0.z) * k);
  const bx = (mid0.x - v0.tx) / clampZoom(v0.z);
  const by = (mid0.y - v0.ty) / clampZoom(v0.z);
  return clampPan({ z, tx: mid.x - bx * z, ty: mid.y - by * z }, w, h);
}

/** Drag by (`dx`, `dy`) in the wrap's design pixels. */
export function panBy(v: Viewport, dx: number, dy: number, w: number, h: number): Viewport {
  return clampPan({ z: v.z, tx: v.tx + dx, ty: v.ty + dy }, w, h);
}

function toLocal(el: HTMLElement, clientX: number, clientY: number): Point {
  const r = el.getBoundingClientRect();
  // The wrap is the board box scaled to the window (and the stage scales that
  // again); `r.width / BOARD_W` is the net factor from board pixels to screen,
  // so dividing by it lands back in board pixels on both axes.
  const k = r.width / BOARD_W || 1;
  return { x: (clientX - r.left) / k, y: (clientY - r.top) / k };
}

interface Gesture {
  /** Active pointers by id, in client pixels. */
  pointers: Map<number, Point>;
  last: Point;
  moved: number;
  dragging: boolean;
  /** Set once a drag / pinch started: swallows the click it would fire. */
  dragged: boolean;
  pinch: { v0: Viewport; mid0: Point; d0: number } | null;
}

export interface ViewportApi {
  wrapRef: RefObject<HTMLDivElement | null>;
  /** `translate + scale` for the zoom layer (board pixels). */
  style: CSSProperties;
  /** Scale of the board box filling its window (856 px -> the wrap's width). */
  fit: number;
  /** The board window's size in stage pixels (6:5). */
  box: { width: number; height: number };
  zoom: number;
  zoomIn: () => void;
  zoomOut: () => void;
  reset: () => void;
  onPointerDown: (e: ReactPointerEvent) => void;
  onPointerMove: (e: ReactPointerEvent) => void;
  onPointerUp: (e: ReactPointerEvent) => void;
  onClickCapture: (e: ReactMouseEvent) => void;
  /** Resets the view on a double-click of empty board area (`data-vp-bg`). */
  onDoubleClick: (e: ReactMouseEvent) => void;
  onKeyDown: (e: ReactKeyboardEvent) => void;
}

/** Pan / zoom state and handlers for the board wrap. See `viewport.ts` for the
 *  math; put `data-vp-bg` on the empty board containers (wrap / ring /
 *  interior) and `data-vp-ctl` on the zoom buttons. */
export function useBoardViewport(): ViewportApi {
  const [v, setV] = useState<Viewport>(fitViewport);
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const vRef = useRef(v);
  vRef.current = v;
  const gesture = useRef<Gesture>({
    pointers: new Map(), last: { x: 0, y: 0 }, moved: 0,
    dragging: false, dragged: false, pinch: null,
  });
  // The board window: the largest 6:5 rect in the slot (the middle column).
  // Measured in JS -- `container-type: size` was unreliable here -- and the
  // same number gives the fit scale (window px / board px).
  const [box, setBox] = useState({ width: BOARD_W, height: BOARD_H });
  useEffect(() => {
    const el = wrapRef.current;
    const slot = el?.parentElement;
    if (!el || !slot) return;
    const measure = () => {
      const w = Math.min(slot.clientWidth, slot.clientHeight * (BOARD_W / BOARD_H));
      setBox({ width: Math.max(240, w), height: Math.max(240, w) * (BOARD_H / BOARD_W) });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(slot);
    return () => ro.disconnect();
  }, []);
  const fit = box.width / BOARD_W;

  const zoomAt = useCallback((dir: 1 | -1) => {
    setV((cur) => stepZoom(cur, dir, BOARD_W / 2, BOARD_H / 2, BOARD_W, BOARD_H));
  }, []);
  const zoomIn = useCallback(() => zoomAt(1), [zoomAt]);
  const zoomOut = useCallback(() => zoomAt(-1), [zoomAt]);
  const reset = useCallback(() => setV(fitViewport()), []);

  // Wheel needs a non-passive listener: React's synthetic wheel is passive, so
  // `preventDefault` there would not stop the browser's own ctrl+wheel zoom.
  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      // A scrollable pocket (the field-card list) keeps its own wheel, but only
      // when it can actually scroll -- over a short list the wheel zooms.
      const t = e.target as Element | null;
      const scroller = t && typeof t.closest === "function" ? t.closest("[data-vp-scroll]") : null;
      if (scroller instanceof HTMLElement && scroller.scrollHeight > scroller.clientHeight + 1) return;
      e.preventDefault();
      const p = toLocal(el, e.clientX, e.clientY);
      const dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaMode === 2 ? e.deltaY * 400 : e.deltaY;
      setV((cur) => wheelZoom(cur, p.x, p.y, dy, BOARD_W, BOARD_H));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  const onPointerDown = useCallback((e: ReactPointerEvent) => {
    const g = gesture.current;
    // A fresh press always clears the drag flag first: the click of a press on
    // the zoom buttons (which is not a drag) must not be swallowed by the one
    // before it.
    g.dragged = false;
    if (e.pointerType === "mouse" && e.button !== 0) return;
    // The zoom buttons are not a drag handle.
    const t = e.target as Element | null;
    if (t && typeof t.closest === "function" && t.closest("[data-vp-ctl]")) return;
    const el = wrapRef.current;
    if (!el) return;
    g.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (g.pointers.size === 1) {
      g.last = { x: e.clientX, y: e.clientY };
      g.moved = 0;
      g.dragging = false;
      g.pinch = null;
    } else if (g.pointers.size === 2) {
      const [a, b] = [...g.pointers.values()];
      const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
      g.pinch = {
        v0: vRef.current,
        mid0: toLocal(el, mid.x, mid.y),
        d0: Math.hypot(a.x - b.x, a.y - b.y) || 1,
      };
      g.dragging = true;
      g.dragged = true;
    }
  }, []);

  const onPointerMove = useCallback((e: ReactPointerEvent) => {
    const g = gesture.current;
    if (!g.pointers.has(e.pointerId)) return;
    g.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    const el = wrapRef.current;
    if (!el) return;
    const w = BOARD_W;
    const h = BOARD_H;
    if (g.pointers.size >= 2 && g.pinch) {
      const [a, b] = [...g.pointers.values()];
      const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
      const d = Math.hypot(a.x - b.x, a.y - b.y) || 1;
      setV(() => pinchAround(g.pinch!.v0, g.pinch!.mid0, toLocal(el, mid.x, mid.y), d / g.pinch!.d0, w, h));
      g.dragging = true;
      g.dragged = true;
      return;
    }
    const cur = { x: e.clientX, y: e.clientY };
    const dx = cur.x - g.last.x;
    const dy = cur.y - g.last.y;
    g.last = cur;
    g.moved += Math.hypot(dx, dy);
    if (!g.dragging && g.moved >= DRAG_THRESHOLD) {
      g.dragging = true;
      g.dragged = true;
      // Capture only once this is a real drag: a tap / click keeps its own
      // target, so tile and card clicks still land on the tile and card.
      try {
        el.setPointerCapture(e.pointerId);
      } catch {
        /* pointer already gone */
      }
    }
    if (g.dragging) {
      const k = el.getBoundingClientRect().width / BOARD_W || 1;
      setV((curV) => panBy(curV, dx / k, dy / k, w, h));
    }
  }, []);

  const onPointerUp = useCallback((e: ReactPointerEvent) => {
    const g = gesture.current;
    if (!g.pointers.delete(e.pointerId)) return;
    if (g.pointers.size < 2) g.pinch = null;
    if (g.pointers.size === 1) {
      // One finger left after a pinch: keep panning with it, without a jump.
      const [p] = [...g.pointers.values()];
      g.last = { ...p };
      g.moved = DRAG_THRESHOLD;
      g.dragging = true;
    }
    if (g.pointers.size === 0) {
      g.dragging = false;
      // The click that follows a drag must not reach a tile; a later click
      // (or a keyboard-activated button) must.
      if (g.dragged) {
        window.setTimeout(() => {
          g.dragged = false;
        }, 0);
      }
    }
    try {
      wrapRef.current?.releasePointerCapture(e.pointerId);
    } catch {
      /* not captured */
    }
  }, []);

  const onClickCapture = useCallback((e: ReactMouseEvent) => {
    const g = gesture.current;
    if (!g.dragged) return;
    g.dragged = false;
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const onDoubleClick = useCallback((e: ReactMouseEvent) => {
    const t = e.target as Element | null;
    // Only empty board area resets -- never a tile, card or button.
    if (!t || typeof t.closest !== "function" || t.closest("[data-vp-bg]") !== t) return;
    reset();
  }, [reset]);

  const onKeyDown = useCallback((e: ReactKeyboardEvent) => {
    const t = e.target as HTMLElement | null;
    if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) return;
    if (e.ctrlKey || e.metaKey || e.altKey) return; // keep ctrl+0 etc. for the browser
    if (e.key === "+" || e.key === "=") {
      e.preventDefault();
      zoomAt(1);
    } else if (e.key === "-" || e.key === "_") {
      e.preventDefault();
      zoomAt(-1);
    } else if (e.key === "0") {
      e.preventDefault();
      reset();
    }
  }, [zoomAt, reset]);

  return {
    wrapRef,
    style: { transform: `translate(${v.tx}px, ${v.ty}px) scale(${v.z})` },
    fit,
    box,
    zoom: v.z,
    zoomIn, zoomOut, reset,
    onPointerDown, onPointerMove, onPointerUp,
    onClickCapture, onDoubleClick, onKeyDown,
  };
}