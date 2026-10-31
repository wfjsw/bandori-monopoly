// Engine-bundle archive: end-to-end checks with the real glue in node
// (`docs/REPLAY.md` §9). Run with:
//
//   node --test tools/test-replay-archive.mjs
//
// 1. The JS bundle-id recipe and the engine's own `record::bundle_id` agree.
// 2. `tools/archive-engine.mjs` produces a loadable bundle whose frozen
//    engine reports exactly the stamp the archive index records -- including
//    `data_sha256`, which is `""` on every record this build writes
//    (`docs/REPLAY.md` §9.1) and therefore in every bundle id.
// 3. A record sealed by build A replays clean through the archived A engine.
//    Host tables do **not** fork that: the board and rule params ride in the
//    record's `MatchSetup`, so a different `data/` is the same bundle and the
//    same replay.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { zstdDecompressSync } from "node:zlib";

import { bundleId, engineBundleId, glueSha256, healEmptyDataAliases, stampKey } from "./engine-bundle.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const TMP = join(ROOT, "target", "scratch", "test-engine-bundle");

// ---------------------------------------------------------------- helpers

/** One engine instance: import the glue under a unique query (node's ESM cache
 *  keys on the URL, so `?tag` gives a fresh wasm instance), init it, and feed
 *  it the data files. This is what `webui/public/assets/engine/replay-worker.js`
 *  does for an archived bundle. */
async function loadGlue({ glueJs, glueWasm, dataDir, tag, glueSha, rules }) {
  const g = await import(pathToFileURL(glueJs).href + "?" + tag);
  await g.default({ module_or_path: new Uint8Array(readFileSync(glueWasm)) });
  const names = JSON.parse(g.data_files());
  const files = {};
  for (const n of names) {
    const p = join(dataDir, n);
    if (existsSync(p)) files[n] = readFileSync(p, "utf8");
  }
  g.load_data(JSON.stringify(files));
  if (rules) {
    // `rules: { index, modulesDir }` -- an archived bundle keeps the index in
    // the bundle and the content-addressed modules in the shared pool. The
    // precompiled guard conditions (`conds-<sha>.bin`, docs/GUARDS.md §8.2)
    // ride beside the index in the bundle; older bundles have neither.
    const index = JSON.parse(readFileSync(rules.index, "utf8"));
    for (const m of index.modules ?? []) {
      g.ruleset_add(new Uint8Array(readFileSync(join(rules.modulesDir, m.file))));
    }
    if (index.conds?.file) {
      const beside = join(dirname(rules.index), index.conds.file);
      const cp = existsSync(beside) ? beside : join(rules.modulesDir, index.conds.file);
      if (!existsSync(cp)) throw new Error(`precompiled conds blob missing: ${cp}`);
      g.ruleset_precompiled(new Uint8Array(readFileSync(cp)));
    }
    g.ruleset_build();
  }
  if (glueSha && typeof g.set_glue_sha === "function") g.set_glue_sha(glueSha);
  return g;
}

/** A short all-bot solo match, sealed as `.bdrec` bytes (zstd). Three seats:
 *  casual mode is 3–10 players (`G-FLOW-12`). */
function recordSomeGame(g, seed) {
  const members = [
    { id: 1, player: "P1", bot: true },
    { id: 2, player: "P2", bot: true },
    { id: 3, player: "P3", bot: true },
  ];
  const m = new g.SoloMatch(JSON.stringify(members), seedHex(seed), 1, "");
  try {
    m.quick_start();
    for (let i = 0; i < 400; i++) {
      m.tick_steps(10);
      if (m.ended()) break;
    }
    return { bytes: m.record_zst("2026-10-07 00:00"), ended: m.ended() };
  } finally {
    m.free();
  }
}

/** `SoloMatch::new` wants the 256-bit seed as hex (`docs/FAIRNESS.md`); tools
 *  just need a deterministic one. */
const seedHex = (n) => (n >>> 0).toString(16).padStart(64, "0");

function recordJson(bytes) {
  return JSON.parse(zstdDecompressSync(bytes).toString("utf8"));
}

// ---------------------------------------------------------------- tests

