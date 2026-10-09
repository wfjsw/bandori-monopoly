// Run with: node --test webui/src/scenes/board/viewport.test.ts
// The pan/zoom math is pure: everything here checks that the board window
// keeps covering the wrap and that zooming is anchored on the cursor point.
// The board rect is the map slot's rectangle (any aspect), so the checks use a
// deliberately wide one -- the math is per-axis.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  MAX_ZOOM, MIN_ZOOM,
  clampPan, clampZoom, fitViewport, panBy, pinchAround, stepZoom, wheelZoom, zoomAround,
  type Viewport,
} from "./viewport.ts";

/** A wide board rect (the 12 x 10 grid stretched past square). */
const W = 1284;
const H = 720;

/** Board point under a window point, for anchor checks. */
const under = (v: Viewport, px: number, py: number) => ({
  x: (px - v.tx) / v.z,
  y: (py - v.ty) / v.z,
});

test("clampZoom holds zoom between fit and 3x", () => {
  assert.equal(clampZoom(0.2), MIN_ZOOM);
  assert.equal(clampZoom(1), 1);
  assert.equal(clampZoom(2.5), 2.5);
  assert.equal(clampZoom(10), MAX_ZOOM);
  assert.equal(clampZoom(NaN), MIN_ZOOM);
  assert.deepEqual(fitViewport(), { z: 1, tx: 0, ty: 0 });
});

test("clampPan pins the fit view and never leaves a gap", () => {
  // At fit the board exactly covers the window: no pan at all.
  assert.deepEqual(clampPan({ z: 1, tx: 40, ty: -40 }, W, H), { z: 1, tx: 0, ty: 0 });
  // At 2x the board is 2W x 2H wide, so it may slide at most W / H either way.
  assert.deepEqual(clampPan({ z: 2, tx: 500, ty: -5000 }, W, H), { z: 2, tx: 0, ty: -H });
  assert.deepEqual(clampPan({ z: 2, tx: -5000, ty: 500 }, W, H), { z: 2, tx: -W, ty: 0 });
  // The board still covers the window at every extreme.
  for (const z of [1, 1.3, 2, 3]) {
    const v = clampPan({ z, tx: 1e6, ty: -1e6 }, W, H);
    assert.ok(v.tx <= 0 && v.tx >= W * (1 - z), `tx ${v.tx} covers at ${z}`);
    assert.ok(v.ty <= 0 && v.ty >= H * (1 - z), `ty ${v.ty} covers at ${z}`);
  }
});

test("zoomAround keeps the point under the cursor fixed", () => {
  const v: Viewport = { z: 1.5, tx: -100, ty: -200 };
  const px = 640;
  const py = 30;
  const before = under(v, px, py);
  const z = zoomAround(v, px, py, 2.25, W, H);
  assert.ok(Math.abs(under(z, px, py).x - before.x) < 1e-9);
  assert.ok(Math.abs(under(z, px, py).y - before.y) < 1e-9);
  // Zooming out around the same point is the same invariant.
  const back = zoomAround(z, px, py, 1.5, W, H);
  assert.ok(Math.abs(under(back, px, py).x - before.x) < 1e-9);
  assert.ok(Math.abs(back.tx - v.tx) < 1e-9 && Math.abs(back.ty - v.ty) < 1e-9);
});

test("zoomAround clamps pan while still hugging the cursor", () => {
  // Zooming into the top-left corner at fit: the corner is already the limit.
  const z = zoomAround(fitViewport(), 0, 0, 3, W, H);
  assert.deepEqual(z, { z: 3, tx: 0, ty: 0 });
  // From the opposite extreme, zooming out to fit recentres (clamp to zero).
  const v: Viewport = { z: 3, tx: W * (1 - 3), ty: H * (1 - 3) };
  assert.deepEqual(zoomAround(v, W / 2, H / 2, 1, W, H), { z: 1, tx: 0, ty: 0 });
  // Any anchor keeps coverage.
  for (const px of [0, W / 2, W]) {
    for (const py of [0, H / 2, H]) {
      const t = zoomAround(v, px, py, 1.7, W, H);
      assert.ok(t.tx <= 0 && t.tx >= W * (1 - t.z));
      assert.ok(t.ty <= 0 && t.ty >= H * (1 - t.z));
    }
  }
});

