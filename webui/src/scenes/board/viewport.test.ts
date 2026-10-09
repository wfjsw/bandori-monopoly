// Run with: node --test webui/src/scenes/board/viewport.test.ts
// The pan/zoom math is pure: everything here checks that the board window
// stays within the overscroll limits and that zooming is anchored on the cursor point.
// The board rect is the map slot's rectangle (any aspect), so the checks use a
// deliberately wide one -- the math is per-axis.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  MAX_RATIO, MAX_ZOOM, MIN_RATIO, MIN_ZOOM, OVERPAN, boardRect,
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

test("clampPan allows half a window of overscroll past each edge", () => {
  // At fit the board may slide half a window either way, no further.
  assert.deepEqual(clampPan({ z: 1, tx: 40, ty: -40 }, W, H), { z: 1, tx: 40, ty: -40 });
  assert.deepEqual(clampPan({ z: 1, tx: 1e4, ty: -1e4 }, W, H), { z: 1, tx: W * OVERPAN, ty: -H * OVERPAN });
  // At 2x the board is 2W x 2H: it slides W / H plus the overscroll.
  assert.deepEqual(clampPan({ z: 2, tx: 1e4, ty: -1e4 }, W, H), { z: 2, tx: W * OVERPAN, ty: -H - H * OVERPAN });
  // Never more than the overscroll past either edge, at every zoom.
  for (const z of [1, 1.3, 2, 3]) {
    for (const t of [1e6, -1e6]) {
      const v = clampPan({ z, tx: t, ty: t }, W, H);
      assert.ok(v.tx <= W * OVERPAN + 1e-9 && v.tx >= W * (1 - z) - W * OVERPAN - 1e-9, `tx ${v.tx} at ${z}`);
      assert.ok(v.ty <= H * OVERPAN + 1e-9 && v.ty >= H * (1 - z) - H * OVERPAN - 1e-9, `ty ${v.ty} at ${z}`);
    }
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
  // Zooming into the top-left corner at fit keeps the corner under the cursor.
  const z = zoomAround(fitViewport(), 0, 0, 3, W, H);
  assert.deepEqual(z, { z: 3, tx: 0, ty: 0 });
  // Zooming all the way out lands on the fit view, overscroll dropped.
  const v: Viewport = { z: 3, tx: W * (1 - 3), ty: H * (1 - 3) };
  assert.deepEqual(zoomAround(v, W / 2, H / 2, 1, W, H), fitViewport());
  // Any anchor stays within the overscroll limits.
  for (const px of [0, W / 2, W]) {
    for (const py of [0, H / 2, H]) {
      const t = zoomAround(v, px, py, 1.7, W, H);
      assert.ok(t.tx <= W * OVERPAN && t.tx >= W * (1 - t.z) - W * OVERPAN);
      assert.ok(t.ty <= H * OVERPAN && t.ty >= H * (1 - t.z) - H * OVERPAN);
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
  assert.ok(huge.tx <= W * OVERPAN && huge.tx >= W * (1 - huge.z) - W * OVERPAN);
});

test("panBy moves the board and clamps at the window edge", () => {
  // Even at fit the board drags, up to half a window.
  assert.deepEqual(panBy(fitViewport(), 30, -30, W, H), { z: 1, tx: 30, ty: -30 });
  assert.deepEqual(panBy(fitViewport(), 1e4, -1e4, W, H), { z: 1, tx: W * OVERPAN, ty: -H * OVERPAN });
  const zoomed = panBy({ z: 2, tx: -400, ty: -400 }, -100, 20, W, H);
  assert.deepEqual(zoomed, { z: 2, tx: -500, ty: -380 });
  const edge = panBy({ z: 2, tx: -500, ty: -500 }, -1e4, 1e4, W, H);
  assert.deepEqual(edge, { z: 2, tx: -W - W * OVERPAN, ty: H * OVERPAN });
});

test("any board rect stays within the overscroll limits under clamp and zoom", () => {
  // The map fills its slot, so the rect may be any aspect; the math is
  // per-axis and must hold for each one.
  for (const [bw, bh] of [[856, 856 * 10 / 12], [W, H], [600, 1100]] as const) {
    const v = clampPan({ z: 2, tx: 0, ty: 0 }, bw, bh);
    assert.deepEqual(v, { z: 2, tx: 0, ty: 0 });
    const slide = clampPan({ z: 2, tx: -1e4, ty: -1e4 }, bw, bh);
    assert.deepEqual(slide, { z: 2, tx: -bw - bw * OVERPAN, ty: -bh - bh * OVERPAN });
    const z = zoomAround({ z: 1, tx: 0, ty: 0 }, bw * 0.9, bh * 0.07, 2, bw, bh);
    assert.ok(Math.abs(under(z, bw * 0.9, bh * 0.07).x - bw * 0.9) < 1e-9);
    assert.ok(Math.abs(under(z, bw * 0.9, bh * 0.07).y - bh * 0.07) < 1e-9);
    assert.ok(z.tx <= bw * OVERPAN && z.tx >= bw * (1 - z.z) - bw * OVERPAN);
    assert.ok(z.ty <= bh * OVERPAN && z.ty >= bh * (1 - z.z) - bh * OVERPAN);
  }
});
test("boardRect keeps the board between square and 16:10", () => {
  // In range: the whole slot.
  assert.deepEqual(boardRect(1200, 900), { width: 1200, height: 900 });
  // Too wide (ultrawide): height-bound, width capped at 1.6x.
  assert.deepEqual(boardRect(2000, 800), { width: 1280, height: 800 });
  // Too tall (narrow window): never narrower than square.
  assert.deepEqual(boardRect(700, 900), { width: 700, height: 700 });
  for (const [w, h] of [[3000, 600], [600, 3000], [1000, 1000], [1600, 1000]]) {
    const r = boardRect(w, h);
    assert.ok(r.width / r.height >= MIN_RATIO - 1e-3 && r.width / r.height <= MAX_RATIO + 1e-3);
    assert.ok(r.width <= w && r.height <= h);
  }
});
