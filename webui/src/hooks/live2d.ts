// The Live2D model as a resource hook: load, lay out, animate, dispose.
//
// `useLive2DModel` owns the Cubism viewer (the canvas + RAF loop of
// `Live2DModel.tsx`) -- the component is then just the DOM box and the status
// banner. `useLive2DStand` owns the character stand's chain on top of it: the
// framing measure, the 3:4 portrait window inside the stand box, and the
// reveal of the model over the static art.

import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { useLatestRef } from "./latest.ts";
import { useAnimationFrame, useTimeout } from "./timers.ts";
import { useResizeObserver } from "./measure.ts";
import { canvasBox, sameParams, type Framing, type StandBox } from "./pure.ts";
import { eyeLine, evalPose, figureTop, type ModelPose } from "../live2d/deform.ts";
import {
  animQueue, animValues, applyProps, createAnim, playReaction, tickAnim, type AnimState,
} from "../live2d/anim.ts";
import { Live2DRenderer } from "../live2d/gl.ts";
import { loadLive2D, loadLive2DAnim, type LoadedLive2D } from "../live2d/load.ts";

export { canvasBox, type Framing, type StandBox };

export type Live2DModelState = "loading" | "ready" | "load-error" | "gl-error";

export interface Live2DModelApi {
  /** Put these on the wrapping box and the canvas. */
  wrapRef: RefObject<HTMLDivElement | null>;
  canvasRef: RefObject<HTMLCanvasElement | null>;
  status: Live2DModelState;
  /** Play a random one-shot reaction; false while one is already playing. */
  playReaction(): boolean;
  /** A click on the model plays a random reaction (reactions never stack). */
  onClick: () => void;
}

/**
 * The animated Cubism 2 viewer: a canvas with a RAF loop driving the ported
 * `Live2DPortrait.RenderFrame` (idle motion, tap reactions, hair physics, eye
 * blink). Without animation (or with no `catalog.json` motion) it holds the
 * posed frame and redraws only when the pose goes dirty.
 *
 * Subscribes to nothing; owns the GL renderer, the model, the animation state
 * and the draw loop, and drops the GL side on unmount (the texture cache stays
 * in `live2d/load.ts`). The drawing buffer is kept matched to the layout box
 * (and the device pixel ratio) via a ResizeObserver. `params` is layered under
 * the motion (it seeds the saved base).
 */
export function useLive2DModel(opts: {
  id: string;
  animate?: boolean;
  /** Parameter values keyed by `PARAM_*` id; omitted keys use the defaults. */
  params?: Record<string, number>;
}): Live2DModelApi {
  const { id, animate = true } = opts;
  const params = opts.params;
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const rendererRef = useRef<Live2DRenderer | null>(null);
  const modelRef = useRef<LoadedLive2D | null>(null);
  const animRef = useRef<AnimState | null>(null);
  const poseRef = useRef<ModelPose | null>(null);
  const evaluatedRef = useRef<Record<string, number> | undefined>(undefined);
  const dirtyRef = useRef(true);
  const paramsRef = useLatestRef(params);
  const [status, setStatus] = useState<Live2DModelState>("loading");

  // Layer the `params` prop under the motion (it seeds the saved base).
  useEffect(() => {
    const anim = animRef.current;
    if (anim) {
      applyProps(anim, params);
      return;
    }
    const model = modelRef.current;
    if (!model || sameParams(evaluatedRef.current, params)) return;
    evaluatedRef.current = params;
    poseRef.current = evalPose(model.model, params);
    dirtyRef.current = true;
  }, [params]);

  // Load model.json + textures (and the motion/physics bundle) for `id`.
  useEffect(() => {
    let cancelled = false;
    setStatus("loading");
    animRef.current = null;
    Promise.all([loadLive2D(id), animate ? loadLive2DAnim(id) : Promise.resolve(null)])
      .then(([data, bundle]) => {
        if (cancelled) return;
        const canvas = canvasRef.current;
        if (!canvas) return;
        let renderer = rendererRef.current;
        try {
          if (!renderer) {
            renderer = new Live2DRenderer(canvas);
            rendererRef.current = renderer;
          }
        } catch {
          setStatus("gl-error");
          return;
        }
        modelRef.current = data;
        renderer.setModel(data.model, data.textures);
        const wrap = wrapRef.current;
        if (wrap) {
          renderer.resize(wrap.clientWidth, wrap.clientHeight, window.devicePixelRatio || 1);
        }
        animRef.current = bundle
          ? createAnim({
              params: data.model.params,
              idle: bundle.idle,
              reactions: bundle.reactions,
              physics: bundle.physics,
              props: paramsRef.current,
              now: 0,
            })
          : null;
        const current = paramsRef.current;
        poseRef.current = animRef.current
          ? evalPose(data.model, animValues(animRef.current))
          : evalPose(data.model, current);
        evaluatedRef.current = current;
        dirtyRef.current = true;
        setStatus("ready");
      })
      .catch(() => {
        if (!cancelled) setStatus("load-error");
      });
    return () => {
      cancelled = true;
    };
  }, [id, animate, paramsRef]);

  // RAF loop: advance the animation and redraw. Static poses still draw only
  // when something marked them dirty.
  useAnimationFrame((dt) => {
    const renderer = rendererRef.current;
    const model = modelRef.current;
    if (!renderer || !model) return;
    const anim = animRef.current;
    if (anim) {
      tickAnim(anim, dt);
      poseRef.current = evalPose(model.model, animValues(anim));
      renderer.render(poseRef.current.meshes);
      return;
    }
    if (!dirtyRef.current) return;
    dirtyRef.current = false;
    const pose = poseRef.current;
    if (pose) renderer.render(pose.meshes);
  });

  // Keep the drawing buffer matched to the layout box (and device pixel ratio).
  useResizeObserver(wrapRef, () => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    rendererRef.current?.resize(wrap.clientWidth, wrap.clientHeight, window.devicePixelRatio || 1);
    dirtyRef.current = true;
  });

  // Debug hooks for throttled tabs and tests: step by hand, dump the queue,
  // pin a parameter.
  useLive2DDebugHooks(animRef, modelRef, rendererRef, poseRef);

  // Drop the GL side on unmount (the texture cache is left to load.ts).
  useEffect(
    () => () => {
      rendererRef.current?.dispose();
      rendererRef.current = null;
      modelRef.current = null;
      animRef.current = null;
      poseRef.current = null;
    },
    [],
  );

  const playReactionNow = useCallback(() => {
    const anim = animRef.current;
    if (!anim) return false;
    const started = playReaction(anim);
    if (started) dirtyRef.current = true;
    return started;
  }, []);

  return {
    wrapRef,
    canvasRef,
    status,
    playReaction: playReactionNow,
    onClick: useCallback(() => {
      playReactionNow();
    }, [playReactionNow]),
  };
}

