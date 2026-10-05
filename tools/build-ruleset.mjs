#!/usr/bin/env node
// Build every card crate in rules/ to its wasm module, validate it, and publish
// content-addressed modules + index.json, plus the crates' locale bundles
// (locales/<lang>.json, the client's `cards` i18n namespace):
//
//   dist/cards/     shipped cards (ONE unified module -- see rules-aggregate.mjs)
//   dist/fixtures/  test-only cards (used by game-rules tests)
//
// Runs on any platform Node runs on (no shell). Usage:
//
//   node tools/build-ruleset.mjs

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, readdirSync, rmSync, unlinkSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const run = (...args) => {
  console.log("+", args.join(" "));
  execFileSync(args[0], args.slice(1), { cwd: ROOT, stdio: "inherit" });
};
const cargo = (...args) => run("cargo", ...args);

// Regenerate the aggregate crate (one shipped module linking every band) so a
// newly added band crate is picked up automatically.
run(process.execPath, "tools/rules-aggregate.mjs");

cargo("build", "--manifest-path", "rules/Cargo.toml", "--workspace",
      "--release", "--target", "wasm32-unknown-unknown");
const built = join(ROOT, "rules", "target", "wasm32-unknown-unknown", "release");

// NOTE: the staging dir is unique per run and dist/ is never deleted here.
// Concurrent runs of this script race on any shared delete; modules are
// content-addressed and index.json is the source of truth, so an overlap of
// stale .wasm files is harmless while a missing file is not. The indexer
// writes index.json last (atomically).
const stage = join(ROOT, "target", `ruleset-stage-${process.pid}`);
for (const sub of ["cards", "fixtures"]) {
  mkdirSync(join(stage, sub), { recursive: true });
  mkdirSync(join(ROOT, "dist", sub), { recursive: true });
}

// The shipped card set is ONE module (card-all links every band crate as an
// rlib). Fixtures stay per-crate. Copy only modules of crates that exist right
// now (cargo leaves artifacts behind when a crate is deleted, which would
// resurrect old card ids).
copyFileSync(join(built, "card_all.wasm"), join(stage, "cards", "card_all.wasm"));
for (const dir of readdirSync(join(ROOT, "rules", "fixtures"), { withFileTypes: true })) {
  if (!dir.isDirectory()) continue;
  const toml = join(ROOT, "rules", "fixtures", dir.name, "Cargo.toml");
  const m = readFileSync(toml, "utf8").match(/^name = "([^"]+)"/m);
  if (!m) throw new Error(`${toml}: no package name`);
  const wasm = `${m[1].replaceAll("-", "_")}.wasm`;
  copyFileSync(join(built, wasm), join(stage, "fixtures", wasm));
}

cargo("run", "-q", "-p", "game-rules", "--example", "ruleset_index",
      "--", join(stage, "cards"), "dist/cards");
cargo("run", "-q", "-p", "game-rules", "--example", "ruleset_index",
      "--", join(stage, "fixtures"), "dist/fixtures");

// Drop module drafts the current index.json does not reference: builds that
// change content address leave the old .wasm behind (we deliberately never
// delete dist/ wholesale -- see above), so prune against the index instead.
for (const out of ["dist/cards", "dist/fixtures"]) {
  const idx = JSON.parse(readFileSync(join(ROOT, out, "index.json"), "utf8"));
  const keep = new Set([...idx.modules.map((m) => m.file), "index.json"]);
  for (const f of readdirSync(join(ROOT, out))) {
    if (f.endsWith(".wasm") && !keep.has(f)) unlinkSync(join(ROOT, out, f));
  }
}

cargo("run", "-q", "-p", "game-rules", "--example", "card_locales",
      "--", "rules/cards", "dist/cards");
cargo("run", "-q", "-p", "game-rules", "--example", "card_locales",
      "--", "rules/fixtures", "dist/fixtures");

// The web client loads these as its `cards` i18n namespace...
const i18n = join(ROOT, "webui", "public", "assets", "i18n");
mkdirSync(i18n, { recursive: true });
for (const f of readdirSync(join(ROOT, "dist", "cards", "locales"))) {
  if (f.endsWith(".json")) copyFileSync(join(ROOT, "dist", "cards", "locales", f), join(i18n, `cards-${f}`));
}

// ...and the modules themselves, to feed the solo match's ruleset.
const rules = join(ROOT, "webui", "public", "assets", "rules");
mkdirSync(rules, { recursive: true });
copyFileSync(join(ROOT, "dist", "cards", "index.json"), join(rules, "index.json"));
for (const f of readdirSync(join(ROOT, "dist", "cards"))) {
  if (f.endsWith(".wasm")) copyFileSync(join(ROOT, "dist", "cards", f), join(rules, f));
}

rmSync(stage, { recursive: true, force: true });
const idx = JSON.parse(readFileSync(join(ROOT, "dist", "cards", "index.json"), "utf8"));
const cards = idx.modules.reduce((n, m) => n + m.cards.length, 0);
const total = idx.modules.reduce((n, m) => n + m.bytes, 0);
console.log(`dist/cards: ${idx.modules.length} module(s), ${cards} cards, ${total} bytes`);