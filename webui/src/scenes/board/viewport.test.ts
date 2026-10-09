// Run with: node --test webui/src/scenes/board/viewport.test.ts
// The pan/zoom math is pure: everything here checks that the board stays
// within the overscroll limits of its window and that zooming is anchored on
// the cursor point. The window is the whole map slot; the board inside it
// keeps the slot held to 1..1.6 (`boardRect` / `mapBox`) and is centred at
// fit -- so on a wide slot the board is narrower than the window and the
// checks use both rectangles.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  FIT_ZOOM, MAX_RATIO, MAX_ZOOM, MIN_RATIO, MIN_ZOOM, OVERPAN, boardRect, mapBox,
  centeredViewport, clampPan, clampZoom, fitViewport, panBy, pinchAround, stepZoom, wheelZoom, zoomAround,
  type MapBox, type Viewport,
} from "./viewport.ts";

/** A wide slot: the window fills it, the board keeps 16:10 inside. */
const WIDE: MapBox = { winW: 2560, winH: 1080, boardW: 1728, boardH: 1080 };
/** A slot already in range: window == board. */
const SQUARE: MapBox = { winW: 1200, winH: 900, boardW: 1200, boardH: 900 };
/** A tall slot: the board is square inside it. */
const TALL: MapBox = { winW: 700, winH: 1100, boardW: 700, boardH: 700 };

/** Board point under a window point, for anchor checks. */
const under = (v: Viewport, px: number, py: number) => ({
  x: (px - v.tx) / v.z,
  y: (py - v.ty) / v.z,
});

test("clampZoom holds zoom between the floor and 3x", () => {
  assert.equal(clampZoom(0.2), MIN_ZOOM);
  assert.equal(clampZoom(MIN_ZOOM), MIN_ZOOM);
  assert.equal(clampZoom(FIT_ZOOM), FIT_ZOOM);
  assert.equal(clampZoom(2.5), 2.5);
  assert.equal(clampZoom(10), MAX_ZOOM);
  assert.equal(clampZoom(NaN), MIN_ZOOM);
  // The floor sits just under fit, so a wheel-out can show a little more of
  // the window than the board's own rect (master's minimum-zoom tweak).
  assert.ok(MIN_ZOOM < FIT_ZOOM);
});

test("fitViewport centres the board in the window", () => {
  assert.deepEqual(fitViewport(SQUARE), { z: FIT_ZOOM, tx: 0, ty: 0 });
  // Wide slot: the board keeps 16:10 and sits centred -- 416px of window on
  // each side. The map does not stretch; the window does.
  assert.deepEqual(fitViewport(WIDE), { z: FIT_ZOOM, tx: (2560 - 1728) / 2, ty: 0 });
  assert.deepEqual(fitViewport(TALL), { z: FIT_ZOOM, tx: 0, ty: (1100 - 700) / 2 });
  // A slightly-smaller-than-fit zoom is still centred.
  const out = centeredViewport(WIDE, MIN_ZOOM);
  assert.equal(out.z, MIN_ZOOM);
  assert.ok(Math.abs(out.tx - (2560 - 1728 * MIN_ZOOM) / 2) < 1e-9);
  // The fit view is always inside the overscroll limits.
  for (const b of [WIDE, SQUARE, TALL]) {
    const f = fitViewport(b);
    assert.deepEqual(clampPan(f, b), f);
  }
});

