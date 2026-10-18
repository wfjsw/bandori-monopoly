#!/usr/bin/env node
// Export rule-authored display text into the webui bundle.
//
// Printed text / display columns (card rule text, persona art ids, band logos,
// ...) are authored in the Rust rule code beside the rule bodies. They no
// longer ride `ruleset_metadata` -- this tool serializes them at build time
// into `webui/src/generated/display.json`, which the webui bundles
// (`docs/DISPLAY.md`).
//
//   node tools/export-display.mjs              # write the default out file
//   node tools/export-display.mjs --out PATH   # default: webui/src/generated/display.json
//
// PATH may be a `.json` file or a directory (then `display.json` is written
// inside it). There is no cache: this feeds the webui build and must track
// every rule-source edit. The underlying example skips the write when the
// bytes are unchanged, so an unchanged export does not bump mtime.
//
// Wraps `cargo run -q -p rules-native --example dump_display`.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_OUT = join(ROOT, "webui", "src", "generated", "display.json");

const args = process.argv.slice(2);
let out = DEFAULT_OUT;
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === "--out") out = resolve(args[++i] ?? ".");
  else if (a === "--help" || a === "-h") {
    console.log("usage: node tools/export-display.mjs [--out PATH]");
    process.exit(0);
  } else {
    console.error(`unknown flag: ${a}`);
    process.exit(2);
  }
}
out = resolve(out);
// `--out DIR` (or any non-`.json` path) means "write display.json there".
if (!out.endsWith(".json")) {
  out = join(out, "display.json");
}

mkdirSync(dirname(out), { recursive: true });
console.log(`exporting display -> ${out}`);
// No CARGO_TARGET_DIR: an externally set one stages stale artifacts (memory).
execFileSync(
  "cargo",
  ["run", "-q", "-p", "rules-native", "--example", "dump_display", "--", "--out", out],
  {
    cwd: ROOT,
    stdio: "inherit",
    env: (() => {
      const e = { ...process.env };
      delete e.CARGO_TARGET_DIR;
      return e;
    })(),
  },
);
if (!existsSync(out)) {
  console.error(`dump_display did not write ${out}`);
  process.exit(1);
}
console.log(`ok: ${out}`);