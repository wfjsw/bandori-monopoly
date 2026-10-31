// Windowing for a long, append-only list inside one scroller (the match log).
// Rows render only around the viewport; two spacers stand in for the rest.
//
// Heights are measured as rows render and cached per row key, so a row that
// scrolls out keeps its real height. Unmeasured rows (never rendered yet) use
// the caller's estimate. A height that settles above the viewport is folded
// into `scrollTop`, so nothing jumps while the reader is scrolled up.
//
// The tail follows like `useScrollFollow`: the list re-pins to its bottom on
// new content only while the reader is already there, and scrolling up opts
// out until they come back.

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type RefObject } from "react";
import { useLatestRef } from "./latest.ts";
import { scrollFollowStep, shouldPinToBottom, type ScrollFollow } from "./pure.ts";
import { designBox } from "../app/stageScale.ts";

/** The rendered slice and the spacer heights that stand in for the rest. */
export interface RowWindow {
  start: number;
  end: number;
  top: number;
  bottom: number;
}

export interface WindowOptions {
  /** px a row gets before it has been measured. */
  estimate: number;
  /** px of overscan rendered above and below the viewport. */
  overscan?: number;
}

/** A window that is rendered while the pane has no height (hidden behind a
 *  tab). Small: it exists so the pane is never empty when it is shown. */
const HIDDEN_ROWS = 30;

/** Running offset of every row (`n + 1` entries; the last is the total). */
export function rowOffsets(heights: readonly (number | undefined)[], estimate: number): number[] {
  const out = new Array<number>(heights.length + 1);
  out[0] = 0;
  for (let i = 0; i < heights.length; i++) out[i + 1] = out[i] + (heights[i] ?? estimate);
  return out;
}

/** The rows intersecting `[scrollTop, scrollTop + viewport)`, grown by
 *  `overscan` px on both sides. Always contains at least one row. */
export function rowRange(offsets: readonly number[], scrollTop: number, viewport: number, overscan: number): { start: number; end: number } {
  const count = offsets.length - 1;
  if (count <= 0) return { start: 0, end: 0 };
  const lo = Math.max(0, scrollTop - overscan);
  const hi = Math.max(lo, scrollTop + Math.max(0, viewport) + overscan);
  // First row whose bottom edge is past `lo`.
  let a = 0;
  let b = count;
  while (a < b) {
    const mid = (a + b) >> 1;
    if (offsets[mid + 1] <= lo) a = mid + 1;
    else b = mid;
  }
  const start = Math.min(a, count - 1);
  // First row whose top edge is at or past `hi`, plus one so the row spanning
  // the bottom edge is included.
  a = start;
  b = count;
  while (a < b) {
    const mid = (a + b) >> 1;
    if (offsets[mid] < hi) a = mid + 1;
    else b = mid;
  }
  const end = Math.min(count, Math.max(start + 1, a));
  return { start, end };
}

/** Observe `el`'s size; `onSize` runs with both dimensions on every change. */
function observeSize(el: HTMLElement, onSize: () => void): () => void {
  onSize();
  if (typeof ResizeObserver === "undefined") {
    window.addEventListener("resize", onSize);
    return () => window.removeEventListener("resize", onSize);
  }
  const ro = new ResizeObserver(onSize);
  ro.observe(el);
  return () => ro.disconnect();
}

/**
 * Window `keys` inside `scroller`. `keys` must be stable per row (measurement
 * is keyed by them) and change only by appending or prepending.
 */
