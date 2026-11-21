#!/usr/bin/env node
// Regenerate missing engine reference records (`archive/engine/refs/<id>.bdrec`).
//
// A reference record is the behaviour check every rebuild must pass. It is a
// deterministic function of the bundle's own bytes: `makeReferenceRecord` seeds
// a fixed match (seed 7) and the bundle replays it. So a record lost from the
// tree -- e.g. a deploy that merged an index carrying entries whose untracked
// refs were never copied -- can be re-derived from the stored bundle without
// rebuilding anything.
//
// The record is only accepted when its sha256 equals the `ref_sha256` the index
// sealed, and when it replays clean through the same glue. That is what makes
// the regenerated file the original file, not merely a valid one.
//
//   node tools/reseal-engine-refs.mjs [--dry-run] [<bundle-id|commit> ...]
//
// With no ids, every entry whose ref is missing gets resealed. Entries whose
// store bytes are gone are reported, not invented.

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { makeReferenceRecord, verifyReferenceRecord } from "./engine-source.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const INDEX = join(ROOT, "archive", "engine", "index.json");
const STORE = join(ROOT, "data", "engine-archive");
const ARCHIVE = join(ROOT, "archive", "engine");

const argv = process.argv.slice(2);
const dryRun = argv.includes("--dry-run");
const wanted = argv.filter((a) => !a.startsWith("--"));

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

/** Load a stored bundle exactly as `tools/archive-engine.mjs` does before it
 *  seals a record: data loaded, precompiled conds + modules in, ruleset built,
 *  glue sha set. A fresh import key keeps two glues loadable in one process. */
async function loadStored(id, entry) {
  const dir = join(STORE, id);
  const rulesIndex = JSON.parse(readFileSync(join(dir, "rules", "index.json"), "utf8"));
  const { pathToFileURL } = await import("node:url");
  const g = await import(
    pathToFileURL(join(dir, "glue.js")).href + `?reseal=${id.slice(0, 8)}`
  );
  await g.default({ module_or_path: new Uint8Array(readFileSync(join(dir, "glue_bg.wasm"))) });
  const files = {};
  for (const n of JSON.parse(g.data_files())) {
    const p = join(dir, "data", n);
    if (existsSync(p)) files[n] = readFileSync(p, "utf8");
  }
  g.load_data(JSON.stringify(files));
  for (const m of rulesIndex.modules ?? []) {
    g.ruleset_add(new Uint8Array(readFileSync(join(STORE, "modules", m.file))));
  }
  if (rulesIndex.conds?.file) {
    g.ruleset_precompiled(new Uint8Array(readFileSync(join(dir, "rules", rulesIndex.conds.file))));
  }
  g.ruleset_build();
  if (typeof g.set_glue_sha === "function") g.set_glue_sha(entry.glueSha256);
  return g;
}

async function reseal(entry) {
  const id = entry.id;
  const refName = entry.ref ?? `refs/${id}.bdrec`;
  const refPath = join(ARCHIVE, refName);
  if (existsSync(refPath)) {
    console.log(`keep  ${id.slice(0, 12)}…  ${refName} exists`);
    return true;
  }
  if (!entry.ref_sha256) {
    console.log(`skip  ${id.slice(0, 12)}…  no sealed ref_sha256 to match`);
    return false;
  }
  if (!existsSync(join(STORE, id, "glue.js"))) {
    console.log(`FAIL  ${id.slice(0, 12)}…  no stored bytes (rebuild the bundle first)`);
    return false;
  }

  const g = await loadStored(id, entry);
  const bytes = makeReferenceRecord(g);
  const sha = sha256(bytes);
  if (sha !== entry.ref_sha256) {
    console.log(`FAIL  ${id.slice(0, 12)}…  regenerated ${sha.slice(0, 12)}… != sealed ${entry.ref_sha256.slice(0, 12)}…`);
    console.log(`      the stored bundle is not the sealed engine -- rebuild it instead`);
    return false;
  }
  const v = verifyReferenceRecord(g, bytes, true);
  if (!v.ok) {
    console.log(`FAIL  ${id.slice(0, 12)}…  record regenerated but does not replay: ${v.reason}`);
    return false;
  }
  if (!dryRun) {
    mkdirSync(dirname(refPath), { recursive: true });
    writeFileSync(refPath, bytes);
  }
  console.log(
    `${dryRun ? "would" : "seal "} ${id.slice(0, 12)}…  ${refName}  ${bytes.length} bytes  ` +
      `sha ${sha.slice(0, 12)}…  replays clean (${v.ticks} ticks)`,
  );
  return true;
}

const index = JSON.parse(readFileSync(INDEX, "utf8"));
const targets = wanted.length
  ? index.bundles.filter((b) => wanted.some((w) => b.id === w || b.commit === w || b.id.startsWith(w)))
  : index.bundles.filter((b) => b.ref && !existsSync(join(ARCHIVE, b.ref)));

if (!targets.length) {
  console.log("reseal: nothing to do");
  process.exit(0);
}

let failed = 0;
for (const entry of targets) {
  try {
    if (!(await reseal(entry))) failed++;
  } catch (e) {
    console.log(`FAIL  ${entry.id.slice(0, 12)}…  ${e.message}`);
    failed++;
  }
}
console.log(`reseal: ${targets.length - failed} ok, ${failed} failed`);
process.exit(failed ? 1 : 0);
