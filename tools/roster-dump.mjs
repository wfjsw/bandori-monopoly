#!/usr/bin/env node
// Write the ruleset roster as JSON files for tools.
//
// The roster is authored in the Rust rule code (ABI v63 `ruleset_metadata`).
// This wraps `cargo run -p rules-native --example dump_roster` and caches the
// three JSON-shaped files (same shape / key order / formatting as the retired
// `data/{cards,characters,bands}.json`) under a build dir -- NOT `data/`.
//
//   node tools/roster-dump.mjs              # dump if absent
//   node tools/roster-dump.mjs --force      # re-dump even if cached
//   node tools/roster-dump.mjs --out DIR    # default: target/roster-dump
//
// `ROSTER_DIR` also selects the out directory (overridden by --out).
// Python tools read the dump via tools/roster.py; see docs/ROSTER-TOOLS.md.
//
// Display-only columns outside the engine structs (`simple`, `counterReason`)
// are not in the dump.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const FILES = ["cards.json", "characters.json", "bands.json"];

const args = process.argv.slice(2);
let out = process.env.ROSTER_DIR || join(ROOT, "target", "roster-dump");
let force = false;
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === "--force") force = true;
  else if (a === "--out") out = resolve(args[++i] ?? ".");
  else if (a === "--help" || a === "-h") {
    console.log("usage: node tools/roster-dump.mjs [--out DIR] [--force]");
    process.exit(0);
  } else {
    console.error(`unknown flag: ${a}`);
    process.exit(2);
  }
}
out = resolve(out);

if (!force && FILES.every((f) => existsSync(join(out, f)))) {
  console.log(`reusing cached roster dump in ${out} (use --force to re-dump)`);
  process.exit(0);
}

mkdirSync(out, { recursive: true });
console.log(`dumping roster -> ${out}`);
// No CARGO_TARGET_DIR: an externally set one stages stale artifacts (memory).
execFileSync("cargo", ["run", "-p", "rules-native", "--example", "dump_roster", "--", out], {
  cwd: ROOT,
  stdio: "inherit",
  env: (() => {
    const e = { ...process.env };
    delete e.CARGO_TARGET_DIR;
    return e;
  })(),
});
for (const f of FILES) {
  if (!existsSync(join(out, f))) {
    console.error(`dump_roster did not write ${join(out, f)}`);
    process.exit(1);
  }
}
console.log(`ok: ${FILES.join(", ")} in ${out}`);