// The 1600 x 900 design canvas (the original's reference resolution) scaled to
// fit the window, over a blurred per-screen backdrop. The stage itself grows to
// fill the window: the design is a *minimum*, not a frame, so a wide monitor
// gets a wider stage and the edge-anchored panels (see the scenes' `right:`)
// slide out to the sides instead of leaving a letterbox.

import { type ReactNode, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { sceneImg } from "../core/assets";
import { useEventListener } from "../hooks/dom";
import { COMPACT_H, COMPACT_MIN_W, STAGE_H, STAGE_W, isCompactWindow } from "../styles/layout";
import s from "./Stage.module.css";

export { STAGE_H, STAGE_W };

let backdrop: string | null = null;
const listeners = new Set<() => void>();

/** Scene backgrounds: bg_common (menu, board), bg_live (lobby), bg_band (prep, gallery, deck). */
export function setBackdrop(name: string | null): void {
  if (backdrop === name) return;
  backdrop = name;
  listeners.forEach((cb) => cb());
}

/** Set the backdrop while a component is mounted. */
export function useBackdrop(name: string | null): void {
  useEffect(() => setBackdrop(name), [name]);
}

/**
 * The uniform scale that fits the 1600 x 900 design in the window, plus the
 * stage size that fills the window at that scale (in design px). The width
 * grows past 1600 so a wide monitor isn't left with letterbox bars. The height
 * stays at the design's 900 by default (a taller window letterboxes instead of
 * stranding the top-anchored UI at the top of a very tall stage); the match
 * screen opts into `useStageFill` and takes the full height instead, so its
 * columns stretch and the tile board has no bars above or below.
 */
function forcedCompact(): boolean | null {
  const q = new URLSearchParams(location.search).get("compact");
  return q === null ? null : q !== "0";
}

/** The device's left / right safe-area insets (notch side), read off a probe element. */
function useSafeInsets(): { left: number; right: number } {
  const read = () => {
    const probe = document.createElement("div");
    probe.style.cssText = "position:fixed;visibility:hidden;padding:0 env(safe-area-inset-right) 0 env(safe-area-inset-left)";
    document.body.appendChild(probe);
    const cs = getComputedStyle(probe);
    const r = { left: parseFloat(cs.paddingLeft) || 0, right: parseFloat(cs.paddingRight) || 0 };
    probe.remove();
    return r;
  };
  const [v, set] = useState(read);
  useEventListener(window, "resize", () => {
    const n = read();
    if (n.left !== v.left || n.right !== v.right) set(n);
  });
  return v;
}

function useStage(tall: boolean): { scale: number; w: number; h: number; compact: boolean; shift: number } {
  // Size against the *visual* viewport where there is one: on a phone the
  // layout viewport (innerWidth / innerHeight) and what is on screen differ
  // while the address bar or keyboard is showing.
  // While a text field is focused the on-screen keyboard shrinks the visual
  // viewport; rescaling the stage to the sliver left would shrink everything,
  // so hold the height from before the keyboard came up (width unchanged).
  const open = useRef<{ vw: number; vh: number } | null>(null);
  const measure = () => {
    const vv = window.visualViewport;
    // A pinch the browser let through zooms the visual viewport: its size is
    // then the magnified sliver, and sizing the stage to it blanks the page.
    // Hold the unzoomed size until the zoom is back at 1.
    if (vv && Math.abs(vv.scale - 1) > 0.01 && open.current) return open.current;
    const cur = { vw: vv?.width ?? window.innerWidth, vh: vv?.height ?? window.innerHeight };
    const a = document.activeElement;
    const editing = a instanceof HTMLInputElement || a instanceof HTMLTextAreaElement;
    const prev = open.current;
    if (editing && prev && Math.abs(prev.vw - cur.vw) < 1 && cur.vh < prev.vh) return prev;
    open.current = cur;
    return cur;
  };
  const [win, set] = useState(measure);
  useEventListener(window, "resize", () => set(measure()));
  useEventListener(() => window.visualViewport, "resize", () => set(measure()));
  useEventListener(() => window.visualViewport, "scroll", () => set(measure()));
  useEventListener(document, "focusout", () => setTimeout(() => set(measure()), 50));
  // Landscape phones: keep the stage out from under the notch / home bar.
  const inset = useSafeInsets();
  const vw = win.vw - inset.left - inset.right;
  const vh = win.vh;
  // The compact (phone) canvas is the board's alone: the other scenes are
  // laid out for the 1600 x 900 design and keep scaling it.
  const compact = tall && isCompactWindow(vw, vh, window.matchMedia("(pointer: coarse)").matches, forcedCompact());
  const scale = compact ? Math.min(vw / COMPACT_MIN_W, vh / COMPACT_H) : Math.min(vw / STAGE_W, vh / STAGE_H);
  const box = { scale, w: vw / scale, h: vh / scale, compact, shift: (inset.left - inset.right) / 2 };
  // A scrolled document desyncs touches from the fixed stage (the mobile
  // touch fix); snap back to the origin wherever the scroll came from.
  const unscroll = () => {
    if (window.scrollX || window.scrollY) window.scrollTo(0, 0);
  };
  useEventListener(window, "scroll", unscroll);
  useEventListener(() => window.visualViewport, "scroll", unscroll);
  return box;
}

let fill = false;
const fillListeners = new Set<() => void>();

/** Fill the window's full height (no top / bottom letterbox) while on. */
export function setStageFill(on: boolean): void {
  if (fill === on) return;
  fill = on;
  fillListeners.forEach((cb) => cb());
}

/** Opt the current scene into the full-height stage. Only the match screen
 *  (the board) does: every other scene keeps the 900-tall letterboxed stage. */
export function useStageFill(on = true): void {
  useEffect(() => {
    setStageFill(on);
    return () => setStageFill(false);
  }, [on]);
}

/** The backdrop the scene set, for the blurred plate behind the stage. */
export function useBackdropValue(): string | null {
  return useSyncExternalStore(
    (cb) => {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    () => backdrop,
  );
}

/** Is the scene filling the window (see `useStageFill`)? */
export function useStageFillValue(): boolean {
  return useSyncExternalStore(
    (cb) => {
      fillListeners.add(cb);
      return () => fillListeners.delete(cb);
    },
    () => fill,
  );
}

/** Is the stage on the compact (phone) canvas? Set by `Stage` for the scenes below it. */
let compactNow = false;
const compactListeners = new Set<() => void>();
export function useCompactValue(): boolean {
  return useSyncExternalStore(
    (cb) => {
      compactListeners.add(cb);
      return () => compactListeners.delete(cb);
    },
    () => compactNow,
  );
}

export function Stage({ children, fading }: { children: ReactNode; fading: boolean }) {
  const tall = useStageFillValue();
  const { scale, w, h, compact, shift } = useStage(tall);
  useEffect(() => {
    document.documentElement.toggleAttribute("data-compact", compact);
    if (compactNow === compact) return;
    compactNow = compact;
    compactListeners.forEach((cb) => cb());
  }, [compact]);
  const bg = useBackdropValue();
  const src = bg ? sceneImg(bg) : "";
  return (
    <>
      <div className={s.backdrop} style={{ backgroundImage: src ? `url("${src}")` : undefined }} />
      <div
        className={s.stage}
        data-stage
        style={{ width: w, height: tall ? h : STAGE_H, transform: `translate(calc(-50% + ${shift}px), -50%) scale(${scale})` }}
      >
        {children}
        <div className={fading ? `${s.fader} ${s.on}` : s.fader} />
      </div>
    </>
  );
}