export function useWindowedList(
  scroller: RefObject<HTMLElement | null>,
  keys: readonly string[],
  opts: WindowOptions,
): RowWindow & { row(key: string, index: number): (el: HTMLElement | null) => void } {
  const { estimate } = opts;
  const overscan = opts.overscan ?? 12;
  const [version, setVersion] = useState(0);
  const [view, setView] = useState({ scrollTop: 0, viewport: 0 });
  const heights = useRef(new Map<string, number>());
  const els = useRef(new Map<string, HTMLElement>());
  const present = useRef(new Map<string, number>());
  const follow = useRef<ScrollFollow>({ atEnd: true, pinned: null });
  const win = useRef({ start: 0, end: 0 });
  const estimateRef = useLatestRef(estimate);
  const frame = useRef(0);

  // Key -> index for the current render, so a measurement knows where its row
  // sits (and whether it is above the viewport).
  present.current = useMemo(() => new Map(keys.map((k, i) => [k, i])), [keys]);

  // Watch the scroller's own box: a hidden pane (display: none) has none, and
  // showing it must size the window afresh.
  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    return observeSize(el, () => setView((v) => (v.viewport === el.clientHeight ? v : { ...v, viewport: el.clientHeight })));
  }, [scroller]);

  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const onScroll = () => {
      follow.current = scrollFollowStep(follow.current, {
        scrollTop: el.scrollTop,
        gap: el.scrollHeight - el.scrollTop - el.clientHeight,
        visible: el.clientHeight > 0,
      });
      // One state write per frame: a wheel burst must not re-render per event.
      if (frame.current) return;
      frame.current = requestAnimationFrame(() => {
        frame.current = 0;
        setView((v) => (v.scrollTop === el.scrollTop ? v : { ...v, scrollTop: el.scrollTop }));
      });
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      el.removeEventListener("scroll", onScroll);
      if (frame.current) cancelAnimationFrame(frame.current);
      frame.current = 0;
    };
  }, [scroller]);

  const offsets = useMemo(() => {
    const hs = keys.map((k) => heights.current.get(k));
    return rowOffsets(hs, estimate);
    // `version` re-reads the (mutable) height map after a measurement.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [keys, estimate, version]);

  // Layout effect: land a pin before the paint that would show the new tail
  // at the old position. Also runs after measurements (a pin's bottom moved).
  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el || view.viewport <= 0) return;
    if (!shouldPinToBottom(follow.current, el.scrollHeight - el.scrollTop - el.clientHeight, true)) return;
    el.scrollTop = el.scrollHeight;
    follow.current = { atEnd: true, pinned: el.scrollTop };
  });

  const measure = useCallback((el: HTMLElement, key: string) => {
    // Design px: the stage is transform-scaled, and `scrollTop` is layout px.
    const h = designBox(el).height;
    const prev = heights.current.get(key);
    if (prev === h) return;
    heights.current.set(key, h);
    const idx = present.current.get(key);
    const sc = scroller.current;
    // A row above the viewport settling to its real height: keep the content
    // under the reader's eyes still (unless the tail is pinning the scroll).
    if (sc && idx !== undefined && idx < win.current.start && !follow.current.atEnd) {
      sc.scrollTop += h - (prev ?? estimateRef.current);
    }
    setVersion((v) => v + 1);
  }, [scroller, estimateRef]);

  const row = useCallback(
    (key: string, index: number) =>
      (el: HTMLElement | null) => {
        const old = els.current.get(key);
        if (old && old !== el && old.dataset) delete old.dataset.winIndex;
        if (!el) {
          els.current.delete(key);
          return;
        }
        els.current.set(key, el);
        el.dataset.winIndex = String(index);
        measure(el, key);
      },
    [measure],
  );

  const offsetsNow = offsets;
  const range = useMemo(() => {
    if (view.viewport <= 0) {
      const start = Math.max(0, keys.length - HIDDEN_ROWS);
      return { start, end: keys.length };
    }
    return rowRange(offsetsNow, view.scrollTop, view.viewport, overscan);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [offsetsNow, view.scrollTop, view.viewport, overscan, keys.length]);

  win.current = range;
  const top = offsetsNow[range.start] ?? 0;
  const bottom = (offsetsNow[offsetsNow.length - 1] ?? 0) - (offsetsNow[range.end] ?? 0);
  return { ...range, top, bottom, row };
}
