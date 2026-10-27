// Card-face marker plan: print order and the countdown-suppression rule.
//   node --test webui/src/scenes/board/markers.test.ts

import { strict as assert } from "node:assert";
import { test } from "node:test";

import { glyphOf, planMarkers, showTurnsChip } from "./markerKinds.ts";

/** Named-counter display phrases for the plan; anything else is unlabelled. */
const nameOf = (name: string) => (name === "fire" ? "火罐" : undefined);

const kinds = (counts: Parameters<typeof planMarkers>[0]) =>
  planMarkers(counts, nameOf).map((m) => `${m.kind}:${m.n}`);

test("each kind carries its own glyph in its own definition", () => {
  assert.equal(glyphOf("turns", 3), "⌛3");
  assert.equal(glyphOf("counter", 1), "×1");
  assert.equal(glyphOf("crystals", 2), "◆2");
  assert.equal(glyphOf("cp", 4), "CP4");
  assert.equal(glyphOf("named", 2, "火罐"), "火罐×2");
});

test("a countdown carried by another marker is not repeated", () => {
  assert.equal(showTurnsChip(3, 3), false, "same number as a crystal count");
  assert.equal(showTurnsChip(2, 1, 5), true, "unrelated counters keep it");
  assert.equal(showTurnsChip(0, 0), false, "nothing scheduled");
});

test("a crystal countdown prints only the crystals", () => {
  assert.deepEqual(kinds({ turnsLeft: 3, crystals: 3 }), ["crystals:3"]);
  assert.deepEqual(kinds({ turnsLeft: 1, crystals: 1, named: [["fire", 1]] }), ["crystals:1", "named:1"]);
});

test("an uncoupled countdown keeps its chip", () => {
  assert.deepEqual(kinds({ turnsLeft: 1 }), ["turns:1"]);
});

test("an event's phase counter does not hide the countdown", () => {
  assert.deepEqual(kinds({ turnsLeft: 2, counter: 1 }), ["turns:2", "counter:1"]);
});

test("markers print countdown first, then named counters last", () => {
  assert.deepEqual(kinds({ turnsLeft: 2, counter: 1, cp: 4, named: [["fire", 3]] }), [
    "turns:2",
    "counter:1",
    "cp:4",
    "named:3",
  ]);
});

test("an unlabelled named counter is dropped, never printed as its wire key", () => {
  assert.deepEqual(kinds({ named: [["mystery", 2]] }), []);
  assert.deepEqual(kinds({ crystals: 0, cp: -1 }), []);
});
