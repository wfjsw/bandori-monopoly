// Pan + zoom for the tile board. Only the ring (and its interior) is
// transformed -- prompts, panels, the banners and the card flash sit outside
// the zoom layer so they keep their size and screen position at any zoom.
//
// Coordinates are the board window's own pixels: the map fills its slot, so
// the board rect is whatever the match screen gives it (the 12 x 10 cell grid
// stretches to that width and height). The zoom layer inside the wrap carries
// `translate(tx px, ty px) scale(z)` with `transform-origin: 0 0`: a board
// point (x, y) lands at (x * z + tx, y * z + ty) in board pixels. `z = 1` is
// "fit": the whole board fills the window at zero pan; dragging may still move it
// up to `OVERPAN` of a window past any edge.

import {
  useCallback, useRef, useState,
  type CSSProperties, type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent, type RefObject,
} from "react";
import { useEventListener } from "../../hooks/dom.ts";
import { useResizeObserver } from "../../hooks/measure.ts";

/** Smallest board window (stage px) the map is ever laid out at. */
export const MIN_BOARD = 240;

/** The board's width : height stays within these. Never narrower than square;
 *  never wider than 16:10 -- the 12 x 10 grid's cells are then 4:3 at most,
 *  beyond which corner tiles turn into wide bars and names float in them. */
export const MIN_RATIO = 1;
export const MAX_RATIO = 1.6;

/** The board rect for a map slot: the whole slot when its ratio is in range,
 *  else the largest rect of the nearest allowed ratio (centred by the slot). */
export function boardRect(slotW: number, slotH: number): { width: number; height: number } {
  const w = Math.max(MIN_BOARD, slotW);
  const h = Math.max(MIN_BOARD, slotH);
  if (w > h * MAX_RATIO) return { width: Math.round(h * MAX_RATIO), height: h };
  if (w < h * MIN_RATIO) return { width: w, height: Math.round(w / MIN_RATIO) };
  return { width: w, height: h };
}

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

/** How far past its edge the board may be dragged, as a fraction of the
 *  window on that axis: half a window of overscroll on every side. */
export const OVERPAN = 0.5;

/** Keep an `w x h` board scaled by `z` within reach of the `w x h` window: its
 *  edges may be dragged up to `OVERPAN` of the window past the window's edges
 *  (so a corner tile can be brought to the middle), never further. The rect is
 *  the actual board size (any aspect): the math is per-axis. */
export function clampPan(v: Viewport, w: number, h: number): Viewport {
  const z = clampZoom(v.z);
  const loX = w * (1 - z) - w * OVERPAN;
  const loY = h * (1 - z) - h * OVERPAN;
  return {
    z,
    tx: Math.min(w * OVERPAN, Math.max(loX, v.tx)),
    ty: Math.min(h * OVERPAN, Math.max(loY, v.ty)),
  };
}

/** Scale to `z`, keeping the window point (`px`, `py`) fixed on the board
 *  (wheel / pinch zooming around the cursor). */
export function zoomAround(v: Viewport, px: number, py: number, z: number, w: number, h: number): Viewport {
  const z2 = clampZoom(z);
  // Zooming all the way out lands on the fit view, overscroll and all.
  if (z2 <= MIN_ZOOM) return fitViewport();
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

/** Client px -> board px. The wrap is laid out in board pixels (its own
 *  offset size) and the stage scales that to the screen, so the net factor is
 *  `client width / offset width` on both axes. */
function toLocal(el: HTMLElement, clientX: number, clientY: number): Point {
  const r = el.getBoundingClientRect();
  const k = r.width / (el.offsetWidth || 1) || 1;
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
  /** The board window's size in stage pixels (it fills the map slot). */
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
  // The board window: the map slot's rectangle held to MIN_RATIO..MAX_RATIO
  // (`boardRect`; the tiles stretch to fill it). Measured in JS --
  // `container-type: size` was unreliable here -- and board pixels are those
  // stage pixels 1:1. A new rect keeps the user's zoom and re-clamps the pan
  // to it: the ResizeObserver callback is the one place the two change
  // together, so there is no effect mirroring `box` into `v`.
  const [box, setBox] = useState({ width: MIN_BOARD, height: MIN_BOARD });
  const boxRef = useRef(box);
  boxRef.current = box;
  useResizeObserver(() => wrapRef.current?.parentElement ?? null, () => {
    const slot = wrapRef.current?.parentElement;
    if (!slot) return;
    const next = boardRect(slot.clientWidth, slot.clientHeight);
    setBox(next);
    setV((cur) => clampPan(cur, next.width, next.height));
  });

  const zoomAt = useCallback((dir: 1 | -1) => {
    const { width: w, height: h } = boxRef.current;
    setV((cur) => stepZoom(cur, dir, w / 2, h / 2, w, h));
  }, []);
  const zoomIn = useCallback(() => zoomAt(1), [zoomAt]);
  const zoomOut = useCallback(() => zoomAt(-1), [zoomAt]);
  const reset = useCallback(() => setV(fitViewport()), []);

  // Wheel needs a non-passive listener: React's synthetic wheel is passive, so
  // `preventDefault` there would not stop the browser's own ctrl+wheel zoom.
  useEventListener(wrapRef, "wheel", (e) => {
    const el = wrapRef.current;
    if (!el) return;
    const ev = e as WheelEvent;
    // A scrollable pocket (the field-card list) keeps its own wheel, but only
    // when it can actually scroll -- over a short list the wheel zooms.
    const t = ev.target as Element | null;
    const scroller = t && typeof t.closest === "function" ? t.closest("[data-vp-scroll]") : null;
    if (scroller instanceof HTMLElement && scroller.scrollHeight > scroller.clientHeight + 1) return;
    ev.preventDefault();
    const p = toLocal(el, ev.clientX, ev.clientY);
    const dy = ev.deltaMode === 1 ? ev.deltaY * 16 : ev.deltaMode === 2 ? ev.deltaY * 400 : ev.deltaY;
    const { width: w, height: h } = boxRef.current;
    setV((cur) => wheelZoom(cur, p.x, p.y, dy, w, h));
  }, { passive: false });

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
    const { width: w, height: h } = boxRef.current;
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
      const k = el.getBoundingClientRect().width / (el.offsetWidth || 1) || 1;
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
    box,
    zoom: v.z,
    zoomIn, zoomOut, reset,
    onPointerDown, onPointerMove, onPointerUp,
    onClickCapture, onDoubleClick, onKeyDown,
  };
}