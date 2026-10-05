// Cubism 2 `.mtn` parser + motion player.
//
// Ported from the shipped runtime (`Live2DUnity.dll`): `Live2DMotion.loadMotion`
// (the byte-stream parser), `AMotion.updateParam` (fade weights + queue ent
// bookkeeping) and `MotionQueueManager` (start / update / finish). The math is
// line-faithful to the decompiled source -- including the quirks called out
// below. Time is an explicit `now` argument (milliseconds) so the whole player
// is a pure function of (state, now) and can run under node.

/** `MOTION_TYPE_PARAM`: a normal parameter curve, lerped between samples. */
export const MTN_PARAM = 0;
/** `MOTION_TYPE_PARTS_VISIBLE`: `VISIBLE:NAME=…`, set without interpolation. */
export const MTN_PARTS_VISIBLE = 1;
/** `MOTION_TYPE_LAYOUT_X` .. `_Y`: `LAYOUT:…` curves; parsed, never applied. */
export const MTN_LAYOUT_X = 100;
export const MTN_LAYOUT_Y = 101;
export const MTN_LAYOUT_ANCHOR_X = 102;
export const MTN_LAYOUT_ANCHOR_Y = 103;
export const MTN_LAYOUT_SCALE_X = 104;
export const MTN_LAYOUT_SCALE_Y = 105;

export interface MotionCurve {
  /**
   * Parameter id. For `VISIBLE:` curves the `VISIBLE:` prefix is kept (the
   * runtime writes `setParamFloat("VISIBLE:PARTS_…", v)` verbatim); for
   * `LAYOUT:` curves it is the name after the prefix.
   */
  paramId: string;
  motionType: number;
  /** One sample per frame; a single value is a constant curve. */
  values: Float64Array;
  /** `-1` = inherit the motion's global fade. */
  fadeInMsec: number;
  fadeOutMsec: number;
}

export interface ParsedMotion {
  curves: MotionCurve[];
  /** `$fps=N` (clamped to `(5, 121)`); default 30. */
  fps: number;
  /** Longest curve, in frames. */
  maxLength: number;
  /** Global `$fadein` / `$fadeout` in msec; default 1000 each. */
  fadeInMsec: number;
  fadeOutMsec: number;
  /** `(int)(1000 * maxLength / fps)` -- `AMotion.getDurationMSec`. */
  loopDurationMSec: number;
}

/**
 * `live2d._3000_3000_0020_0020_0020._0020`: the fade weight curve.
 *
 * Not clamp01: inside `[0,1]` it is the cosine ease `0.5 - 0.5*cos(t*PI)`,
 * clamped to 0 below 0 and 1 above 1. Every fade in the runtime goes through
 * this (global and per-parameter), so fade-in accelerates and fade-out slows
 * down instead of moving linearly.
 */
export function fadeWeight(t: number): number {
  if (t < 0) return 0;
  if (t > 1) return 1;
  return 0.5 - 0.5 * Math.cos(t * Math.PI);
}

/**
 * The runtime's float scanner (`_3000_0020_0020_3000._3000_3000`): optional
 * leading `-`, digits, optional `.` + fraction. No `+`, no exponent.
 */
const NUMBER_RE = /-?(?:\d+(?:\.\d*)?|\.\d+)/g;

function scanNumbers(src: string): number[] {
  const out: number[] = [];
  NUMBER_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = NUMBER_RE.exec(src)) !== null) out.push(parseFloat(m[0]));
  return out;
}

function firstOrLastNumber(src: string, last: boolean): number {
  const n = scanNumbers(src);
  if (n.length === 0) return NaN;
  return last ? n[n.length - 1] : n[0];
}

/**
 * `Live2DMotion.loadMotion` on the `.mtn` text.
 *
 * Format: `#` comment lines; `$fps=N`; `$fadein=MS` / `$fadeout=MS`;
 * `$fadein:PARAM=MS` / `$fadeout:PARAM=MS` per-parameter overrides (applied to
 * the first curve with that id after the whole file is read);
 * `PARAM=v0,v1,…` sample lists; `VISIBLE:PARAM=…` (set, no lerp);
 * `LAYOUT:ANCHOR_X|ANCHOR_Y|SCALE_X|SCALE_Y|X|Y=…` layout curves (parsed but
 * never applied by `updateParamExe`, matching the runtime).
 */
