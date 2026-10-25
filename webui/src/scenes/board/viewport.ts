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
// window pixels. The default / reset view (`DEFAULT_ZOOM`, just under the
// board's own `FIT_ZOOM`) is the whole board, centred, no pan; the zoom floor
// (`MIN_ZOOM`) is that same 0.95. Dragging may still move the board up to
// `OVERPAN` of the *window* past any edge.

import {
  useCallback, useRef, useState,
  type CSSProperties, type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent, type RefObject,
} from "react";
import { useEventListener } from "../../hooks/dom.ts";
import { designBox } from "../../app/stageScale.ts";
import { useResizeObserver } from "../../hooks/measure.ts";
import { localDelta, localPoint } from "../../app/rotation.ts";

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

/** The board's own rect size in the window (the board's "1:1"). Not a view
 *  anyone lands on: the default sits just under it (DEFAULT_ZOOM). */
export const FIT_ZOOM = 1;
/** The zoom floor -- and the default / reset view (user request: default zoom
 *  0.95). At 0.95 the whole board shows centred with a small margin around
 *  it; the `1:1` / reset control and the first slot measure land here. */
export const MIN_ZOOM = 0.75;
export const DEFAULT_ZOOM = 0.9;
export const MAX_ZOOM = 3;
/** Screen-pixel movement before a press turns into a pan. Clicks stay under
 *  this so tiles, field cards and tokens remain tappable. */
export const DRAG_THRESHOLD = 6;
/** Wheel pixels (at deltaMode 0) per e-fold of zoom. */
const WHEEL_K = 0.0022;
/** Each button / key step multiplies the zoom by this. */
const STEP = 1.25;

/** The board centred in the window at zoom `z`, whole and un-panned. */
export function centeredViewport(b: MapBox, z: number = DEFAULT_ZOOM): Viewport {
  const z2 = clampZoom(z);
  return { z: z2, tx: (b.winW - b.boardW * z2) / 2, ty: (b.winH - b.boardH * z2) / 2 };
}

/** The default view: the whole board, a hair under its rect size, centred.
 *  The `1:1` / reset control and the first slot measure land here. */
export function fitViewport(b: MapBox): Viewport {
  return centeredViewport(b, DEFAULT_ZOOM);
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
  // Zooming out to fit (or below) lands on the centred view at that zoom --
  // the whole board visible, overscroll and all. The cursor anchor only
  // applies while zoomed in past fit.
  if (z2 <= FIT_ZOOM) return centeredViewport(b, z2);
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
  return localPoint(el, clientX, clientY);
}

/** Multi-touch / drag bookkeeping. Pure: the hook owns the DOM around it. */
export interface Gesture {
  /** Active pointers by id, in client pixels. */
  pointers: Map<number, Point>;
  last: Point;
  moved: number;
  dragging: boolean;
  /** Set once a drag / pinch started: swallows the click it would fire. */
  dragged: boolean;
  /** Pinch baseline: the pair it is computed on and the view when that pair
   *  formed. Rebased (never left stale) when a finger is swapped in. */
  pinch: { ids: [number, number]; v0: Viewport; mid0: Point; d0: number } | null;
}

export function emptyGesture(): Gesture {
  return {
    pointers: new Map(), last: { x: 0, y: 0 }, moved: 0,
    dragging: false, dragged: false, pinch: null,
  };
}

/**
 * Keep `g.pinch` on the pair it was started on while both are still down, and
 * rebase it the moment that pair breaks (a third finger taking over, say) so
 * the next move is measured from the view the fingers are actually on -- a
 * stale baseline makes the map jump to a view the gesture never asked for.
 * Below two pointers the pinch is dropped entirely.
 */
export function ensurePinch(g: Gesture, v: Viewport, local: (p: Point) => Point): void {
  if (g.pointers.size < 2) {
    g.pinch = null;
    return;
  }
  const ids = [...g.pointers.keys()];
  const keep: [number, number] =
    g.pinch && g.pointers.has(g.pinch.ids[0]) && g.pointers.has(g.pinch.ids[1])
      ? g.pinch.ids
      : [ids[0], ids[1]];
  if (g.pinch && g.pinch.ids[0] === keep[0] && g.pinch.ids[1] === keep[1]) return;
  const a = g.pointers.get(keep[0])!;
  const b = g.pointers.get(keep[1])!;
  const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
  g.pinch = {
    ids: keep,
    v0: v,
    mid0: local(mid),
    d0: Math.hypot(a.x - b.x, a.y - b.y) || 1,
  };
}

