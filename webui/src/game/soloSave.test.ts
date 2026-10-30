// Unit tests for the solo save store. Run with:
//   node --test webui/src/game/soloSave.test.ts

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { installFakeIdb } from "../core/idbFake.ts";
import { clearSoloSave, hasSoloSave, hydrateSoloSave, loadSoloSave, saveSoloSave, type SoloSave } from "./soloSave.ts";

function save(over: Partial<SoloSave> = {}): SoloSave {
  return { v: 1, match: '{"world":1}', weights: { combat: 1, property: 1 }, recorded: false, last: 7, ...over };
}

/** Wait until the serialized write queue has settled. */
async function settle(): Promise<void> {
  for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 5));
}

test("a save round-trips through the store", async () => {
  installFakeIdb();
  saveSoloSave(save({ rec: "rec-1", replayId: "r1" }));
  assert.ok(hasSoloSave(), "existence flips synchronously");
  await settle();
  const got = await loadSoloSave();
  assert.equal(got?.match, '{"world":1}');
  assert.equal(got?.rec, "rec-1");
  assert.equal(got?.last, 7);
  assert.equal(got?.replayId, "r1");
});

test("only the newest of a burst is kept", async () => {
  installFakeIdb();
  for (let i = 0; i < 25; i++) saveSoloSave(save({ match: `{"i":${i}}`, last: i }));
  await settle();
  assert.equal((await loadSoloSave())?.match, '{"i":24}');
});

test("hydration reports an existing save without loading the payload", async () => {
  installFakeIdb();
  saveSoloSave(save());
  await settle();
  await hydrateSoloSave();
  assert.ok(hasSoloSave());
});

test("clear drops the save", async () => {
  installFakeIdb();
  saveSoloSave(save());
  await settle();
  clearSoloSave();
  assert.equal(hasSoloSave(), false);
  await settle();
  assert.equal(await loadSoloSave(), null);
});

test("a row lost behind our back loads as null and clears existence", async () => {
  const tables = installFakeIdb();
  saveSoloSave(save());
  await settle();
  tables.get("bm/saves")!.delete("solo");
  assert.equal(await loadSoloSave(), null);
  assert.equal(hasSoloSave(), false);
});

test("an unreadable row yields null", async () => {
  const tables = installFakeIdb();
  tables.set("bm/saves", new Map([["solo", { id: "solo", v: 1, z: true, match: new Uint8Array([1, 2, 3]), weights: {}, recorded: false, last: 0 }]]));
  assert.equal(await loadSoloSave(), null);
});
