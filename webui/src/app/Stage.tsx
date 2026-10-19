// The 1600 x 900 design canvas (the original's reference resolution) scaled to
// fit the window, over a blurred per-screen backdrop. The stage itself grows to
// fill the window: the design is a *minimum*, not a frame, so a wide monitor
// gets a wider stage and the edge-anchored panels (see the scenes' `right:`)
// slide out to the sides instead of leaving a letterbox.

import { type ReactNode, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { backdropUrl } from "./backdrops";
import { cx } from "../core/cx";
import { useEventListener } from "../hooks/dom";
import { clearForced, isForced, subscribeForced, usePortraitPhone } from "./rotation";
import { STAGE_H, STAGE_W } from "../styles/layout";
import { type Insets, type StageBox, stageBox, windowBox } from "./stageBox";
import { setStageRotated, setStageScale } from "./stageScale";
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

/**
 * The window to lay the stage out on -- see `windowBox` in `stageBox.ts` for
 * why this is the device screen on a handset and not the layout / visual
 * viewport. The `100lvw` / `100lvh` probe is one of the candidates: those
 * units are the large viewport (browser chrome hidden) by definition.
 */
function largeViewport(): { vw: number; vh: number } {
  let el = lvProbe;
  if (!el) {
    el = document.createElement("div");
    el.style.cssText = "position:fixed;top:0;left:0;width:100lvw;height:100lvh;visibility:hidden;pointer-events:none";
    document.body.appendChild(el);
    lvProbe = el;
  }
  const r = el.getBoundingClientRect();
  return windowBox({
    screenW: window.screen.width,
    screenH: window.screen.height,
    innerW: window.innerWidth,
    innerH: window.innerHeight,
    probeW: r.width,
    probeH: r.height,
    portrait: window.matchMedia("(orientation: portrait)").matches,
    coarse: window.matchMedia("(pointer: coarse)").matches,
  });
}
let lvProbe: HTMLElement | null = null;

/**
 * The device's safe-area insets on all four sides, read off a probe element.
 * `env(safe-area-inset-*)` is only available to CSS, so a hidden fixed div
 * with the values in its padding forces a sync layout that reports them.
 * The values can resolve late on a PWA cold start and go stale across a
 * rotation, so re-probe on the events below rather than trusting one read.
 */
function useSafeInsets(): Insets {
  const read = (): Insets => {
    const probe = document.createElement("div");
    probe.style.cssText =
      "position:fixed;visibility:hidden;padding:env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left)";
    document.body.appendChild(probe);
    const cs = getComputedStyle(probe);
    const r = {
      top: parseFloat(cs.paddingTop) || 0,
      right: parseFloat(cs.paddingRight) || 0,
      bottom: parseFloat(cs.paddingBottom) || 0,
      left: parseFloat(cs.paddingLeft) || 0,
    };
    probe.remove();
    return r;
  };
  const [v, set] = useState<Insets>(read);
  useEffect(() => {
    // Functional update so the comparison never closes over a stale value.
    const probe = () => {
      const n = read();
      set((prev) => (prev.top === n.top && prev.right === n.right && prev.bottom === n.bottom && prev.left === n.left ? prev : n));
    };
    // iOS reports stale geometry through intermediate resize events while
    // turning, and iOS 17+ takes 500ms+ to settle; sample past both.
    const orientation = () => {
      window.setTimeout(probe, 300);
      window.setTimeout(probe, 800);
    };
    // `env()` can resolve late on a PWA cold start without ever firing resize.
    const cold = [100, 500, 1000].map((ms) => window.setTimeout(probe, ms));
    const visible = () => {
      if (document.visibilityState === "visible") probe();
    };
    window.addEventListener("resize", probe);
    window.addEventListener("orientationchange", orientation);
    window.addEventListener("pageshow", probe);
    document.addEventListener("visibilitychange", visible);
    return () => {
      window.removeEventListener("resize", probe);
      window.removeEventListener("orientationchange", orientation);
      window.removeEventListener("pageshow", probe);
      document.removeEventListener("visibilitychange", visible);
      cold.forEach((id) => clearTimeout(id));
    };
  }, []);
  return v;
}

function useStage(tall: boolean): StageBox {
  // Size against the whole screen (see `largeViewport` / `windowBox`): the
  // same rectangle in both orientations, so forced rotation and a real turn
  // of the phone agree and the stage never letterboxes into blank bars.
  // While a text field is focused the on-screen keyboard can still shrink the
  // layout on some browsers; hold the height from before it came up.
  const open = useRef<{ vw: number; vh: number } | null>(null);
  const measure = () => {
    const vv = window.visualViewport;
    // A pinch the browser let through zooms the visual viewport: its size is
    // then the magnified sliver, and sizing the stage to it blanks the page.
    // Hold the unzoomed size until the zoom is back at 1.
    if (vv && Math.abs(vv.scale - 1) > 0.01 && open.current) return open.current;
    const cur = largeViewport();
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
  // Keep the controls clear of the notch / home indicator. The insets just
  // trade sides under `rotate(90deg)` (see `stageBox`), so forced rotation
  // protects the same stage edge a real turn does.
  const inset = useSafeInsets();
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
    inset,
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
  const { scale, w, h, safe, compact, rotated } = useStage(tall);
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
  //
  // `--safe-*`: hardware on each stage edge, in design px (see `stageBox`).
  // The stage's edges ARE the real screen edges, so a retractable strip
  // anchored there runs to the border on purpose; controls pad by these
  // instead of sitting under the notch / home indicator.
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
        <div
          className={s.stage}
          data-stage
          style={{
            width: w,
            height: tall ? h : STAGE_H,
            zoom: scale,
            ["--safe-top" as string]: `${safe.top}px`,
            ["--safe-right" as string]: `${safe.right}px`,
            ["--safe-bottom" as string]: `${safe.bottom}px`,
            ["--safe-left" as string]: `${safe.left}px`,
          }}
        >
          {children}
        </div>
      </div>
    </>
  );
}