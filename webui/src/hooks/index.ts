// The semantic hooks of the web UI. Each one names a *contract* (what it
// subscribes to, what it cleans up) so components read as intent: a screen
// asks for a countdown, a hotkey list, a Live2D model -- it does not schedule
// timers or manage listeners of its own.
//
// Split by concern:
//   latest.ts   -- useLatestRef (the dependency-churn escape hatch)
//   dom.ts      -- event listeners, pagehide, hotkeys, focus
//   timers.ts   -- interval / timeout / animation frame / countdowns
//   measure.ts  -- ResizeObserver, scroll follow / reset
//   mount.ts    -- one-shot mount actions, overlay auto-close
//   async.ts    -- cancellable async work
//   scene.ts    -- the scene cross-fade
//   live2d.ts   -- the Live2D model / GL resource
//
// Store and session bindings live beside their stores (`core/hooks.ts`,
// `app/Stage.tsx`): useProfile, useSettings, useWakeLock, useTick, useMatchView,
// useSessionOther, useAutoMode, useAutoplay, useLangVersion, useBackdrop,
// useStageFill.

export { useLatestRef } from "./latest.ts";
export {
  useEventListener, usePageHide, useHotkeys, useAutoFocus,
  isEditableTarget, isControlTarget, matchHotkey,
  type Hotkey, type EventTargetLike,
} from "./dom.ts";
export {
  useInterval, useTick, useTimeout, useAnimationFrame,
  useCountdown, usePerfCountdown, useRestartAnimation,
  countdownLeft,
} from "./timers.ts";
export { useResizeObserver, useScrollFollow, useResetScroll } from "./measure.ts";
export { useMountEffect, useCloseWhen } from "./mount.ts";
export { useAsync, type AsyncState } from "./async.ts";
export { useCrossfade, type Crossfade } from "./scene.ts";
export {
  useLive2DModel, useLive2DStand, canvasBox, framingOf,
  type Live2DModelApi, type Live2DModelState, type Live2DStandApi,
  type Framing, type StandBox,
} from "./live2d.ts";