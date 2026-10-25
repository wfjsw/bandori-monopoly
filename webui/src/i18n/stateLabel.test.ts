// Run with: node --test webui/src/i18n/stateLabel.test.ts
//
// The markers list labels a player's keyed state through `stateLabel`. A wire
// key is not its display phrase: it resolves via `board.state.*` (ui), the
// shared `log.state.*` phrase, or the key itself in `game`. These tests pin the
// mapping and the locale entries the markers list relies on, so a state key
// cannot reach the player as a raw wire spelling.
//
// The probe walks the chain against a bare i18next instance (same steps
// `stateLabel` takes) because `stateLabel` itself pulls the app i18n runtime,
// which wants `localStorage`. `fallbackLabel` is the one live import -- it is
// the step that decides whether an unresolved key is dropped or is already its
// display name.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import i18next from "i18next";
import { msgLookup } from "./msgKey.ts";
import { STATE_LOG, fallbackLabel, stateKeyOf } from "./stateKey.ts";

const LOCALES = fileURLToPath(new URL("locales/", import.meta.url));
const SENTINEL = "<<raw-key>>";

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
  "sports_talent_next",
  "fate_hit",
  "no_expectation_stacks",
  "want_human_x",
  "parking_space_stay",
];

/** Card scratch the markers list has shown. Either it carries a phrase or the
 *  row is dropped -- what must never happen is the wire spelling on screen. */
const SCRATCH_KEYS = [
  "sports_talent_next",
  "fate_hit",
  "change_world_turn",
  "chuchu_music_user",
  "layerUnused",
  "guerrilla_armed",
  "here_held_len",
  "tomorrows_door_step",
  "want_human_ab_before",
  "layer_keep_count",
];

/** Mark kinds and kin name themselves (「水母标记」, 「PAREO标记」). */
const DISPLAY_NAME_KEYS = ["水母标记", "PAREO标记", "角色标记:灯"];

async function probe(lng: string): Promise<(key: string) => string> {
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
    resources: { [lng]: { ui, game, cards: {} } } as never,
  });
  return (key: string) => {
    // Same chain `stateLabel` walks: `board.state.*` override, then the game key.
    const override = i18n.t(`board.state.${key}`, { ns: "ui", defaultValue: SENTINEL }) as string;
    if (override !== SENTINEL) return override;
    const k = stateKeyOf(key);
    return i18n.t(k, { ...msgLookup(k), defaultValue: SENTINEL }) as string;
  };
}

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
  for (const lng of ["zh-CN", "en"]) {
    const resolve = await probe(lng);
    for (const key of MARKER_STATE_KEYS) {
      const got = resolve(key);
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

test("a bare wire key with no phrase is dropped, never printed", async () => {
  for (const lng of ["zh-CN", "en"]) {
    const resolve = await probe(lng);
    for (const key of SCRATCH_KEYS) {
      const got = resolve(key);
      if (got === SENTINEL) {
        // No phrase: `stateLabel` falls through to `fallbackLabel`, which must
        // drop a bare wire key rather than hand the key to the renderer.
        assert.equal(fallbackLabel(key), null, `${lng}: state key ${key} would print as its raw wire key`);
        continue;
      }
      assert.notEqual(got, key, `${lng}: state key ${key} leaked as its raw wire key`);
      assert.ok(got.length > 0, `${lng}: state key ${key} is empty`);
    }
    // The two the markers list used to spell out verbatim now carry phrases.
    for (const key of ["sports_talent_next", "fate_hit", "no_expectation_stacks", "want_human_x", "parking_space_stay"]) {
      assert.notEqual(resolve(key), SENTINEL, `${lng}: state key ${key} lost its phrase`);
      assert.notEqual(resolve(key), key);
    }
    // Bookkeeping a card keeps under its own slot has no phrase and no row.
    for (const key of ["change_world_turn", "chuchu_music_user", "here_held_len", "tomorrows_door_step"]) {
      assert.equal(resolve(key), SENTINEL, `${lng}: state key ${key} unexpectedly carries a phrase`);
      assert.equal(fallbackLabel(key), null);
    }
  }
});

test("a mark kind that is its display name falls back to that name", () => {
  for (const key of DISPLAY_NAME_KEYS) {
    assert.equal(fallbackLabel(key), key);
  }
});

test("an on-card counter resolves through the shared log counter phrase", async () => {
  for (const lng of ["zh-CN", "en"]) {
    const game = JSON.parse(readFileSync(`${LOCALES}${lng}/game.json`, "utf8"));
    const i18n = i18next.createInstance();
    await i18n.init({
      lng,
      fallbackLng: "zh-CN",
      defaultNS: "game",
      ns: ["game"],
      keySeparator: false,
      nsSeparator: ":",
      interpolation: { escapeValue: false },
      returnNull: false,
      resources: { [lng]: { game } } as never,
    });
    const resolve = (name: string) => {
      const k = `log.counter.${name}`;
      const s = i18n.t(k, { ...msgLookup(k), defaultValue: SENTINEL }) as string;
      return s === SENTINEL ? fallbackLabel(name) : s;
    };
    for (const name of ["crystals", "points", "bonus"]) {
      assert.notEqual(resolve(name), SENTINEL, `${lng}: counter ${name} has no phrase`);
    }
    // Unknown wire spelling is dropped; the log's `log.counter.other` quoting
    // is for the engine, not for the markers list.
    for (const name of ["layer_keep_count", "chuchu_music_hits"]) {
      assert.equal(resolve(name), null, `${lng}: counter ${name} would print as its raw wire key`);
    }
  }
});

test("the hand-limit sentinel is not a count", () => {
  const NO_HAND_LIMIT = 2147483647;
  assert.equal(NO_HAND_LIMIT, 0x7fffffff);
  // The markers list maps the sentinel to `∞`, matching the hand tab's own
  // `board.hand` (`{{n}}/{{max}}` with `max: limit ?? "∞"`), not the i32 max.
  const ui = JSON.parse(readFileSync(`${LOCALES}zh-CN/ui.json`, "utf8"));
  assert.equal(ui["board.hand"], "手牌 {{n}}/{{max}}");
});
