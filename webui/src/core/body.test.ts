// Run with: node --test webui/src/core/body.test.ts
// `splitTypeTag`: `[手]` / `[反击]` / `[持续]` are subtitles when they *open*
// a run, and ordinary prose when they appear inside one. The split must never
// reach into the sentence.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { itemSubs, splitTypeTag, type Item } from "./body.ts";

test("a leading type tag splits off as the subtitle", () => {
  assert.deepEqual(splitTypeTag("[反击] 当你被其他人的卡的效果影响时，你将此卡放置在自己场上。"), {
    tag: "[反击]",
    rest: " 当你被其他人的卡的效果影响时，你将此卡放置在自己场上。",
  });
  assert.deepEqual(splitTypeTag("[反击]当你将要向其他玩家支付时打出此卡"), {
    tag: "[反击]",
    rest: "当你将要向其他玩家支付时打出此卡",
  });
  assert.deepEqual(splitTypeTag("[手]：将此卡放置在[使用者]的[场地]"), {
    tag: "[手]",
    rest: "：将此卡放置在[使用者]的[场地]",
  });
});

test("a run that is nothing but the tag is a pure section header", () => {
  assert.deepEqual(splitTypeTag("[反击]"), { tag: "[反击]", rest: "" });
  assert.deepEqual(splitTypeTag("[持续]："), { tag: "[持续]", rest: "：" });
});

test("consecutive type tags all split off", () => {
  assert.deepEqual(splitTypeTag("[反击][持续]效果"), {
    tag: "[反击][持续]",
    rest: "效果",
  });
});

test("an inline mention is never transformed", () => {
  // Mid-sentence -- the tag is prose, not a header.
  assert.deepEqual(splitTypeTag("此卡可在你的移动掷骰后作为[反击]使用。"), {
    tag: "",
    rest: "此卡可在你的移动掷骰后作为[反击]使用。",
  });
  assert.deepEqual(splitTypeTag("打出[反击]卡的玩家可抽一张卡"), {
    tag: "",
    rest: "打出[反击]卡的玩家可抽一张卡",
  });
  assert.deepEqual(splitTypeTag("所有效果包含[奇迹水晶]的卡"), {
    tag: "",
    rest: "所有效果包含[奇迹水晶]的卡",
  });
  // A non-type tag at the start is also prose.
  assert.deepEqual(splitTypeTag("[拥有者]使用自己原有的技能（2）时"), {
    tag: "",
    rest: "[拥有者]使用自己原有的技能（2）时",
  });
});

test("the remainder is passed through verbatim", () => {
  const rest = splitTypeTag("[反击]：当你…").rest;
  assert.equal(rest, "：当你…");
  assert.equal(splitTypeTag("[反击]  两个空格").rest, "  两个空格");
});

// The display export (`dump_display.rs`) omits `sub` on a numbered effect that
// has no nested `1. / 2.` steps, so a body item read straight off the wire can
// be `{ text }` alone. `SkillBody` used to read `it.sub.length` unguarded --
// `Cannot read properties of undefined (reading 'length')` on every plain
// effect, which unmounted the tree and blanked the board.
test("a display-export item with no sub key reads as no sub-steps", () => {
  const bare = JSON.parse('{"text":"支付1000元。"}') as Item;
  assert.deepEqual(itemSubs(bare), []);
  // Explicit empty and populated forms stay as they are.
  assert.deepEqual(itemSubs({ text: "x", sub: [] }), []);
  assert.deepEqual(itemSubs({ text: "x", sub: ["a", "b"] }), ["a", "b"]);
});