test("clampPan allows half a window of overscroll past each edge", () => {
  // At fit a window == board rect may slide half a window either way, no further.
  assert.deepEqual(clampPan({ z: 1, tx: 40, ty: -40 }, SQUARE), { z: 1, tx: 40, ty: -40 });
  assert.deepEqual(clampPan({ z: 1, tx: 1e4, ty: -1e4 }, SQUARE), { z: 1, tx: SQUARE.winW * OVERPAN, ty: -SQUARE.winH * OVERPAN });
  // At 2x the board is 2W x 2H: it slides W / H plus the overscroll.
  assert.deepEqual(clampPan({ z: 2, tx: 1e4, ty: -1e4 }, SQUARE), { z: 2, tx: SQUARE.winW * OVERPAN, ty: -SQUARE.winH - SQUARE.winH * OVERPAN });
  // Never more than the overscroll past either edge, at every zoom. The
  // invariant is on the window: it shows at most OVERPAN of itself empty per
  // side (so a corner tile can be brought to the middle), whichever rect the
  // board is.
  for (const b of [WIDE, SQUARE, TALL]) {
    for (const z of [1, 1.3, 2, 3]) {
      for (const t of [1e6, -1e6]) {
        const v = clampPan({ z, tx: t, ty: t }, b);
        assert.ok(v.tx <= b.winW * OVERPAN + 1e-9, `tx ${v.tx} at ${z}`);
        assert.ok(v.tx + b.boardW * z >= b.winW * (1 - OVERPAN) - 1e-9, `tx cover ${v.tx} at ${z}`);
        assert.ok(v.ty <= b.winH * OVERPAN + 1e-9, `ty ${v.ty} at ${z}`);
        assert.ok(v.ty + b.boardH * z >= b.winH * (1 - OVERPAN) - 1e-9, `ty cover ${v.ty} at ${z}`);
      }
    }
  }
});

test("zoomAround keeps the point under the cursor fixed", () => {
  const v: Viewport = { z: 1.5, tx: -100, ty: -200 };
  const px = 640;
  const py = 30;
  const before = under(v, px, py);
  const z = zoomAround(v, px, py, 2.25, SQUARE);
  assert.ok(Math.abs(under(z, px, py).x - before.x) < 1e-9);
  assert.ok(Math.abs(under(z, px, py).y - before.y) < 1e-9);
  // Zooming out around the same point is the same invariant.
  const back = zoomAround(z, px, py, 1.5, SQUARE);
  assert.ok(Math.abs(under(back, px, py).x - before.x) < 1e-9);
  assert.ok(Math.abs(back.tx - v.tx) < 1e-9 && Math.abs(back.ty - v.ty) < 1e-9);
});

test("zoomAround clamps pan while still hugging the cursor", () => {
  // Zooming into the top-left corner at fit keeps the corner under the cursor.
  const z = zoomAround(fitViewport(SQUARE), 0, 0, 3, SQUARE);
  assert.deepEqual(z, { z: 3, tx: 0, ty: 0 });
  // Zooming all the way out lands on the centred view at the zoom floor,
  // overscroll dropped.
  const v: Viewport = { z: 3, tx: SQUARE.winW * (1 - 3), ty: SQUARE.winH * (1 - 3) };
  assert.deepEqual(zoomAround(v, SQUARE.winW / 2, SQUARE.winH / 2, 1, SQUARE), fitViewport(SQUARE));
  assert.deepEqual(zoomAround(v, SQUARE.winW / 2, SQUARE.winH / 2, 0.5, SQUARE), centeredViewport(SQUARE, MIN_ZOOM));
  // Any anchor stays within the overscroll limits -- on the wide slot too,
  // where the fit view is already centred with room to spare.
  for (const b of [WIDE, SQUARE]) {
    const v0 = { z: 3, tx: b.winW * (1 - 3), ty: b.winH * (1 - 3) };
    for (const px of [0, b.winW / 2, b.winW]) {
      for (const py of [0, b.winH / 2, b.winH]) {
        const t = zoomAround(v0, px, py, 1.7, b);
        assert.ok(t.tx <= b.winW * OVERPAN && t.tx + b.boardW * t.z >= b.winW * (1 - OVERPAN) - 1e-9);
        assert.ok(t.ty <= b.winH * OVERPAN && t.ty + b.boardH * t.z >= b.winH * (1 - OVERPAN) - 1e-9);
      }
    }
  }
});

