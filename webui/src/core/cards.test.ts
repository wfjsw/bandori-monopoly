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
//                     [反击] cards carry the field)
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
  // Every `counterReason` row must carry a non-empty reason (the prompt's
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
  // The field is extracted from / hand-written against `text`; it must appear
  // there (punctuation and the 「当…时」 wrapper may be stripped off).
  const off = cards
    .filter((c) => reasonById.get(c.id))
    .filter((c) => {
      const t = c.text.replace(/【/g, "[").replace(/】/g, "]");
      const r = reasonById.get(c.id)!.replace(/【/g, "[").replace(/】/g, "]");
      return !t.includes(r.replace(/^(任意时刻当|任意时刻|当)/, "").replace(/时$/, ""));
    })
    .map((c) => c.id);
  assert.deepEqual(off, []);
});