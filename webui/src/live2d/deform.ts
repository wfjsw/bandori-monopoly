// Cubism 2 deform pipeline: keyform-lattice evaluation, bilinear cage FFD,
// pivot rotation, and the walk from a mesh's local space up to the canvas.
// Pure math -- no DOM, no I/O -- so it can be checked standalone (check.mjs).
// Semantics are fixed by docs/LIVE2D-SCHEMA.md.

import type {
  KeyformGrid,
  MeshJson,
  ModelJson,
  RotDeformerJson,
  WarpDeformerJson,
} from "./types";

/** Parameter values indexed like `model.params`. */
export type ParamValues = ArrayLike<number>;

/** Interpolated per-mesh scalars plus composed canvas-space positions. */
export interface MeshFrame {
  /** Flat `x,y,…` in canvas pixels, `2 * nverts` long. */
  positions: Float32Array;
  opacity: number;
  /** False when a pose gate in the parent chain is off (see `detectPoseGates`). */
  visible: boolean;
  drawOrder: number;
  multiply: [number, number, number, number];
  screen: [number, number, number, number];
}

// ------------------------------------------------------------- keyform grid

/** Canonical lookup key for a corner: sorted `axis:keyIndex` pairs joined. */
function comboKey(at: readonly (readonly number[])[]): string {
  const pairs = at.map(([axis, key]) => `${axis}:${key}`);
  pairs.sort();
  return pairs.join(",");
}

/**
 * Multilinear weights over a keyform grid's form indices at parameter values
 * `values`. Per axis the parameter value is bracketed between two key values
 * (clamped outside the range, `t = 0` for a single key); each corner of the
 * resulting box contributes the product of its per-axis weights. A corner
 * with no form falls back to the present corner sharing the most
 * `(axis, key)` pairs.
 */
export function formWeights(grid: KeyformGrid, values: ParamValues): Map<number, number> {
  const { binds, at: entries } = grid;
  const weights = new Map<number, number>();
  if (binds.length === 0) {
    if (entries.length > 0) weights.set(entries[0].form, 1);
    return weights;
  }

  const present = new Map<string, number>();
  for (const e of entries) present.set(comboKey(e.at), e.form);

  // Per axis: [i0, i1, w0, w1]; i0 === i1 when clamped or single-keyed.
  const axes: [number, number, number, number][] = [];
  for (const b of binds) {
    const keys = b.keys;
    const v = values[b.param];
    if (keys.length === 1 || v <= keys[0]) {
      axes.push([0, 0, 1, 0]);
    } else if (v >= keys[keys.length - 1]) {
      const n = keys.length - 1;
      axes.push([n, n, 1, 0]);
    } else {
      let i0 = 0;
      for (let a = 0; a < keys.length - 1; a++) {
        if (keys[a] <= v && v <= keys[a + 1]) {
          i0 = a;
          break;
        }
      }
      const i1 = i0 + 1;
      const span = keys[i1] - keys[i0];
      const t = span === 0 ? 0 : (v - keys[i0]) / span;
      axes.push([i0, i1, 1 - t, t]);
    }
  }

  const fixed: [number, number][] = [];
  const free: number[] = [];
  for (let a = 0; a < axes.length; a++) {
    if (axes[a][0] === axes[a][1]) fixed.push([a, axes[a][0]]);
    else free.push(a);
  }

  const corners = 1 << free.length;
  for (let bits = 0; bits < corners; bits++) {
    let w = 1;
    const combo: [number, number][] = fixed.slice();
    for (let k = 0; k < free.length; k++) {
      const a = free[k];
      const [i0, i1, w0, w1] = axes[a];
      if (bits & (1 << k)) {
        combo.push([a, i1]);
        w *= w1;
      } else {
        combo.push([a, i0]);
        w *= w0;
      }
    }
    let form = present.get(comboKey(combo));
    if (form === undefined) {
      // Nearest present corner: most matching (axis, key) pairs.
      const cset = new Set(combo.map(([axis, key]) => `${axis}:${key}`));
      let best = -1;
      let score = -1;
      for (const e of entries) {
        let sc = 0;
        for (const [axis, key] of e.at) if (cset.has(`${axis}:${key}`)) sc++;
        if (sc > score) {
          score = sc;
          best = e.form;
        }
      }
      if (best < 0) continue;
      form = best;
    }
    weights.set(form, (weights.get(form) ?? 0) + w);
  }
  return weights;
}

