// Run with: node --test webui/src/i18n/msgKey.test.ts
//
// A tile-mark kind **is** its i18n key (`trigger_why::mark_kind_name` ->
// `Msg::new(kind)`), so a key like `mark:cp` must resolve in `game`. i18next's
// `nsSeparator` would otherwise split it as namespace `mark` and every mark on
// the board would render as a raw key. These tests pin the lookup options and
// the locale entries the kind strings rely on.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import i18next from "i18next";
import { msgLookup } from "./msgKey.ts";

const LOCALES = fileURLToPath(new URL("locales/", import.meta.url));

/** Every tile-mark kind a shipped rule places, other than a card's own key. */
const MARK_KINDS = ["mark:cp", "mark:designate", "ember", "skillBlock", "noTarget", "skill.uikaIdol.pot"];

test("a mark kind stays a whole key in the game namespace", () => {
  assert.deepEqual(msgLookup("mark:cp"), { ns: "game", nsSeparator: false });
  assert.deepEqual(msgLookup("ember"), { ns: "game", nsSeparator: false });
  assert.deepEqual(msgLookup("log.outcome.mark_add"), { ns: "game", nsSeparator: false });
});

test("a card key keeps naming its own namespace", () => {
  assert.deepEqual(msgLookup("cards:card-mygo.anon_tokyo_mark"), { ns: undefined, nsSeparator: ":" });
});

test("every mark kind resolves to its display text in both locales", async () => {
  // A translation may legitimately equal the kind (`"ember"` -> `"ember"`), so
  // resolution is probed with a sentinel default, not by comparing to the key.
  const SENTINEL = "<<raw-key>>";
  for (const lng of ["zh-CN", "en"]) {
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
          ui: {},
          game,
          cards: JSON.parse(readFileSync(`${LOCALES}${lng}/cards.json`, "utf8")),
        },
      } as never,
    });
    for (const kind of MARK_KINDS) {
      const got = i18n.t(kind, { ...msgLookup(kind), defaultValue: SENTINEL }) as string;
      assert.notEqual(got, SENTINEL, `${lng}: mark kind ${kind} fell back to its raw key`);
      assert.ok(got.length > 0, `${lng}: mark kind ${kind} is empty`);
    }
  }
});

test("a card-namespaced mark kind still resolves", async () => {
  const lng = "zh-CN";
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
        ui: {},
        game: JSON.parse(readFileSync(`${LOCALES}${lng}/game.json`, "utf8")),
        // `loadCardLocales` flattens each bundle to `cards:<crate>.<key>`.
        cards: { "card-mygo.anon_tokyo_mark": "Anon Tokyo 连接" },
      },
    } as never,
  });
  const k = "cards:card-mygo.anon_tokyo_mark";
  assert.equal(i18n.t(k, { ...msgLookup(k), defaultValue: "RAW" }), "Anon Tokyo 连接");
});