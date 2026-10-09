// DOM / window subscriptions as semantic hooks. Everything here keeps the
// handler in a latest-ref, so the listener is added once per (target, type,
// options) and never goes stale: call sites pass a fresh closure every render
// and the subscription stays put.

import { useEffect, useRef, type RefObject } from "react";
import { useLatestRef } from "./latest.ts";
import { isControlTarget, isEditableTarget, matchHotkey, type Hotkey } from "./pure.ts";

export { isControlTarget, isEditableTarget, matchHotkey, type Hotkey };

/** A target that may not exist yet (a ref filled after mount) or at all. */
export type EventTargetLike =
  | EventTarget
  | null
  | undefined
  | RefObject<EventTarget | null>
  | (() => EventTarget | null | undefined);

function resolve(t: EventTargetLike): EventTarget | null {
  if (!t) return null;
  if (typeof t === "function") return t() ?? null;
  return "current" in t ? t.current : t;
}

/**
 * Listen to `type` on `target` for the life of the component (or until `type`
 * / `capture` / `passive` change). The handler and the target are read through
 * refs, so a fresh closure or a fresh `() => ref.current` getter each render
 * does not re-subscribe. The *resolved* element must be stable across the
 * component's life (a ref, `window`, `document`, `visualViewport`) -- if it is
 * swapped for another node, re-mount the consumer.
 *
 * `passive: false` is what the board's wheel-zoom needs to `preventDefault`
 * the browser's own ctrl+wheel.
 */
export function useEventListener<K extends string>(
  target: EventTargetLike,
  type: K,
  handler: (e: Event) => void,
  options?: { capture?: boolean; passive?: boolean },
): void {
  const latest = useLatestRef(handler);
  const targetRef = useLatestRef(target);
  const capture = !!options?.capture;
  const passive = options?.passive;
  useEffect(() => {
    const el = resolve(targetRef.current);
    if (!el) return;
    const fn = (e: Event) => latest.current(e);
    // A plain object (not `false`) keeps the capture/passive pair identical for
    // add/removeEventListener.
    const opts = passive === undefined ? capture : { capture, passive };
    el.addEventListener(type, fn, opts);
    return () => el.removeEventListener(type, fn, opts);
  }, [type, capture, passive, latest, targetRef]);
}

/**
 * Run `handler` when the page is going away (tab close, reload, navigation to
 * another origin, mobile app switch that discards the page). This is the one
 * moment the browser still lets a page write out its state -- the match
 * snapshot and the replay transport ride on it. `visibilitychange` is *not*
 * included: it fires on every tab switch and would persist far too often.
 */
export function usePageHide(handler: (e: Event) => void): void {
  useEventListener(typeof window === "undefined" ? null : window, "pagehide", handler);
}

/**
 * Keyboard shortcuts on `window`. One call names every key the screen answers
 * to; the hook owns the listener and the "is the user typing?" guard. Handlers
 * call `preventDefault` themselves when they take the key. Bindings are read
 * through a ref, so a new array every render is fine.
 */
export function useHotkeys(keys: readonly Hotkey[], opts?: { capture?: boolean }): void {
  const latest = useLatestRef(keys);
  useEventListener(typeof window === "undefined" ? null : window, "keydown", (e) => {
    const ev = e as KeyboardEvent;
    for (const hk of latest.current) {
      if (matchHotkey(hk, ev)) {
        hk.run(ev);
        return;
      }
    }
  }, opts);
}

/**
 * Focus `ref` while `open`, and put focus back where it came from on close.
 * The console panel's contract: the input takes over, and the control the user
 * was on is restored (if it is still on the page). `onOpen` runs once each
 * time the panel opens (the console resets its history cursor there).
 */
export function useAutoFocus(open: boolean, ref: RefObject<HTMLElement | null>, onOpen?: () => void): void {
  const previous = useRef<HTMLElement | null>(null);
  const live = useLatestRef(ref);
  const opened = useLatestRef(onOpen);
  useEffect(() => {
    if (!open) return;
    previous.current = document.activeElement as HTMLElement | null;
    live.current.current?.focus();
    opened.current?.();
    return () => {
      const el = previous.current;
      if (el?.isConnected) el.focus();
    };
  }, [open, live, opened]);
}