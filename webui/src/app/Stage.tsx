// The 1600 x 900 design canvas (the original's reference resolution) scaled to
// fit the window, over a blurred per-screen backdrop. The stage itself grows to
// fill the window: the design is a *minimum*, not a frame, so a wide monitor
// gets a wider stage and the edge-anchored panels (see the scenes' `right:`)
// slide out to the sides instead of leaving a letterbox.

import { type ReactNode, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { backdropUrl } from "./backdrops";
import { cx } from "../core/cx";
import { useEventListener } from "../hooks/dom";
import { clearForced, isForced, setStageRotated, subscribeForced, usePortraitPhone } from "./rotation";
import { STAGE_H, STAGE_W } from "../styles/layout";
import { type StageBox, stageBox } from "./stageBox";
import { setStageScale } from "./stageScale";
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

function useStage(tall: boolean): StageBox {
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
  // A phone held upright, once the user asked for it: lay the stage out on the swapped (landscape) window
  // and turn it a quarter clockwise.
  const forced = useSyncExternalStore(subscribeForced, isForced);
  const portrait = usePortraitPhone();
  useEffect(() => {
    if (!portrait) clearForced();
  }, [portrait]);
  const rotated = forced && portrait;
  const box = stageBox({
    vw: win.vw,
    vh: win.vh,
    rotated,
    tall,
    coarse: window.matchMedia("(pointer: coarse)").matches,
    compactForced: forcedCompact(),
  });
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

/**
 * Two stacked backdrop plates that swap by `opacity`. The images arrive
 * pre-blurred (see `backdrops.ts`), so the cross-fade is a pure compositor
 * effect with no filter to re-rasterise.
 */
function Backdrop({ src, snap }: { src: string; snap: boolean }) {
  const [plates, setPlates] = useState<{ a: string; b: string; front: "a" | "b" }>({ a: src, b: "", front: "a" });
  useEffect(() => {
    setPlates((p) => {
      if (p[p.front] === src) return p;
      const back = p.front === "a" ? "b" : "a";
      return { ...p, [back]: src, front: back };
    });
  }, [src]);
  // While a scene change is being dissolved, the two plates must land on their
  // new opacities in the same frame the swap is snapshotted -- a slow CSS
  // cross-fade would be captured mid-flight and the backdrop would never
  // appear to change.
  const plate = (k: "a" | "b") => (
    <div
      className={cx(plates.front === k ? `${s.backdrop} ${s.front}` : s.backdrop, snap && s.snap)}
      style={{ backgroundImage: plates[k] ? `url("${plates[k]}")` : undefined }}
    />
  );
  return <>{plate("a")}{plate("b")}</>;
}

export function Stage({ children, transitioning }: { children: ReactNode; transitioning: boolean }) {
  const tall = useStageFillValue();
  const { scale, w, h, compact, rotated } = useStage(tall);
  setStageRotated(rotated);
  setStageScale(scale);
  useEffect(() => {
    document.documentElement.toggleAttribute("data-compact", compact);
    if (compactNow === compact) return;
    compactNow = compact;
    compactListeners.forEach((cb) => cb());
  }, [compact]);
  const bg = useBackdropValue();
  const src = bg ? backdropUrl(bg) : "";
  // Split the scaled element in two: the placer owns screen-space geometry
  // (the fixed-centre transform), and the stage lays out at design size under
  // `zoom`. `zoom` scales used length values, so the engine rasterizes glyphs
  // at the final device size instead of texture-scaling a design-sized layer
  // (the source of the softness).
  return (
    <>
      <Backdrop src={src} snap={transitioning} />
      <div
        className={s.placer}
        style={{
          width: w * scale,
          height: (tall ? h : STAGE_H) * scale,
          transform: `translate(-50%, -50%)${rotated ? " rotate(90deg)" : ""}`,
        }}
      >
        <div className={s.stage} data-stage style={{ width: w, height: tall ? h : STAGE_H, zoom: scale }}>
          {children}
        </div>
      </div>
    </>
  );
}