/** `window.__live2dTick` / `__live2dState` / `__live2dForce` for tests. */
function useLive2DDebugHooks(
  anim: { current: AnimState | null },
  model: { current: LoadedLive2D | null },
  renderer: { current: Live2DRenderer | null },
  pose: { current: ModelPose | null },
): void {
  useEffect(() => {
    const step = (dtMsec = 1000 / 60) => {
      const a = anim.current;
      const m = model.current;
      const r = renderer.current;
      if (!a || !m) return undefined;
      tickAnim(a, dtMsec);
      pose.current = evalPose(m.model, animValues(a));
      if (r) r.render(pose.current.meshes);
      return { values: animValues(a), isPlayingReaction: a.isPlayingReaction, now: a.now };
    };
    const dump = () => {
      const a = anim.current;
      if (!a) return undefined;
      return {
        now: a.now,
        isPlayingReaction: a.isPlayingReaction,
        ents: animQueue(a).map((e) => ({
          start: e.startTimeMSec,
          end: e.endTimeMSec,
          finished: e.finished,
          loop: e.motion.loop,
          dur: e.motion.getDurationMSec(),
          fadeIn: e.motion.fadeInMsec,
          fadeOut: e.motion.fadeOutMsec,
          nCurves: e.motion.data.curves.length,
          maxLength: e.motion.data.maxLength,
          fps: e.motion.data.fps,
        })),
      };
    };
    const force = (id: string, v: number) => {
      const a = anim.current;
      if (!a) return false;
      // Pin the value against loadParam: no motion writes, value in the base.
      a.idle = null;
      a.queue.stopAllMotions();
      a.isPlayingReaction = false;
      a.store.setParam(id, v);
      a.store.saveInto(a.saved);
      return true;
    };
    window.__live2dTick = step;
    window.__live2dState = dump;
    window.__live2dForce = force;
    return () => {
      if (window.__live2dTick === step) delete window.__live2dTick;
      if (window.__live2dState === dump) delete window.__live2dState;
      if (window.__live2dForce === force) delete window.__live2dForce;
    };
  }, [anim, model, renderer, pose]);
}

// ---------------------------------------------------------------- the stand

// `webui/public/assets/live2d/framing.json` (manifest.live2d.framing), one fetch.
let framingFile: Promise<Map<string, Framing>> | null = null;

