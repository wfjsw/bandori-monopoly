#!/usr/bin/env node
// Union two engine-archive indexes (`archive/engine/index.json`). The archive
// is append-only: a bundle entry is keyed by its id, immutable once sealed, and
// ordered newest first. A deploy therefore never has to resolve an index
// conflict -- it can park its own entries, merge master on a clean tree, and
// put the parked entries back in with this union.
//
//   node tools/merge-engine-archive.mjs --into <index.json> --from <index.json>
//                                       [--refs-from <dir>]
//
// `--into` is the index that wins a duplicate id (it is the already-committed
// side); `--from` contributes the entries `--into` lacks. Reference records
// named by the imported entries are copied out of `--refs-from` when missing
// beside the destination index. `current` is kept from `--into` unless it is
// empty. The result is written back to `--into` (default pretty-print, one
// space, exactly as tools/archive-engine.mjs writes it).

import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

function fail(message) {
  console.error(`merge-engine-archive: ${message}`);
  process.exit(2);
}

const args = { into: null, from: null, refsFrom: null };
for (let i = 2; i < process.argv.length; i++) {
  const a = process.argv[i];
  const next = () => {
    const v = process.argv[++i];
    if (v == null) fail(`${a}: missing value`);
    return v;
  };
  if (a === "--into") args.into = next();
  else if (a === "--from") args.from = next();
  else if (a === "--refs-from") args.refsFrom = next();
  else fail(`unknown argument: ${a}`);
}
if (!args.from) fail("--from is required");
if (!args.into) fail("--into is required");

const EMPTY = { version: 1, current: null, bundles: [] };

function readIndex(path) {
  if (!existsSync(path)) return { ...EMPTY, bundles: [] };
  const index = JSON.parse(readFileSync(path, "utf8"));
  if (!Array.isArray(index.bundles)) fail(`${path}: no bundles array`);
  // Ids are the identity this union keys on; refuse an entry without one.
  for (const b of index.bundles) {
    if (!b || typeof b.id !== "string" || !b.id) fail(`${path}: bundle entry without an id`);
  }
  return index;
}

const into = readIndex(args.into);
const from = readIndex(args.from);

const seen = new Set(into.bundles.map((b) => b.id));
let imported = 0;
for (const entry of from.bundles) {
  if (seen.has(entry.id)) continue;
  seen.add(entry.id);
  into.bundles.push(entry);
  imported++;
}

// Newest first, the order archive-engine maintains. Equal timestamps keep the
// id as a stable tie-break so a repeated union cannot reorder the file.
into.bundles.sort((a, b) => (b.created ?? "").localeCompare(a.created ?? "") || a.id.localeCompare(b.id));
into.version = 1;
if (!into.current) into.current = from.current ?? null;

if (args.refsFrom) {
  const refsDir = join(dirname(args.into), "refs");
  let restored = 0;
  let missing = 0;
  for (const entry of from.bundles) {
    if (!entry.ref) continue;
    const dest = join(dirname(args.into), entry.ref);
    if (existsSync(dest)) continue;
    const src = join(args.refsFrom, entry.id + ".bdrec");
    if (!existsSync(src)) {
      console.error(`warn  ${entry.id.slice(0, 12)}…  reference record ${entry.ref} is not in the park either`);
      missing++;
      continue;
    }
    mkdirSync(refsDir, { recursive: true });
    copyFileSync(src, dest);
    restored++;
  }
  if (restored) console.log(`refs  restored ${restored} reference record(s)`);
  if (missing) console.log(`refs  ${missing} still missing -- archive-engine --check will report them`);
}

mkdirSync(dirname(args.into), { recursive: true });
writeFileSync(args.into, JSON.stringify(into, null, 1));
console.log(`merge   ${imported} imported, ${into.bundles.length} bundles total`);
