// Run with: node --test webui/src/app/stageBox.test.ts
// The stage geometry is pure math once the safe-area insets are probed: these
// checks pin the window measurement (`windowBox`), the forced-rotation axis
// swap, the edge-to-edge fit (the stage fills the window -- no safe-area inset
// on the box itself), the safe mapping through the rotation (forced and a real
// turn of the phone must agree), and the compact-canvas predicate so the phone
// layouts stay predictable.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { COMPACT_H, COMPACT_MIN_W, STAGE_H, STAGE_W, TOUCH_TOP_MIN } from "../styles/layout.ts";
import { type Insets, stageBox, windowBox } from "./stageBox.ts";

const NO_INSET: Insets = { top: 0, right: 0, bottom: 0, left: 0 };

test("windowBox: a handset uses the device screen, not the shrunken viewport", () => {
  // Portrait iPhone in Safari / PWA: the layout and visual viewports are short
  // of the 852pt screen (address bar, status-bar strip), `screen` is not.
  const portrait = windowBox({
    screenW: 393, screenH: 852,
    innerW: 393, innerH: 750,
    layoutW: 393, layoutH: 750,
    probeW: 393, probeH: 793,
    portrait: true, coarse: true,
  });
  assert.deepEqual(portrait, { vw: 393, vh: 852 });
  // The same handset turned: every source agrees, and the rectangle is the
  // portrait one with the sides traded -- that is what makes forced (swapped)
  // and a real turn share one canvas.
  const landscape = windowBox({
    screenW: 852, screenH: 393,
    innerW: 852, innerH: 393,
    layoutW: 852, layoutH: 393,
    probeW: 852, probeH: 393,
    portrait: false, coarse: true,
  });
  assert.deepEqual(landscape, { vw: 852, vh: 393 });
});

test("windowBox: the long side reaches the phone's bottom, not just under the island", () => {
  // The reported bug: under forced rotation the stage was short on the long
  // axis, so the leftover strip hid under the cutout at the top and showed as
  // a blank bar at the BOTTOM of the phone. The stage's screen height under
  // `rotate(90deg)` is its design width -- pin that it is the device's 852.
  const win = windowBox({
    screenW: 393, screenH: 852,
    innerW: 393, innerH: 750,
    layoutW: 393, layoutH: 750,
    probeW: 393, probeH: 793,
    portrait: true, coarse: true,
  });
  const forced = stageBox({
    vw: win.vw, vh: win.vh,
    inset: NO_INSET, rotated: true, tall: true, coarse: true, compactForced: null,
  });
  // After the 90deg turn the placer's screen box is h*scale x w*scale.
  const screenW = forced.h * forced.scale;
  const screenH = forced.w * forced.scale;
  assert.equal(Math.round(screenH), 852, `long axis must be the device's 852, got ${screenH}`);
  assert.equal(Math.round(screenW), 393, `short axis must be the device's 393, got ${screenW}`);
});

test("windowBox: never comes in short of the window (no blank bar at an end)", () => {
  // Even if `screen` is missing or wrong, the largest candidate wins -- a
  // stage smaller than the window letterboxes into a blank bar at each end.
  const box = windowBox({
    screenW: 0, screenH: 0,
    innerW: 393, innerH: 852,
    layoutW: 393, layoutH: 852,
    probeW: 0, probeH: 0,
    portrait: true, coarse: true,
  });
  assert.deepEqual(box, { vw: 393, vh: 852 });
});

test("windowBox: a desktop window is not blown up to the monitor", () => {
  // A touch-screen laptop: coarse, but the short side is well past handset
  // size, so `screen` must not count.
  const box = windowBox({
    screenW: 1920, screenH: 1080,
    innerW: 1200, innerH: 800,
    layoutW: 1200, layoutH: 800,
    probeW: 1200, probeH: 800,
    portrait: false, coarse: true,
  });
  assert.deepEqual(box, { vw: 1200, vh: 800 });
});

test("desktop: the stage fills the window 1:1", () => {
  const box = stageBox({
    vw: 1600, vh: 900, inset: NO_INSET, rotated: false, tall: false, coarse: false, compactForced: null,
  });
  assert.equal(box.scale, 1);
  assert.equal(box.w, 1600);
  assert.equal(box.h, 900);
  assert.equal(box.compact, false);
  assert.equal(box.rotated, false);
});

test("wide window letterboxes height on a non-tall scene", () => {
  const box = stageBox({
    vw: 2000, vh: 900, inset: NO_INSET, rotated: false, tall: false, coarse: false, compactForced: null,
  });
  // scale is still min(2000/1600, 900/900) = 1; the stage width grows, height does not.
  assert.equal(box.scale, 1);
  assert.equal(box.w, 2000);
  assert.equal(box.h, 900);
});

