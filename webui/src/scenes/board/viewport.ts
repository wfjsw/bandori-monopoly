// Pan + zoom for the tile board. Only the ring (and its interior) is
// transformed -- prompts, panels, the banners and the card flash sit outside
// the zoom layer so they keep their size and screen position at any zoom.
//
// Two rectangles live in the map slot: the *window* (the wrap, which clips) is
// the whole slot -- 100% of its width and height -- and the *board* (the tile
// ring plus its interior) keeps the slot held to MIN_RATIO..MAX_RATIO
// (`boardRect`) and sits centred in the window at fit. The map never stretches
// to a long thin slot; the pannable view of it does.
//
// Coordinates are the window's own pixels: the zoom layer carries
// `translate(tx px, ty px) scale(z)` with `transform-origin: 0 0`, so a board
// point (x, y) -- 0..boardW / 0..boardH -- lands at (x * z + tx, y * z + ty) in
// window pixels. `z = 1` is "fit": the whole board, centred, no pan. Dragging
// may still move it up to `OVERPAN` of the *window* past any edge.

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

/** The map's two rectangles. `win` is the clipping window (the whole slot);
 *  `board` is the tile ring + interior at its clamped aspect, centred in the
 *  window at fit. */
export interface MapBox {
  /** Window: the visible slot (clips). */
  winW: number;
  winH: number;
  /** Board: the ring + interior (keeps MIN_RATIO..MAX_RATIO). */
  boardW: number;
  boardH: number;
}

/** Both rectangles for a map slot. The window is the slot (floored at
 *  MIN_BOARD); the board is `boardRect` of it. */
export function mapBox(slotW: number, slotH: number): MapBox {
  const winW = Math.max(MIN_BOARD, slotW);
  const winH = Math.max(MIN_BOARD, slotH);
  const b = boardRect(winW, winH);
  return { winW, winH, boardW: b.width, boardH: b.height };
}

export interface Viewport {
  /** User zoom, 1 = fit. */
  z: number;
  /** Board top-left in window pixels. */
  tx: number;
  ty: number;
}

export interface Point {
  x: number;
  y: number;
}

/** Fit view (the pre-zoom default): the whole board, centred in the window. */
export const MIN_ZOOM = 1;
export const MAX_ZOOM = 3;
/** Screen-pixel movement before a press turns into a pan. Clicks stay under
 *  this so tiles, field cards and tokens remain tappable. */
export const DRAG_THRESHOLD = 6;
/** Wheel pixels (at deltaMode 0) per e-fold of zoom. */
const WHEEL_K = 0.0022;
/** Each button / key step multiplies the zoom by this. */
const STEP = 1.25;

/** Fit: the board centred in the window, whole and un-panned. */
export function fitViewport(b: MapBox): Viewport {
  return { z: MIN_ZOOM, tx: (b.winW - b.boardW) / 2, ty: (b.winH - b.boardH) / 2 };
}

export function clampZoom(z: number): number {
  if (!Number.isFinite(z)) return MIN_ZOOM;
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));
}

/** How far past its edge the board may be dragged, as a fraction of the
 *  *window* on that axis: half a window of overscroll on every side. */
export const OVERPAN = 0.5;

/** Keep the board (scaled by `z`) within reach of the window: the window may
 *  show up to `OVERPAN` of its own size of empty / off-board past either edge
 *  (so a corner tile can be brought to the middle), never more. The board is
 *  `boardW x boardH`; the window is `winW x winH` -- on a wide slot the board
 *  is narrower than the window and may sit anywhere that still covers the
 *  middle `1 - OVERPAN` of it. */
export function clampPan(v: Viewport, b: MapBox): Viewport {
  const z = clampZoom(v.z);
  // Board covers [tx, tx + boardW*z]; the window keeps at most OVERPAN of
  // itself empty on each side, so:
  //   tx <= winW * OVERPAN
  //   tx + boardW * z >= winW * (1 - OVERPAN)
  const loX = b.winW * (1 - OVERPAN) - b.boardW * z;
  const hiX = b.winW * OVERPAN;
  const loY = b.winH * (1 - OVERPAN) - b.boardH * z;
  const hiY = b.winH * OVERPAN;
  return {
    z,
    tx: Math.min(hiX, Math.max(loX, v.tx)),
    ty: Math.min(hiY, Math.max(loY, v.ty)),
  };
}

/** Scale to `z`, keeping the window point (`px`, `py`) fixed on the board
 *  (wheel / pinch zooming around the cursor). */
export function zoomAround(v: Viewport, px: number, py: number, z: number, b: MapBox): Viewport {
  const z2 = clampZoom(z);
  // Zooming all the way out lands on the fit view, overscroll and all.
  if (z2 <= MIN_ZOOM) return fitViewport(b);
  const k = z2 / clampZoom(v.z);
  return clampPan({ z: z2, tx: px - (px - v.tx) * k, ty: py - (py - v.ty) * k }, b);
}