test("wheelZoom steps continuously and caps at fit / 3x", () => {
  let v = fitViewport();
  for (let i = 0; i < 40; i++) v = wheelZoom(v, 400, 300, -120, W, H); // wheel up
  assert.equal(v.z, MAX_ZOOM);
  for (let i = 0; i < 80; i++) v = wheelZoom(v, 400, 300, 120, W, H); // wheel down
  assert.equal(v.z, MIN_ZOOM);
  assert.deepEqual(v, fitViewport());
});

test("stepZoom moves in fixed increments around its anchor", () => {
  const v = stepZoom(fitViewport(), 1, W / 2, H / 2, W, H);
  assert.ok(Math.abs(v.z - 1.25) < 1e-9);
  const two = stepZoom(v, 1, W / 2, H / 2, W, H);
  assert.ok(Math.abs(two.z - 1.25 * 1.25) < 1e-9);
  // Stepping around the centre keeps the board's centre under it.
  assert.ok(Math.abs(under(two, W / 2, H / 2).x - W / 2) < 1e-9);
  assert.ok(Math.abs(under(two, W / 2, H / 2).y - H / 2) < 1e-9);
  let max = v;
  for (let i = 0; i < 6; i++) max = stepZoom(max, 1, 0, 0, W, H);
  assert.equal(max.z, MAX_ZOOM);
});

test("pinchAround scales on the moving midpoint", () => {
  const v0: Viewport = { z: 1.25, tx: -60, ty: -40 };
  const mid0 = { x: 300, y: 200 };
  // Same midpoint, double the span: like zoomAround by 2 on that point.
  const grown = pinchAround(v0, mid0, mid0, 2, W, H);
  assert.ok(Math.abs(under(grown, mid0.x, mid0.y).x - under(v0, mid0.x, mid0.y).x) < 1e-9);
  assert.ok(Math.abs(under(grown, mid0.x, mid0.y).y - under(v0, mid0.x, mid0.y).y) < 1e-9);
  // Midpoint slides 80px right: the same board point follows it.
  const mid = { x: 380, y: 200 };
  const slid = pinchAround(v0, mid0, mid, 2, W, H);
  assert.ok(Math.abs(under(slid, mid.x, mid.y).x - under(v0, mid0.x, mid0.y).x) < 1e-9);
  assert.ok(Math.abs(under(slid, mid.x, mid.y).y - under(v0, mid0.x, mid0.y).y) < 1e-9);
  // Pinch-out past the cap still covers the window.
  const huge = pinchAround(v0, mid0, { x: 0, y: 0 }, 10, W, H);
  assert.equal(huge.z, MAX_ZOOM);
  assert.ok(huge.tx <= 0 && huge.tx >= W * (1 - huge.z));
});

test("panBy moves the board and clamps at the window edge", () => {
  const v = panBy(fitViewport(), 30, -30, W, H);
  assert.deepEqual(v, fitViewport()); // nowhere to go at fit
  const zoomed = panBy({ z: 2, tx: -400, ty: -400 }, -100, 20, W, H);
  assert.deepEqual(zoomed, { z: 2, tx: -500, ty: -380 });
  const edge = panBy({ z: 2, tx: -500, ty: -500 }, -1e4, 1e4, W, H);
  assert.deepEqual(edge, { z: 2, tx: -W, ty: 0 });
});

test("any board rect keeps covering its window under clamp and zoom", () => {
  // The map fills its slot, so the rect may be any aspect; the math is
  // per-axis and must hold for each one.
  for (const [bw, bh] of [[856, 856 * 10 / 12], [W, H], [600, 1100]] as const) {
    const v = clampPan({ z: 2, tx: 0, ty: 0 }, bw, bh);
    assert.deepEqual(v, { z: 2, tx: 0, ty: 0 });
    const slide = clampPan({ z: 2, tx: -1e4, ty: -1e4 }, bw, bh);
    assert.deepEqual(slide, { z: 2, tx: -bw, ty: -bh });
    const z = zoomAround({ z: 1, tx: 0, ty: 0 }, bw * 0.9, bh * 0.07, 2, bw, bh);
    assert.ok(Math.abs(under(z, bw * 0.9, bh * 0.07).x - bw * 0.9) < 1e-9);
    assert.ok(Math.abs(under(z, bw * 0.9, bh * 0.07).y - bh * 0.07) < 1e-9);
    assert.ok(z.tx <= 0 && z.tx >= bw * (1 - z.z));
    assert.ok(z.ty <= 0 && z.ty >= bh * (1 - z.z));
  }
});