test("pre-fix archive entries gain the id a record names", () => {
  // Sealed before d481e521 landed in the tools: `data_sha256` held a hash of
  // the boot tables, so the stored id is not what `engine_stamp()` writes into
  // a record. The heal maps the record's lookup key onto the stored id.
  const stamp = {
    format: 3,
    save_version: 12,
    abi: 95,
    ruleset_sha256: "r".repeat(64),
    data_sha256: "d".repeat(64), // the old (wrong) recipe
    engine: "game-core",
    build: "0.1.0",
  };
  const gsha = "g".repeat(64);
  const storedId = bundleId(gsha, stamp);
  const recordId = engineBundleId(gsha, stamp);
  assert.notEqual(recordId, storedId, "the two recipes disagree (that is the bug)");
  assert.equal(recordId, bundleId(gsha, { ...stamp, data_sha256: "" }));

  const index = { version: 1, current: storedId, bundles: [{ id: storedId, glueSha256: gsha, stamp, files: {} }] };
  assert.equal(healEmptyDataAliases(index), 1, "one entry needed the alias");
  assert.equal(index.bundles[0].aliases[recordId], storedId, "record id -> stored id");
  // Idempotent, and a post-fix entry (already empty) is left alone.
  assert.equal(healEmptyDataAliases(index), 0, "second pass is a no-op");
  const fixed = { bundles: [{ id: recordId, glueSha256: gsha, stamp: { ...stamp, data_sha256: "" } }] };
  assert.equal(healEmptyDataAliases(fixed), 0, "an empty-data entry needs no alias");
  assert.equal(fixed.bundles[0].aliases, undefined);
});

test("JS recipe and the engine's record::bundle_id agree", async () => {
  const wasmDir = join(ROOT, "webui", "src", "wasm");
  const gsha = glueSha256(readFileSync(join(wasmDir, "glue.js")), readFileSync(join(wasmDir, "glue_bg.wasm")));
  const g = await loadGlue({
    glueJs: join(wasmDir, "glue.js"),
    glueWasm: join(wasmDir, "glue_bg.wasm"),
    dataDir: join(ROOT, "data"),
    tag: "recipe",
    glueSha: gsha,
  });
  const stamp = JSON.parse(g.engine_stamp());
  assert.equal(stamp.bundle, bundleId(gsha, stamp), "engine stamp.bundle == JS bundleId");
  assert.equal(stamp.bundle.length, 64);
  // No glue identity -> unknown bundle, never a wrong one.
  g.set_glue_sha("");
  assert.equal(JSON.parse(g.engine_stamp()).bundle, "");
});

test("archive-engine.mjs produces a loadable bundle of the current build", async () => {
  const archive = join(TMP, "archive");
  rmSync(archive, { recursive: true, force: true });
  execFileSync(process.execPath, [join(ROOT, "tools", "archive-engine.mjs"),
    "--archive", archive, "--no-dist"], { cwd: ROOT, stdio: "pipe" });

  const index = JSON.parse(readFileSync(join(archive, "index.json"), "utf8"));
  assert.equal(index.version, 1);
  assert.equal(index.bundles.length, 1);
  const entry = index.bundles[0];
  assert.equal(entry.id, index.current);
  assert.equal(entry.id, bundleId(entry.glueSha256, entry.stamp));

  const dir = join(archive, entry.id);
  const manifest = JSON.parse(readFileSync(join(dir, "bundle.json"), "utf8"));
  assert.equal(manifest.id, entry.id);
  for (const f of ["glue.js", "glue_bg.wasm"]) assert.ok(existsSync(join(dir, f)), f);
  for (const name of manifest.layout.dataFiles) assert.ok(existsSync(join(dir, "data", name)), name);
  assert.ok(existsSync(join(archive, "modules", JSON.parse(readFileSync(join(dir, "rules", "index.json"), "utf8")).modules[0].file)));

  // Load it the way the worker does, and the stamp must match the index.
  const g = await loadGlue({
    glueJs: join(dir, "glue.js"),
    glueWasm: join(dir, "glue_bg.wasm"),
    dataDir: join(dir, "data"),
    tag: "arch",
    glueSha: entry.glueSha256,
    rules: { index: join(dir, "rules", "index.json"), modulesDir: join(archive, "modules") },
  });
  const stamp = JSON.parse(g.engine_stamp());
  assert.equal(stamp.ruleset_sha256, entry.stamp.ruleset_sha256);
  assert.equal(stamp.data_sha256, entry.stamp.data_sha256);
  assert.equal(stamp.data_sha256, "", "the archive seals the engine's data_sha256 (empty)");
  assert.equal(entry.id, bundleId(entry.glueSha256, { ...entry.stamp, data_sha256: "" }),
    "the entry id is the id a record names (empty data_sha256)");
  assert.equal(stampKey(stamp), entry.key ?? stampKey(entry.stamp));
});