export function parseMotion(text: string): ParsedMotion {
  const curves: MotionCurve[] = [];
  const fadeInOverride = new Map<string, number>();
  const fadeOutOverride = new Map<string, number>();
  let fps = 30;
  let fadeInMsec = 1000;
  let fadeOutMsec = 1000;
  let maxLength = 0;

  for (const raw of text.split(/\r?\n/)) {
    const hash = raw.indexOf("#");
    const line = (hash >= 0 ? raw.slice(0, hash) : raw).trim();
    if (!line) continue;

    if (line.startsWith("$")) {
      const m = /^\$(fps|fadein|fadeout)(?::([^=]+))?=(.*)$/.exec(line);
      if (!m) continue;
      const kind = m[1];
      const overrideName = m[2];
      // The runtime keeps the *last* number successfully scanned on the line.
      const value = firstOrLastNumber(m[3], true);
      if (!Number.isFinite(value)) continue;
      if (kind === "fps") {
        // `$fps=` only: `num3 == num2 + 4`, and the value must be in (5, 121).
        if (overrideName === undefined && 5 < value && value < 121) fps = value;
      } else if (kind === "fadein") {
        if (overrideName === undefined) {
          if (value >= 0) fadeInMsec = Math.trunc(value);
        } else if (value >= 0) {
          fadeInOverride.set(overrideName, Math.trunc(value));
        }
      } else if (overrideName === undefined) {
        if (value >= 0) fadeOutMsec = Math.trunc(value);
      } else if (value >= 0) {
        fadeOutOverride.set(overrideName, Math.trunc(value));
      }
      continue;
    }

    const eq = line.indexOf("=");
    if (eq <= 0) continue;
    // A curve line must start with a letter or `_`.
    const c0 = line.charCodeAt(0);
    const isIdent = (c0 >= 97 && c0 <= 122) || (c0 >= 65 && c0 <= 90) || c0 === 95;
    if (!isIdent) continue;

    const name = line.slice(0, eq);
    const values = scanNumbers(line.slice(eq + 1));
    if (values.length === 0) continue;

    let motionType = MTN_PARAM;
    let paramId = name;
    if (name.startsWith("VISIBLE:")) {
      // The runtime keeps the whole `VISIBLE:…` string as the parameter id.
      motionType = MTN_PARTS_VISIBLE;
    } else if (name.startsWith("LAYOUT:")) {
      paramId = name.slice(7);
      if (paramId.startsWith("ANCHOR_X")) motionType = MTN_LAYOUT_ANCHOR_X;
      else if (paramId.startsWith("ANCHOR_Y")) motionType = MTN_LAYOUT_ANCHOR_Y;
      else if (paramId.startsWith("SCALE_X")) motionType = MTN_LAYOUT_SCALE_X;
      else if (paramId.startsWith("SCALE_Y")) motionType = MTN_LAYOUT_SCALE_Y;
      else if (paramId.startsWith("X")) motionType = MTN_LAYOUT_X;
      else if (paramId.startsWith("Y")) motionType = MTN_LAYOUT_Y;
      // Unrecognised `LAYOUT:` names fall through as PARAM (runtime default).
    }

    curves.push({
      paramId,
      motionType,
      values: Float64Array.from(values),
      fadeInMsec: -1,
      fadeOutMsec: -1,
    });
    if (values.length > maxLength) maxLength = values.length;
  }

  // Per-parameter fade overrides hit the first curve with that id.
  for (const [id, ms] of fadeInOverride) {
    const curve = curves.find((c) => c.paramId === id);
    if (curve) curve.fadeInMsec = ms;
  }
  for (const [id, ms] of fadeOutOverride) {
    const curve = curves.find((c) => c.paramId === id);
    if (curve) curve.fadeOutMsec = ms;
  }

  return {
    curves,
    fps,
    maxLength,
    fadeInMsec,
    fadeOutMsec,
    loopDurationMSec: Math.trunc((1000 * maxLength) / fps),
  };
}

