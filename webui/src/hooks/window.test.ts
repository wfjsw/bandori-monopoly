// Window math for the match log: offsets count measured rows and fall back to
// the estimate, and the visible range is grown by the overscan on both sides
// and never comes back empty.

import assert from "node:assert/strict";
import { test } from "node:test";
import { rowOffsets, rowRange } from "./window.ts";

test("offsets use the estimate for unmeasured rows", () => {
  assert.deepEqual(rowOffsets([10, undefined, 30], 20), [0, 10, 30, 60]);
  assert.deepEqual(rowOffsets([], 20), [0]);
});

test("offsets use a measured zero, not the estimate", () => {
  assert.deepEqual(rowOffsets([0, 5], 20), [0, 0, 5]);
});

test("range starts and ends with the viewport plus overscan", () => {
  const offsets = rowOffsets(new Array(100).fill(20), 20);
  // Viewport [200, 300), overscan 40 -> candidates [160, 340).
  const r = rowRange(offsets, 200, 100, 40);
  assert.equal(r.start, 8);
  assert.equal(r.end, 17);
});

test("range begins at zero at the top and covers one row when tiny", () => {
  const offsets = rowOffsets(new Array(10).fill(20), 20);
  assert.deepEqual(rowRange(offsets, 0, 100, 40), { start: 0, end: 7 });
  assert.deepEqual(rowRange(offsets, 0, 0, 0), { start: 0, end: 1 });
});

test("range clamps at the tail", () => {
  const offsets = rowOffsets(new Array(10).fill(20), 20);
  assert.deepEqual(rowRange(offsets, 1000, 100, 40), { start: 9, end: 10 });
});

test("range of an empty list is empty", () => {
  assert.deepEqual(rowRange(rowOffsets([], 20), 0, 100, 40), { start: 0, end: 0 });
});
