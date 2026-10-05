// Cubism 2 hair / clothes physics.
//
// Ported from the shipped runtime (`Live2DUnity.dll`): `PhysicsHair` (the
// two-point spring chain) and `live2d.framework.L2DPhysics` (the `physics.json`
// loader and per-frame driver). The integration is copied step for step --
// including the p2 velocity being recomputed from the position delta after the
// length constraint, and the "first call only seeds the clock" warm-up. Time is
// passed in explicitly so the sim is a pure function of (state, time).

import type { ParamTarget } from "./motion";

const DEG_TO_RAD = Math.PI / 180;

/** `physics.json` (`"type": "Live2D Physics"`). */
export interface PhysicsJson {
  type?: string;
  physics_hair: PhysicsHairSpec[];
}

export interface PhysicsHairSpec {
  label?: string;
  setup: { length: number; regist: number; mass: number };
  src: PhysicsSrcSpec[];
  targets: PhysicsTargetSpec[];
}

export interface PhysicsSrcSpec {
  id: string;
  /** `"x"` | `"y"` | `"angle"`. */
  ptype: string;
  scale: number;
  weight: number;
}

export interface PhysicsTargetSpec {
  id: string;
  /** `"angle"` | `"angle_v"`. */
  ptype: string;
  scale: number;
  weight: number;
}

/** Parse `physics.json` text into hair specs. */
export function parsePhysics(text: string): PhysicsHairSpec[] {
  const data = JSON.parse(text.replace(/^﻿/, "")) as PhysicsJson;
  const hairs = Array.isArray(data.physics_hair) ? data.physics_hair : [];
  return hairs.map((h) => ({
    label: h.label,
    setup: {
      length: Number(h.setup?.length) || 0,
      regist: Number(h.setup?.regist) || 0,
      mass: Number(h.setup?.mass) || 1,
    },
    src: (h.src ?? []).map((s) => ({
      id: String(s.id),
      ptype: String(s.ptype),
      scale: Number(s.scale) || 0,
      weight: Number(s.weight) || 0,
    })),
    targets: (h.targets ?? []).map((t) => ({
      id: String(t.id),
      ptype: String(t.ptype),
      scale: Number(t.scale) || 0,
      weight: Number(t.weight) || 0,
    })),
  }));
}

/** `PhysicsHair.PhysicsPoint`. */
class PhysicsPoint {
  mass = 1;
  x = 0;
  y = 0;
  vx = 0;
  vy = 0;
  ax = 0;
  ay = 0;
  fx = 0;
  fy = 0;
  last_x = 0;
  last_y = 0;
  last_vx = 0;
  last_vy = 0;

  save(): void {
    this.last_x = this.x;
    this.last_y = this.y;
    this.last_vx = this.vx;
    this.last_vy = this.vy;
  }
}

export type PhysicsSrcType = "x" | "y" | "angle";
export type PhysicsTargetType = "angle" | "angle_v";

/**
 * `PhysicsHair`: p1 is the root (dragged around by the source parameters), p2
 * is the tip (integrated under gravity + air resistance, then pinned back to
 * `baseLengthM` from p1). The tip angle feeds the target parameters.
 */
export class PhysicsHair {
  private readonly p1 = new PhysicsPoint();
  private readonly p2 = new PhysicsPoint();
  private baseLengthM = 0.3;
  private gravityAngleDeg = 0;
  private airResistance = 0.5;
  private angleP1toP2Deg = 0;
  private last_angleP1toP2Deg = 0;
  private angleP1toP2Deg_v = 0;
  private startTime = 0;
  private lastTime = 0;

  private readonly srcList: { type: PhysicsSrcType; id: string; scale: number; weight: number }[] = [];
  private readonly targetList: { type: PhysicsTargetType; id: string; scale: number; weight: number }[] = [];

  constructor(baseLengthM = 0.3, airResistance = 0.5, mass = 0.1) {
    this.setup(baseLengthM, airResistance, mass);
  }

  static fromSpec(spec: PhysicsHairSpec): PhysicsHair {
    const hair = new PhysicsHair(spec.setup.length, spec.setup.regist, spec.setup.mass);
    for (const s of spec.src) {
      const type: PhysicsSrcType = s.ptype === "y" ? "y" : s.ptype === "angle" ? "angle" : "x";
      hair.addSrcParam(type, s.id, s.scale, s.weight);
    }
    for (const t of spec.targets) {
      const type: PhysicsTargetType = t.ptype === "angle_v" ? "angle_v" : "angle";
      hair.addTargetParam(type, t.id, t.scale, t.weight);
    }
    return hair;
  }

  /** `PhysicsHair.setup(length, regist, mass)`. */
  setup(baseLengthM: number, airResistance: number, mass: number): void {
    this.baseLengthM = baseLengthM;
    this.airResistance = airResistance;
    this.p1.mass = mass;
    this.p2.mass = mass;
    this.p2.y = baseLengthM;
    this.setupPoints();
  }

  /** `PhysicsHair.setup()` (no args): snapshot the tip angle and p2 state. */
  private setupPoints(): void {
    this.last_angleP1toP2Deg = this.angleOf();
    this.p2.save();
  }

  addSrcParam(type: PhysicsSrcType, id: string, scale: number, weight: number): void {
    this.srcList.push({ type, id, scale, weight });
  }

