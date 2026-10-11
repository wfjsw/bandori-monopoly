// Run with: node --test webui/src/scenes/board/anim.test.ts
// Ruling 1 + the field-pulse reconciliation: a field card with a self-only
// effect neither pulses nor flashes; a cross-player effect pulses once.
// The decision never reads the header position, `results`, or `parent`.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { activationPresentation } from "./presentation.ts";
import type { PresentationEvent } from "./presentation.ts";

function ev(over: Partial<PresentationEvent>): PresentationEvent {
  return {
    id: 1,
    type: "card",
    playerId: 0,
    other: -1,
    value: 0,
    from: 0,
    to: 0,
    dice: 0,
    card: "TEST:field",
    msg: { k: "log.effect_applied" } as PresentationEvent["msg"],
    kind: "hook",
    negated: false,
    parent: -1,
    results: [],
    uid: 7,
    flash: true,
    ...over,
  };
}

test("a field card with a self-only effect neither pulses nor flashes", () => {
  const e = ev({ flash: false });
  assert.equal(activationPresentation(e), "none");
});

test("a field card with a cross-player effect pulses once (field slot)", () => {
  const e = ev({ flash: true, uid: 7, msg: { k: "log.effect_applied" } as PresentationEvent["msg"] });
  assert.equal(activationPresentation(e), "pulse");
});

test("a hand play flashes the centre face (no field uid)", () => {
  const e = ev({ uid: -1, msg: { k: "" } as PresentationEvent["msg"], kind: "play" });
  assert.equal(activationPresentation(e), "flash");
});

test("a negated activation still shows (无效 is not suppressed by flash)", () => {
  const e = ev({ flash: false, negated: true });
  assert.notEqual(activationPresentation(e), "none");
});

test("the decision never reads header position / results / parent", () => {
  // Two events identical except for parent / results / id must decide alike.
  const a = ev({ flash: true });
  const b = ev({ flash: true, parent: 42, id: 99, results: [{ k: "log.effect_nothing" } as never] });
  assert.equal(activationPresentation(a), activationPresentation(b));
});