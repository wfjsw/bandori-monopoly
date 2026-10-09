// Element measurement and DOM sync: ResizeObserver (with a window-resize
// fallback for the old layout engines) and the two scroll behaviours the UI
// needs (pin to bottom on new content; snap to top when the subject changes).

import { useEffect, useLayoutEffect, useState, type RefObject } from "react";
import { useLatestRef } from "./latest.ts";

type ElementLike = RefObject<HTMLElement | null> | (() => HTMLElement | null | undefined);

function elOf(t: ElementLike | null | undefined): HTMLElement | null {
  if (!t) return null;
  return (typeof t === "function" ? t() : t.current) ?? null;
}

/**
 * Call `onSize` with the element whenever its box changes (and once on mount,
 * and again whenever `inputs` change -- the numbers the caller folds into the
 * measurement). Observes the element the target resolves to: pass a getter to
 * observe something other than the obvious node (the board observes its map
 * slot, not the wrap). Falls back to `window.resize` where ResizeObserver is
 * missing -- the call is the same either way. The handler and target are read
 * through refs, so a fresh closure each render does not re-observe.
 *
 * `inputs` must have a constant length across renders (it is a real dep list).
 */
export function useResizeObserver(
  target: ElementLike | null | undefined,
  onSize: (el: HTMLElement) => void,
  inputs: readonly unknown[] = [],
): void {
  const latest = useLatestRef(onSize);
  const live = useLatestRef(target);
  // A change to the inputs feeding the measurement (a new canvas, a new zoom)
  // must re-place the contents even when the box itself did not move.
  useEffect(() => {
    const el = elOf(live.current);
    if (el) latest.current(el);
    // `inputs` is the caller's dep list (constant length).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, inputs);
  useEffect(() => {
    const el = elOf(live.current);
    if (!el) return;
    const call = () => latest.current(el);
    call();
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", call);
      return () => window.removeEventListener("resize", call);
    }
    const ro = new ResizeObserver(call);
    ro.observe(el);
    return () => ro.disconnect();
  }, [latest, live]);
}

/**
 * Keep the scroller pinned to its bottom while `enabled` (the console output,
 * the match log). Re-pins when any of `inputs` changes -- the content whose
 * arrival is the reason to follow. `inputs` must have a constant length.
 */
export function useScrollFollow(
  ref: RefObject<HTMLElement | null>,
  inputs: readonly unknown[] = [],
  enabled = true,
): void {
  const live = useLatestRef({ ref, enabled });
  useEffect(() => {
    const { ref: r, enabled: on } = live.current;
    const el = r.current;
    if (on && el) el.scrollTop = el.scrollHeight;
    // `inputs` is the caller's dep list (constant length).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...inputs, enabled, live]);
}

/** Scroll `ref` back to the top whenever `key` changes (a new hovered card). */
export function useResetScroll(ref: RefObject<HTMLElement | null>, key: unknown): void {
  const live = useLatestRef(ref);
  useEffect(() => {
    const el = live.current.current;
    if (el) el.scrollTop = 0;
  }, [key, live]);
}

/**
 * Measure an element's layout box right after layout (`useLayoutEffect`), so
 * the caller can clamp it before the user sees it -- the tooltip pattern.
 * Re-measures when `key` changes (a new tooltip); the box object only changes
 * when the element actually moved. Returns 0 x 0 while the element is absent.
 */
export function useLayoutSize(
  ref: RefObject<HTMLElement | null>,
  key: unknown,
): { width: number; height: number } {
  const [box, setBox] = useState({ width: 0, height: 0 });
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const width = el.offsetWidth;
    const height = el.offsetHeight;
    setBox((b) => (b.width === width && b.height === height ? b : { width, height }));
  }, [key, ref]);
  return box;
}