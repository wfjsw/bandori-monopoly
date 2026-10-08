#!/usr/bin/env node
// Build crates/bot-glue (the advanced-bot worker bundle, docs/BOT.md B6) for
// the browser and emit its JS bindings into
// webui/public/assets/engine/bot-glue/ -- beside the worker that loads it
// (webui/public/assets/engine/bot-worker.js). The main page's glue
// (webui/src/wasm/) is untouched: the bot bundle is lazy-loaded only when an
// advanced seat / advanced 托管 is in use.
//
// Runs on any platform Node runs on (no shell). Usage:
//
//   node tools/build-bot-glue.mjs
//
// Determinism rules are the same as tools/build-glue.mjs (`docs/REPLAY.md` §9
// option C): `CARGO_INCREMENTAL=0`, `--remap-path-prefix` for the workspace /
// cargo-home / sysroot, and the wasm-bindgen CLI version checked against the
// crate pin in `crates/bot-glue/Cargo.toml`. This bundle does NOT ride the
// engine-bundle id (replays never run the bot -- `docs/REPLAY.md`), so no
// `engine_id.json` is written here.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, statSync, rmSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const OUT_DIR = join(ROOT, "webui", "public", "assets", "engine", "bot-glue");
const run = (...args) => {
  console.log("+", args.join(" "));
  execFileSync(args[0], args.slice(1), { cwd: ROOT, stdio: "inherit" });
};
const runOut = (...args) =>
  execFileSync(args[0], args.slice(1), { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();

// ---------------------------------------------------------------- toolchain

/** The wasm-bindgen version `crates/bot-glue` pins (=X.Y.Z). */
function pinnedBindgen() {
  const toml = readFileSync(join(ROOT, "crates", "bot-glue", "Cargo.toml"), "utf8");
  const m = toml.match(/wasm-bindgen\s*=\s*"=([0-9.]+)"/);
  return m ? m[1] : null;
}

function checkBindgen() {
  let v;
  try {
    v = runOut("wasm-bindgen", "--version");
  } catch {
    console.error("build-bot-glue: wasm-bindgen CLI not on PATH");
    process.exit(1);
  }
  const m = v.match(/(\d+\.\d+\.\d+)/);
  const got = m ? m[1] : v;
  const want = pinnedBindgen();
  if (want && got !== want) {
    console.error(
      `build-bot-glue: wasm-bindgen CLI ${got} does not match the crate pin ${want}\n` +
        `  (crates/bot-glue pins "= ${want}"; keep the two in step -- see tools/build-glue.mjs)`,
    );
    process.exit(1);
  }
  return v;
}

// ---------------------------------------------------------------- rustc flags

/**
 * Same `--remap-path-prefix` set as `tools/build-glue.mjs`: the workspace,
 * the cargo home and the sysroot. `CARGO_ENCODED_RUSTFLAGS` (\x1f-separated)
 * because this repo's path contains spaces and non-ASCII characters.
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

const bindgen = checkBindgen();
console.log(`wasm-bindgen ${bindgen}`);

const inherited = (process.env.RUSTFLAGS || "").split(/\s+/).filter(Boolean);
const encoded = [...inherited, ...rustFlagList()].join("\x1f");
const env = {
  ...process.env,
  CARGO_INCREMENTAL: "0",
  // Own wasm target dir (same as tools/build-glue.mjs): the path below is
  // stable whatever CARGO_TARGET_DIR the caller exports (target/coord etc.).
  CARGO_TARGET_DIR: join(ROOT, "target"),
  CARGO_ENCODED_RUSTFLAGS: encoded,
};
delete env.RUSTFLAGS;

console.log(`CARGO_ENCODED_RUSTFLAGS ${JSON.stringify(encoded)}`);
execFileSync("cargo", ["build", "-p", "bot-glue", "--target", "wasm32-unknown-unknown", "--release"], {
  cwd: ROOT,
  stdio: "inherit",
  env,
});

// Fresh out dir: stale glue.js next to a new glue_bg.wasm is a broken bundle.
rmSync(OUT_DIR, { recursive: true, force: true });
mkdirSync(OUT_DIR, { recursive: true });
run(
  "wasm-bindgen",
  "--target",
  "web",
  "--out-dir",
  OUT_DIR,
  "--out-name",
  "glue",
  join(ROOT, "target", "wasm32-unknown-unknown", "release", "bot_glue.wasm"),
);
const wasmPath = join(OUT_DIR, "glue_bg.wasm");
const jsPath = join(OUT_DIR, "glue.js");
if (!existsSync(wasmPath) || !existsSync(jsPath)) {
  console.error("build-bot-glue: wasm-bindgen did not emit glue.js + glue_bg.wasm");
  process.exit(1);
}
console.log(`webui/public/assets/engine/bot-glue/glue_bg.wasm: ${statSync(wasmPath).size} bytes`);
console.log(`webui/public/assets/engine/bot-glue/glue.js: ${statSync(jsPath).size} bytes`);

// A tiny manifest so the worker (and the size gate) can find the pieces and
// report which commit produced them. Not part of the engine-bundle id.
let commit = null;
let dirty = true;
try {
  commit = runOut("git", "rev-parse", "HEAD");
  const st = runOut("git", "status", "--porcelain", "--",
    "Cargo.toml", "Cargo.lock",
    "crates/bot-core", "crates/bot-glue", "crates/game-core", "crates/game-rules",
    "tools/build-bot-glue.mjs",
  );
  dirty = st.split("\n").filter(Boolean).length > 0;
} catch {
  /* not a git checkout */
}
writeFileSync(
  join(OUT_DIR, "bundle.json"),
  JSON.stringify(
    {
      id: "bot-glue",
      glueJs: "glue.js",
      glueWasm: "glue_bg.wasm",
      built: new Date().toISOString(),
      commit: dirty ? null : commit,
      dirty,
      toolchain: {
        rustc: runOut("rustc", "-V"),
        wasm_bindgen: runOut("wasm-bindgen", "--version"),
      },
    },
    null,
    2,
  ) + "\n",
);
console.log(`webui/public/assets/engine/bot-glue/bundle.json`);
if (dirty) console.log("NOTE: working tree is dirty -- this bundle has no source commit.");