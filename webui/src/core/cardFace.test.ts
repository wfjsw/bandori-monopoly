// Run with: node --test webui/src/core/cardFace.test.ts
// Pure card-face titles (no i18n runtime): how a row with no trimmed `name` is
// still named in the log -- the sheet's `rawName`, else `#<cell>`.
//
// The two real cases this exists for:
//   `HHW:（美咲）`   name="" rawName="（美咲）"  -> 「（美咲）」
//   `Mujica:#J11`   name="" rawName="" cell="J11" -> 「#J11」
// (the name every `j11_*` log line already uses, so the 「效果适用」 header
// matches the card's own narration instead of 「（未命名）」).

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { cardFaceTitle } from "./cardFace.ts";

const UNNAMED = "（未命名）";

test("a trimmed name wins", () => {
  assert.equal(cardFaceTitle({ name: "笑容大游行", rawName: "（kkr）笑容大游行", cell: "F3" }, UNNAMED), "笑容大游行");
});

test("an empty name falls back to the sheet's rawName", () => {
  assert.equal(cardFaceTitle({ name: "", rawName: "（美咲）", cell: "F15" }, UNNAMED), "（美咲）");
  assert.equal(cardFaceTitle({ rawName: "（喵梦）" }, UNNAMED), "（喵梦）");
});

test("no name at all titles the card after its sheet cell", () => {
  assert.equal(cardFaceTitle({ name: "", rawName: "", cell: "J11" }, UNNAMED), "#J11");
  assert.equal(cardFaceTitle({ cell: "L11" }, UNNAMED), "#L11");
});

test("only a row with none of the three is （未命名）", () => {
  assert.equal(cardFaceTitle({ name: "", rawName: "", cell: "" }, UNNAMED), UNNAMED);
  assert.equal(cardFaceTitle(undefined, UNNAMED), UNNAMED);
});