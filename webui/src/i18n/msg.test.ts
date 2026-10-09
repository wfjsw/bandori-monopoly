// Run with: node --test webui/src/i18n/msg.test.ts
// `msgCards` / `optionCard` live in returnsSelection.ts (next to each other,
// both pure message inspection) so this can run without the i18n runtime.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import type { Msg } from "./msg.ts";
import { msgCards, optionCard } from "../scenes/board/returnsSelection.ts";

test("a pure card option names its card", () => {
  const o: Msg = { k: "ask.cardOption", a: { card: { card: "PPP:Popipa" } } };
  assert.deepEqual(msgCards(o), ["PPP:Popipa"]);
});

test("an option that names a player and a card still yields the card", () => {
  // Returns 「复制哪名玩家的团卡？」 -- the pill shows the 团卡 beside the name.
  const o: Msg = {
    k: "ask.player",
    a: { who: { playerId: 1 }, card: { card: "skill:Afterglow:商店街的宠儿" } },
  };
  assert.deepEqual(msgCards(o), ["skill:Afterglow:商店街的宠儿"]);
});

test("a prompt body names its cards, including nested detail messages", () => {
  const text: Msg = {
    k: "ask.counteract.text",
    a: {
      detail: {
        msg: {
          k: "ask.counteract.detail",
          a: { who: { playerId: 0 }, card: { card: "MyGO:无路矢" }, n: { i: 3 } },
        },
      },
    },
  };
  assert.deepEqual(msgCards(text), ["MyGO:无路矢"]);
});

test("several cards are all collected, in order and without repeats", () => {
  const o: Msg = {
    k: "x",
    a: {
      first: { card: "A" },
      second: { card: "B" },
      again: { card: "A" },
    },
  };
  assert.deepEqual(msgCards(o), ["A", "B"]);
});

test("a message that names no card yields nothing", () => {
  assert.deepEqual(msgCards({ k: "ask.buy.yes" }), []);
  assert.deepEqual(msgCards({ k: "ask.player", a: { who: { playerId: 2 } } }), []);
  assert.deepEqual(msgCards(null), []);
});

test("a pure card option is a card choice; a named card inside a label is not", () => {
  // ask.cardOption / Returns' eight-pick: the option IS the card -> card grid.
  assert.equal(optionCard({ k: "ask.cardOption", a: { card: { card: "PPP:Popipa" } } }), "PPP:Popipa");
  // 「打出「{{card}}」」 / 「复制…团卡」: the option names a card -> mini face in
  // the pill, not a whole grid of faces.
  assert.equal(optionCard({ k: "ask.counteract.play", a: { card: { card: "MyGO:无路矢" } } }), "MyGO:无路矢");
  assert.equal(
    optionCard({ k: "ask.player", a: { who: { playerId: 1 }, card: { card: "skill:Afterglow:商店街的宠儿" } } }),
    null,
  );
});