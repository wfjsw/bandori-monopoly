// Regression guard for the built browser engine: play a whole seeded
// SoloMatch through the real wasm glue. Run with:
//   node --test webui/src/game/soloMatch.test.ts
//
// This is the test that would have caught the wasm clock read
// (`Instant::now` traps on wasm32-unknown-unknown in any host, node
// included): a solo match that only ran in the browser died at the first
// tick and no gate saw it. The path exercised is exactly the solo worker's —
// `new SoloMatch` + `set_fair` + `quick_start` + `tick_steps` to completion
// + `record_zst` — with the commit-reveal recipe along for the ride.
//
// Needs `node tools/build-glue.mjs` first (the deploy gate runs it before
// this). The card ruleset is NOT loaded: StubRules plays a complete game and
// the clock/engine paths are the same — `ruleset.test.ts` covers the cards.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const WEBUI = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const ROOT = resolve(WEBUI, "..");
const WASM_DIR = join(WEBUI, "src", "wasm");
const DATA_DIR = join(ROOT, "data");

/** One glue instance: import under a unique query (node's ESM cache keys on
 *  the URL, so `?tag` gives a fresh wasm instance), init it, feed the game
 *  tables and the glue identity. Mirrors `ruleset.test.ts` /
 *  `tools/test-replay-archive.mjs` and the page's own boot
 *  (`webui/src/core/data.ts`). */
async function loadGlue(tag: string): Promise<Record<string, any>> {
  const glueJs = join(WASM_DIR, "glue.js");
  const glueWasm = join(WASM_DIR, "glue_bg.wasm");
  assert.ok(existsSync(glueJs) && existsSync(glueWasm), "run tools/build-glue.mjs");
  const glue = (await import(pathToFileURL(glueJs).href + "?" + tag)) as Record<string, any>;
  await glue.default({ module_or_path: new Uint8Array(readFileSync(glueWasm)) });
  const names: string[] = JSON.parse(glue.data_files());
  const files: Record<string, string> = {};
  for (const n of names) {
    const p = join(DATA_DIR, n);
    if (existsSync(p)) files[n] = readFileSync(p, "utf8");
  }
  glue.load_data(JSON.stringify(files));
  // The engine-bundle identity the page passes at boot (`docs/REPLAY.md` §9):
  // a record whose stamp names no bundle is refused by the fairness
  // verifier's `bundle` step.
  const id = JSON.parse(readFileSync(join(WASM_DIR, "engine_id.json"), "utf8")) as {
    glueSha256?: string;
  };
  if (id.glueSha256 && typeof glue.set_glue_sha === "function") {
    glue.set_glue_sha(id.glueSha256);
  }
  return glue;
}

/** The solo worker's openings (`soloWorker.ts` `soloFair`), against this glue. */
function soloFair(glue: Record<string, any>, members: unknown[], mode: number, human: number) {
  const hex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
  const seed = hex(crypto.getRandomValues(new Uint8Array(32)));
  const salt = hex(crypto.getRandomValues(new Uint8Array(32)));
  const nonce = hex(crypto.getRandomValues(new Uint8Array(32)));
  const stamp = JSON.parse(glue.engine_stamp()) as { bundle?: string; ruleset_sha256?: string };
  const settings = glue.fair_canon_settings(mode, 0.05, JSON.stringify({}), JSON.stringify(members));
  const commit = glue.fair_commit(seed, salt, stamp.bundle ?? "", stamp.ruleset_sha256 ?? "");
  const derived = glue.fair_derive_seed(seed, JSON.stringify([{ member: human, nonce }]));
  assert.equal(commit.length, 64);
  assert.equal(derived.length, 64);
  return {
    derived,
    open: { v: 2, commit, seed, salt, nonces: [{ member: human, nonce }], settings },
  };
}

// All-bot table: the match plays itself to completion without a human
// pressing anything (the clock / engine path is what this guards).
const members = [
  { id: 1, player: "P1", character: "", cnId: "", ready: true, host: true, bot: true, away: false, mentality: "standard" },
  { id: 2, player: "P2", character: "", cnId: "", ready: true, host: false, bot: true, away: false, mentality: "standard" },
  { id: 3, player: "P3", character: "", cnId: "", ready: true, host: false, bot: true, away: false, mentality: "standard" },
];

test("a seeded SoloMatch plays to completion in the built wasm glue and seals a record", async () => {
  const glue = await loadGlue("solo-match");
  const fair = soloFair(glue, members, 0, 1);
  const m = new glue.SoloMatch(JSON.stringify(members), fair.derived, 0, JSON.stringify({}));
  m.set_fair(JSON.stringify(fair.open));
  m.quick_start();

  // Tick it out the way the solo worker does. A wasm-side clock read or any
  // other trap in the engine dies right here instead of in a player's game.
  let steps = 0;
  while (!m.ended()) {
    m.tick_steps(4);
    steps += 1;
    assert.ok(steps < 200_000, `game did not finish in ${steps} tick calls`);
  }

  const zst = m.record_zst("2026-10-09 12:00");
  assert.ok(zst instanceof Uint8Array && zst.length > 100, `record_zst: ${zst?.length}`);

  // The sealed record opens its fairness material under the same recipe.
  const json = new TextDecoder().decode(glue.zst_decompress(zst, 64 * 1024 * 1024));
  const file = JSON.parse(json);
  assert.equal(file.header.fair.v, 2);
  assert.equal(file.header.fair.commit, fair.open.commit);
  const rep = JSON.parse(glue.verify_fair_hash(zst));
  assert.equal(rep.present, true);
  assert.ok(rep.ok, `verify_fair_hash failed: ${JSON.stringify(rep.steps)}`);

  // And the engine half agrees the log replays on those openings. (A second
  // full run -- worth it, this is the check the whole scheme exists for.)
  const full = JSON.parse(glue.verify_fair(zst));
  assert.ok(full.ok, `verify_fair failed: ${JSON.stringify(full.steps)}`);
  void steps;
});