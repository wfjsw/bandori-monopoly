import { useEffect, useImperativeHandle, useRef, useState, type Ref } from "react";

import { cx } from "../core/cx";
import { useLangVersion } from "../core/hooks";
import { t as tr } from "../i18n/t";
import { animQueue, animValues, applyProps, createAnim, playReaction, tickAnim, type AnimState } from "./anim";
import { evalPose, type ModelPose } from "./deform";
import { Live2DRenderer } from "./gl";
import { loadLive2D, loadLive2DAnim, type LoadedLive2D } from "./load";
import s from "./Live2DModel.module.css";

declare global {
  interface Window {
    /**
     * Debug hook for throttled tabs / tests: step the mounted stand's
     * animation by `dtMsec` (default 16.667) and return the resulting state
     * (parameter values, whether a reaction is playing, the clock).
     */
    __live2dTick?: (dtMsec?: number) =>
      | { values: Record<string, number>; isPlayingReaction: boolean; now: number }
      | undefined;
    /** Debug hook: dump the mounted stand's motion queue. */
    __live2dState?: () => unknown;
    /** Debug hook: force one parameter into the live store (physics tests). */
    __live2dForce?: (id: string, v: number) => boolean;
  }
}

export interface Live2DModelHandle {
  /** Play a random one-shot reaction; false while one is already playing. */
  playReaction(): boolean;
}

export interface Live2DModelProps {
  /** Model id, i.e. the folder under `webui/public/assets/live2d`. */
  id: string;
  /** Parameter values keyed by `PARAM_*` id; omitted keys use the defaults. */
  params?: Record<string, number>;
  className?: string;
  /** Idle motion, click reactions and physics (the game's RenderFrame). */
  animate?: boolean;
  ref?: Ref<Live2DModelHandle>;
}

type Status = "loading" | "ready" | "load-error" | "gl-error";

function sameParams(a: Record<string, number> | undefined, b: Record<string, number> | undefined): boolean {
  if (a === b) return true;
  const ka = a ?? {};
  const kb = b ?? {};
  const keys = Object.keys(ka);
  if (keys.length !== Object.keys(kb).length) return false;
  return keys.every((k) => ka[k] === kb[k]);
}

/**
 * Cubism 2 model viewer: a canvas with a RAF loop driving the ported
 * `Live2DPortrait.RenderFrame` (idle motion, tap reactions, hair physics, eye
 * blink) and redrawing the deformed pose every frame. Without animation (or
 * with no `catalog.json` motion) it holds the posed frame like before.
 */
export function Live2DModel({ id, params, className, animate = true, ref }: Live2DModelProps) {
  useLangVersion();
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rendererRef = useRef<Live2DRenderer | null>(null);
  const modelRef = useRef<LoadedLive2D | null>(null);
  const animRef = useRef<AnimState | null>(null);
  const poseRef = useRef<ModelPose | null>(null);
  const paramsRef = useRef<Record<string, number> | undefined>(params);
  const evaluatedRef = useRef<Record<string, number> | undefined>(undefined);
  const dirtyRef = useRef(true);
  const [status, setStatus] = useState<Status>("loading");

  useImperativeHandle(
    ref,
    () => ({
      playReaction: () => {
        const anim = animRef.current;
        if (!anim) return false;
        const started = playReaction(anim);
        if (started) dirtyRef.current = true;
        return started;
      },
    }),
    [],
  );

  // Layer the `params` prop under the motion (it seeds the saved base).
  useEffect(() => {
    paramsRef.current = params;
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
    const ready = Promise.all([loadLive2D(id), animate ? loadLive2DAnim(id) : Promise.resolve(null)]);
    ready
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
  }, [id, animate]);

  // RAF loop: advance the animation and redraw. Static poses still draw only
  // when something marked them dirty.
  useEffect(() => {
    let raf = 0;
    let last = performance.now();
    const tick = (t: number) => {
      raf = requestAnimationFrame(tick);
      const dt = t - last;
      last = t;
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
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  // Debug hook: step the stand by hand from a throttled tab or the console.
  useEffect(() => {
    const step = (dtMsec = 1000 / 60) => {
      const anim = animRef.current;
      const model = modelRef.current;
      const renderer = rendererRef.current;
      if (!anim || !model) return undefined;
      tickAnim(anim, dtMsec);
      poseRef.current = evalPose(model.model, animValues(anim));
      if (renderer) renderer.render(poseRef.current.meshes);
      return { values: animValues(anim), isPlayingReaction: anim.isPlayingReaction, now: anim.now };
    };
    window.__live2dTick = step;
    return () => {
      if (window.__live2dTick === step) delete window.__live2dTick;
    };
  }, []);

  // Debug hook: expose the live AnimState so tests can inspect the queue.
  useEffect(() => {
    const dump = () => {
      const anim = animRef.current;
      if (!anim) return undefined;
      return {
        now: anim.now,
        isPlayingReaction: anim.isPlayingReaction,
        ents: animQueue(anim).map((e) => ({
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
    window.__live2dState = dump;
    window.__live2dForce = (id: string, v: number) => {
      const anim = animRef.current;
      if (!anim) return false;
      // Pin the value against loadParam: no motion writes, value in the base.
      anim.idle = null;
      anim.queue.stopAllMotions();
      anim.isPlayingReaction = false;
      anim.store.setParam(id, v);
      anim.store.saveInto(anim.saved);
      return true;
    };
    return () => {
      if (window.__live2dState === dump) delete window.__live2dState;
      delete window.__live2dForce;
    };
  }, []);

  // Keep the drawing buffer matched to the layout box (and device pixel ratio).
  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    const fit = () => {
      rendererRef.current?.resize(wrap.clientWidth, wrap.clientHeight, window.devicePixelRatio || 1);
      dirtyRef.current = true;
    };
    fit();
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", fit);
      return () => window.removeEventListener("resize", fit);
    }
    const ro = new ResizeObserver(fit);
    ro.observe(wrap);
    return () => ro.disconnect();
  }, []);

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

  // A click on the model plays a random reaction (reactions never stack).
  const handleClick = () => {
    const anim = animRef.current;
    if (anim && playReaction(anim)) dirtyRef.current = true;
  };

  return (
    <div ref={wrapRef} className={cx(s.wrap, className)} onClick={handleClick}>
      <canvas ref={canvasRef} className={s.canvas} />
      {status !== "ready" && (
        <div className={cx(s.state, status !== "loading" && s.error)}>
          {status === "loading"
            ? tr("live2d.loading")
            : status === "gl-error"
              ? tr("live2d.renderFailed")
              : tr("live2d.loadFailed", { id })}
        </div>
      )}
    </div>
  );
}