/** Weighted sum of flat form rows (`Σ w·row`); weights sum to 1. */
export function lerpForms(
  rows: readonly (readonly number[])[],
  weights: Map<number, number>,
): Float64Array {
  let out: Float64Array | null = null;
  for (const [idx, w] of weights) {
    const row = rows[idx];
    if (!row) continue;
    if (!out) out = new Float64Array(row.length);
    for (let i = 0; i < row.length; i++) out[i] += row[i] * w;
  }
  return out ?? new Float64Array(0);
}

// ------------------------------------------------------------------- warp FFD

/** Diagonal-split quad patch: corners `q0=(0,0) q1=(1,0) q2=(0,1) q3=(1,1)`. */
function quad(
  q0x: number, q0y: number,
  q1x: number, q1y: number,
  q2x: number, q2y: number,
  q3x: number, q3y: number,
  s: number, t: number,
): [number, number] {
  if (s + t <= 1) {
    return [q0x + (q1x - q0x) * s + (q2x - q0x) * t,
            q0y + (q1y - q0y) * s + (q2y - q0y) * t];
  }
  return [q3x + (q2x - q3x) * (1 - s) + (q1x - q3x) * (1 - t),
          q3y + (q2y - q3y) * (1 - s) + (q1y - q3y) * (1 - t)];
}

/**
 * FFD over a `cols × rows` warp lattice (row-major flat `x,y,…` in the
 * warp's parent space). `(u, v)` are the cage parameters, nominally `[0,1]²`.
 *
 * Transcription of the shipped Cubism 2 runtime's `BDBoxGrid` (static
 * `transformPoints` in `Live2DUnity.dll`):
 *
 * * in-cell points are barycentrically interpolated over the cell's
 *   `s + t = 1` diagonal (NOT bilinear -- bilinear differs from the runtime
 *   anywhere strictly inside a cell);
 * * points outside the lattice use a parallelogram frame built from the four
 *   lattice corners (`F(a,b) = O + a·U + b·V`): a 5×5 skirt of corner/edge
 *   patches covering `(-2,3)²`, and pure linear extrapolation of the frame
 *   beyond that.
 */
export function ffdPoint(
  lat: ArrayLike<number>,
  cols: number,
  rows: number,
  u: number,
  v: number,
): [number, number] {
  const col = cols - 1; // the runtime's `col`/`row` are cell counts
  const row = rows - 1;
  const x = u * col;
  const y = v * row;
  if (x < 0 || y < 0 || x >= col || y >= row) {
    return ffdSkirt(lat, cols, col, row, u, v);
  }

  // in-cell: barycentric over the cell diagonal
  const i = Math.floor(x);
  const j = Math.floor(y);
  const fx = x - i;
  const fy = y - j;
  const k = (j * cols + i) * 2;
  const srow = cols * 2;
  const x00 = lat[k], y00 = lat[k + 1];
  const x10 = lat[k + 2], y10 = lat[k + 3];
  const x01 = lat[k + srow], y01 = lat[k + srow + 1];
  const x11 = lat[k + srow + 2], y11 = lat[k + srow + 3];
  if (fx + fy < 1) {
    return [x00 * (1 - fx - fy) + x10 * fx + x01 * fy,
            y00 * (1 - fx - fy) + y10 * fx + y01 * fy];
  }
  return [x11 * (fx - 1 + fy) + x01 * (1 - fx) + x10 * (1 - fy),
          y11 * (fx - 1 + fy) + y01 * (1 - fx) + y10 * (1 - fy)];
}

