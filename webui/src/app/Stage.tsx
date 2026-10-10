// The 1600 x 900 design canvas (the original's reference resolution) scaled to
// fit the window, over a blurred per-screen backdrop. The stage itself grows to
// fill the window: the design is a *minimum*, not a frame, so a wide monitor
// gets a wider stage and the edge-anchored panels (see the scenes' `right:`)
// slide out to the sides instead of leaving a letterbox.

import { type ReactNode, useEffect, useState, useSyncExternalStore } from "react";
import { sceneImg } from "../core/assets";
import { useEventListener } from "../hooks/dom";
import { STAGE_H, STAGE_W } from "../styles/layout";
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
function useStage(): { scale: number; w: number; h: number } {
  // Size against the *visual* viewport where there is one: on a phone the
  // layout viewport (innerWidth / innerHeight) and what is on screen differ
  // while the address bar or keyboard is showing.
  const calc = () => {
    const vv = window.visualViewport;
    const vw = vv?.width ?? window.innerWidth;
    const vh = vv?.height ?? window.innerHeight;
    const scale = Math.min(vw / STAGE_W, vh / STAGE_H);
    return { scale, w: vw / scale, h: vh / scale };
  };
  const [box, set] = useState(calc);
  useEventListener(window, "resize", () => set(calc()));
  useEventListener(() => window.visualViewport, "resize", () => set(calc()));
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

export function Stage({ children, fading }: { children: ReactNode; fading: boolean }) {
  const { scale, w, h } = useStage();
  const bg = useBackdropValue();
  const tall = useStageFillValue();
  const src = bg ? sceneImg(bg) : "";
  return (
    <>
      <div className={s.backdrop} style={{ backgroundImage: src ? `url("${src}")` : undefined }} />
      <div
        className={s.stage}
        data-stage
        style={{ width: w, height: tall ? h : STAGE_H, transform: `translate(-50%, -50%) scale(${scale})` }}
      >
        {children}
        <div className={fading ? `${s.fader} ${s.on}` : s.fader} />
      </div>
    </>
  );
}