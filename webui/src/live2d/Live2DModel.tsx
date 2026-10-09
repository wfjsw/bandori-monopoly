import { useImperativeHandle, type Ref } from "react";

import { cx } from "../core/cx";
import { useLangVersion } from "../core/hooks";
import { useLive2DModel } from "../hooks/live2d";
import { t as tr } from "../i18n/t";
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

/**
 * Cubism 2 model viewer: a canvas with a RAF loop driving the ported
 * `Live2DPortrait.RenderFrame` (idle motion, tap reactions, hair physics, eye
 * blink) and redrawing the deformed pose every frame. Without animation (or
 * with no `catalog.json` motion) it holds the posed frame like before.
 *
 * All of the resource work (load, draw loop, GL teardown, debug hooks) lives
 * in `useLive2DModel`; this component is the box and the status banner.
 */
export function Live2DModel({ id, params, className, animate = true, ref }: Live2DModelProps) {
  useLangVersion();
  const model = useLive2DModel({ id, params, animate });
  useImperativeHandle(ref, () => ({ playReaction: model.playReaction }), [model]);

  return (
    <div ref={model.wrapRef} className={cx(s.wrap, className)} onClick={model.onClick}>
      <canvas ref={model.canvasRef} className={s.canvas} />
      {model.status !== "ready" && (
        <div className={cx(s.state, model.status !== "loading" && s.error)}>
          {model.status === "loading"
            ? tr("live2d.loading")
            : model.status === "gl-error"
              ? tr("live2d.renderFailed")
              : tr("live2d.loadFailed", { id })}
        </div>
      )}
    </div>
  );
}