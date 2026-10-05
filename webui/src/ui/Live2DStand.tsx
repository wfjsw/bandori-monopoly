// A character stand: the compiled Live2D model (docs/LIVE2D.md) over the static
// stand art, which stays as the fallback until the model is on screen and
// whenever a model is missing. Model ids are the character art ids
// (`D.artId(c)`), see core/data.ts and docs/LIVE2D.md.
//
// Fit mirrors the original `BandoriMonopoly.UI.Live2DPortrait`: the model draws
// into a 3:4 window (the Unity portrait's 768x1024 render target, AspectRatio-
// Fitter FitInParent) centred in the stand box, with `zoom` / `focusTop` /
// `headroom` and the per-id `framing.json` measuring sheet shifting and scaling
// the canvas exactly as Live2DPortrait.Draw does.

import { useEffect, useRef, useState } from "react";
import { eyeLine } from "../live2d/deform";
import { Live2DModel, loadLive2D, type Live2DModelHandle } from "../live2d";
import { charArt } from "../core/assets";
import { cx } from "../core/cx";
import { t as tr } from "../i18n/t";
import s from "./Live2DStand.module.css";

export interface Live2DStandProps {
  /** Live2D model id = the character art id (`D.artId(c)`): "001", "036c", … */
  id: string;
  /** Scene class that positions the stand box (stand art is 582 × 910). */
  className?: string;
  /** Restart the stand bounce when this changes (the Menu dialogue lines). */
  pulse?: number;
  alt?: string;
  onClick?: () => void;
  /** Parameter values for the model, e.g. `{ PARAM_ANGLE_X: 5 }`. */
  params?: Record<string, number>;
  /** Display zoom (Live2DPortrait.Show); 1 fits the canvas to the window. */
  zoom?: number;
  /** Cap for the framing shift (0 = never push the focus further down). */
  focusTop?: number;
  /** Minimum gap above the head as a fraction of the window height. */
  headroom?: number;
  /** Window height fraction the eye line sits on (characters align here). */
  eyeFrac?: number;
}

/** One `framing.json` entry (Live2DPortrait.Framing). */
interface Framing {
  top: number;
  scale: number;
}

/** Canvas rect inside the 3:4 window, in px relative to the window. */
interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

// `webui/public/assets/live2d/framing.json` (manifest.live2d.framing), one fetch.
let framingFile: Promise<Map<string, Framing>> | null = null;

function framingOf(id: string): Promise<Framing | undefined> {
  framingFile ??= fetch("/assets/live2d/framing.json")
    .then((r) => (r.ok ? r.text() : "{}"))
    .then((text) => {
      const data = JSON.parse(text.replace(/^﻿/, "")) as { models?: ({ id: string } & Framing)[] };
      return new Map((data.models ?? []).map((m) => [m.id, { top: m.top, scale: m.scale }]));
    })
    .catch(() => new Map<string, Framing>());
  return framingFile.then((m) => m.get(id));
}

/**
 * Map the canvas into the 3:4 window (Live2DPortrait.Draw).
 *
 * When `eyeY` is known the figure is anchored by its **eyes**: every character
 * lands its eye line on `eyeFrac` of the window height, so casts of different
 * heights line up. Anchoring on the head top instead makes the eye line wander
 * with each model's height. `top`/`headroom` from framing.json remain a
 * fallback for models without eye params.
 */
function canvasBox(
  winW: number,
  canvas: [number, number],
  f: Framing | undefined,
  zoom: number,
  focusTop: number,
  headroom: number,
  eyeY: number | null,
  eyeFrac: number,
): Box {
  const [cw, ch] = canvas;
  const z = zoom * (f && f.scale > 0 ? f.scale : 1);
  const num = Math.max(cw, ch * 0.75);
  const num2 = num / 0.75;
  const viewW = num / z;
  const left = (cw - num) / 2 + (num - viewW) / 2;
  const k = winW / viewW; // canvas unit -> px
  let top: number;
  if (eyeY != null) {
    // Put the eye line at `eyeFrac` of the window height (the window is 3:4,
    // so its height in canvas units is viewW / 0.75).
    top = eyeY - (eyeFrac * viewW) / 0.75;
  } else {
    const focus = f && f.top >= 0 && headroom > 0 ? Math.min(focusTop, f.top - headroom / z) : focusTop;
    top = (ch - num2) / 2 + num2 * focus;
  }
  return { left: -left * k, top: -top * k, width: cw * k, height: ch * k };
}