test("wheelZoom steps continuously and caps at the floor / 3x", () => {
  for (const b of [WIDE, SQUARE]) {
    let v = fitViewport(b);
    for (let i = 0; i < 40; i++) v = wheelZoom(v, 400, 300, -120, b); // wheel up
    assert.equal(v.z, MAX_ZOOM);
    for (let i = 0; i < 80; i++) v = wheelZoom(v, 400, 300, 120, b); // wheel down
    // Zooming out past fit lands centred on the zoom floor.
    assert.equal(v.z, MIN_ZOOM);
    assert.deepEqual(v, centeredViewport(b, MIN_ZOOM));
  }
});

test("stepZoom moves in fixed increments around its anchor", () => {
  const b = WIDE;
  const v = stepZoom(fitViewport(b), 1, b.winW / 2, b.winH / 2, b);
  assert.ok(Math.abs(v.z - 1.25) < 1e-9);
  const two = stepZoom(v, 1, b.winW / 2, b.winH / 2, b);
  assert.ok(Math.abs(two.z - 1.25 * 1.25) < 1e-9);
  // Stepping around the window centre keeps the board's centre under it.
  assert.ok(Math.abs(under(two, b.winW / 2, b.winH / 2).x - b.boardW / 2) < 1e-9);
  assert.ok(Math.abs(under(two, b.winW / 2, b.winH / 2).y - b.boardH / 2) < 1e-9);
  let max = v;
  for (let i = 0; i < 6; i++) max = stepZoom(max, 1, 0, 0, b);
  assert.equal(max.z, MAX_ZOOM);
});

test("pinchAround scales on the moving midpoint", () => {
  const b = SQUARE;
  const v0: Viewport = { z: 1.25, tx: -60, ty: -40 };
  const mid0 = { x: 300, y: 200 };
  // Same midpoint, double the span: like zoomAround by 2 on that point.
  const grown = pinchAround(v0, mid0, mid0, 2, b);
  assert.ok(Math.abs(under(grown, mid0.x, mid0.y).x - under(v0, mid0.x, mid0.y).x) < 1e-9);
  assert.ok(Math.abs(under(grown, mid0.x, mid0.y).y - under(v0, mid0.x, mid0.y).y) < 1e-9);
  // Midpoint slides 80px right: the same board point follows it.
  const mid = { x: 380, y: 200 };
  const slid = pinchAround(v0, mid0, mid, 2, b);
  assert.ok(Math.abs(under(slid, mid.x, mid.y).x - under(v0, mid0.x, mid0.y).x) < 1e-9);
  assert.ok(Math.abs(under(slid, mid.x, mid.y).y - under(v0, mid0.x, mid0.y).y) < 1e-9);
  // Pinch-out past the cap still covers the window.
  const huge = pinchAround(v0, mid0, { x: 0, y: 0 }, 10, b);
  assert.equal(huge.z, MAX_ZOOM);
  assert.ok(huge.tx <= b.winW * OVERPAN && huge.tx + b.boardW * huge.z >= b.winW * (1 - OVERPAN) - 1e-9);
});

test("panBy moves the board and clamps at the window edge", () => {
  // Even at fit the board drags, up to half a window.
  assert.deepEqual(panBy(fitViewport(SQUARE), 30, -30, SQUARE), { z: 1, tx: 30, ty: -30 });
  assert.deepEqual(panBy(fitViewport(SQUARE), 1e4, -1e4, SQUARE), { z: 1, tx: SQUARE.winW * OVERPAN, ty: -SQUARE.winH * OVERPAN });
  const zoomed = panBy({ z: 2, tx: -400, ty: -400 }, -100, 20, SQUARE);
  assert.deepEqual(zoomed, { z: 2, tx: -500, ty: -380 });
  const edge = panBy({ z: 2, tx: -500, ty: -500 }, -1e4, 1e4, SQUARE);
  assert.deepEqual(edge, { z: 2, tx: -SQUARE.winW - SQUARE.winW * OVERPAN, ty: SQUARE.winH * OVERPAN });
  // On a wide slot the fit pan is already the board's centring offset, so a
  // small drag just nudges it -- and the window (not the board) sets the limit.
  const wideFit = fitViewport(WIDE);
  const nudged = panBy(wideFit, 30, -30, WIDE);
  assert.deepEqual(nudged, { z: 1, tx: wideFit.tx + 30, ty: wideFit.ty - 30 });
  assert.deepEqual(panBy(wideFit, 1e4, -1e4, WIDE), { z: 1, tx: WIDE.winW * OVERPAN, ty: -WIDE.winH * OVERPAN });
});

