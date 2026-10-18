// Run with: node --test webui/src/app/stageScale.test.ts
// The stage's screen mapping (scale + forced rotation) is recorded, not
// measured back out of the DOM: browsers disagree on what `offsetWidth` reports
// under `zoom`, and `getBoundingClientRect` reports the *axis-aligned* box --
// which under the stage's `rotate(90deg)` has width and height traded. These
// checks pin the round-trip, the clamping and the un-rotation, so neither a bad
// `setStageScale` nor a rotated stage can poison every design-px measurement
// below the stage (board slot, field row, card preview, Live2D stand, ...).
import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  designBox,
  isStageRotated,
  screenCssBox,
  setStageRotated,
  setStageScale,
  stageScale,
} from "./stageScale.ts";

/** An element standing in for one inside the stage: only `getBoundingClientRect` is used. */
function stub(width: number, height: number): HTMLElement {
  return {
    getBoundingClientRect: () => ({ width, height, left: 0, top: 0, right: width, bottom: height }),
  } as unknown as HTMLElement;
}

test("stageScale defaults to 1 before Stage records a value", () => {
  setStageScale(1);
  assert.equal(stageScale(), 1);
});

test("setStageScale records a positive finite scale", () => {
  setStageScale(0.41);
  assert.equal(stageScale(), 0.41);
  setStageScale(1.5);
  assert.equal(stageScale(), 1.5);
});

test("setStageScale rejects non-positive and non-finite values, falling back to 1", () => {
  for (const bad of [0, -1, -0.5, Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
    setStageScale(2);
    setStageScale(bad);
    assert.equal(stageScale(), 1, `setStageScale(${String(bad)}) should fall back to 1`);
  }
});

test("isStageRotated is false until Stage records the rotation", () => {
  setStageRotated(false);
  assert.equal(isStageRotated(), false);
  setStageRotated(true);
  assert.equal(isStageRotated(), true);
  setStageRotated(false);
  assert.equal(isStageRotated(), false);
});

test("designBox divides the screen box out to design px when unrotated", () => {
  setStageScale(0.5);
  setStageRotated(false);
  // A 400x200 design box at scale 0.5 reports a 200x100 screen box.
  assert.deepEqual(designBox(stub(200, 100)), { width: 400, height: 200 });
});

test("designBox un-swaps the axis-aligned box under forced rotation", () => {
  setStageScale(0.5);
  setStageRotated(true);
  // The same 400x200 design box turned 90deg reports a 100x200 screen box.
  // Reading that as-is would hand the Live2D stand and the board slot a
  // transposed 200x400 -- the deformation seen in force-rotation mode.
  assert.deepEqual(designBox(stub(100, 200)), { width: 400, height: 200 });
});

test("screenCssBox is the element's own axes in screen CSS px", () => {
  setStageScale(0.5);
  setStageRotated(true);
  // Own axes, not the rotated AABB: 400x200 design at scale 0.5 is 200x100
  // screen CSS px even though the box on screen measures 100x200. This is the
  // shape a canvas drawing buffer needs.
  assert.deepEqual(screenCssBox(stub(100, 200)), { width: 200, height: 100 });
  setStageRotated(false);
  assert.deepEqual(screenCssBox(stub(200, 100)), { width: 200, height: 100 });
});