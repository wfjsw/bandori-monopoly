// Run with: node --test webui/src/scenes/lobby/enterMatch.test.ts
//
// The room follows the session into a live match, but never into a finished
// one: that is the results screen's frame, and an ended frame combined with a
// `playing` flag still true for the seal-to-reset tick used to bounce the
// player straight back out of the room they asked for.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { shouldEnterMatch } from "./enterMatch.ts";

const base = { scene: "board" as const, phase: "play", playing: true, spectator: false, out: false };

test("a live match pulls every seat in", () => {
  assert.equal(shouldEnterMatch(base), true);
});

test("a finished match stays behind, even while `playing` is still true", () => {
  assert.equal(shouldEnterMatch({ ...base, phase: "ended" }), false);
});

test("setup phases pull the seats in", () => {
  for (const phase of ["order", "ban", "pick", "deck"]) {
    assert.equal(shouldEnterMatch({ ...base, scene: phase === "deck" ? "deck" : "select", phase }), true, phase);
  }
});

test("no room match, or a room that is not playing, stays put", () => {
  assert.equal(shouldEnterMatch({ ...base, scene: null }), false);
  assert.equal(shouldEnterMatch({ ...base, playing: false }), false);
});

test("a seat that forfeited is not dragged back; a spectator is", () => {
  assert.equal(shouldEnterMatch({ ...base, out: true }), false);
  assert.equal(shouldEnterMatch({ ...base, out: true, spectator: true }), true);
});
