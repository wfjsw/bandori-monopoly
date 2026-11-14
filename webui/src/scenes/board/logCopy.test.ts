// Run with: node --test webui/src/scenes/board/logCopy.test.ts
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { copiedLogText } from "./logCopy.ts";

const FULL = "turn 1\nplayed a card\nended turn";

test("a selection copies only the selected text", () => {
  assert.equal(copiedLogText(FULL, "played a card"), "played a card");
});

test("a partial-line selection keeps exactly what was selected", () => {
  assert.equal(copiedLogText(FULL, "played"), "played");
});

test("no selection copies the whole log", () => {
  assert.equal(copiedLogText(FULL, null), FULL);
});

test("a whitespace-only selection is kept, not treated as no selection", () => {
  assert.equal(copiedLogText(FULL, " "), " ");
});
