// Unit tests for the mirrored key-value tables. Run with:
//   node --test webui/src/core/kv.test.ts

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { installFakeIdb } from "./idbFake.ts";
import { hydrateKv, kv } from "./kv.ts";

test("a write is readable at once and reloads after hydration", async () => {
  const tables = installFakeIdb();
  const prefs = kv("prefs");
  prefs.set("lang", "en");
  assert.equal(prefs.get("lang"), "en", "synchronous mirror");
  await new Promise((r) => setTimeout(r, 10));
  assert.equal(tables.get("bm/prefs")?.get("lang")?.value, "en");
  // A fresh table object over the same store would normally start empty; the
  // hydrate reads what the previous view wrote.
  await hydrateKv("prefs");
  assert.equal(kv("prefs").get("lang"), "en");
});

test("remove drops both mirror and row", async () => {
  const tables = installFakeIdb();
  const prefs = kv("prefs");
  prefs.set("k", "v");
  await new Promise((r) => setTimeout(r, 10));
  prefs.remove("k");
  assert.equal(prefs.get("k"), "");
  await new Promise((r) => setTimeout(r, 10));
  assert.equal(tables.get("bm/prefs")?.has("k"), false);
});

test("an absent key reads as the empty string", () => {
  installFakeIdb();
  assert.equal(kv("account").get("missing"), "");
});
