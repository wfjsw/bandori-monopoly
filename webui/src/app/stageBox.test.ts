// Run with: node --test webui/src/app/stageBox.test.ts
// The stage geometry is pure math: these checks pin the forced-rotation axis
// swap, the edge-to-edge fit (the stage fills the window -- no safe-area
// inset), and the compact-canvas predicate so the phone layouts stay predictable.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { COMPACT_H, COMPACT_MIN_W, STAGE_H, STAGE_W } from "../styles/layout.ts";
import { stageBox } from "./stageBox.ts";

test("desktop: the stage fills the window 1:1", () => {
  const box = stageBox({
    vw: 1600, vh: 900, rotated: false, tall: false, coarse: false, compactForced: null,
  });
  assert.equal(box.scale, 1);
  assert.equal(box.w, 1600);
  assert.equal(box.h, 900);
  assert.equal(box.compact, false);
  assert.equal(box.rotated, false);
});

test("wide window letterboxes height on a non-tall scene", () => {
  const box = stageBox({
    vw: 2000, vh: 900, rotated: false, tall: false, coarse: false, compactForced: null,
  });
  // scale is still min(2000/1600, 900/900) = 1; the stage width grows, height does not.
  assert.equal(box.scale, 1);
  assert.equal(box.w, 2000);
  assert.equal(box.h, 900);
});

test("landscape iPhone: the stage runs edge to edge on both axes", () => {
  const box = stageBox({
    vw: 852, vh: 393, rotated: false, tall: true, coarse: true, compactForced: null,
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
    vw: 393, vh: 852, rotated: true, tall: true, coarse: true, compactForced: null,
  });
  assert.equal(box.w, 852 / box.scale);
  assert.equal(box.h, 393 / box.scale);
  assert.equal(box.rotated, true);
});

test("fine pointer, compact forced off: stays on the 1600 x 900 canvas", () => {
  const box = stageBox({
    vw: 500, vh: 300, rotated: false, tall: true, coarse: false, compactForced: false,
  });
  assert.equal(box.compact, false);
  assert.equal(box.scale, Math.min(500 / STAGE_W, 300 / STAGE_H));
});

test("compactForced=false wins even when the window would auto-select compact", () => {
  const box = stageBox({
    vw: 852, vh: 352, rotated: false, tall: true, coarse: true, compactForced: false,
  });
  assert.equal(box.compact, false);
});

test("a degenerate window still yields a positive scale", () => {
  const box = stageBox({
    vw: 0, vh: 0, rotated: false, tall: true, coarse: true, compactForced: null,
  });
  assert.ok(box.scale > 0);
  assert.ok(Number.isFinite(box.w));
  assert.ok(Number.isFinite(box.h));
});