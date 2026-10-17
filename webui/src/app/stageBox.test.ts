// Run with: node --test webui/src/app/stageBox.test.ts
// The stage geometry is pure math once the safe-area insets are probed: these
// checks pin the axis swap on rotation, the touch top dead-zone floor, and the
// compact-canvas predicate so the phone layouts stay predictable.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { COMPACT_H, COMPACT_MIN_W, STAGE_H, STAGE_W, TOUCH_TOP_MIN } from "../styles/layout.ts";
import { type StageBoxInput, stageBox } from "./stageBox.ts";

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

test("landscape iPhone with Dynamic Island floors the top inset to the touch dead zone", () => {
  const box = stageBox({
    vw: 852, vh: 393,
    inset: { top: 0, right: 59, bottom: 21, left: 59 },
    rotated: false, tall: true, coarse: true, compactForced: null,
  });
  assert.ok(TOUCH_TOP_MIN > 0);
  const availW = 852 - 59 - 59; // 734
  const availH = 393 - TOUCH_TOP_MIN - 21; // 352
  assert.equal(box.scale, Math.min(availW / COMPACT_MIN_W, availH / COMPACT_H));
  assert.ok(box.scale > 0);
  assert.equal(box.w, availW / box.scale);
  assert.equal(box.h, availH / box.scale);
  assert.equal(box.shiftX, 0);
  assert.equal(box.shiftY, (TOUCH_TOP_MIN - 21) / 2);
  assert.equal(box.compact, true);
});

test("the top dead zone is not applied for fine pointers", () => {
  const box = stageBox({
    vw: 852, vh: 393,
    inset: { top: 0, right: 59, bottom: 21, left: 59 },
    rotated: false, tall: true, coarse: false, compactForced: null,
  });
  // top stays 0, so the vertical fit is the raw window minus the bottom inset.
  const availH = 393 - 21;
  assert.equal(box.scale, Math.min((852 - 118) / STAGE_W, availH / STAGE_H));
  assert.equal(box.shiftY, (0 - 21) / 2);
  assert.equal(box.compact, false);
});

test("rotated maps the insets onto the swapped axes", () => {
  // Portrait phone forced to landscape: the window's height carries the stage width.
  const box = stageBox({
    vw: 393, vh: 852,
    inset: { top: 59, right: 0, bottom: 34, left: 0 },
    rotated: true, tall: true, coarse: true, compactForced: null,
  });
  const stageW = 852 - 59 - 34; // 759, from the window *height*
  const stageH = 393; // full window width (no side insets)
  assert.equal(box.w * box.scale, stageW);
  assert.equal(box.h * box.scale, stageH);
  assert.equal(box.w, stageW / box.scale);
  assert.equal(box.h, stageH / box.scale);
  assert.equal(box.shiftX, 0);
  assert.equal(box.shiftY, (59 - 34) / 2);
  assert.equal(box.rotated, true);
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