// Live2D stand animation: the frame semantics of
// `BandoriMonopoly.UI.Live2DPortrait.RenderFrame()`.
//
//   loadParam()                                  // restore the saved base
//   if (motions.isFinished()) {                  // idle restarts the queue
//     IsPlayingReaction = false;
//     if (idle) motions.startMotion(idle);
//   }
//   motions.updateParam(model);                  // bake the queue into params
//   if (!IsPlayingReaction) saveParam();         // motion -> saved base
//   if (!IsPlayingReaction) blink.updateParam(); // eye blink on top (not saved)
//   physics.updateParam(model);                  // hair/clothes on top
//   model.update();                              // evaluate the pose
//
// Clicking the stand plays a random one-shot reaction (`TryPlayReaction`);
// while it plays the bake and the blink are suppressed and reactions do not
// stack. Everything is a pure function of (state, dt) so it can run under node.

// The `.ts` extensions keep this module loadable under plain `node`
// (type-stripped, no bundler) for check-motion.mjs.
import { Live2DMotion, MotionQueueManager, type MotionEnt, type ParamTarget, type ParsedMotion } from "./motion.ts";
import { L2DPhysics, type PhysicsHairSpec } from "./physics.ts";
import type { ModelParam } from "./types.ts";

/** The parameter array of `ModelContext`, with C# `setParamFloat` semantics. */
export class ParamStore implements ParamTarget {
  readonly ids: string[] = [];
  values: Float64Array;
  private mins: Float64Array;
  private maxs: Float64Array;
  private defs: Float64Array;
  private readonly index = new Map<string, number>();

  constructor(params: readonly ModelParam[]) {
    const n = params.length;
    this.values = new Float64Array(n);
    this.mins = new Float64Array(n);
    this.maxs = new Float64Array(n);
    this.defs = new Float64Array(n);
    for (let i = 0; i < n; i++) {
      const p = params[i];
      this.ids.push(p.id);
      this.values[i] = p.def;
      this.mins[i] = p.min;
      this.maxs[i] = p.max;
      this.defs[i] = p.def;
      this.index.set(p.id, i);
    }
  }

  /** `ModelContext.getParamIndex`: unknown ids become ±1e6 ranged params. */
  ensure(id: string): number {
    let i = this.index.get(id);
    if (i !== undefined) return i;
    i = this.ids.length;
    this.ids.push(id);
    const grow = (src: Float64Array, fill: number): Float64Array => {
      const next = new Float64Array(i + 1);
      next.set(src);
      next[i] = fill;
      return next;
    };
    this.values = grow(this.values, 0);
    this.mins = grow(this.mins, -1e6);
    this.maxs = grow(this.maxs, 1e6);
    this.defs = grow(this.defs, 0);
    this.index.set(id, i);
    return i;
  }

  getParam(id: string): number {
    return this.values[this.ensure(id)];
  }

  /** `ModelContext.setParamFloat`: NaN -> 0, clamp to [min, max]. */
  setParam(id: string, value: number): void {
    const i = this.ensure(id);
    if (!Number.isFinite(value)) value = 0;
    if (value < this.mins[i]) value = this.mins[i];
    if (value > this.maxs[i]) value = this.maxs[i];
    this.values[i] = value;
  }

  /** `setParamFloat(id, value, weight)`: `cur * (1 - w) + value * w`, clamped. */
  setParamWeighted(id: string, value: number, weight: number): void {
    const i = this.ensure(id);
    this.setParam(id, this.values[i] * (1 - weight) + value * weight);
  }

  getParamMin(id: string): number {
    return this.mins[this.ensure(id)];
  }

  getParamMax(id: string): number {
    return this.maxs[this.ensure(id)];
  }

  getParamDef(id: string): number {
    return this.defs[this.ensure(id)];
  }

  /** `loadParam`: restore the saved snapshot. */
  loadFrom(saved: Float64Array): void {
    const n = Math.min(saved.length, this.values.length);
    this.values.set(saved.subarray(0, n));
  }

  /** `saveParam`: snapshot the current values. */
  saveInto(saved: Float64Array): Float64Array {
    const out = saved.length === this.values.length ? saved : new Float64Array(this.values.length);
    out.set(this.values);
    return out;
  }

