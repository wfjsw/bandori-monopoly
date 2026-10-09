// Run with: node --test webui/src/hooks/live2d.test.ts
// The pure portrait-fit math behind `useLive2DStand` (Live2DPortrait.Draw).

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { canvasBox, sameParams, type Framing } from "./pure.ts";

const FRAMING: Framing = { top: 0.1, scale: 1.2 };

test("the eye line lands on eyeFrac of the window", () => {
  // 1000-unit canvas, zoom 1, no framing scale: viewW = viewH*0.75.
  const b = canvasBox(300, [1000, 1000], undefined, 1, 0, 0.02, 200, 0.28, null);
  // viewW = max(1000, 750) / 1 = 1000; viewH = 1000/0.75; k = 300/1000.
  const viewH = 1000 / 0.75;
  const eyeTop = 200 - 0.28 * viewH; // the `top` the hook computes
  assert.equal(b.top, -eyeTop * 0.3);
  assert.equal(b.width, 1000 * 0.3);
});

test("headroom pushes the canvas down off the window's top edge", () => {
  // figTop far above the eye line: the clamp must win over the eye anchor.
  const withClamp = canvasBox(300, [1000, 1000], undefined, 1, 0, 0.2, 200, 0.28, 50);
  const noClamp = canvasBox(300, [1000, 1000], undefined, 1, 0, 0.2, 200, 0.28, null);
  assert.ok(withClamp.top > noClamp.top, "the figure is pushed further down (a larger canvas top)");
});

test("without eye params the framing sheet's top / scale steer the crop", () => {
  const b = canvasBox(300, [800, 1200], FRAMING, 1, 0, 0.02, null, 0.28, null);
  const z = 1 * FRAMING.scale;
  const num = Math.max(800, 1200 * 0.75); // 900
  const viewW = num / z;
  const k = 300 / viewW;
  const focus = Math.min(0, FRAMING.top - 0.02 / z); // focusTop 0 clamps
  const top = (1200 - num / 0.75) / 2 + (num / 0.75) * focus;
  assert.equal(b.top, -top * k);
  assert.equal(b.width, 800 * k);
});

test("zoom grows the canvas rect: a bigger zoom shows less of the model", () => {
  const near = canvasBox(300, [1000, 1000], undefined, 1, 0, 0, 200, 0.28, null);
  const far = canvasBox(300, [1000, 1000], undefined, 2, 0, 0, 200, 0.28, null);
  assert.ok(far.width > near.width, "the canvas rect grows with zoom");
  assert.ok(far.width === 2 * near.width, "exactly: the view window halves");
});

test("two identical param maps compare equal, a changed one does not", () => {
  assert.equal(sameParams(undefined, {}), true, "absent is the empty map");
  assert.equal(sameParams({ a: 1, b: 2 }, { b: 2, a: 1 }), true);
  assert.equal(sameParams({ a: 1 }, { a: 2 }), false);
  assert.equal(sameParams({ a: 1 }, { a: 1, b: 2 }), false);
});