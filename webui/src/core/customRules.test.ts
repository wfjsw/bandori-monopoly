// Unit tests for the custom-ruleset store. Run with:
//   node --test webui/src/core/customRules.test.ts

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { installFakeIdb } from "./idbFake.ts";
import {
  currentCustomRules,
  customRulesHash,
  forgetCustomRules,
  hydrateCustomRules,
  storeCustomRules,
} from "./customRules.ts";

test("a stored ruleset survives into a fresh hydration", async () => {
  installFakeIdb();
  assert.equal(await storeCustomRules(new Uint8Array([1, 2, 3]), "mine.wasm"), true);
  assert.deepEqual([...currentCustomRules()!.bytes], [1, 2, 3]);
  await hydrateCustomRules();
  assert.equal(currentCustomRules()?.name, "mine.wasm");
  await forgetCustomRules();
});

test("store and forget keep memory and the row in step", async () => {
  installFakeIdb();
  await storeCustomRules(new Uint8Array([4]), "a.wasm");
  await forgetCustomRules();
  assert.equal(currentCustomRules(), null);
});

test("the hash separates same-length modules", () => {
  const a = customRulesHash({ name: "x.wasm", bytes: new Uint8Array([1, 2, 3]) });
  const b = customRulesHash({ name: "x.wasm", bytes: new Uint8Array([1, 2, 4]) });
  assert.notEqual(a, b);
  assert.equal(a, customRulesHash({ name: "x.wasm", bytes: new Uint8Array([1, 2, 3]) }));
  assert.equal(customRulesHash(null), "");
});

test("with no indexedDB the module still lives in memory", async () => {
  (globalThis as { indexedDB?: unknown }).indexedDB = undefined;
  assert.equal(await storeCustomRules(new Uint8Array([9]), "n.wasm"), false);
  assert.equal(currentCustomRules()?.name, "n.wasm");
  await forgetCustomRules();
  assert.equal(currentCustomRules(), null);
});
