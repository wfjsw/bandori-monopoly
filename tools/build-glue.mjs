#!/usr/bin/env node
// Build crates/web-glue for the browser and generate its JS bindings into
// webui/src/wasm/ (imported by webui/src/core/data.ts).
//
// Runs on any platform Node runs on (no shell). Usage:
//
//   node tools/build-glue.mjs

import { execFileSync } from "node:child_process";
import { statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const run = (...args) => {
  console.log("+", args.join(" "));
  execFileSync(args[0], args.slice(1), { cwd: ROOT, stdio: "inherit" });
};

run("cargo", "build", "-p", "web-glue", "--target", "wasm32-unknown-unknown", "--release");
run("wasm-bindgen", "--target", "web", "--out-dir", "webui/src/wasm", "--out-name", "glue",
    join(ROOT, "target", "wasm32-unknown-unknown", "release", "web_glue.wasm"));
console.log(`webui/src/wasm/glue_bg.wasm: ${statSync(join(ROOT, "webui", "src", "wasm", "glue_bg.wasm")).size} bytes`);