  /** The `PARAM_*` record the deform evaluator takes. */
  record(known?: readonly ModelParam[]): Record<string, number> {
    const out: Record<string, number> = {};
    if (known) {
      for (const p of known) out[p.id] = this.getParam(p.id);
    } else {
      for (let i = 0; i < this.ids.length; i++) out[this.ids[i]] = this.values[i];
    }
    return out;
  }

  allFinite(): boolean {
    for (let i = 0; i < this.values.length; i++) if (!Number.isFinite(this.values[i])) return false;
    return true;
  }
}

/**
 * `live2d.framework.L2DEyeBlink` -- the game runs it enabled next to the idle
 * (`blink.SetEnabled(true)`), and `updateParam` *overwrites*
 * `PARAM_EYE_L_OPEN` / `PARAM_EYE_R_OPEN` after the motion bake. The eye
 * blinks baked into `idle.mtn` are therefore replaced at runtime; only during a
 * reaction is the blink suppressed so the reaction's own eye curves show.
 */
export class EyeBlink {
  enabled = true;
  closeIfZero = true;
  eyeID_L = "PARAM_EYE_L_OPEN";
  eyeID_R = "PARAM_EYE_R_OPEN";
  blinkIntervalMsec = 4000;
  closingMotionMsec = 100;
  closedMotionMsec = 50;
  openingMotionMsec = 150;

  private nextBlinkTime = 0;
  private stateStartTime = 0;
  private eyeState = 0; // 0 FIRST, 1 INTERVAL, 2 CLOSING, 3 CLOSED, 4 OPENING
  private readonly rand: () => number;

  constructor(rand: () => number = Math.random) {
    this.rand = rand;
  }

  calcNextBlink(now: number): number {
    return now + this.rand() * (2 * this.blinkIntervalMsec - 1);
  }

  updateParam(model: ParamTarget, now: number): void {
    if (!this.enabled) return;
    let t = 0;
    let open: number;
    switch (this.eyeState) {
      case 2: // CLOSING
        t = (now - this.stateStartTime) / this.closingMotionMsec;
        if (t >= 1) {
          t = 1;
          this.eyeState = 3;
          this.stateStartTime = now;
        }
        open = 1 - t;
        break;
      case 3: // CLOSED
        t = (now - this.stateStartTime) / this.closedMotionMsec;
        if (t >= 1) {
          this.eyeState = 4;
          this.stateStartTime = now;
        }
        open = 0;
        break;
      case 4: // OPENING
        t = (now - this.stateStartTime) / this.openingMotionMsec;
        if (t >= 1) {
          t = 1;
          this.eyeState = 1;
          this.nextBlinkTime = this.calcNextBlink(now);
        }
        open = t;
        break;
      case 1: // INTERVAL
        if (this.nextBlinkTime < now) {
          this.eyeState = 2;
          this.stateStartTime = now;
        }
        open = 1;
        break;
      default: // FIRST
        this.eyeState = 1;
        this.nextBlinkTime = this.calcNextBlink(now);
        open = 1;
        break;
    }
    if (!this.closeIfZero) open = -open;
    model.setParam(this.eyeID_L, open);
    model.setParam(this.eyeID_R, open);
  }
}

export interface AnimOptions {
  /** `model.json` params (the store's initial parameter set). */
  params: readonly ModelParam[];
  /** `catalog.json` `motion` (already parsed); null = no idle. */
  idle?: ParsedMotion | null;
  /** `catalog.json` `reactions`, in order. */
  reactions?: readonly ParsedMotion[];
  /** `physics.json` hairs; null/omitted = no physics. */
  physics?: readonly PhysicsHairSpec[] | null;
  /** Initial clock (ms). The game's clock starts near 0; either is fine. */
  now?: number;
  /** Injected RNG for blink scheduling / reaction choice (tests seed it). */
  rand?: () => number;
  /** External parameter values layered *under* the motion (the `params` prop). */
  props?: Record<string, number>;
}

/**
 * `Live2DPortrait` state: the param store + its saved base, the motion queue,
 * idle/reactions, physics and blink.
 */
export interface AnimState {
  store: ParamStore;
  saved: Float64Array;
  queue: MotionQueueManager;
  idle: Live2DMotion | null;
  reactions: Live2DMotion[];
  physics: L2DPhysics | null;
  blink: EyeBlink;
  isPlayingReaction: boolean;
  lastReaction: number;
  now: number;
  rand: () => number;
  /** Keys currently supplied by the `params` prop (for reset-on-drop). */
  propKeys: Set<string>;
}

