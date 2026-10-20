// Run with: node --test webui/src/app/stageBox.test.ts
// The stage geometry is pure math once the safe-area insets are probed: these
// checks pin the window measurement (`windowBox` -- the layout viewport, never
// larger, so the fixed placer cannot overflow the box it is centered in), the
// forced-rotation axis swap, the edge-to-edge fit (the stage fills the window
// -- no safe-area inset on the box itself), the safe mapping through the
// rotation (forced and a real turn of the phone must agree), and the
// compact-canvas predicate so the phone layouts stay predictable.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { COMPACT_H, COMPACT_MIN_W, STAGE_H, STAGE_W, TOUCH_TOP_MIN } from "../styles/layout.ts";
import { type Insets, stageBox, windowBox } from "./stageBox.ts";

const NO_INSET: Insets = { top: 0, right: 0, bottom: 0, left: 0 };

test("windowBox: the layout viewport is the window -- never larger", () => {
  // The placer is `position: fixed; left: 50%; top: 50%`, so it is centered in
  // the layout viewport. Anything bigger overflows that box: the stage grows
  // past the cutout on one side and leaves a blank border on the other.
  assert.deepEqual(
    windowBox({ layoutW: 393, layoutH: 750, innerW: 393, innerH: 750 }),
    { vw: 393, vh: 750 },
    "the layout viewport is the window, chrome and all",
  );
  // Fallback where the probe is missing.
  assert.deepEqual(
    windowBox({ layoutW: 0, layoutH: 0, innerW: 852, innerH: 393 }),
    { vw: 852, vh: 393 },
  );
});

test("windowBox: forced rotation fills the box it is centered in -- no overflow, no gap", () => {
  // Portrait handset with browser chrome: the layout viewport is 393x750 of a
  // 393x852 screen. Sizing to the screen would overflow past the island and
  // leave a blank at the opposite edge; sizing to the layout viewport fills
  // the box exactly. Under `rotate(90deg)` the placer's screen box is
  // h*scale x w*scale.
  const win = windowBox({ layoutW: 393, layoutH: 750, innerW: 393, innerH: 750 });
  const forced = stageBox({
    vw: win.vw, vh: win.vh,
    screenLong: 0, inset: NO_INSET, rotated: true, tall: true, coarse: true, compactForced: null,
  });
  const screenW = forced.h * forced.scale;
  const screenH = forced.w * forced.scale;
  assert.equal(Math.round(screenW), 393, `must fill the box's width, got ${screenW}`);
  assert.equal(Math.round(screenH), 750, `must fill the box's height, got ${screenH}`);
});

test("windowBox: a desktop window is the window, not the monitor", () => {
  assert.deepEqual(
    windowBox({ layoutW: 1200, layoutH: 800, innerW: 1200, innerH: 800 }),
    { vw: 1200, vh: 800 },
  );
});

test("desktop: the stage fills the window 1:1", () => {
  const box = stageBox({
    vw: 1600, vh: 900, screenLong: 0, inset: NO_INSET, rotated: false, tall: false, coarse: false, compactForced: null,
  });
  assert.equal(box.scale, 1);
  assert.equal(box.w, 1600);
  assert.equal(box.h, 900);
  assert.equal(box.compact, false);
  assert.equal(box.rotated, false);
});

test("wide window letterboxes height on a non-tall scene", () => {
  const box = stageBox({
    vw: 2000, vh: 900, screenLong: 0, inset: NO_INSET, rotated: false, tall: false, coarse: false, compactForced: null,
  });
  // scale is still min(2000/1600, 900/900) = 1; the stage width grows, height does not.
  assert.equal(box.scale, 1);
  assert.equal(box.w, 2000);
  assert.equal(box.h, 900);
});

test("landscape iPhone: the stage runs edge to edge on both axes", () => {
  const box = stageBox({
    vw: 852, vh: 393, screenLong: 0, inset: NO_INSET, rotated: false, tall: true, coarse: true, compactForced: null,
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
    vw: 393, vh: 852, screenLong: 0, inset: NO_INSET, rotated: true, tall: true, coarse: true, compactForced: null,
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
    vw: 393, vh: 852, screenLong: 0, inset: upright, rotated: true, tall: true, coarse: true, compactForced: null,
  });
  const actual = stageBox({
    vw: 852, vh: 393, screenLong: 0, inset: turned, rotated: false, tall: true, coarse: true, compactForced: null,
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
    vw: 393, vh: 852, screenLong: 0, inset, rotated: true, tall: true, coarse: false, compactForced: null,
  });
  assert.equal(box.safe.top, 20 / box.scale);
  assert.equal(box.safe.right, 30 / box.scale);
  assert.equal(box.safe.bottom, 40 / box.scale);
  assert.equal(box.safe.left, 10 / box.scale);
});