test("a record of bundle A replays clean on archived A; host tables do not fork it", async () => {
  rmSync(TMP, { recursive: true, force: true });
  mkdirSync(TMP, { recursive: true });
  const wasmDir = join(ROOT, "webui", "src", "wasm");
  const glueJs = join(wasmDir, "glue.js");
  const glueWasm = join(wasmDir, "glue_bg.wasm");
  const gsha = glueSha256(readFileSync(glueJs), readFileSync(glueWasm));
  const dataDir = join(ROOT, "data");

  // -- build A seals a record ---------------------------------------------
  const A = await loadGlue({ glueJs, glueWasm, dataDir, tag: "A", glueSha: gsha });
  const { bytes } = recordSomeGame(A, 7);
  const header = JSON.parse(A.record_header_bytes(bytes));
  const idA = header.engine.bundle;
  assert.equal(idA, bundleId(gsha, header.engine), "the record names its bundle");
  assert.equal(header.engine.data_sha256, "", "the record's data_sha256 is empty");
  const body = recordJson(bytes).body;
  assert.ok(body.inputs.length > 0, "the record has inputs");
  assert.ok(body.checkpoints.length > 0, "the record has turn-boundary checkpoints");
  // `Init::Seed(MatchSetup)` -- the board a replay uses, not the host's.
  assert.ok(body.init?.Seed?.board?.length, "the record carries its own board (MatchSetup)");

  // Freeze A the way the archive does (glue + the boot tables, which are
  // stored but do not enter the bundle id).
  const frozen = join(TMP, "bundleA");
  mkdirSync(join(frozen, "data"), { recursive: true });
  copyFileSync(glueJs, join(frozen, "glue.js"));
  copyFileSync(glueWasm, join(frozen, "glue_bg.wasm"));
  for (const n of JSON.parse(A.data_files())) {
    if (existsSync(join(dataDir, n))) copyFileSync(join(dataDir, n), join(frozen, "data", n));
  }

  // -- host tables are not identity ---------------------------------------
  // Same engine, a tweaked board on disk: `data_sha256` stays empty and the
  // bundle id is unchanged, because the match's board rides in `MatchSetup`
  // and a replay never re-reads the host's copy (`docs/REPLAY.md` §9.1).
  const dataB = join(TMP, "dataB");
  mkdirSync(dataB, { recursive: true });
  for (const n of JSON.parse(A.data_files())) {
    const p = join(dataDir, n);
    if (!existsSync(p)) continue;
    if (n !== "board.json") {
      copyFileSync(p, join(dataB, n));
      continue;
    }
    const board = JSON.parse(readFileSync(p, "utf8"));
    if (board.tiles?.length) board.tiles[0].price = (board.tiles[0].price || 0) + 1;
    writeFileSync(join(dataB, n), JSON.stringify(board));
  }
  const B = await loadGlue({ glueJs, glueWasm, dataDir: dataB, tag: "B", glueSha: gsha });
  const stampB = JSON.parse(B.engine_stamp());
  assert.equal(stampB.data_sha256, "", "host tables are not hashed into the stamp");
  assert.equal(stampB.bundle, idA, "the same engine with other host tables is the same bundle");

  // -- the archive path: a fresh instance of frozen A ---------------------
  const C = await loadGlue({
    glueJs: join(frozen, "glue.js"),
    glueWasm: join(frozen, "glue_bg.wasm"),
    dataDir: join(frozen, "data"),
    tag: "C",
    glueSha: gsha,
  });
  const stampC = JSON.parse(C.engine_stamp());
  assert.equal(stampC.bundle, idA, "the frozen engine is the record's bundle");
  const m = C.ReplayMatch.from_record_bytes(bytes, false); // no force
  assert.equal(m.compat(), "[]", "no stamp difference on the right bundle");
  let st;
  for (let i = 0; i < 5000; i++) {
    st = JSON.parse(m.step(10));
    if (st.ended) break;
  }
  assert.ok(st && st.ended, "the record plays to the end");
  assert.equal(st.diverged, false, "no checkpoint diverged on the matching bundle");
  m.free();

  // B (tweaked host board) must play the record identically: the outcome
  // comes from the file, not from whatever is on disk now.
  const m2 = B.ReplayMatch.from_record_bytes(bytes, false);
  assert.equal(m2.compat(), "[]", "host tables are not a stamp difference");
  let s2;
  for (let i = 0; i < 5000; i++) {
    s2 = JSON.parse(m2.step(10));
    if (s2.ended) break;
  }
  assert.ok(s2 && s2.ended, "the record plays to the end on the tweaked host too");
  assert.equal(s2.diverged, false, "the record's own board wins -- no checkpoint diverged");
  m2.free();

  // -- glue identity still forks the bundle -------------------------------
  // A different `glueSha256` is a different engine build, so a different
  // bundle id -- the lookup key `exportPortable` uses to find the bytes.
  const D = await loadGlue({ glueJs, glueWasm, dataDir, tag: "D", glueSha: "f".repeat(64) });
  const stampD = JSON.parse(D.engine_stamp());
  assert.notEqual(stampD.bundle, idA, "a different glue sha is a different bundle");
  assert.equal(stampD.bundle, bundleId("f".repeat(64), stampD), "and it is the recipe's id");
});