// Run with: node --test webui/src/scenes/board/turnFlow.test.ts
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { movementControl, shouldFinishTurn } from "./turnFlow.ts";

const controls = { auto: false, animating: false, readOnly: false, connected: true, modalOpen: false };
const landed = {
  S: { phase: "play" as const, step: 4, busy: false, roller: 0, canRollHere: false, canEndHere: true },
  playerId: 0, myTurn: true, asking: false, out: false, overHand: false,
};

test("landing decisions and animations finish before the turn advances", () => {
  assert.equal(shouldFinishTurn(landed, controls), true, "quiet settlement after the deed closes");
  assert.equal(shouldFinishTurn(landed, { ...controls, modalOpen: true }), false, "buy/build, leave, or inspection is open");
  assert.equal(shouldFinishTurn(landed, { ...controls, animating: true }), false, "dice and movement are still playing");
  assert.equal(shouldFinishTurn({ ...landed, asking: true }, controls), false, "an engine prompt still needs an answer");
  assert.equal(shouldFinishTurn({ ...landed, S: { ...landed.S, busy: true } }, controls), false);
});

test("hand limits, another seat, replay, autopilot and disconnection block automatic end", () => {
  assert.equal(shouldFinishTurn({ ...landed, overHand: true }, controls), false);
  assert.equal(shouldFinishTurn({ ...landed, S: { ...landed.S, canEndHere: false } }, controls), false);
  assert.equal(shouldFinishTurn({ ...landed, myTurn: false }, controls), false);
  assert.equal(shouldFinishTurn({ ...landed, out: true }, controls), false);
  for (const c of [{ auto: true }, { readOnly: true }, { connected: false }]) {
    assert.equal(shouldFinishTurn(landed, { ...controls, ...c }), false);
  }
  assert.equal(shouldFinishTurn({ ...landed, S: { ...landed.S, phase: "ended" } }, controls), false);
});

test("[Stay] leaves OPS available for cards, with skip on the compact movement button", () => {
  const stay = { ...landed, S: { ...landed.S, step: 2 } };
  assert.equal(shouldFinishTurn(stay, controls), false, "never automatically skip OPS");
  assert.equal(movementControl(stay, controls), "end");
  assert.equal(movementControl({ ...stay, overHand: true }, controls), null, "discard excess cards first");
});

test("normal and delegated dice use the correct engine permission", () => {
  const ops = { ...landed, S: { ...landed.S, step: 2, canEndHere: false, canRollHere: true } };
  assert.equal(movementControl(ops, controls), "roll");
  assert.equal(movementControl({ ...ops, S: { ...ops.S, canRollHere: false } }, controls), null);
  assert.equal(movementControl({ ...ops, S: { ...ops.S, roller: 1 } }, controls), null, "wait for the delegated roller");
  assert.equal(movementControl({ ...ops, myTurn: false, S: { ...ops.S, canRollHere: false } }, controls), "roll", "roll for another seat");
  assert.equal(movementControl(landed, controls), null, "settlement is automatic, not another dice action");
});