test("coarse pointers floor the top inset at the touch dead zone", () => {
  const box = stageBox({
    vw: 852, vh: 393, screenLong: 0, inset: NO_INSET, rotated: false, tall: true, coarse: true, compactForced: null,
  });
  // iOS landscape eats the top ~20 CSS px of touches even with no cutout.
  assert.equal(box.safe.top, TOUCH_TOP_MIN / box.scale);
  const fine = stageBox({
    vw: 852, vh: 393, screenLong: 0, inset: NO_INSET, rotated: false, tall: true, coarse: false, compactForced: null,
  });
  assert.equal(fine.safe.top, 0);
});

test("fine pointer, compact forced off: stays on the 1600 x 900 canvas", () => {
  const box = stageBox({
    vw: 500, vh: 300, screenLong: 0, inset: NO_INSET, rotated: false, tall: true, coarse: false, compactForced: false,
  });
  assert.equal(box.compact, false);
  assert.equal(box.scale, Math.min(500 / STAGE_W, 300 / STAGE_H));
});

test("compactForced=false wins even when the window would auto-select compact", () => {
  const box = stageBox({
    vw: 852, vh: 352, screenLong: 0, inset: NO_INSET, rotated: false, tall: true, coarse: true, compactForced: false,
  });
  assert.equal(box.compact, false);
});

test("a degenerate window still yields a positive scale", () => {
  const box = stageBox({
    vw: 0, vh: 0, screenLong: 0, inset: NO_INSET, rotated: false, tall: true, coarse: true, compactForced: null,
  });
  assert.ok(box.scale > 0);
  assert.ok(Number.isFinite(box.w));
  assert.ok(Number.isFinite(box.h));
});

test("iPhone forced rotation: the stage spans the whole screen, island to home indicator", () => {
  // The reported bug: the fixed box is 393x750 of a 393x852 screen -- it
  // starts under the island and stops above Safari's bottom toolbar, which
  // Safari paints with the bare page colour. A box-sized stage left that
  // strip blank; a screen-sized stage centred in the box overflowed past the
  // island and still left it. Screen-sized AND anchored at the box's top
  // covers 0..852 exactly.
  const box = stageBox({
    vw: 393, vh: 750, screenLong: 852,
    inset: NO_INSET, rotated: true, tall: true, coarse: true, compactForced: null,
  });
  // Under rotate(90deg) the placer's screen AABB is h*scale wide, w*scale tall.
  const aabbW = box.h * box.scale;
  const aabbH = box.w * box.scale;
  const top = box.cy - aabbH / 2;
  const left = box.cx - aabbW / 2;
  assert.equal(Math.round(top), 0, "top edge at the screen top (under the island)");
  assert.equal(Math.round(top + aabbH), 852, "bottom edge at the screen bottom (home indicator)");
  assert.equal(Math.round(left), 0);
  assert.equal(Math.round(left + aabbW), 393);
});

test("iPhone forced rotation: the stretch under the toolbar counts as hardware for controls", () => {
  // Stage right lands on the screen bottom under rotate(90deg); the 102px
  // past the fixed box is under Safari's toolbar, so controls on that edge
  // pad by at least that much (more than the 34px home indicator).
  const box = stageBox({
    vw: 393, vh: 750, screenLong: 852,
    inset: { top: 59, right: 0, bottom: 34, left: 0 },
    rotated: true, tall: true, coarse: true, compactForced: null,
  });
  assert.equal(box.safe.right, 102 / box.scale);
  // The island side is unchanged: the portrait top inset on the stage's left.
  assert.equal(box.safe.left, 59 / box.scale);
});

test("the box is kept where there is no stretch: real landscape, Android", () => {
  // Real landscape on an iPhone: nothing changes -- centre of the box.
  const real = stageBox({
    vw: 852, vh: 393, screenLong: 852,
    inset: NO_INSET, rotated: false, tall: true, coarse: true, compactForced: null,
  });
  assert.deepEqual([real.cx, real.cy], [426, 196.5]);
  assert.equal(real.w * real.scale, 852);
  assert.equal(real.h * real.scale, 393);
  // Android (screenLong 0): forced rotation stays on the box, centred.
  const android = stageBox({
    vw: 393, vh: 750, screenLong: 0,
    inset: NO_INSET, rotated: true, tall: true, coarse: true, compactForced: null,
  });
  assert.deepEqual([android.cx, android.cy], [196.5, 375]);
  assert.equal(Math.round(android.w * android.scale), 750);
});

test("a screen reading far past the box is not trusted", () => {
  // Past MAX_REACH: a bad `screen` (zoomed display, desktop emulation) must
  // not blow the stage off the bottom.
  const box = stageBox({
    vw: 393, vh: 500, screenLong: 852,
    inset: NO_INSET, rotated: true, tall: true, coarse: true, compactForced: null,
  });
  assert.equal(box.cy, 250);
  assert.equal(Math.round(box.w * box.scale), 500);
});