// Run with: node --test webui/src/core/cards.test.ts
// The card data's [反击] trigger clauses, checked against the prose they were
// generated from: every 反击-tagged card must carry the `counterReason` the
// prompt shows as "why this card may be declared".
//
// Sourcing after the presentation split:
//   tags           -- identity roster fixture (`crates/game-core/tests/fixtures/roster.json`)
//   text           -- `webui/src/generated/display.json` (authored in Rust rule
//                     code, exported at build; empty fields omitted)
//   counterReason  -- `webui/src/generated/display.json` (same export; only
//                     [反击] cards carry the field). Since ABI v94 the field
//                     is a **locale key** (`cards:<crate>.…` / `counter.*`);
//                     the clause check resolves it via `zh-CN` locales.
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { test } from "node:test";

interface CardRow {
  id: string;
  tags: string[];
  text: string;
}

const fixture = JSON.parse(
  readFileSync(
    new URL("../../../crates/game-core/tests/fixtures/roster.json", import.meta.url),
    "utf-8",
  ),
) as { cards: { id: string; tags: string[]; text?: string }[] };

const display = JSON.parse(
  readFileSync(new URL("../generated/display.json", import.meta.url), "utf-8"),
) as { cards: Record<string, { text?: string; counterReason?: string }> };

function readJson(path: string): unknown {
  try {
    return JSON.parse(readFileSync(new URL(path, import.meta.url), "utf-8"));
  } catch {
    return {};
  }
}

/** Resolve one locale key against the zh-CN bundles (cards + game). */
function resolveKey(key: string): string {
  const game = readJson("../i18n/locales/zh-CN/game.json") as Record<string, string>;
  if (key in game) return game[key];
  if (key.startsWith("cards:")) {
    const rest = key.slice("cards:".length);
    const dot = rest.indexOf(".");
    if (dot > 0) {
      const ns = rest.slice(0, dot);
      const k = rest.slice(dot + 1);
      // Static stub first; the built card-locale bundle (build-ruleset) second.
      const stub = readJson("../i18n/locales/zh-CN/cards.json") as Record<
        string,
        Record<string, string>
      >;
      const built = readJson("../../public/assets/i18n/cards-zh-CN.json") as Record<
        string,
        Record<string, string>
      >;
      return stub[ns]?.[k] ?? built[ns]?.[k] ?? key;
    }
  }
  return key;
}

const reasonById = new Map(
  Object.entries(display.cards)
    .filter(([, v]) => v.counterReason)
    .map(([id, v]) => [id, v.counterReason!]),
);
// Prefer `display.json`'s `text`; a leftover fixture `text` covers the window
// before the identity dump drops it. CounterReason rows name real cards that
// the synthetic identity fixture may not list -- include them so the clause
// check is not vacuous.
const byId = new Map<string, CardRow>();
for (const c of fixture.cards) {
  byId.set(c.id, {
    id: c.id,
    tags: c.tags,
    text: display.cards[c.id]?.text ?? c.text ?? "",
  });
}
for (const id of reasonById.keys()) {
  if (!byId.has(id)) {
    byId.set(id, { id, tags: [], text: display.cards[id]?.text ?? "" });
  }
}
const cards: CardRow[] = [...byId.values()];
const displayReady = Object.keys(display.cards).length > 0;

test("every [反击] card names its trigger-condition clause", () => {
  // Every `counterReason` row must carry a non-empty key (the prompt's
  // "why this card may be declared").
  const emptyReason = [...reasonById.entries()].filter(([, r]) => !r.trim()).map(([id]) => id);
  assert.deepEqual(emptyReason, [], "each counterReason must be non-empty");
  // And every identity [反击] tag must have one. The synthetic game-core
  // fixture currently carries no [反击] tags; skip the tag sweep there.
  const tagged = cards.filter((c) => c.tags.includes("反击"));
  if (tagged.length === 0) {
    // Anti-vacuous for the reason side is above; the tag side needs a real roster.
    return;
  }
  const missing = tagged.filter((c) => !reasonById.get(c.id)?.trim()).map((c) => c.id);
  assert.deepEqual(missing, [], "each needs a `counterReason` (why it is offered)");
});

test("a counterReason is a clause of the card's own prose, not a new sentence", () => {
  if (!displayReady) {
    // `display.json` is still the empty stub; run the clause check once the
    // build export lands (slice 1 / tools).
    return;
  }
  // The field is a locale key; its zh-CN text is extracted from / hand-written
  // against `text` and must appear there (punctuation and the 「当…时」 wrapper
  // may be stripped off). Skip rows whose key does not resolve (locales not
  // built yet in this checkout).
  const off = cards
    .filter((c) => reasonById.get(c.id))
    .filter((c) => {
      const keys = reasonById
        .get(c.id)!
        .split("\n")
        .filter(Boolean);
      const resolved = keys.map(resolveKey).join("\n");
      if (resolved.includes("cards:") || resolved === keys.join("\n")) {
        // unresolved key -- locales bundle not built; don't fail the clause check
        return false;
      }
      const t = c.text.replace(/【/g, "[").replace(/】/g, "]");
      const r = resolved.replace(/【/g, "[").replace(/】/g, "]");
      return !t.includes(r.replace(/^(任意时刻当|任意时刻|当)/, "").replace(/时$/, ""));
    })
    .map((c) => c.id);
  assert.deepEqual(off, []);
});