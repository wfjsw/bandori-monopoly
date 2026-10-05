// Standalone validation of the deform math in ./deform.ts (run with `node`).
//
//   node src/live2d/check.mjs
//
// Covers the synthetic cases from the renderer brief (identity FFD, sheared
// cage, 90-degree rotation, keyform lerp at t = 0.5) plus a real-model pass
// that walks every mesh of model 001 up its deformer chain and asserts the
// result lands in the canvas box.

import { readFileSync } from "node:fs";
import {
  boundsOf,
  composeInto,
  evalDeformers,
  evalMeshFrame,
  evalParamValues,
  evalPose,
  ffdInto,
  ffdPoint,
  formWeights,
  lerpForms,
  makeRotXform,
  rotFormFromRow,
  rotInto,
  rotPoint,
} from "./deform.ts";

let failures = 0;
let checks = 0;

function ok(name, cond, detail = "") {
  checks += 1;
  if (cond) {
    
console.log(`  ok   ${name}`);
  } else {
    failures += 1;
    console.log(`  FAIL ${name}${detail ? ` -- ${detail}` : ""}`);
  }
}

function close(name, a, b, eps = 1e-6) {
  const bad = [];
  for (let i = 0; i < a.length; i++) {
    if (!Number.isFinite(a[i]) || Math.abs(a[i] - b[i]) > eps) {
      bad.push(`[${i}] ${a[i]} != ${b[i]}`);
    }
  }
  ok(name, bad.length === 0, bad.slice(0, 3).join(", "));
}

function identityLattice(cols, rows) {
  const lat = new Float64Array(cols * rows * 2);
  for (let j = 0; j < rows; j++) {
    for (let i = 0; i < cols; i++) {
      lat[(j * cols + i) * 2] = i / (cols - 1);
      lat[(j * cols + i) * 2 + 1] = j / (rows - 1);
    }
  }
  return lat;
}

// ---------------------------------------------------------------- synthetic

console.log("synthetic");

{
  // 1. Identity FFD reproduces its input cage parameters.
  const cols = 4;
  const rows = 3;
  const lat = identityLattice(cols, rows);
  const inputs = [
    [0, 0],
    [1, 1],
    [0.25, 0.5],
    [0.9, 0.1],
  ];
  let all = true;
  for (const [u, v] of inputs) {
    const [x, y] = ffdPoint(lat, cols, rows, u, v);
    if (Math.abs(x - u) > 1e-6 || Math.abs(y - v) > 1e-6) all = false;
  }
  ok("identity FFD reproduces the input", all);

  // 1b. Extrapolation: just outside [0,1] continues the edge cell.
  const [ex, ey] = ffdPoint(lat, cols, rows, -0.1, 1.1);
  close("identity FFD extrapolates outside [0,1]", [ex, ey], [-0.1, 1.1], 1e-6);
}

{
  // 2. A sheared cage (x' = u + 0.5 v) moves interior points as expected.
  const cols = 2;
  const rows = 2;
  const lat = new Float64Array([
    0, 0, // (u,v) = (0,0) -> (0, 0)
    1, 0, // (1,0) -> (1, 0)
    0.5, 1, // (0,1) -> (0.5, 1)
    1.5, 1, // (1,1) -> (1.5, 1)
  ]);
  const [x, y] = ffdPoint(lat, cols, rows, 0.5, 0.5);
  close("sheared cage maps the interior point", [x, y], [0.75, 0.5], 1e-6);
}

{
  // 3. Rotation by 90 degrees maps (1,0) -> (0,1).
  const t = makeRotXform(rotFormFromRow([90, 0, 0, 1, 0, 0]), 1);
  const [x, y] = rotPoint(t, 1, 0);
  close("90 degree rotation maps (1,0) -> (0,1)", [x, y], [0, 1], 1e-6);
  const [rx, ry] = rotPoint(makeRotXform(rotFormFromRow([0, 0, 0, 1, 1, 1]), 1), 2, -3);
  close("reflect flags flip both axes", [rx, ry], [-2, 3], 1e-6);
  // Live2DUnity rule: the scale factor is the chain's totalScale (product of
  // rotation scales), applied px->px. It is NOT divided by ppu -- the shipped
  // runtime never converts rotation offsets into warp cage parameters.
  const [wx, wy] = rotPoint(makeRotXform(rotFormFromRow([0, 0, 0, 0.25, 0, 0]), 0.25), 1000, 0);
  close("rotation scale is totalScale px->px (no /ppu)", [wx, wy], [250, 0], 1e-9);
}