  addTargetParam(type: PhysicsTargetType, id: string, scale: number, weight: number): void {
    this.targetList.push({ type, id, scale, weight });
  }

  /** `-_0020()`: the p1->p2 angle in degrees. */
  private angleOf(): number {
    return (-180 * Math.atan2(this.p1.x - this.p2.x, -(this.p1.y - this.p2.y))) / Math.PI;
  }

  get angle(): number {
    return this.angleP1toP2Deg;
  }

  get angleV(): number {
    return this.angleP1toP2Deg_v;
  }

  /**
   * `PhysicsHair.update(model, time)` -- `time` is milliseconds since the
   * `L2DPhysics` was constructed. The first call only seeds the clock (the
   * runtime treats `startTime == 0` as "not started", so a call at `time == 0`
   * costs one extra warm-up frame).
   */
  update(model: ParamTarget, time: number): void {
    if (this.startTime === 0) {
      this.startTime = time;
      this.lastTime = time;
      this.baseLengthM = Math.sqrt(
        (this.p1.x - this.p2.x) * (this.p1.x - this.p2.x) + (this.p1.y - this.p2.y) * (this.p1.y - this.p2.y),
      );
      return;
    }
    const dt = (time - this.lastTime) / 1000;
    if (dt !== 0) {
      for (let i = this.srcList.length - 1; i >= 0; i--) {
        const src = this.srcList[i];
        const value = src.scale * model.getParam(src.id);
        if (src.type === "x") {
          this.p1.x += (value - this.p1.x) * src.weight;
        } else if (src.type === "y") {
          this.p1.y += (value - this.p1.y) * src.weight;
        } else {
          let g = this.gravityAngleDeg;
          g += (value - g) * src.weight;
          this.gravityAngleDeg = g;
        }
      }
      this.integrate(dt);
      this.angleP1toP2Deg = this.angleOf();
      this.angleP1toP2Deg_v = (this.angleP1toP2Deg - this.last_angleP1toP2Deg) / dt;
      this.last_angleP1toP2Deg = this.angleP1toP2Deg;
    }
    for (let i = this.targetList.length - 1; i >= 0; i--) {
      const target = this.targetList[i];
      const value = target.scale * (target.type === "angle_v" ? this.angleP1toP2Deg_v : this.angleP1toP2Deg);
      model.setParamWeighted(target.id, value, target.weight);
    }
    this.lastTime = time;
  }

  /** `PhysicsHair._3000_3000_3000`: one spring step of `dt` seconds. */
  private integrate(dt: number): void {
    const { p1, p2 } = this;
    const inv = 1 / dt;
    p1.vx = (p1.x - p1.last_x) * inv;
    p1.vy = (p1.y - p1.last_y) * inv;
    p1.ax = (p1.vx - p1.last_vx) * inv;
    p1.ay = (p1.vy - p1.last_vy) * inv;
    p1.fx = p1.ax * p1.mass;
    p1.fy = p1.ay * p1.mass;
    p1.save();

    const ang = -Math.atan2(p1.y - p2.y, p1.x - p2.x);
    const cos = Math.cos(ang);
    const sin = Math.sin(ang);
    const g = 9.8 * p2.mass;
    const ga = this.gravityAngleDeg * DEG_TO_RAD;
    const gProj = g * Math.cos(ang - ga);
    const fxGravity = gProj * sin;
    const fyGravity = gProj * cos;
    const fxFromP1 = -p1.fx * sin * sin;
    const fyFromP1 = -p1.fy * sin * cos;
    const fxDrag = -p2.vx * this.airResistance;
    const fyDrag = -p2.vy * this.airResistance;
    p2.fx = fxGravity + fxFromP1 + fxDrag;
    p2.fy = fyGravity + fyFromP1 + fyDrag;
    p2.ax = p2.fx / p2.mass;
    p2.ay = p2.fy / p2.mass;
    p2.vx += p2.ax * dt;
    p2.vy += p2.ay * dt;
    p2.x += p2.vx * dt;
    p2.y += p2.vy * dt;

    // Pin the tip back to the rest length.
    const dist = Math.sqrt((p1.x - p2.x) * (p1.x - p2.x) + (p1.y - p2.y) * (p1.y - p2.y));
    p2.x = p1.x + (this.baseLengthM * (p2.x - p1.x)) / dist;
    p2.y = p1.y + (this.baseLengthM * (p2.y - p1.y)) / dist;
    p2.vx = (p2.x - p2.last_x) * inv;
    p2.vy = (p2.y - p2.last_y) * inv;
    p2.save();
  }
}

/**
 * `live2d.framework.L2DPhysics`: a bag of `PhysicsHair` driven off one clock
 * (`updateParam` passes `now - startTimeMSec` to each hair).
 */
export class L2DPhysics {
  readonly physicsList: PhysicsHair[];
  readonly startTimeMSec: number;

  constructor(physicsList: PhysicsHair[], startTimeMSec = 0) {
    this.physicsList = physicsList;
    this.startTimeMSec = startTimeMSec;
  }

  static fromSpecs(specs: readonly PhysicsHairSpec[], startTimeMSec = 0): L2DPhysics {
    return new L2DPhysics(
      specs.map((s) => PhysicsHair.fromSpec(s)),
      startTimeMSec,
    );
  }

  updateParam(model: ParamTarget, now: number): void {
    const time = now - this.startTimeMSec;
    for (const hair of this.physicsList) hair.update(model, time);
  }
}