export function Live2DStand({ id, className, pulse, alt, onClick, params, zoom = 1, focusTop = 0, headroom = 0.02, eyeFrac = 0.28 }: Live2DStandProps) {
  const box = useRef<HTMLDivElement>(null);
  const anim = useRef<HTMLDivElement>(null);
  const modelHandle = useRef<Live2DModelHandle>(null);
  const [canvas, setCanvas] = useState<[number, number] | null>(null);
  const [framing, setFraming] = useState<Framing | undefined>(undefined);
  const [eyeY, setEyeY] = useState<number | null>(null);
  const [win, setWin] = useState<Box | null>(null);
  const [model, setModel] = useState<Box | null>(null);
  const [shown, setShown] = useState(false);
  const [gone, setGone] = useState(false);
  const [failed, setFailed] = useState(false);
  const art = charArt(id, "stand");

  // Load model.json + textures first: the static art stays until the model is
  // ready to draw, and stays for good when the model is missing or broken.
  useEffect(() => {
    setCanvas(null);
    setFraming(undefined);
    setEyeY(null);
    setWin(null);
    setModel(null);
    setShown(false);
    setGone(false);
    setFailed(false);
    let live = true;
    void Promise.all([loadLive2D(id), framingOf(id)]).then(
      ([data, f]) => {
        if (!live) return;
        setCanvas(data.model.canvas);
        setFraming(f);
        setEyeY(eyeLine(data.model));
        setEyeY(eyeLine(data.model));
      },
      () => {
        if (live) setFailed(true);
      },
    );
    return () => {
      live = false;
    };
  }, [id]);

  // Lay the canvas out inside the stand box (re-run on box resize).
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const place = () => {
      const w = el.clientWidth;
      const h = el.clientHeight;
      if (!canvas || !w || !h) {
        setWin(null);
        setModel(null);
        return;
      }
      // the 3:4 portrait window, centred like AspectRatioFitter.FitInParent
      const dw = Math.min(w, h * 0.75);
      const dh = dw / 0.75;
      setWin({ left: (w - dw) / 2, top: (h - dh) / 2, width: dw, height: dh });
      setModel(canvasBox(dw, canvas, framing, zoom, focusTop, headroom, eyeY, eyeFrac));
    };
    place();
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", place);
      return () => window.removeEventListener("resize", place);
    }
    const ro = new ResizeObserver(place);
    ro.observe(el);
    return () => ro.disconnect();
  }, [canvas, framing, zoom, focusTop, headroom, eyeY, eyeFrac]);

  // The renderer draws on its first RAF after mount; fade the static art out
  // once the model has had a few frames to paint, then drop it entirely. The
  // drop is on a timer rather than `transitionend`: a hidden tab freezes CSS
  // transition timelines (`currentTime` stays 0), which would leave the art
  // ghosting through the canvas's transparent pixels forever.
  useEffect(() => {
    if (!model) return;
    const fade = setTimeout(() => setShown(true), 150);
    const drop = setTimeout(() => setGone(true), 500);
    return () => {
      clearTimeout(fade);
      clearTimeout(drop);
    };
  }, [model]);

  // Restart the CSS bounce without remounting (which would reload the model).
  useEffect(() => {
    if (!pulse) return;
    const el = anim.current;
    if (!el) return;
    el.style.animation = "none";
    void el.offsetHeight;
    el.style.animation = "";
  }, [pulse]);

  // Clicking the stand plays a random reaction (Live2DPortrait.TryPlayReaction;
  // reactions never stack) and still runs the scene's own click handler.
  const handleClick = () => {
    modelHandle.current?.playReaction();
    onClick?.();
  };

  return (
    <div ref={box} className={cx(s.stand, className)} onClick={handleClick}>
      <div ref={anim} className={cx(s.anim, !!pulse && s.bounce)}>
        {art ? (
          !gone && <img className={cx(s.art, shown && s.hide)} src={art} alt={alt ?? ""} draggable={false} />
        ) : (
          !gone && <div className={s.missing}>{failed ? tr("live2d.missing") : tr("live2d.loading")}</div>
        )}
        {win && model && (
          <div className={s.window} style={win}>
            <div className={s.model} style={model}>
              <Live2DModel ref={modelHandle} id={id} params={params} className={s.fill} />
            </div>
          </div>
        )}
      </div>
      {art && !shown && !failed && <div className={s.loading}>{tr("live2d.loading")}</div>}
    </div>
  );
}