{
  // 4. Keyform lerp at t = 0.5 lands midway.
  const grid = {
    binds: [{ param: 0, keys: [0, 10] }],
    at: [
      { at: [[0, 0]], form: 0 },
      { at: [[0, 1]], form: 1 },
    ],
  };
  const rows = [[0, 100], [10, 200]];
  const w = formWeights(grid, [5]);
  const mid = lerpForms(rows, w);
  close("keyform lerp at t=0.5 lands midway", Array.from(mid), [5, 150], 1e-6);

  // Clamped outside the key range and a single-key axis.
  const low = lerpForms(rows, formWeights(grid, [-5]));
  close("keyform lerp clamps below the key range", Array.from(low), [0, 100], 1e-6);
  const single = lerpForms(rows, formWeights({ binds: [{ param: 0, keys: [3] }], at: [{ at: [[0, 0]], form: 1 }] }, [7]));
  close("single-key axis uses its only form", Array.from(single), [10, 200], 1e-6);

  // Multilinear corner: two axes, missing corner falls back to the nearest.
  const grid2 = {
    binds: [
      { param: 0, keys: [0, 1] },
      { param: 1, keys: [0, 1] },
    ],
    at: [
      { at: [[0, 0], [1, 0]], form: 0 },
      { at: [[0, 1], [1, 0]], form: 1 },
      { at: [[0, 0], [1, 1]], form: 2 },
      // corner (1,1) missing
    ],
  };
  const rows2 = [[0], [10], [100]];
  // Midpoint: four 0.25 corners; the missing (1,1) corner falls back to the
  // first present corner sharing a pair (form 1), so form 1 carries 0.5.
  const v2 = lerpForms(rows2, formWeights(grid2, [0.5, 0.5]));
  close("missing corner falls back to the nearest present form", Array.from(v2), [30], 1e-6);

  // ffdInto / rotInto mutate flat buffers like the per-point helpers.
  const lat = identityLattice(3, 3);
  const buf = new Float32Array([0.25, 0.75]);
  ffdInto(lat, 3, 3, buf);
  close("ffdInto matches ffdPoint", Array.from(buf), [0.25, 0.75], 1e-6);
  const rbuf = new Float32Array([1, 0]);
  rotInto(makeRotXform(rotFormFromRow([90, 0, 0, 1, 0, 0]), 1), rbuf);
  close("rotInto matches rotPoint", Array.from(rbuf), [0, 1], 1e-6);
}

{
  // 5. A mesh nested rot -> warp composes end to end (synthetic chain).
  // Mirrors Live2DUnity's flattened composition: the warp only pre-warps the
  // rotation's pivot; the mesh's px offsets are scaled by totalScale and
  // rotated about the canvas-space pivot.
  const values = [0];
  const grid = { binds: [], at: [{ at: [], form: 0 }] };
  const warp = {
    name: "w",
    kind: "warp",
    parent: -1,
    visible: true,
    cols: 2,
    rows: 2,
    forms: [Array.from(identityLattice(2, 2))],
    grid,
  };
  const rot = {
    name: "r",
    kind: "rot",
    parent: 0,
    visible: true,
    // pivot (0.5, 0.5) in the warp's cage params; scale 1; angle 90.
    rot: [[90, 0.5, 0.5, 1, 0, 0]],
    grid,
  };
  const defs = evalDeformers({ deformers: [warp, rot], ppu: 2000, params: [], meshes: [], canvas: [2000, 2500], id: "x", textures: [] }, values);
  const out = new Float32Array(2);
  // Identity warp carries the pivot (0.5, 0.5) to canvas (0.5, 0.5); the
  // identity cage has no local rotation, so the 90 deg stands. Child offset
  // (1, 0) -> rotated 90 deg -> (0, 1), then + pivot.
  composeInto([1, 0], 1, defs, out);
  close("synthetic chain: rot under warp composes to canvas", Array.from(out), [0.5, 1.5], 1e-3);
  // The warp maps cage (u,v) through the identity lattice into canvas.
  composeInto([0.5, 0.5], 0, defs, out);
  close("synthetic chain: identity warp passes (0.5,0.5) through", Array.from(out), [0.5, 0.5], 1e-6);
  // Nested rotation: the chain's totalScale multiplies, the angles add.
  const rot2 = {
    name: "r2",
    kind: "rot",
    parent: 1,
    visible: true,
    rot: [[30, 0, 0, 2, 0, 0]],
    grid,
  };
  const defs2 = evalDeformers({ deformers: [warp, rot, rot2], ppu: 2000, params: [], meshes: [], canvas: [2000, 2500], id: "x", textures: [] }, values);
  // rot2 is a child of the 90 deg rot: canvas angle 90 + 30 = 120, totalScale
  // 1 * 2 = 2. Offset (1, 0) -> 2 * (cos120, sin120) = (-1, sqrt(3)), plus the
  // parent's pivot (0.5, 0.5).
  composeInto([1, 0], 2, defs2, out);
  const [nx, ny] = out;
  ok(
    "nested rotations add angles and multiply totalScale",
    Math.abs(nx - (0.5 - 1)) < 1e-3 && Math.abs(ny - (0.5 + Math.sqrt(3))) < 1e-3,
    `got (${nx}, ${ny})`,
  );
}

