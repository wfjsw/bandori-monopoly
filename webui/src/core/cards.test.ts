// Run with: node --test webui/src/core/cards.test.ts
// The card data's [反击] trigger clauses, checked against the prose they were
// generated from: every 反击-tagged card must carry the `counterReason` the
// prompt shows as "why this card may be declared".
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { test } from "node:test";

interface CardRow {
  id: string;
  tags: string[];
  text: string;
  counterReason?: string;
}

const cards: CardRow[] = JSON.parse(
  readFileSync(new URL("../../../data/cards.json", import.meta.url), "utf-8"),
).cards;

test("every [反击] card names its trigger-condition clause", () => {
  const tagged = cards.filter((c) => c.tags.includes("反击"));
  assert.ok(tagged.length > 0, "the set does carry [反击] cards");
  const missing = tagged.filter((c) => !c.counterReason?.trim()).map((c) => c.id);
  assert.deepEqual(missing, [], "each needs a `counterReason` (why it is offered)");
});

test("a counterReason is a clause of the card's own prose, not a new sentence", () => {
  // The field is extracted from / hand-written against `text`; it must appear
  // there (punctuation and the 「当…时」 wrapper may be stripped off).
  const off = cards
    .filter((c) => c.counterReason)
    .filter((c) => {
      const t = c.text.replace(/【/g, "[").replace(/】/g, "]");
      const r = c.counterReason!.replace(/【/g, "[").replace(/】/g, "]");
      return !t.includes(r.replace(/^(任意时刻当|任意时刻|当)/, "").replace(/时$/, ""));
    })
    .map((c) => c.id);
  assert.deepEqual(off, []);
});