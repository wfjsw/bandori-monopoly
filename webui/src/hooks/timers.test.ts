// Run with: node --test webui/src/hooks/timers.test.ts
// The pure countdown arithmetic behind `useCountdown` / `usePerfCountdown`.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { countdownLeft } from "./pure.ts";

test("countdownLeft never goes negative", () => {
  assert.equal(countdownLeft(1000, 250), 750);
  assert.equal(countdownLeft(1000, 1000), 0);
  assert.equal(countdownLeft(1000, 2500), 0, "a late tick lands on zero, not a negative hold");
});

test("a deadline ahead of the clock counts down in its own unit", () => {
  // The match clocks are performance.now(); the banner ones Date.now(). The
  // math is the same either way -- just never mix the two epochs.
  assert.equal(countdownLeft(5_000, 4_700), 300);
});