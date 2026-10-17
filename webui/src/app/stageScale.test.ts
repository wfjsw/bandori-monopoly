// Run with: node --test webui/src/app/stageScale.test.ts
// The stage scale is a recorded number, not something measured back out of the
// DOM: browsers disagree on what `offsetWidth` reports under `zoom`. These
// checks pin the round-trip and the clamping so a bad `setStageScale` cannot
// poison every design-px measurement below the stage.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { setStageScale, stageScale } from "./stageScale.ts";

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