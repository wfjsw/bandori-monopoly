// Run with: node --test webui/src/console/cardDebug.test.ts
// The card debug lines: which events get one and what they carry.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import type { MatchEvent } from "../core/types.ts";
import { cardDebugLine, hasCardDebug } from "./cardDebug.ts";

const base = {
  id: 7, type: "card", playerId: 2, other: -1, value: 0, from: -1, to: -1, dice: 0,
  card: "c-foo", msg: { k: "log.effect_applied" },
} as unknown as MatchEvent;

const format = (m: { k: string }) => m.k;

test("card and flash-with-card events get a debug line; other events do not", () => {
  assert.equal(hasCardDebug(base), true);
  assert.equal(hasCardDebug({ ...base, type: "flash" }), true);
  assert.equal(hasCardDebug({ ...base, type: "roll", card: "" }), false);
});

test("the line names the card, trigger, seat and outcome", () => {
  const line = cardDebugLine({ ...base, kind: "play", negated: true, results: [{ k: "log.effect_nothing" }] }, format);
  assert.match(line, /^\[card\] /);
  assert.match(line, /card=c-foo/);
  assert.match(line, /kind=play/);
  assert.match(line, /seat=#2/);
  assert.match(line, /negated/);
  assert.match(line, /result=/);
});

test("absent fields are left out, not printed empty", () => {
  const line = cardDebugLine({ ...base, type: "flash" }, format);
  assert.doesNotMatch(line, /kind=/);
  assert.doesNotMatch(line, /uid=/);
  assert.doesNotMatch(line, /parent=/);
  assert.doesNotMatch(line, /target=/);
  assert.doesNotMatch(line, /negated/);
});

test("field instance and parent ride the line when present", () => {
  const line = cardDebugLine({ ...base, uid: 12, parent: 3, other: 1 }, format);
  assert.match(line, /uid=12/);
  assert.match(line, /parent=#3/);
  assert.match(line, /target=#1/);
});