/** The `BDBoxGrid` out-of-lattice skirt (see `ffdPoint`). */
function ffdSkirt(
  lat: ArrayLike<number>,
  cols: number,
  col: number,
  row: number,
  u: number,
  v: number,
): [number, number] {
  const x = u * col;
  const y = v * row;
  const srow = cols * 2;
  // lattice corners: TL, TR, BL, BR
  const tlx = lat[0], tly = lat[1];
  const trx = lat[col * 2], trY = lat[col * 2 + 1];
  const blx = lat[row * srow], bly = lat[row * srow + 1];
  const brx = lat[(col + row * cols) * 2], bry = lat[(col + row * cols) * 2 + 1];
  // frame: F(a,b) = O + a·U + b·V through the four corners
  const cx = 0.25 * (tlx + trx + blx + brx);
  const cy = 0.25 * (tly + trY + bly + bry);
  const d1x = brx - tlx, d1y = bry - tly;
  const d2x = trx - blx, d2y = trY - bly;
  const ux = (d1x + d2x) * 0.5, uy = (d1y + d2y) * 0.5;
  const vx = (d1x - d2x) * 0.5, vy = (d1y - d2y) * 0.5;
  const ox = cx - 0.5 * (ux + vx);
  const oy = cy - 0.5 * (uy + vy);
  const Fx = (a: number, b: number) => ox + a * ux + b * vx;
  const Fy = (a: number, b: number) => oy + a * uy + b * vy;

  // beyond the skirt: pure linear extension of the frame
  if (!(u > -2 && u < 3 && v > -2 && v < 3)) {
    return [Fx(u, v), Fy(u, v)];
  }

  // lattice points, parameter-space row/column helpers
  const P = (i: number, j: number): [number, number] =>
    [lat[(j * cols + i) * 2], lat[(j * cols + i) * 2 + 1]];

  if (u <= 0) {
    if (v <= 0) {
      // corner: unit square in (u,v) ∈ [-2,0]² maps onto D–C–B–TL
      const b: [number, number] = [Fx(-2, 0), Fy(-2, 0)];
      const c: [number, number] = [Fx(0, -2), Fy(0, -2)];
      const d: [number, number] = [Fx(-2, -2), Fy(-2, -2)];
      return quad(d[0], d[1], c[0], c[1], b[0], b[1], tlx, tly,
                  0.5 * (u + 2), 0.5 * (v + 2));
    }
    if (v >= 1) {
      const bl: [number, number] = [blx, bly];
      const q0: [number, number] = [Fx(-2, 1), Fy(-2, 1)];
      const q2: [number, number] = [Fx(-2, 3), Fy(-2, 3)];
      const q3: [number, number] = [Fx(0, 3), Fy(0, 3)];
      return quad(q0[0], q0[1], bl[0], bl[1], q2[0], q2[1], q3[0], q3[1],
                  0.5 * (u + 2), 0.5 * (v - 1));
    }
    // left edge: v within one lattice row
    const j = Math.min(Math.floor(y), row - 1);
    const v0 = j / row;
    const v1 = (j + 1) / row;
    const p0 = P(0, j);
    const p1 = P(0, j + 1);
    const q0: [number, number] = [Fx(-2, v0), Fy(-2, v0)];
    const q2: [number, number] = [Fx(-2, v1), Fy(-2, v1)];
    return quad(q0[0], q0[1], p0[0], p0[1], q2[0], q2[1], p1[0], p1[1],
                0.5 * (u + 2), y - j);
  }
  if (u >= 1) {
    if (v <= 0) {
      const tr: [number, number] = [trx, trY];
      const q0: [number, number] = [Fx(1, -2), Fy(1, -2)];
      const q1: [number, number] = [Fx(3, -2), Fy(3, -2)];
      const q3: [number, number] = [Fx(3, 0), Fy(3, 0)];
      return quad(q0[0], q0[1], q1[0], q1[1], tr[0], tr[1], q3[0], q3[1],
                  0.5 * (u - 1), 0.5 * (v + 2));
    }
    if (v >= 1) {
      const br: [number, number] = [brx, bry];
      const q1: [number, number] = [Fx(3, 1), Fy(3, 1)];
      const q2: [number, number] = [Fx(1, 3), Fy(1, 3)];
      const q3: [number, number] = [Fx(3, 3), Fy(3, 3)];
      return quad(br[0], br[1], q1[0], q1[1], q2[0], q2[1], q3[0], q3[1],
                  0.5 * (u - 1), 0.5 * (v - 1));
    }
    // right edge
    const j = Math.min(Math.floor(y), row - 1);
    const v0 = j / row;
    const v1 = (j + 1) / row;
    const p0 = P(col, j);
    const p1 = P(col, j + 1);
    const q1: [number, number] = [Fx(3, v0), Fy(3, v0)];
    const q3: [number, number] = [Fx(3, v1), Fy(3, v1)];
    return quad(p0[0], p0[1], q1[0], q1[1], p1[0], p1[1], q3[0], q3[1],
                0.5 * (u - 1), y - j);
  }
  if (v <= 0) {
    // bottom edge
    const i = Math.min(Math.floor(x), col - 1);
    const u0 = i / col;
    const u1 = (i + 1) / col;
    const p0 = P(i, 0);
    const p1 = P(i + 1, 0);
    const q0: [number, number] = [Fx(u0, -2), Fy(u0, -2)];
    const q1: [number, number] = [Fx(u1, -2), Fy(u1, -2)];
    return quad(q0[0], q0[1], q1[0], q1[1], p0[0], p0[1], p1[0], p1[1],
                x - i, 0.5 * (v + 2));
  }
  // top edge (v >= 1)
  const i = Math.min(Math.floor(x), col - 1);
  const u0 = i / col;
  const u1 = (i + 1) / col;
  const p0 = P(i, row);
  const p1 = P(i + 1, row);
  const q2: [number, number] = [Fx(u0, 3), Fy(u0, 3)];
  const q3: [number, number] = [Fx(u1, 3), Fy(u1, 3)];
  return quad(p0[0], p0[1], p1[0], p1[1], q2[0], q2[1], q3[0], q3[1],
              x - i, 0.5 * (v - 1));
}