// --------------------------------------------------------------- real model

console.log("real model 001");

{
  const url = new URL("../../public/assets/live2d/001/model.json", import.meta.url);
  const model = JSON.parse(readFileSync(url, "utf8"));
  const { meshes, deformers, params, canvas, ppu } = model;
  const [w, h] = canvas;

  const defaults = evalParamValues(model, undefined);
  ok("default parameter values match the model", defaults.length === params.length && defaults.every((v, i) => v === params[i].def));

  const pose = evalPose(model, undefined);
  ok("pose covers every mesh", pose.meshes.length === meshes.length);

  // Every mesh composes into the canvas box: the strict box plus a margin for
  // geometry that legitimately overflows (hands / sleeves at the edges), and
  // every mesh must at least intersect the strict box.
  const strict = [0, 0, w, h];
  const fitBox = [-0.25 * w, -0.25 * h, 1.25 * w, 1.5 * h];
  let insideStrict = 0;
  let outsideFit = 0;
  let disjoint = 0;
  for (let i = 0; i < meshes.length; i++) {
    const [minX, minY, maxX, maxY] = boundsOf(pose.meshes[i].positions);
    if (minX >= strict[0] && minY >= strict[1] && maxX <= strict[2] && maxY <= strict[3]) insideStrict += 1;
    if (minX < fitBox[0] || minY < fitBox[1] || maxX > fitBox[2] || maxY > fitBox[3]) outsideFit += 1;
    if (maxX < strict[0] || minX > strict[2] || maxY < strict[1] || minY > strict[3]) disjoint += 1;
  }
  ok("every mesh composes into the canvas fit box", outsideFit === 0, `${outsideFit} meshes outside`);
  ok("every mesh intersects the strict canvas box", disjoint === 0, `${disjoint} disjoint`);
  ok("most meshes fit the strict canvas box", insideStrict >= Math.floor(meshes.length * 0.85), `${insideStrict}/${meshes.length}`);

  const all = new Float32Array(meshes.reduce((n, m, i) => n + pose.meshes[i].positions.length, 0));
  let off = 0;
  for (const f of pose.meshes) {
    all.set(f.positions, off);
    off += f.positions.length;
  }
  const [minX, minY, maxX, maxY] = boundsOf(all);
  console.log(`  info composed bounds x[${minX.toFixed(1)}, ${maxX.toFixed(1)}] y[${minY.toFixed(1)}, ${maxY.toFixed(1)}] of canvas ${w}x${h}`);
  ok("composed bounds are near the canvas", minX > -w && maxX < 2 * w && minY > -h && maxY < 2 * h);

  // Face sanity: the topmost mesh should sit in the upper half of the canvas
  // (head area), not at the bottom.
  let top = pose.meshes[0];
  let topY = Infinity;
  for (const f of pose.meshes) {
    const [, minY2] = boundsOf(f.positions);
    if (minY2 < topY) {
      topY = minY2;
      top = f;
    }
  }
  ok("topmost mesh sits in the upper canvas half", topY > 0 && topY < h * 0.5, `y=${topY.toFixed(1)}`);
  const tb = boundsOf(top.positions);
  const size = Math.max(tb[2] - tb[0], tb[3] - tb[1]);
  ok("topmost mesh has a sane size", size > 1 && size < w, `size=${size.toFixed(1)}`);

  // Parameter changes actually move things (deform is live, not baked).
  const pose2 = evalPose(model, { PARAM_ANGLE_X: 30 });
  let moved = 0;
  for (let i = 0; i < meshes.length; i++) {
    const a = pose.meshes[i].positions;
    const b = pose2.meshes[i].positions;
    for (let k = 0; k < a.length; k += 2) {
      if (Math.abs(a[k] - b[k]) > 0.5 || Math.abs(a[k + 1] - b[k + 1]) > 0.5) {
        moved += 1;
        break;
      }
    }
  }
  ok("parameter change moves the model", moved > 0, `moved ${moved} meshes`);

  // Chain integrity: composing a mesh through the shared defs agrees with the
  // per-mesh evaluation (composeInto is what the renderer uses).
  const defs = evalDeformers(model, defaults);
  const m = meshes[0];
  const frame = evalMeshFrame(m, defaults, defs);
  const again = new Float32Array(frame.positions.length);
  const wts = formWeights(m.grid, defaults);
  const local = lerpForms(m.forms, wts);
  composeInto(local, m.parent, defs, again);
  close("evalMeshFrame matches composeInto", Array.from(again), Array.from(frame.positions), 1e-4);
}

