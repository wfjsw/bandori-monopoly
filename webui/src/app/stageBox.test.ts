// Run with: node --test webui/src/app/stageBox.test.ts
// The stage geometry is pure math once the safe-area insets are probed: these
// checks pin the axis swap on rotation, the touch top dead-zone floor, the
// retractable-axis bleed (the field row / hand must reach the real screen
// edge), and the compact-canvas predicate so the phone layouts stay predictable.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { COMPACT_H, COMPACT_MIN_W, STAGE_H, STAGE_W, TOUCH_TOP_MIN } from "../styles/layout.ts";
import { stageBox } from "./stageBox.ts";

const NO_INSET = { top: 0, right: 0, bottom: 0, left: 0 };

test("no insets, desktop, not rotated: the stage fills the window 1:1", () => {
  const box = stageBox({
    vw: 1600, vh: 900, inset: NO_INSET, rotated: false, tall: false, coarse: false, compactForced: null,
  });
  assert.equal(box.scale, 1);
  assert.equal(box.w, 1600);
  assert.equal(box.h, 900);
  assert.equal(box.shiftX, 0);
  assert.equal(box.shiftY, 0);
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

test("landscape iPhone with Dynamic Island: inset on the cutout axis, flush on the retractable one", () => {
  const box = stageBox({
    vw: 852, vh: 393,
    inset: { top: 0, right: 59, bottom: 21, left: 59 },
    rotated: false, tall: true, coarse: true, compactForced: null,
  });
  assert.ok(TOUCH_TOP_MIN > 0);
  const spanW = 852 - 59 - 59; // 734 -- the side columns clear the cutout
  const spanH = 393; // FULL height: the hand and the field row reach the border
  assert.equal(box.scale, Math.min(spanW / COMPACT_MIN_W, spanH / COMPACT_H));
  assert.ok(box.scale > 0);
  assert.equal(box.w * box.scale, spanW);
  assert.equal(box.h * box.scale, spanH);
  // Flush with both real edges -- this is the property the retractable strips
  // depend on. Inset here and the hand stops short of the border.
  assert.equal(box.h * box.scale, 393);
  assert.equal(box.shiftX, 0);
  assert.equal(box.shiftY, 0);
  assert.equal(box.compact, true);
  // Hardware reported per stage edge, in design px.
  assert.equal(box.safe.top, TOUCH_TOP_MIN / box.scale);
  assert.equal(box.safe.bottom, 21 / box.scale);
  assert.equal(box.safe.left, 59 / box.scale);
  assert.equal(box.safe.right, 59 / box.scale);
});

test("the top dead zone is not applied for fine pointers", () => {
  const box = stageBox({
    vw: 852, vh: 393,
    inset: { top: 0, right: 59, bottom: 21, left: 59 },
    rotated: false, tall: true, coarse: false, compactForced: null,
  });
  assert.equal(box.scale, Math.min((852 - 118) / STAGE_W, 393 / STAGE_H));
  assert.equal(box.h * box.scale, 393);
  assert.equal(box.shiftY, 0);
  assert.equal(box.compact, false);
  assert.equal(box.safe.top, 0);
  assert.equal(box.safe.bottom, 21 / box.scale);
});

test("rotated maps the insets onto the swapped axes and still bleeds the retractable one", () => {
  // Portrait phone forced to landscape: the window's height carries the stage width.
  const box = stageBox({
    vw: 393, vh: 852,
    inset: { top: 59, right: 0, bottom: 34, left: 0 },
    rotated: true, tall: true, coarse: true, compactForced: null,
  });
  const spanW = 852 - 59 - 34; // 759, from the window *height* (the cutout axis)
  const spanH = 393; // full window width: the retractable axis
  assert.equal(box.w * box.scale, spanW);
  assert.equal(box.h * box.scale, spanH);
  assert.equal(box.shiftX, 0);
  assert.equal(box.shiftY, (59 - 34) / 2);
  assert.equal(box.rotated, true);
  // rotate(90deg) puts the stage's top edge on the screen's right edge and its
  // left edge on the screen's top edge, so the hardware follows.
  assert.equal(box.safe.top, 0 / box.scale);
  assert.equal(box.safe.bottom, 0 / box.scale);
  assert.equal(box.safe.left, 59 / box.scale);
  assert.equal(box.safe.right, 34 / box.scale);
});

test("fine pointer, no insets, compact forced off: stays on the 1600 x 900 canvas", () => {
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

test("a pathological window still yields a positive scale", () => {
  const box = stageBox({
    vw: 10, vh: 10,
    inset: { top: 50, right: 50, bottom: 50, left: 50 },
    rotated: false, tall: true, coarse: true, compactForced: null,
  });
  assert.ok(box.scale > 0);
  assert.ok(Number.isFinite(box.w));
  assert.ok(Number.isFinite(box.h));
});