/** In-place `ffdPoint` over a flat `x,y,…` buffer. */
export function ffdInto(
  lat: ArrayLike<number>,
  cols: number,
  rows: number,
  xy: Float32Array | Float64Array,
): void {
  for (let k = 0; k < xy.length; k += 2) {
    const [x, y] = ffdPoint(lat, cols, rows, xy[k], xy[k + 1]);
    xy[k] = x;
    xy[k + 1] = y;
  }
}

// ------------------------------------------------------------------ rotation

/** Interpolated rotation form: one `rot` row after keyform blending. */
export interface RotForm {
  /** Degrees, counter-clockwise. */
  angle: number;
  originX: number;
  originY: number;
  scale: number;
  reflectX: boolean;
  reflectY: boolean;
}

/**
 * Rotation transform from its local pixel space straight to canvas pixels.
 *
 * Matches the shipped Cubism 2 runtime (`Live2DUnity` `transformPoints` for
 * `BDAffine`): `dst = O + totalScale · Rot(θ) · reflect · src`, where `O` is
 * the pivot already carried to canvas space and `totalScale` is the product
 * of every rotation scale on the chain. Ancestor warps pre-warp the pivot
 * only -- they never rescale or FFD the child points themselves.
 */
export interface RotXform extends RotForm {
  /** `totalScale` = product of rotation scales from the root through this node. */
  k: number;
  cos: number;
  sin: number;
}

/** Blend six `[angle, ox, oy, sx, rx, ry]` rows' worth of scalars. */
export function rotFormFromRow(row: ArrayLike<number>): RotForm {
  return {
    angle: row[0],
    originX: row[1],
    originY: row[2],
    scale: row[3],
    reflectX: row[4] >= 0.5,
    reflectY: row[5] >= 0.5,
  };
}