export function framingOf(id: string): Promise<Framing | undefined> {
  framingFile ??= fetch("/assets/live2d/framing.json")
    .then((r) => (r.ok ? r.text() : "{}"))
    .then((text) => {
      const data = JSON.parse(text.replace(/^﻿/, "")) as { models?: ({ id: string } & Framing)[] };
      return new Map((data.models ?? []).map((m) => [m.id, { top: m.top, scale: m.scale }]));
    })
    .catch(() => new Map<string, Framing>());
  return framingFile.then((m) => m.get(id));
}

/** The measured model (what the stand lays out), or null while it loads. */
interface StandMeasure {
  canvas: [number, number];
  framing: Framing | undefined;
  eyeY: number | null;
  figTop: number | null;
}

export interface Live2DStandApi {
  /** The stand box (measures itself) and the node the bounce restarts on. */
  boxRef: RefObject<HTMLDivElement | null>;
  animRef: RefObject<HTMLDivElement | null>;
  /** Forwarded to the inner `Live2DModel` (reactions). */
  modelHandle: RefObject<{ playReaction(): boolean } | null>;
  /** The 3:4 portrait window inside the stand box. */
  win: StandBox | null;
  /** Where the canvas sits inside that window. */
  model: StandBox | null;
  /** The static art / loading tile is all there is. */
  failed: boolean;
  /** The model has started painting: fade the static art out. */
  shown: boolean;
  /** The model is up for good: drop the static art from the tree. */
  gone: boolean;
}

/**
 * The character stand's Live2D layer: loads the model + `framing.json` for
 * `id`, lays the canvas out in the stand box (re-measured on resize), and
 * reveals it over the static art. The static art stays until the model has had
 * a few frames to paint, and stays for good when the model is missing or
 * broken. Everything restarts cleanly when `id` changes.
 */
export function useLive2DStand(opts: {
  id: string;
  zoom?: number;
  focusTop?: number;
  headroom?: number;
  eyeFrac?: number;
  fit?: "center" | "top";
}): Live2DStandApi {
  const { id, zoom = 1, focusTop = 0, headroom = 0.02, eyeFrac = 0.28, fit = "center" } = opts;
  const boxRef = useRef<HTMLDivElement | null>(null);
  const animRef = useRef<HTMLDivElement | null>(null);
  const modelHandle = useRef<{ playReaction(): boolean } | null>(null);
  const [measure, setMeasure] = useState<StandMeasure | null>(null);
  const [win, setWin] = useState<StandBox | null>(null);
  const [model, setModel] = useState<StandBox | null>(null);
  const [failed, setFailed] = useState(false);
  const [shown, setShown] = useState(false);
  const [gone, setGone] = useState(false);

  // Load model.json + textures first: the static art stays until the model is
  // ready to draw, and stays for good when the model is missing or broken.
  useEffect(() => {
    setMeasure(null);
    setWin(null);
    setModel(null);
    setShown(false);
    setGone(false);
    setFailed(false);
    let live = true;
    void Promise.all([loadLive2D(id), framingOf(id)]).then(
      ([data, f]) => {
        if (!live) return;
        setMeasure({
          canvas: data.model.canvas,
          framing: f,
          eyeY: eyeLine(data.model),
          figTop: figureTop(data.model),
        });
      },
      () => {
        if (live) setFailed(true);
      },
    );
    return () => {
      live = false;
    };
  }, [id]);

  // Lay the canvas out inside the stand box (re-run on box resize and when the
  // model or any of the framing inputs change).
  useResizeObserver(boxRef, (el) => {
    const w = el.clientWidth;
    const h = el.clientHeight;
    if (!measure || !w || !h) {
      setWin(null);
      setModel(null);
      return;
    }
    // the 3:4 portrait window: FitInParent centres it, `fit="top"` pulls it
    // up flush with the stand's top edge so the head has room to breathe
    const dw = Math.min(w, h * 0.75);
    const dh = dw / 0.75;
    setWin({ left: (w - dw) / 2, top: fit === "top" ? 0 : (h - dh) / 2, width: dw, height: dh });
    setModel(canvasBox(dw, measure.canvas, measure.framing, zoom, focusTop, headroom, measure.eyeY, eyeFrac, measure.figTop));
  }, [measure, zoom, focusTop, headroom, eyeFrac, fit]);

  // The renderer draws on its first RAF after mount; fade the static art out
  // once the model has had a few frames to paint, then drop it entirely. The
  // drop is on a timer rather than `transitionend`: a hidden tab freezes CSS
  // transition timelines (`currentTime` stays 0), which would leave the art
  // ghosting through the canvas's transparent pixels forever.
  useTimeout(() => setShown(true), model ? 150 : null);
  useTimeout(() => setGone(true), model ? 500 : null);

  return { boxRef, animRef, modelHandle, win, model, failed, shown, gone };
}