/** `Live2DMotion.LoadOneShot`: one-shot reaction, fades 250/350. */
function toReaction(data: ParsedMotion): Live2DMotion {
  const m = new Live2DMotion(data);
  m.setLoop(false);
  m.setFadeIn(250);
  m.setFadeOut(350);
  return m;
}

export function createAnim(options: AnimOptions): AnimState {
  const rand = options.rand ?? Math.random;
  const store = new ParamStore(options.params);
  const idle = options.idle ? new Live2DMotion(options.idle) : null;
  const reactions = (options.reactions ?? []).map(toReaction);
  const state: AnimState = {
    store,
    saved: new Float64Array(store.values.length),
    queue: new MotionQueueManager(),
    idle,
    reactions,
    physics: options.physics && options.physics.length ? L2DPhysics.fromSpecs(options.physics, options.now ?? 0) : null,
    blink: new EyeBlink(rand),
    isPlayingReaction: false,
    lastReaction: -1,
    now: options.now ?? 0,
    rand,
    propKeys: new Set<string>(),
  };
  // The game calls `model.saveParam()` once after load, so the saved base is
  // the model defaults (seeded here with the `params` prop).
  applyProps(state, options.props);
  state.saved = state.store.saveInto(state.saved);
  return state;
}

/**
 * Layer external parameter values under the motion: they seed the saved base,
 * so the motion blends from them and untouched parameters keep them. Keys the
 * prop used to set fall back to the model default when they are dropped.
 */
export function applyProps(state: AnimState, props?: Record<string, number>): void {
  const { store } = state;
  for (const id of state.propKeys) {
    if (!props || !(id in props)) store.setParam(id, store.getParamDef(id));
  }
  state.propKeys.clear();
  if (props) {
    for (const [id, v] of Object.entries(props)) {
      store.setParam(id, v);
      state.propKeys.add(id);
    }
  }
}

/** `model.loadParam()`. */
export function loadParam(state: AnimState): void {
  state.store.loadFrom(state.saved);
}

/** `model.saveParam()`. */
export function saveParam(state: AnimState): void {
  state.saved = state.store.saveInto(state.saved);
}

/**
 * `Live2DPortrait.TryPlayReaction`: pick a random reaction (never the one that
 * just played, when there is a choice) and start it. Returns false while a
 * reaction is already playing -- reactions do not stack.
 */
export function playReaction(state: AnimState): boolean {
  if (state.reactions.length === 0 || state.isPlayingReaction) return false;
  const n = state.reactions.length;
  const span = n - (state.lastReaction >= 0 && n > 1 ? 1 : 0);
  let pick = Math.floor(state.rand() * span);
  if (pick >= span) pick = span - 1;
  if (n > 1 && state.lastReaction >= 0 && pick >= state.lastReaction) pick++;
  state.queue.startMotion(state.reactions[pick], state.now);
  state.lastReaction = pick;
  state.isPlayingReaction = true;
  return true;
}

/**
 * One `RenderFrame()` step minus the drawing: restore the base, advance the
 * motion queue (restarting the idle when it empties), bake, blink, physics.
 */
export function renderFrame(state: AnimState): void {
  loadParam(state);
  if (state.queue.isFinished()) {
    state.isPlayingReaction = false;
    if (state.idle) state.queue.startMotion(state.idle, state.now);
  }
  state.queue.updateParam(state.store, state.now);
  if (!state.isPlayingReaction) saveParam(state);
  if (!state.isPlayingReaction) state.blink.updateParam(state.store, state.now);
  // Lip sync is not ported (no voice audio on the web stands).
  state.physics?.updateParam(state.store, state.now);
}

/** Advance the clock by `dtMsec` and run one frame. */
export function tickAnim(state: AnimState, dtMsec: number): void {
  state.now += dtMsec;
  renderFrame(state);
}

/** Current parameter values (for the deform evaluator / debug hooks). */
export function animValues(state: AnimState): Record<string, number> {
  return state.store.record();
}

/** All queued ents (tests inspect the idle restart). */
export function animQueue(state: AnimState): readonly MotionEnt[] {
  return state.queue.ents;
}