/**
 * Build a rotation's canvas-space transform. `form.angle` is the final
 * canvas-space angle in degrees (the deformer's own angle plus the parent
 * chain's local rotation at the pivot); `totalScale` is the product of
 * rotation scales along the chain. `rotPoint` then maps local pixel offsets
 * to canvas pixels with no further unit conversion.
 */
export function makeRotXform(form: RotForm, totalScale: number): RotXform {
  const angle = (form.angle * Math.PI) / 180;
  return {
    ...form,
    k: totalScale,
    cos: Math.cos(angle),
    sin: Math.sin(angle),
  };
}

/**
 * `p_canvas = O + k · Rot(θ) · q` with `q = reflect(p)`.
 * `Rot(θ)` is counter-clockwise.
 */
export function rotPoint(t: RotXform, x: number, y: number): [number, number] {
  const qx = t.reflectX ? -x : x;
  const qy = t.reflectY ? -y : y;
  return [t.originX + t.k * (qx * t.cos - qy * t.sin), t.originY + t.k * (qx * t.sin + qy * t.cos)];
}

/** In-place `rotPoint` over a flat `x,y,…` buffer. */
export function rotInto(t: RotXform, xy: Float32Array | Float64Array): void {
  const { cos, sin, k, originX: ox, originY: oy, reflectX, reflectY } = t;
  for (let i = 0; i < xy.length; i += 2) {
    const qx = reflectX ? -xy[i] : xy[i];
    const qy = reflectY ? -xy[i + 1] : xy[i + 1];
    xy[i] = ox + k * (qx * cos - qy * sin);
    xy[i + 1] = oy + k * (qx * sin + qy * cos);
  }
}

// ------------------------------------------------------------- model evaluation

/**
 * A deformer with its keyform-evaluated geometry/transform, flattened to
 * canvas pixels the way the shipped Cubism 2 runtime does it.
 *
 * Each deformer's `transformPoints` maps its *child* space straight to
 * canvas space (a warp: cage params `(u, v)` -> canvas via its lattice; a
 * rotation: pivot-relative pixel offsets -> canvas). The parent chain is
 * folded into the evaluation: a warp's lattice is carried to canvas before
 * it is used, and a rotation's pivot/angle/scale absorb the chain exactly as
 * `Live2DUnity` computes them. `composeInto` is therefore a single step.
 */
export interface DeformerEval {
  kind: "warp" | "rot";
  parent: number;
  /** Warp: lattice ALREADY IN CANVAS PIXELS, `2 * cols * rows` long. */
  lattice: Float64Array | null;
  cols: number;
  rows: number;
  /** Rot: child-space -> canvas transform. */
  rot: RotXform | null;
  /** Product of rotation scales from the root through this node (warps pass through). */
  totalScale: number;
}

/**
 * Clamp to `min`/`max` and honor `repeat` (wrap within `[min, max]`),
 * starting from each parameter's default.
 */
export function evalParamValues(model: ModelJson, params?: Record<string, number>): Float64Array {
  const values = new Float64Array(model.params.length);
  for (let i = 0; i < model.params.length; i++) {
    const p = model.params[i];
    let v = params && p.id in params ? params[p.id] : p.def;
    if (!Number.isFinite(v)) v = p.def;
    if (p.repeat && p.max > p.min) {
      v = p.min + (((v - p.min) % (p.max - p.min)) + (p.max - p.min)) % (p.max - p.min);
    }
    values[i] = Math.min(Math.max(v, p.min), p.max);
  }
  return values;
}

/** Apply one deformer's flattened child->canvas transform to flat `x,y,…`. */
export function applyDeformer(d: DeformerEval, xy: Float32Array | Float64Array): void {
  if (d.kind === "warp" && d.lattice) {
    ffdInto(d.lattice, d.cols, d.rows, xy);
  } else if (d.rot) {
    rotInto(d.rot, xy);
  }
}

