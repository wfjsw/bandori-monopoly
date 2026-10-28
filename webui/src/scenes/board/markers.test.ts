// Card-face marker plan: print order and the countdown-suppression rule.
//   node --test webui/src/scenes/board/markers.test.ts
//
// The rows are rules data (`rules/bandori-std/src/marker.rs` -> display.json);
// the fixture below mirrors them so this file runs without a build.

import { strict as assert } from "node:assert";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

import { glyphOf, planMarkers, showTurnsChip, type MarkerDef } from "./markerKinds.ts";

/** The exported rows' shape, in print order (see the drift test at the end). */
const DEFS: MarkerDef[] = [
  { key: "turnsLeft", source: "prop", tone: "turns", glyph: "⌛{n}", countdown: true, note: { key: "board.turnsLeft" }, label: { key: "board.turnsLabel" } },
  { key: "counter", source: "counter", tone: "count", glyph: "×{n}", note: { text: "×{n}" }, label: { text: "×{n}" } },
  { key: "counter2", source: "counter", tone: "count", glyph: "×{n}", note: { text: "×{n}" }, label: { text: "×{n}" } },
  { key: "crystals", source: "counter", tone: "crystal", glyph: "◆{n}", note: { key: "board.crystals" }, label: { key: "board.crystalsLabel" } },
  { key: "cp", source: "counter", tone: "cp", glyph: "CP{n}", note: { key: "board.cp" }, label: { text: "CP" } },
  { source: "counter", tone: "count", glyph: "{name}×{n}", fallback: true, note: { text: "{name} ×{n}" }, label: { text: "{name}" } },
];

/** Named-counter display phrases for the plan; anything else is unlabelled. */
const nameOf = (name: string) => (name === "fire" ? "火罐" : undefined);

const kinds = (counts: Parameters<typeof planMarkers>[1]) =>
  planMarkers(DEFS, counts, nameOf).map((m) => `${m.key}:${m.n}`);

test("each row carries its own glyph", () => {
  const def = DEFS[0];
  assert.equal(glyphOf(def, 3), "⌛3");
  assert.equal(glyphOf(DEFS[3], 2), "◆2");
  assert.equal(glyphOf(DEFS[4], 4), "CP4");
  assert.equal(glyphOf(DEFS[5], 2, "火罐"), "火罐×2");
});

test("a countdown carried by another marker is not repeated", () => {
  assert.equal(showTurnsChip(3, 3), false, "same number as a crystal count");
  assert.equal(showTurnsChip(2, 1, 5), true, "unrelated counters keep it");
  assert.equal(showTurnsChip(0, 0), false, "nothing scheduled");
});

test("a crystal countdown prints only the crystals", () => {
  assert.deepEqual(kinds({ props: { turnsLeft: 3 }, counters: { crystals: 3 } }), ["crystals:3"]);
  assert.deepEqual(kinds({ props: { turnsLeft: 1 }, counters: { crystals: 1, fire: 1 } }), ["crystals:1", "c:fire:1"]);
});

test("an uncoupled countdown keeps its chip", () => {
  assert.deepEqual(kinds({ props: { turnsLeft: 1 } }), ["turnsLeft:1"]);
});

test("an event's phase counter does not hide the countdown", () => {
  assert.deepEqual(kinds({ props: { turnsLeft: 2 }, counters: { counter: 1 } }), ["turnsLeft:2", "counter:1"]);
});

test("markers print in row order, named counters last", () => {
  assert.deepEqual(kinds({ props: { turnsLeft: 2 }, counters: { counter: 1, cp: 4, fire: 3 } }), [
    "turnsLeft:2",
    "counter:1",
    "cp:4",
    "c:fire:3",
  ]);
});

test("an unlabelled named counter is dropped, never printed as its wire key", () => {
  assert.deepEqual(kinds({ counters: { mystery: 2 } }), []);
  assert.deepEqual(kinds({ counters: { crystals: 0, cp: -1 } }), []);
});

test("a counter a row covers is not repeated by the fallback row", () => {
  assert.deepEqual(kinds({ counters: { cp: 2 } }), ["cp:2"]);
});

test("the fixture matches the exported rule table when one is built", (t) => {
  const url = new URL("../../generated/display.json", import.meta.url);
  if (!existsSync(url)) return t.skip("display.json not generated");
  const live = (JSON.parse(readFileSync(url, "utf-8")) as { markers: MarkerDef[] }).markers;
  assert.deepEqual(live, DEFS);
});