test("any map box stays within the overscroll limits under clamp and zoom", () => {
  // The board keeps its aspect; the window may be any size around it. The
  // overscroll math is per-axis against the window and must hold throughout.
  for (const b of [WIDE, SQUARE, TALL, mapBox(3000, 600), mapBox(600, 3000), mapBox(1000, 1000), mapBox(1600, 1000)]) {
    // A pan of 0 may itself need a nudge when the board is narrower than the
    // window (it has to sit where it still covers the window's middle).
    const v = clampPan({ z: 2, tx: 0, ty: 0 }, b);
    assert.equal(v.z, 2);
    assert.ok(v.tx <= b.winW * OVERPAN && v.tx + b.boardW * v.z >= b.winW * (1 - OVERPAN) - 1e-9);
    assert.ok(v.ty <= b.winH * OVERPAN && v.ty + b.boardH * v.z >= b.winH * (1 - OVERPAN) - 1e-9);
    const slide = clampPan({ z: 2, tx: -1e4, ty: -1e4 }, b);
    assert.deepEqual(slide, {
      z: 2,
      tx: b.winW * (1 - OVERPAN) - b.boardW * 2,
      ty: b.winH * (1 - OVERPAN) - b.boardH * 2,
    });
    const z = zoomAround(fitViewport(b), b.winW * 0.9, b.winH * 0.07, 2, b);
    // The cursor stays anchored unless the overscroll clamp pulls it back --
    // so either the board point is unchanged, or the pan sits on a limit.
    const ax = under(z, b.winW * 0.9, b.winH * 0.07).x;
    const ay = under(z, b.winW * 0.9, b.winH * 0.07).y;
    const wantX = b.winW * 0.9 - fitViewport(b).tx;
    const wantY = b.winH * 0.07 - fitViewport(b).ty;
    const clampedX = z.tx <= b.winW * OVERPAN + 1e-9 && z.tx + b.boardW * z.z >= b.winW * (1 - OVERPAN) - 1e-9;
    const clampedY = z.ty <= b.winH * OVERPAN + 1e-9 && z.ty + b.boardH * z.z >= b.winH * (1 - OVERPAN) - 1e-9;
    assert.ok(clampedX && clampedY);
    assert.ok(
      Math.abs(ax - wantX) < 1e-6
        || Math.abs(z.tx - b.winW * OVERPAN) < 1e-9
        || Math.abs(z.tx - (b.winW * (1 - OVERPAN) - b.boardW * z.z)) < 1e-9,
      `x anchor ${ax} vs ${wantX} at tx ${z.tx}`,
    );
    assert.ok(
      Math.abs(ay - wantY) < 1e-6
        || Math.abs(z.ty - b.winH * OVERPAN) < 1e-9
        || Math.abs(z.ty - (b.winH * (1 - OVERPAN) - b.boardH * z.z)) < 1e-9,
      `y anchor ${ay} vs ${wantY} at ty ${z.ty}`,
    );
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

test("mapBox fills the slot as the window and clamps only the board", () => {
  // The window is always the slot (the visibility range is 100% of it).
  for (const [w, h] of [[2560, 1080], [2000, 800], [700, 900], [3000, 600]]) {
    const b = mapBox(w, h);
    assert.equal(b.winW, w);
    assert.equal(b.winH, h);
    // The board keeps its aspect inside -- it does not stretch with the slot.
    assert.ok(b.boardW / b.boardH >= MIN_RATIO - 1e-3 && b.boardW / b.boardH <= MAX_RATIO + 1e-3);
    assert.ok(b.boardW <= b.winW && b.boardH <= b.winH);
  }
  assert.deepEqual(mapBox(2000, 800), { winW: 2000, winH: 800, boardW: 1280, boardH: 800 });
});