/** One point through a deformer's flattened child->canvas transform. */
function deformerPoint(d: DeformerEval, x: number, y: number): [number, number] {
  if (d.kind === "warp" && d.lattice) return ffdPoint(d.lattice, d.cols, d.rows, x, y);
  if (d.rot) return rotPoint(d.rot, x, y);
  return [x, y];
}

/**
 * Evaluate every deformer's form at `values` (one pass, shared by all meshes).
 *
 * Parents are evaluated before children; each deformer lands in canvas
 * pixels. This mirrors `Live2DUnity`'s `setupTransform` pass (`BDAffine` /
 * `BDBoxGrid`): a rotation's pivot is pushed through the parent's
 * `transformPoints`, its angle picks up the parent chain's local rotation at
 * that pivot (measured with the runtime's finite-difference probe), and its
 * `totalScale` is the parent chain's `totalScale` times its own scale.
 */
export function evalDeformers(model: ModelJson, values: ParamValues): DeformerEval[] {
  const memo: (DeformerEval | undefined)[] = new Array(model.deformers.length);
  const evalOne = (idx: number, depth: number): DeformerEval => {
    const cached = memo[idx];
    if (cached) return cached;
    if (depth > 64) throw new Error(`deformer cycle at index ${idx}`);
    const d = model.deformers[idx];
    const parent: DeformerEval | null = d.parent >= 0 ? evalOne(d.parent, depth + 1) : null;

    if (d.kind === "warp") {
      const w = d as WarpDeformerJson;
      const local = lerpForms(w.forms, formWeights(d.grid, values));
      let lattice: Float64Array;
      if (parent) {
        lattice = new Float64Array(local.length);
        for (let i = 0; i < local.length; i += 2) {
          const [x, y] = deformerPoint(parent, local[i], local[i + 1]);
          lattice[i] = x;
          lattice[i + 1] = y;
        }
      } else {
        lattice = local;
      }
      const ev: DeformerEval = {
        kind: "warp",
        parent: d.parent,
        lattice,
        cols: w.cols,
        rows: w.rows,
        rot: null,
        totalScale: parent ? parent.totalScale : 1,
      };
      memo[idx] = ev;
      return ev;
    }

    const r = d as RotDeformerJson;
    const form = rotFormFromRow(lerpForms(r.rot, formWeights(d.grid, values)));
    if (!parent) {
      // Root: coordinates are already canvas pixels; the runtime scales the
      // local offsets by the rotation's own scale and rotates about O.
      const ev: DeformerEval = {
        kind: "rot",
        parent: d.parent,
        lattice: null,
        cols: 0,
        rows: 0,
        rot: makeRotXform(form, form.scale),
        totalScale: form.scale,
      };
      memo[idx] = ev;
      return ev;
    }
    // Pivot to canvas through the parent (a warp parent pre-warps it).
    const [ox, oy] = deformerPoint(parent, form.originX, form.originY);
    // Parent chain's local rotation at the pivot: finite-difference probe,
    // exactly as Live2DUnity's BDAffine.setupTransform measures it. The probe
    // direction is (0, -1): magnitude 10 under an affine (rot) parent, 0.1
    // under a box-grid (warp) parent -- the runtime's default draw method 2.1.
    const probe = parent.kind === "rot" ? -10 : -0.1;
    const [px, py] = deformerPoint(parent, form.originX, form.originY + probe);
    const parentAngle = Math.atan2(py - oy, px - ox) - Math.atan2(probe, 0);
    const totalScale = parent.totalScale * form.scale;
    const ev: DeformerEval = {
      kind: "rot",
      parent: d.parent,
      lattice: null,
      cols: 0,
      rows: 0,
      rot: makeRotXform(
        {
          ...form,
          angle: form.angle + (parentAngle * 180) / Math.PI,
          originX: ox,
          originY: oy,
        },
        totalScale,
      ),
      totalScale,
    };
    memo[idx] = ev;
    return ev;
  };
  return model.deformers.map((_, i) => evalOne(i, 0));
}