test("landscape iPhone: the stage runs edge to edge on both axes", () => {
  const box = stageBox({
    vw: 852, vh: 393, inset: NO_INSET, rotated: false, tall: true, coarse: true, compactForced: null,
  });
  assert.equal(box.scale, Math.min(852 / COMPACT_MIN_W, 393 / COMPACT_H));
  assert.ok(box.scale > 0);
  // Flush with every real screen edge -- the field row clips at the top and the
  // hand falls off the bottom of this box, so it must be the window itself.
  assert.equal(box.w, 852 / box.scale);
  assert.equal(box.h, 393 / box.scale);
  assert.equal(box.compact, true);
});

test("rotated lays the stage out on the swapped window", () => {
  // Portrait phone forced to landscape: the window's height carries the stage width.
  const box = stageBox({
    vw: 393, vh: 852, inset: NO_INSET, rotated: true, tall: true, coarse: true, compactForced: null,
  });
  assert.equal(box.w, 852 / box.scale);
  assert.equal(box.h, 393 / box.scale);
  assert.equal(box.rotated, true);
});

test("forced rotation and a real turn of the phone share one canvas", () => {
  // The same physical handset. Held upright (forced): the island is the
  // screen's top inset, the home indicator the bottom. Turned the way forced
  // rotation pretends (top edge -> left): the island is the left inset.
  // The window is the large viewport in both cases, so the two must land on
  // the identical stage box -- that is what "forced looks like a real turn" means.
  const upright: Insets = { top: 59, right: 0, bottom: 34, left: 0 };
  const turned: Insets = { top: 0, right: 34, bottom: 0, left: 59 };
  const forced = stageBox({
    vw: 393, vh: 852, inset: upright, rotated: true, tall: true, coarse: true, compactForced: null,
  });
  const actual = stageBox({
    vw: 852, vh: 393, inset: turned, rotated: false, tall: true, coarse: true, compactForced: null,
  });
  assert.equal(forced.scale, actual.scale);
  assert.equal(forced.w, actual.w);
  assert.equal(forced.h, actual.h);
  assert.equal(forced.compact, actual.compact);
  // And the island clears the same stage edge in both: the design's left.
  assert.equal(forced.safe.left, upright.top / forced.scale);
  assert.equal(actual.safe.left, turned.left / actual.scale);
  assert.ok(forced.safe.left > 0);
});

test("rotate(90deg) trades the inset sides onto the stage edges", () => {
  // Clockwise: stage top <- screen right, stage right <- screen bottom,
  // stage bottom <- screen left, stage left <- screen top.
  const inset: Insets = { top: 10, right: 20, bottom: 30, left: 40 };
  const box = stageBox({
    vw: 393, vh: 852, inset, rotated: true, tall: true, coarse: false, compactForced: null,
  });
  assert.equal(box.safe.top, 20 / box.scale);
  assert.equal(box.safe.right, 30 / box.scale);
  assert.equal(box.safe.bottom, 40 / box.scale);
  assert.equal(box.safe.left, 10 / box.scale);
});

test("coarse pointers floor the top inset at the touch dead zone", () => {
  const box = stageBox({
    vw: 852, vh: 393, inset: NO_INSET, rotated: false, tall: true, coarse: true, compactForced: null,
  });
  // iOS landscape eats the top ~20 CSS px of touches even with no cutout.
  assert.equal(box.safe.top, TOUCH_TOP_MIN / box.scale);
  const fine = stageBox({
    vw: 852, vh: 393, inset: NO_INSET, rotated: false, tall: true, coarse: false, compactForced: null,
  });
  assert.equal(fine.safe.top, 0);
});

test("fine pointer, compact forced off: stays on the 1600 x 900 canvas", () => {
  const box = stageBox({
    vw: 500, vh: 300, inset: NO_INSET, rotated: false, tall: true, coarse: false, compactForced: false,
  });
  assert.equal(box.compact, false);
  assert.equal(box.scale, Math.min(500 / STAGE_W, 300 / STAGE_H));
});

test("compactForced=false wins even when the window would auto-select compact", () => {
  const box = stageBox({
    vw: 852, vh: 352, inset: NO_INSET, rotated: false, tall: true, coarse: true, compactForced: false,
  });
  assert.equal(box.compact, false);
});

test("a degenerate window still yields a positive scale", () => {
  const box = stageBox({
    vw: 0, vh: 0, inset: NO_INSET, rotated: false, tall: true, coarse: true, compactForced: null,
  });
  assert.ok(box.scale > 0);
  assert.ok(Number.isFinite(box.w));
  assert.ok(Number.isFinite(box.h));
});