/** The parameter side of `ALive2DModel` that the player touches. */
export interface ParamTarget {
  getParam(id: string): number;
  setParam(id: string, value: number): void;
  setParamWeighted(id: string, value: number, weight: number): void;
  getParamMin(id: string): number;
  getParamMax(id: string): number;
}

/** `MotionQueueEnt`: one queued motion instance with its own clock. */
export interface MotionEnt {
  motion: Live2DMotion;
  available: boolean;
  finished: boolean;
  startTimeMSec: number;
  fadeInStartTimeMSec: number;
  endTimeMSec: number;
  motionQueueEntNo: number;
}

let nextMotionEntNo = 1;

export function createMotionEnt(motion: Live2DMotion): MotionEnt {
  return {
    motion,
    available: true,
    finished: false,
    startTimeMSec: -1,
    fadeInStartTimeMSec: -1,
    endTimeMSec: -1,
    motionQueueEntNo: nextMotionEntNo++,
  };
}

/**
 * `AMotion` + `Live2DMotion`: one parsed file plus the per-instance control
 * state the game mutates (`setLoop`, `setFadeIn`, `setFadeOut`, `setWeight`).
 */
export class Live2DMotion {
  readonly data: ParsedMotion;
  /** `AMotion` defaults: 1000/1000, weight 1; `Live2DMotion` starts unlooped. */
  fadeInMsec: number;
  fadeOutMsec: number;
  weight = 1;
  loop = false;

  constructor(data: ParsedMotion) {
    this.data = data;
    this.fadeInMsec = data.fadeInMsec;
    this.fadeOutMsec = data.fadeOutMsec;
  }

  static fromText(text: string): Live2DMotion {
    return new Live2DMotion(parseMotion(text));
  }

  setLoop(loop: boolean): void {
    this.loop = loop;
  }

  setFadeIn(msec: number): void {
    this.fadeInMsec = msec;
  }

  setFadeOut(msec: number): void {
    this.fadeOutMsec = msec;
  }

  setWeight(weight: number): void {
    this.weight = weight;
  }

  /** `AMotion.getDurationMSec`: `-1` (never ends) while looping. */
  getDurationMSec(): number {
    return this.loop ? -1 : this.data.loopDurationMSec;
  }

  getLoopDurationMSec(): number {
    return this.data.loopDurationMSec;
  }

  /**
   * `AMotion.updateParam`: initialise the ent's clock on the first call, fold
   * the global fades into the weight, run `updateParamExe`, then mark the ent
   * finished once its (optional) end time has passed.
   */
  updateParam(model: ParamTarget, now: number, ent: MotionEnt): void {
    if (!ent.available || ent.finished) return;
    if (ent.startTimeMSec < 0) {
      ent.startTimeMSec = now;
      ent.fadeInStartTimeMSec = now;
      const durationMSec = this.getDurationMSec();
      if (ent.endTimeMSec < 0) {
        ent.endTimeMSec = durationMSec <= 0 ? -1 : ent.startTimeMSec + durationMSec;
      }
    }
    let w = this.weight;
    const wIn = this.fadeInMsec === 0 ? 1 : fadeWeight((now - ent.fadeInStartTimeMSec) / this.fadeInMsec);
    const wOut =
      this.fadeOutMsec === 0 || ent.endTimeMSec < 0 ? 1 : fadeWeight((ent.endTimeMSec - now) / this.fadeOutMsec);
    w = w * wIn * wOut;
    this.updateParamExe(model, now, w, ent);
    if (ent.endTimeMSec > 0 && ent.endTimeMSec < now) ent.finished = true;
  }