/**
 * Push a mesh's local-space points through its parent's transform to canvas
 * pixels. `local` is a flat `x,y,…` array; `out` receives the canvas points.
 * Each deformer's transform is already flattened to canvas, so this is one
 * step (identity for a root-parented mesh).
 */
export function composeInto(
  local: ArrayLike<number>,
  parent: number,
  defs: readonly DeformerEval[],
  out: Float32Array,
): void {
  for (let i = 0; i < out.length; i++) out[i] = local[i];
  if (parent < 0) return;
  applyDeformer(defs[parent], out);
}

/** Keyform-evaluate a single mesh and compose its vertices to canvas space. */
export function evalMeshFrame(
  mesh: MeshJson,
  values: ParamValues,
  defs: readonly DeformerEval[],
): MeshFrame {
  const wts = formWeights(mesh.grid, values);
  const local = lerpForms(mesh.forms, wts);
  const positions = new Float32Array(local.length);
  composeInto(local, mesh.parent, defs, positions);

  const pick = (rows: readonly number[]): number => {
    let acc = 0;
    for (const [idx, w] of wts) if (rows[idx] !== undefined) acc += rows[idx] * w;
    return acc;
  };
  const pickColor = (rows: readonly (readonly number[])[]): [number, number, number, number] => {
    const acc: [number, number, number, number] = [0, 0, 0, 0];
    for (const [idx, w] of wts) {
      const row = rows[idx];
      if (!row) continue;
      for (let c = 0; c < 4; c++) acc[c] += (row[c] ?? 0) * w;
    }
    return acc;
  };

  return {
    positions,
    opacity: pick(mesh.opacity),
    drawOrder: pick(mesh.drawOrder),
    multiply: pickColor(mesh.multiplyColor),
    screen: pickColor(mesh.screenColor),
    visible: true, // pose gates are applied by evalPose
  };
}

/** Everything the renderer needs for one parameter pose. */
export interface ModelPose {
  values: Float64Array;
  defs: DeformerEval[];
  /** One entry per `model.meshes`, in model order. */
  meshes: MeshFrame[];
}


/**
 * Pose gates: sibling groups of deformers that each bind one 0/1 toggle
 * (`PARAM_HAND_R_01_001` .. `PARAM_HAND_R_09_001` and friends). Exactly one
 * member of a group is on at a time, and only that member's subtree draws --
 * the alternatives are byte-identical copies of the same art in the source,
 * so drawing them all stacks their anti-aliased edges ("layers").
 *
 * A group is real only when its toggles share a name family (digits wildcarded)
 * and at least two siblings take part: a lone toggle like `PARAM_BREATH` is a
 * continuous driver, not a selector.
 *
 * Returns deformer index -> the toggle param indices gating it.
 */
export function detectPoseGates(model: ModelJson): Map<number, number[]> {
  const family = (id: string) => id.replace(/\d+/g, "#");
  const isToggle = (param: number, keys: number[]) =>
    keys.length > 0
    && keys.every((k) => k === 0 || k === 0.5 || k === 1)
    && model.params[param]?.min === 0
    && model.params[param]?.max === 1;

  // How many params share each name family? A family with several members is a
  // pose selector (`PARAM_HAND_R_01_001` .. `PARAM_HAND_R_09_001`); a family of
  // one (`PARAM_BREATH`, `PARAM_MOUTH_OPEN_Y`) is a continuous driver.
  const familySize = new Map<string, number>();
  for (const q of model.params) {
    const f = family(q.id);
    familySize.set(f, (familySize.get(f) ?? 0) + 1);
  }

  const gates = new Map<number, number[]>();
  for (let i = 0; i < model.deformers.length; i++) {
    const binds = model.deformers[i].grid?.binds ?? [];
    if (binds.length !== 1) continue;
    const b = binds[0];
    if (!isToggle(b.param, b.keys)) continue;
    if ((familySize.get(family(model.params[b.param].id)) ?? 0) < 2) continue;
    const g = gates.get(i);
    if (g) g.push(b.param);
    else gates.set(i, [b.param]);
  }
  return gates;
}

