#!/usr/bin/env node
// Build crates/web-glue for the browser and generate its JS bindings into
// webui/src/wasm/ (imported by webui/src/core/data.ts).
//
// Runs on any platform Node runs on (no shell). Usage:
//
//   node tools/build-glue.mjs
//
// Cargo profile follows `NODE_ENV` (`docs/SERVER.md`): `production` builds
// `--release` -- no `debug_assertions`, so the console cheats
// (`game-core/src/engine/debug.rs`) are not compiled in and `cheats_enabled()`
// answers false. Anything else (development / unset, as under `npm run dev`)
// builds the debug profile with the cheats. The one exception is npm's
// `prebuild` lifecycle: it exists only to service `npm run build`, the deploy
// path, and npm does not set `NODE_ENV` for scripts -- so `prebuild` is always
// production, whatever `NODE_ENV` says.
//
// Determinism (`docs/REPLAY.md` §9 option C): two builds of the same commit on
// the same toolchain must produce byte-identical `glue.js` + `glue_bg.wasm`,
// whatever directory they run in, so an engine bundle can be rebuilt from its
// source commit and still hash to the same bundle id.
//
//   * `CARGO_INCREMENTAL=0` -- release builds already avoid incremental, but
//     the env is belt and braces.
//   * `--remap-path-prefix` strips the absolute workspace / cargo-home /
//     sysroot paths rustc records in panic `Location`s. Without it the bytes
//     name the build machine's home directory. Workspace crates already get
//     relative paths; this covers the registry and the std sources too.
//   * the wasm-bindgen CLI version is checked against the crate pin in
//     `crates/web-glue/Cargo.toml` (`=X.Y.Z`): the CLI rewrites the wasm and
//     its own version ends up in the `producers` section, so a mismatch is a
//     different bundle.
//
// `webui/src/wasm/engine_id.json` then records the glue identity AND the
// provenance (`commit`, `dirty`, toolchain) of the bytes just built --
// `tools/archive-engine.mjs` copies that into the archive index so
// `tools/rebuild-engine.mjs` knows which commit reproduces this bundle.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { glueSha256 } from "./engine-bundle.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const run = (...args) => {
  console.log("+", args.join(" "));
  execFileSync(args[0], args.slice(1), { cwd: ROOT, stdio: "inherit" });
};
const runOut = (...args) =>
  execFileSync(args[0], args.slice(1), { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();

// ---------------------------------------------------------------- toolchain

/** The wasm-bindgen version `crates/web-glue` pins (=X.Y.Z). */
function pinnedBindgen() {
  const toml = readFileSync(join(ROOT, "crates", "web-glue", "Cargo.toml"), "utf8");
  const m = toml.match(/wasm-bindgen\s*=\s*"=([0-9.]+)"/);
  return m ? m[1] : null;
}

function checkBindgen() {
  let v;
  try {
    v = runOut("wasm-bindgen", "--version");
  } catch {
    console.error("build-glue: wasm-bindgen CLI not on PATH");
    process.exit(1);
  }
  const m = v.match(/(\d+\.\d+\.\d+)/);
  const got = m ? m[1] : v;
  const want = pinnedBindgen();
  if (want && got !== want) {
    console.error(
      `build-glue: wasm-bindgen CLI ${got} does not match the crate pin ${want}\n` +
        `  (crates/web-glue pins "= ${want}"; the CLI rewrites the wasm and its version\n` +
        `   lands in the producers section, so this would be a different bundle)`,
    );
    process.exit(1);
  }
  return v;
}

// ---------------------------------------------------------------- provenance

/**
 * Which commit produced these bytes. The workspace path is remapped out of the
 * artifact, but the *inputs* still differ per commit, so the archive has to
 * know the tree the build ran on -- including whether it was dirty.
 */
function provenance() {
  const toolchain = {
    rustc: runOut("rustc", "-V"),
    cargo: runOut("cargo", "-V"),
    wasm_bindgen: runOut("wasm-bindgen", "--version"),
  };
  let commit = null;
  let dirty = true;
  let dirtyPaths = [];
  try {
    commit = runOut("git", "rev-parse", "HEAD");
    const st = runOut("git", "status", "--porcelain", "--",
      "Cargo.toml", "Cargo.lock",
      "crates/game-core", "crates/game-rules", "crates/web-glue",
      "rules/cond", "crates/rules-native",
      "rules", "data",
      "tools/build-glue.mjs", "tools/build-ruleset.mjs", "tools/rules-aggregate.mjs",
    );
    dirtyPaths = st.split("\n").filter(Boolean);
    dirty = dirtyPaths.length > 0;
  } catch {
    /* not a git checkout: dirty, no commit */
  }
  return { commit, dirty, dirtyPaths, toolchain };
}

// ---------------------------------------------------------------- rustc flags

/**
 * `--remap-path-prefix` for every absolute prefix rustc might record:
 * the workspace, the cargo home (registry + git checkouts) and the sysroot
 * (std / alloc sources under `rustlib/src`). After this the panic strings say
 * `/.cargo/registry/...` and `/rustc/sysroot/...` on every machine and in
 * every directory. Paths are passed with native separators -- rustc matches
 * the prefix against the path as the OS gives it -- and through
 * `CARGO_ENCODED_RUSTFLAGS` (\x1f-separated) because this repo's path
 * contains spaces and non-ASCII characters.
 */
function rustFlagList() {
  const flags = [];
  const remap = (from, to) => {
    if (from) flags.push(`--remap-path-prefix=${from}=${to}`);
  };
  remap(ROOT, ".");
  const home = process.env.USERPROFILE || process.env.HOME || process.env.USER_HOME;
  if (home) {
    remap(process.env.CARGO_HOME || join(home, ".cargo"), "/.cargo");
    remap(home, "/home/build");
  }
  try {
    remap(runOut("rustc", "--print", "sysroot"), "/rustc/sysroot");
  } catch {
    /* sysroot remap is best-effort */
  }
  return flags;
}

// ---------------------------------------------------------------- main

// `docs/SERVER.md`: production / `prebuild` → the release engine (no cheats);
// anything else → the debug engine (cheats in).
const release =
  process.env.NODE_ENV === "production" || process.env.npm_lifecycle_event === "prebuild";
const profile = release ? "release" : "debug";
// Honour `CARGO_TARGET_DIR` (cargo reads it too), so a build can put its
// scratch artifacts on another disk. The bytes are unaffected: the sources are
// already path-remapped and the target dir never reaches them.
const targetDir = process.env.CARGO_TARGET_DIR
  ? resolve(process.env.CARGO_TARGET_DIR)
  : join(ROOT, "target");
const profileDir = join(targetDir, "wasm32-unknown-unknown", profile);
console.log(`build-glue: profile ${profile} (NODE_ENV=${process.env.NODE_ENV ?? "<unset>"})`);

const bindgen = checkBindgen();
console.log(`wasm-bindgen ${bindgen}`);

// Merge with any caller RUSTFLAGS (space-split, as cargo would) and never
// enable incremental. `CARGO_ENCODED_RUSTFLAGS` outranks `RUSTFLAGS`.
const inherited = (process.env.RUSTFLAGS || "").split(/\s+/).filter(Boolean);
const encoded = [...inherited, ...rustFlagList()].join("\x1f");
const env = {
  ...process.env,
  CARGO_INCREMENTAL: "0",
  CARGO_ENCODED_RUSTFLAGS: encoded,
};
delete env.RUSTFLAGS;

console.log(`CARGO_ENCODED_RUSTFLAGS ${JSON.stringify(encoded)}`);
execFileSync("cargo", ["build", "-p", "web-glue", "--target", "wasm32-unknown-unknown",
  ...(release ? ["--release"] : [])], {
  cwd: ROOT,
  stdio: "inherit",
  env,
});
run("wasm-bindgen", "--target", "web", "--out-dir", "webui/src/wasm", "--out-name", "glue",
    join(profileDir, "web_glue.wasm"));
console.log(`webui/src/wasm/glue_bg.wasm: ${statSync(join(ROOT, "webui", "src", "wasm", "glue_bg.wasm")).size} bytes`);

// Glue identity for the engine-bundle id (`docs/REPLAY.md` §9): the webui
// passes this to `set_glue_sha` at boot so a record names the bundle that
// wrote it, and `tools/archive-engine.mjs` hashes the same two files.
const wasmDir = join(ROOT, "webui", "src", "wasm");
const gsha = glueSha256(readFileSync(join(wasmDir, "glue.js")), readFileSync(join(wasmDir, "glue_bg.wasm")));
const prov = provenance();
if (prov.dirty) {
  console.log(`NOTE: working tree is dirty (${prov.dirtyPaths.length} input paths) --`);
  console.log(`      this build will be sealed rebuild:false (files only, no source commit).`);
}
writeFileSync(
  join(wasmDir, "engine_id.json"),
  JSON.stringify(
    {
      glueSha256: gsha,
      // "release" | "debug" -- `tools/archive-engine.mjs` refuses to archive
      // anything but a release engine (no debug build may become a bundle).
      profile,
      built: new Date().toISOString(),
      commit: prov.dirty ? null : prov.commit,
      dirty: prov.dirty,
      toolchain: prov.toolchain,
    },
    null,
    1,
  ) + "\n",
);
console.log(`webui/src/wasm/engine_id.json: glueSha256 ${gsha}`);
if (!prov.dirty) console.log(`  from commit ${prov.commit}`);