/** The pair a pinch is computed on, or `null` when there is no live pair. */
export function pinchPair(g: Gesture): { a: Point; b: Point; v0: Viewport; mid0: Point; d0: number } | null {
  if (!g.pinch) return null;
  const a = g.pointers.get(g.pinch.ids[0]);
  const b = g.pointers.get(g.pinch.ids[1]);
  if (!a || !b) return null;
  return { a, b, v0: g.pinch.v0, mid0: g.pinch.mid0, d0: g.pinch.d0 };
}

/**
 * Drop `id` from the gesture. `false` means it was not tracked -- the caller
 * (or a second delivery of the same up: wrap handler then window safety net)
 * must leave the gesture alone. One finger left keeps dragging without a jump;
 * none left ends the gesture. A pinch whose pair broke is cleared here and
 * rebased by `ensurePinch` on the next move if two fingers are still down.
 */
export function endPointer(g: Gesture, id: number): boolean {
  if (!g.pointers.delete(id)) return false;
  if (g.pinch && (!g.pointers.has(g.pinch.ids[0]) || !g.pointers.has(g.pinch.ids[1]))) {
    g.pinch = null;
  }
  if (g.pointers.size === 1) {
    const [p] = [...g.pointers.values()];
    g.last = { ...p };
    g.moved = DRAG_THRESHOLD;
    g.dragging = true;
    g.pinch = null;
  }
  if (g.pointers.size === 0) {
    g.dragging = false;
    g.pinch = null;
    g.moved = 0;
  }
  return true;
}

/**
 * Abandon every tracked pointer. The escape hatch for a pointer whose up never
 * arrives: the page is hidden or loses focus mid-pinch, iOS drops a `pointerup`
 * after the multi-touch `preventDefault`, or the browser simply cancels the
 * sequence without telling us. Without this a ghost finger stays in the map
 * and every later one-finger pan is read as a pinch around it.
 */