/** Evaluate every mesh and deformer of a model at `params`. */
export function evalPose(model: ModelJson, params?: Record<string, number>): ModelPose {
  const values = evalParamValues(model, params);
  const defs = evalDeformers(model, values);
  const gates = detectPoseGates(model);
  const meshes = model.meshes.map((m) => {
    const f = evalMeshFrame(m, values, defs);
    // walk the deformer chain: every pose gate must be on for the mesh to draw
    let visible = true;
    for (let d = m.parent; d >= 0 && visible; d = model.deformers[d].parent) {
      for (const g of gates.get(d) ?? []) {
        if (values[g] < 0.5) { visible = false; break; }
      }
    }
    f.visible = visible;
    // The runtime buckets draw order to whole numbers; a float sort lets two
    // meshes swap depth mid-animation when their interpolated orders cross.
    f.drawOrder = Math.round(f.drawOrder);
    return f;
  });
  return { values, defs, meshes };
}


/**
 * Canvas-space y of the eyes at the default pose -- the anchor the stands line
 * characters up on. The eye meshes are the ones bound to the eye-open params;
 * their composed bbox centre is the eye line. Falls back to `null` when a model
 * has no eye params (the caller then uses the old head-top anchor).
 */
export function eyeLine(model: ModelJson): number | null {
  const eyeParams = new Set<number>();
  model.params.forEach((q, i) => {
    if (q.id === "PARAM_EYE_L_OPEN" || q.id === "PARAM_EYE_R_OPEN") eyeParams.add(i);
  });
  if (eyeParams.size === 0) return null;

  // A mesh is an eye when it or any deformer on its parent chain binds an
  // eye-open param (models differ in whether the bind sits on the mesh or on
  // the deformer above it).
  const eyeDeformers = new Set<number>();
  model.deformers.forEach((d, i) => {
    if ((d.grid?.binds ?? []).some((b) => eyeParams.has(b.param))) eyeDeformers.add(i);
  });
  const isEye = (parent: number): boolean => {
    for (let d = parent, n = 0; d >= 0 && n < 12; d = model.deformers[d].parent, n++) {
      if (eyeDeformers.has(d)) return true;
    }
    return false;
  };

  const values = evalParamValues(model);
  const defs = evalDeformers(model, values);
  let lo = Infinity, hi = -Infinity;
  for (const m of model.meshes) {
    const direct = (m.grid?.binds ?? []).some((b) => eyeParams.has(b.param));
    if (!direct && !isEye(m.parent)) continue;
    const f = evalMeshFrame(m, values, defs);
    for (let k = 1; k < f.positions.length; k += 2) {
      lo = Math.min(lo, f.positions[k]);
      hi = Math.max(hi, f.positions[k]);
    }
  }
  if (lo <= hi) return (lo + hi) / 2;

  // No eye binds (simple models like 015): the eye line sits at ~15% of the
  // figure height across the cast (measured 14.5-15.0% on the full set), so
  // fall back to that fraction of the visible figure's bbox.
  let top = Infinity, bot = -Infinity;
  const pose = evalPose(model);
  for (const f of pose.meshes) {
    if (!f.visible) continue;
    for (let k = 1; k < f.positions.length; k += 2) {
      top = Math.min(top, f.positions[k]);
      bot = Math.max(bot, f.positions[k]);
    }
  }
  return top <= bot ? top + 0.15 * (bot - top) : null;
}

/** Canvas-space bounding box of a composed mesh (2 * nverts flat). */
export function boundsOf(positions: ArrayLike<number>): [number, number, number, number] {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (let i = 0; i < positions.length; i += 2) {
    const x = positions[i];
    const y = positions[i + 1];
    if (x < minX) minX = x;
    if (x > maxX) maxX = x;
    if (y < minY) minY = y;
    if (y > maxY) maxY = y;
  }
  return [minX, minY, maxX, maxY];
}