// BDBoxGrid ground truth: these 22 vectors were produced by invoking the shipped
// Cubism 2 runtime itself (Live2DUnity.dll, static grid transform) on a 3x3
// sheared lattice, via reflection. They cover the in-cell triangles, all 8
// skirt regions of (-2,3)^2 and the linear far field. Do not regenerate them
// from this implementation -- they are the reference.
{
  const lat = [0, 0, 2, 0.5, 4, 0, 0, 3, 2, 3.2, 4, 3, 0, 6, 2, 5.5, 4, 6];
  const cases = [
    [0.25, 0.25, 1, 1.75], [0.75, 0.6, 3, 3.56], [0.5, 0.5, 2, 3.2],
    [-0.5, 0.5, -2, 3], [1.5, 0.5, 6, 3], [0.5, -0.5, 2, -2.625],
    [0.5, 1.5, 2, 8.625], [-0.5, -0.5, -2, -3], [-0.5, 1.5, -2, 9],
    [1.5, -0.5, 6, -3], [1.5, 1.5, 6, 9], [-1.5, 2.5, -6, 15],
    [2.5, -1.5, 10, -9], [2.9, 2.9, 11.6, 17.400002], [-3, 0.5, -12, 3],
    [4, 0.5, 16, 3], [0.5, -3, 2, -18], [10, 10, 40, 60],
    [0, 0, 0, 0], [1, 1, 4, 6], [1, 0, 4, 0], [0, 1, 0, 6],
  ];
  let worst = 0;
  for (const [u, v, ex, ey] of cases) {
    const [x, y] = ffdPoint(lat, 3, 3, u, v);
    worst = Math.max(worst, Math.abs(x - ex), Math.abs(y - ey));
  }
  ok("BDBoxGrid matches Live2DUnity ground truth (22 vectors)", worst < 1e-4, `max err ${worst}`);
  // continuity across the in-cell / skirt seam
  const seam = Math.abs(ffdPoint(lat, 3, 3, -1e-6, 0.5)[1] - ffdPoint(lat, 3, 3, 1e-6, 0.5)[1]);
  ok("BDBoxGrid continuous across u=0 seam", seam < 1e-3, `delta ${seam}`);
}


// Pose gates: sibling groups of one-toggle deformers select exactly one art
// set (the hand poses). Only the selected member's subtree may draw.
{
  const model = JSON.parse(readFileSync('public/assets/live2d/036/model.json', 'utf8'));
  const { evalPose, detectPoseGates } = await import('./deform.ts');
  const gates = detectPoseGates(model);
  ok('pose gates detected (hand selectors)', gates.size >= 14, 'gated deformers: ' + gates.size);
  const pose = evalPose(model, { PARAM_HAND_R_05_001: 1, PARAM_HAND_L_01_001: 1 });
  const hands = model.meshes.map((m, i) => ({ m, f: pose.meshes[i] }))
    .filter((x) => x.f.visible && x.m.name.startsWith('D_PSD1_9'));
  const named = hands.map((x) => x.m.name);
  ok('only the selected right-hand pose draws', named.includes('D_PSD1_95') && !named.includes('D_PSD1_88'),
    named.join(','));
  const zeros = Object.fromEntries(model.params.filter((q) => /HAND_/.test(q.id)).map((q) => [q.id, 0]));
  const off = evalPose(model, zeros);
  const anyHand = model.meshes.some((m, i) => off.meshes[i].visible && /^D_PSD1_(88|9[2-7])$/.test(m.name));
  ok('no hand pose draws when every toggle is off', !anyHand);
}


console.log(`check: ${checks - failures}/${checks} passed`);
process.exit(failures ? 1 : 0);