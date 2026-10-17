// Run with: node --test webui/src/i18n/stateLabel.test.ts
//
// The markers list labels a player's keyed state through `stateLabel`. A wire
// key is not its display phrase: it resolves via `board.state.*` (ui), the
// shared `log.state.*` phrase, or the key itself in `game`. These tests pin the
// mapping and the locale entries the markers list relies on, so a state key
// cannot reach the player as a raw wire spelling.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import i18next from "i18next";
import { msgLookup } from "./msgKey.ts";
import { STATE_LOG, stateKeyOf } from "./stateKey.ts";

const LOCALES = fileURLToPath(new URL("locales/", import.meta.url));

/** State keys the markers list has shown, wire spelling as the engine writes them. */
const MARKER_STATE_KEYS = [
  "charge",
  "handLimit",
  "lastWalk",
  "noRoad.who",
  "skill.mygo.over",
  "skill.mygo.start",
  "startHand",
  "stay",
  "stun",
  "exile",
  "ppFansUp",
];

test("a standard engine key shares the log's display phrase", () => {
  assert.equal(stateKeyOf("charge"), "log.state.charge");
  assert.equal(stateKeyOf("handLimit"), "log.state.hand_limit");
  assert.equal(stateKeyOf("startHand"), "log.state.start_hand");
  assert.equal(stateKeyOf("stunStart"), "log.state.stun");
  assert.equal(stateKeyOf("exileTo"), "log.state.exile");
});

test("a scratch key is its own game key", () => {
  assert.equal(stateKeyOf("skill.mygo.start"), "skill.mygo.start");
  assert.equal(stateKeyOf("skill.mygo.over"), "skill.mygo.over");
  assert.equal(stateKeyOf("unknown.key"), "unknown.key");
});

test("every marker state key resolves to its display text in both locales", async () => {
  // A translation may legitimately equal the lookup key, so resolution is
  // probed with a sentinel default, not by comparing to the key.
  const SENTINEL = "<<raw-key>>";
  for (const lng of ["zh-CN", "en"]) {
    const ui = JSON.parse(readFileSync(`${LOCALES}${lng}/ui.json`, "utf8"));
    const game = JSON.parse(readFileSync(`${LOCALES}${lng}/game.json`, "utf8"));
    const i18n = i18next.createInstance();
    await i18n.init({
      lng,
      fallbackLng: "zh-CN",
      defaultNS: "ui",
      ns: ["ui", "game", "cards"],
      keySeparator: false,
      nsSeparator: ":",
      interpolation: { escapeValue: false },
      returnNull: false,
      resources: {
        [lng]: {
          ui,
          game,
          cards: {},
        },
      } as never,
    });
    for (const key of MARKER_STATE_KEYS) {
      // Same chain `stateLabel` walks: `board.state.*` override, then the game key.
      const override = i18n.t(`board.state.${key}`, { ns: "ui", defaultValue: SENTINEL }) as string;
      const k = stateKeyOf(key);
      const got =
        override !== SENTINEL
          ? override
          : (i18n.t(k, { ...msgLookup(k), defaultValue: SENTINEL }) as string);
      assert.notEqual(got, SENTINEL, `${lng}: state key ${key} fell back to its raw wire key`);
      assert.ok(got.length > 0, `${lng}: state key ${key} is empty`);
    }
  }
});

test("every shared log phrase a state key maps to exists in both locales", () => {
  for (const lng of ["zh-CN", "en"]) {
    const game = JSON.parse(readFileSync(`${LOCALES}${lng}/game.json`, "utf8"));
    for (const [key, phrase] of Object.entries(STATE_LOG)) {
      assert.ok(game[phrase], `${lng}: ${key} maps to ${phrase}, which no locale carries`);
    }
  }
});