/** Mouse wheel / trackpad pinch step. `dy` is the normalized wheel delta
 *  (positive = wheel down = zoom out). */
export function wheelZoom(v: Viewport, px: number, py: number, dy: number, b: MapBox): Viewport {
  return zoomAround(v, px, py, clampZoom(v.z) * Math.exp(-dy * WHEEL_K), b);
}

/** Button / keyboard step (dir > 0 zooms in), around (`px`, `py`). */
export function stepZoom(v: Viewport, dir: 1 | -1, px: number, py: number, b: MapBox): Viewport {
  return zoomAround(v, px, py, clampZoom(v.z) * (dir > 0 ? STEP : 1 / STEP), b);
}

/** Two-finger pinch, recomputed from the gesture start so it cannot drift:
 *  scale `k` (current span / starting span) around the moving midpoint. */
export function pinchAround(v0: Viewport, mid0: Point, mid: Point, k: number, b: MapBox): Viewport {
  const z = clampZoom(clampZoom(v0.z) * k);
  const bx = (mid0.x - v0.tx) / clampZoom(v0.z);
  const by = (mid0.y - v0.ty) / clampZoom(v0.z);
  return clampPan({ z, tx: mid.x - bx * z, ty: mid.y - by * z }, b);
}

/** Drag by (`dx`, `dy`) in the window's design pixels. */
export function panBy(v: Viewport, dx: number, dy: number, b: MapBox): Viewport {
  return clampPan({ z: v.z, tx: v.tx + dx, ty: v.ty + dy }, b);
}

/** Client px -> window px. The wrap is laid out in window pixels (its own
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
  /** `translate + scale` for the zoom layer (window pixels). */
  style: CSSProperties;
  /** The clipping window's size (the whole map slot). */
  box: { width: number; height: number };
  /** The board rect (ring + interior), centred in the window at fit. */
  board: { width: number; height: number };
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
  const [map, setMap] = useState<MapBox>(() => mapBox(MIN_BOARD, MIN_BOARD));
  const mapRef = useRef(map);
  mapRef.current = map;
  const [v, setV] = useState<Viewport>(() => fitViewport(map));
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const vRef = useRef(v);
  vRef.current = v;
  const gesture = useRef<Gesture>({
    pointers: new Map(), last: { x: 0, y: 0 }, moved: 0,
    dragging: false, dragged: false, pinch: null,
  });
  // The window is the map slot (`mapBox`); the board inside it keeps
  // MIN_RATIO..MAX_RATIO and is centred at fit. Measured in JS --
  // `container-type: size` was unreliable here -- and window pixels are those
  // stage pixels 1:1. A new box keeps the user's zoom and their pan relative to
  // fit (so the board stays where they put it as the slot re-centres), then
  // re-clamps: the ResizeObserver callback is the one place the two change
  // together, so there is no effect mirroring `map` into `v`.
  useResizeObserver(() => wrapRef.current?.parentElement ?? null, () => {
    const slot = wrapRef.current?.parentElement;
    if (!slot) return;
    const next = mapBox(slot.clientWidth, slot.clientHeight);
    const prev = mapRef.current;
    const oldFit = fitViewport(prev);
    const newFit = fitViewport(next);
    setMap(next);
    setV((cur) => clampPan({
      z: cur.z,
      tx: cur.tx + (newFit.tx - oldFit.tx),
      ty: cur.ty + (newFit.ty - oldFit.ty),
    }, next));
  });

  const zoomAt = useCallback((dir: 1 | -1) => {
    const b = mapRef.current;
    setV((cur) => stepZoom(cur, dir, b.winW / 2, b.winH / 2, b));
  }, []);
  const zoomIn = useCallback(() => zoomAt(1), [zoomAt]);
  const zoomOut = useCallback(() => zoomAt(-1), [zoomAt]);
  const reset = useCallback(() => setV(fitViewport(mapRef.current)), []);

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
    setV((cur) => wheelZoom(cur, p.x, p.y, dy, mapRef.current));
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
    const b = mapRef.current;
    if (g.pointers.size >= 2 && g.pinch) {
      const [a, c] = [...g.pointers.values()];
      const mid = { x: (a.x + c.x) / 2, y: (a.y + c.y) / 2 };
      const d = Math.hypot(a.x - c.x, a.y - c.y) || 1;
      setV(() => pinchAround(g.pinch!.v0, g.pinch!.mid0, toLocal(el, mid.x, mid.y), d / g.pinch!.d0, b));
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
      setV((curV) => panBy(curV, dx / k, dy / k, b));
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
    box: { width: map.winW, height: map.winH },
    board: { width: map.boardW, height: map.boardH },
    zoom: v.z,
    zoomIn, zoomOut, reset,
    onPointerDown, onPointerMove, onPointerUp,
    onClickCapture, onDoubleClick, onKeyDown,
  };
}