  /**
   * `Live2DMotion.updateParamExe`: sample every curve at `t = (now - start) *
   * fps / 1000`, lerping frame `i` toward `i+1` unless the jump exceeds
   * `0.4 * (max - min)` (wrap/discontinuity guard -- hold `i`). Fade weights
   * use the cosine ease; a curve with its own `$fadein:`/`$fadeout:` override
   * replaces the global fades for that curve (the override path multiplies the
   * motion's base `weight`, not the already-faded `_weight`).
   */
  updateParamExe(model: ParamTarget, now: number, weight: number, ent: MotionEnt): void {
    const { curves, fps, maxLength } = this.data;
    const elapsed = now - ent.startTimeMSec;
    const t = (elapsed * fps) / 1000;
    const i = Math.trunc(t);
    const frac = t - i;
    const gIn = this.fadeInMsec === 0 ? 1 : fadeWeight((now - ent.fadeInStartTimeMSec) / this.fadeInMsec);
    const gOut =
      this.fadeOutMsec === 0 || ent.endTimeMSec < 0 ? 1 : fadeWeight((ent.endTimeMSec - now) / this.fadeOutMsec);

    for (const curve of curves) {
      const n = curve.values.length;
      const at = i >= n ? n - 1 : i;
      if (curve.motionType === MTN_PARTS_VISIBLE) {
        // Set, no interpolation.
        model.setParam(curve.paramId, curve.values[at]);
        continue;
      }
      // Layout curves (100..105) are parsed and then ignored, as in the DLL.
      if (curve.motionType >= MTN_LAYOUT_X && curve.motionType <= MTN_LAYOUT_SCALE_Y) continue;

      const paramMax = model.getParamMax(curve.paramId);
      const paramMin = model.getParamMin(curve.paramId);
      const threshold = 0.4 * (paramMax - paramMin);
      const current = model.getParam(curve.paramId);
      const v0 = curve.values[at];
      const v1 = curve.values[i + 1 >= n ? n - 1 : i + 1];
      const sample = Math.abs(v1 - v0) > threshold ? v0 : v0 + (v1 - v0) * frac;

      let w: number;
      if (curve.fadeInMsec < 0 && curve.fadeOutMsec < 0) {
        w = weight;
      } else {
        const cIn =
          curve.fadeInMsec >= 0
            ? curve.fadeInMsec === 0
              ? 1
              : fadeWeight((now - ent.fadeInStartTimeMSec) / curve.fadeInMsec)
            : gIn;
        const cOut =
          curve.fadeOutMsec >= 0
            ? curve.fadeOutMsec === 0 || ent.endTimeMSec < 0
              ? 1
              : fadeWeight((ent.endTimeMSec - now) / curve.fadeOutMsec)
            : gOut;
        w = this.weight * cIn * cOut;
      }
      model.setParam(curve.paramId, current + (sample - current) * w);
    }

    if (i >= maxLength) {
      if (this.loop) {
        // Restart in place: the samples above (clamped to the last frame) are
        // what the wrap frame shows; the next call reads from frame 0 again.
        ent.startTimeMSec = now;
        ent.fadeInStartTimeMSec = now;
      } else {
        ent.finished = true;
      }
    }
  }
}

/**
 * `MotionQueueManager`: a list of motion ents. Starting a motion schedules a
 * fade-out on everything already queued (each with its *own* fade-out time)
 * and appends the new one; `updateParam` applies the surviving ents in order
 * and drops the finished ones.
 */
export class MotionQueueManager {
  readonly ents: MotionEnt[] = [];

  startMotion(motion: Live2DMotion, now: number): number {
    for (const ent of this.ents) {
      // `MotionQueueEnt._3000_0020`: pull the end time in to `now + fadeOut`.
      const end = now + ent.motion.fadeOutMsec;
      if (ent.endTimeMSec < 0 || end < ent.endTimeMSec) ent.endTimeMSec = end;
    }
    const ent = createMotionEnt(motion);
    this.ents.push(ent);
    return ent.motionQueueEntNo;
  }

  updateParam(model: ParamTarget, now: number): boolean {
    let result = false;
    for (let i = 0; i < this.ents.length; i++) {
      const ent = this.ents[i];
      if (!ent || !ent.motion) {
        this.ents.splice(i, 1);
        i--;
        continue;
      }
      ent.motion.updateParam(model, now, ent);
      result = true;
      if (ent.finished) {
        this.ents.splice(i, 1);
        i--;
      }
    }
    return result;
  }

  isFinished(): boolean {
    for (const ent of this.ents) {
      if (ent && ent.motion && !ent.finished) return false;
    }
    return true;
  }

  isFinishedEnt(motionQueueEntNo: number): boolean {
    for (const ent of this.ents) {
      if (ent && ent.motionQueueEntNo === motionQueueEntNo && !ent.finished) return false;
    }
    return true;
  }

  stopAllMotions(): void {
    this.ents.length = 0;
  }
}