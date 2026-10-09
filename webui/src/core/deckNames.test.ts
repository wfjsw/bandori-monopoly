// Run with: node --test webui/src/core/deckNames.test.ts
// Pure deck-list helpers (no i18n runtime): name display, sanitize, parse.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { DECK_NAME_MAX, deckLabelWith, parseDeckList, sanitizeDeckName } from "./deckNames.ts";

const auto = (n: number) => `Deck ${n}`;

test("an empty name shows the localized auto name from the deck id", () => {
  assert.equal(deckLabelWith({ id: 1, name: "" }, auto), "Deck 1");
  assert.equal(deckLabelWith({ id: 7, name: "   " }, auto), "Deck 7", "blank is auto");
});

test("a user name wins and keeps duplicates", () => {
  assert.equal(deckLabelWith({ id: 2, name: "要乐奈专用" }, auto), "要乐奈专用");
  assert.equal(deckLabelWith({ id: 3, name: "要乐奈专用" }, auto), "要乐奈专用");
  assert.equal(deckLabelWith({ id: 4, name: " 快攻 " }, auto), "快攻", "trimmed for display");
});

test("sanitize trims, caps by characters and blanks to auto", () => {
  assert.equal(sanitizeDeckName("  a  "), "a");
  assert.equal(sanitizeDeckName(" \t "), "");
  assert.equal(sanitizeDeckName("あ".repeat(DECK_NAME_MAX + 5)).length, DECK_NAME_MAX);
  assert.equal([...sanitizeDeckName("你好世界")].length, 4, "CJK counts as one each");
});

test("parseDeckList reads the glue shape and rejects junk", () => {
  assert.deepEqual(parseDeckList('[{"id":1,"name":"","cards":["a"]}]'), [
    { id: 1, name: "", cards: ["a"] },
  ]);
  assert.deepEqual(parseDeckList("null"), []);
  assert.deepEqual(parseDeckList("{}"), []);
});