// Run with: node --test webui/src/scenes/board/model.test.ts
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { canSend } from "./inputLock.ts";

const off = { readOnly: false, autoMode: "off" };

test("托管 / 混沌 locks ordinary play, but not the leave dialog's exits", () => {
  for (const autoMode of ["bot", "chaos", "advanced"] as const) {
    assert.equal(canSend({ readOnly: false, autoMode }), false, `${autoMode} owns the seat`);
    assert.equal(canSend({ readOnly: false, autoMode }, true), true, `${autoMode} still lets the player leave`);
  }
  assert.equal(canSend(off), true);
  assert.equal(canSend(off, true), true);
});

test("a read-only seat never sends, not even the leave exits", () => {
  assert.equal(canSend({ readOnly: true, autoMode: "off" }), false);
  assert.equal(canSend({ readOnly: true, autoMode: "off" }, true), false);
  assert.equal(canSend({ readOnly: true, autoMode: "chaos" }, true), false);
});