export function resetGesture(g: Gesture): void {
  g.pointers.clear();
  g.pinch = null;
  g.dragging = false;
  g.moved = 0;
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
  const gesture = useRef<Gesture>(emptyGesture());
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
    const slotBox = designBox(slot);
    const next = mapBox(slotBox.width, slotBox.height);
    const prev = mapRef.current;
    const oldFit = fitViewport(prev);
    const newFit = fitViewport(next);
    setMap(next);
    setV((cur) => {
      // First measurement (still the MIN_BOARD placeholder): land on the true
      // fit. A later slot change keeps the user's zoom and their pan relative
      // to fit, so the board stays where they put it as the slot re-centres.
      const first = prev.winW === MIN_BOARD && prev.winH === MIN_BOARD;
      if (first) return newFit;
      return clampPan({
        z: cur.z,
        tx: cur.tx + (newFit.tx - oldFit.tx),
        ty: cur.ty + (newFit.ty - oldFit.ty),
      }, next);
    });
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

  /** Local px for a client point on the wrap. */
  const toLocalOf = useCallback((p: Point) => {
    const el = wrapRef.current;
    return el ? toLocal(el, p.x, p.y) : p;
  }, []);

  /** Hold `id` on the wrap so its up / cancel is delivered even when the finger
   *  slides off the board or the tile under it unmounts mid-gesture. */
  const capture = useCallback((id: number) => {
    try {
      wrapRef.current?.setPointerCapture(id);
    } catch {
      /* pointer already gone */
    }
  }, []);

  /**
   * A tracked pointer ended (up, cancel, lost capture, or the window-level
   * safety net). Idempotent: the wrap's handler and the window listener both
   * call this for the same up, and the second is a no-op.
   */
  const finishPointer = useCallback((id: number) => {
    const g = gesture.current;
    if (!endPointer(g, id)) return;
    try {
      wrapRef.current?.releasePointerCapture(id);
    } catch {
      /* not captured */
    }
    // One finger left after a pinch: keep panning with it, now under capture
    // (its up must reach us even outside the wrap).
    if (g.pointers.size === 1) {
      const [left] = [...g.pointers.keys()];
      capture(left);
    }
    if (g.pointers.size === 0 && g.dragged) {
      // The click that follows a drag must not reach a tile; a later click
      // (or a keyboard-activated button) must.
      window.setTimeout(() => {
        g.dragged = false;
      }, 0);
    }
  }, [capture]);

  /** Abandon the whole gesture -- no further up is coming for these pointers. */
  const abandonGesture = useCallback(() => {
    const g = gesture.current;
    if (g.pointers.size === 0 && !g.pinch) return;
    for (const id of [...g.pointers.keys()]) {
      try {
        wrapRef.current?.releasePointerCapture(id);
      } catch {
        /* not captured */
      }
    }
    resetGesture(g);
    g.dragged = false;
  }, []);

  const onPointerDown = useCallback((e: ReactPointerEvent) => {
    const g = gesture.current;
    // A press that starts a fresh gesture clears the drag flag first: the click
    // of a press on the zoom buttons (which is not a drag) must not be swallowed
    // by the one before it. A *second* finger joining a drag must not clear it
    // -- that drag's own click is still on its way.
    if (g.pointers.size === 0) g.dragged = false;
    if (e.pointerType === "mouse" && e.button !== 0) return;
    if (!Number.isFinite(e.clientX) || !Number.isFinite(e.clientY)) return;
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
      return;
    }
    // Two or more fingers: this is a pinch (or a third finger joining one).
    // Capture every tracked pointer -- the click is already swallowed, and a
    // lost up here is exactly what leaves a ghost finger in the map.
    g.dragging = true;
    g.dragged = true;
    ensurePinch(g, vRef.current, toLocalOf);
    for (const id of g.pointers.keys()) capture(id);
  }, [capture, toLocalOf]);

  const onPointerMove = useCallback((e: ReactPointerEvent) => {
    const g = gesture.current;
    if (!g.pointers.has(e.pointerId)) return;
    if (!Number.isFinite(e.clientX) || !Number.isFinite(e.clientY)) return;
    g.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    const el = wrapRef.current;
    if (!el) return;
    const b = mapRef.current;
    if (g.pointers.size >= 2) {
      ensurePinch(g, vRef.current, toLocalOf);
      const pp = pinchPair(g);
      if (pp) {
        const mid = { x: (pp.a.x + pp.b.x) / 2, y: (pp.a.y + pp.b.y) / 2 };
        const d = Math.hypot(pp.a.x - pp.b.x, pp.a.y - pp.b.y) || 1;
        // Read everything now: the updater runs later, during render, by which
        // time a finger-up may have cleared `g.pinch` (a throw there blanks the page).
        const { v0, mid0, d0 } = pp;
        const next = pinchAround(v0, mid0, toLocal(el, mid.x, mid.y), d / d0, b);
        setV(next);
        g.dragging = true;
        g.dragged = true;
        return;
      }
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
      capture(e.pointerId);
    }
    if (g.dragging) {
      const d = localDelta(el, dx, dy);
      setV((curV) => panBy(curV, d.x, d.y, b));
    }
  }, [capture, toLocalOf]);

  const onPointerUp = useCallback((e: ReactPointerEvent) => {
    finishPointer(e.pointerId);
  }, [finishPointer]);

  // Safety net: a pointer whose up / cancel never reaches the wrap -- captured
  // by the browser after a multi-touch `preventDefault`, lifted outside the
  // board without capture, or cancelled by a system gesture -- would otherwise
  // stay in the map forever and turn every later one-finger pan into a pinch
  // around a ghost. These fire for every pointer; `finishPointer` ignores ids
  // the wrap already closed out.
  useEventListener(typeof window === "undefined" ? null : window, "pointerup", (e) => {
    finishPointer((e as PointerEvent).pointerId);
  });
  useEventListener(typeof window === "undefined" ? null : window, "pointercancel", (e) => {
    finishPointer((e as PointerEvent).pointerId);
  });
  useEventListener(typeof window === "undefined" ? null : window, "lostpointercapture", (e) => {
    finishPointer((e as PointerEvent).pointerId);
  });
  // iOS may deliver the touch series but drop the matching pointer events.
  useEventListener(typeof window === "undefined" ? null : window, "touchend", (e) => {
    if ((e as TouchEvent).touches.length === 0) abandonGesture();
  });
  useEventListener(typeof window === "undefined" ? null : window, "touchcancel", () => {
    abandonGesture();
  });
  // A notification / app switch mid-pinch: no up will ever come.
  useEventListener(typeof window === "undefined" ? null : window, "blur", () => abandonGesture());
  useEventListener(typeof window === "undefined" ? null : window, "pagehide", () => abandonGesture());
  useEventListener(typeof document === "undefined" ? null : document, "visibilitychange", () => {
    if (document.visibilityState === "hidden") abandonGesture();
  });

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
