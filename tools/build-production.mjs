#!/usr/bin/env node
// Production build: everything a deploy needs, in the order replays require
// (docs/REPLAY.md §9: the engine is archived BEFORE the new dist goes live).
//
//   node tools/build-production.mjs [--skip-server]
//
//   1. build-ruleset   -> dist/cards, webui/public/assets/rules
//   2. build-glue      -> webui/src/wasm
//   3. build-bot-glue  -> webui/public/assets/engine/bot-glue
//   4. webui build     (NODE_ENV=production) -> webui/dist
//   5. archive-engine  seal this engine into data/engine-archive, copy every
//                      stored bundle into webui/dist/assets/engine
//   6. archive-engine --check   (fails the build on a missing / bad bundle)
//   7. cargo build --release    server, bot-service, rules-worker
//
// Everything is written under the checkout this script lives in; run it from
// a worktree, never from the tree a dev server is serving. CARGO_TARGET_DIR is
// dropped (build-ruleset would stage stale card wasm with it set).
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const env = { ...process.env, NODE_ENV: "production" };
delete env.CARGO_TARGET_DIR;
const skipServer = process.argv.includes("--skip-server");

function step(name, cmd, args, cwd = ROOT) {
  console.log(`\n== ${name}`);
  const r = spawnSync(cmd, args, { cwd, env, stdio: "inherit" });
  if (r.status !== 0) {
    console.error(`build-production: "${name}" failed (exit ${r.status ?? r.signal})`);
    process.exit(r.status || 1);
  }
}

const node = process.execPath;
step("ruleset", node, ["tools/build-ruleset.mjs"]);
step("glue", node, ["tools/build-glue.mjs"]);
step("bot glue", node, ["tools/build-bot-glue.mjs"]);
step("webui", "npm", ["run", "build"], join(ROOT, "webui"));
step("archive engine", node, ["tools/archive-engine.mjs"]);
step("verify archive", node, ["tools/archive-engine.mjs", "--check"]);
if (!skipServer) {
  step("server", "cargo", ["build", "--release", "-p", "server", "-p", "bot-service", "-p", "rules-worker"]);
}
console.log("\nbuild-production: done (webui/dist, dist/cards, data/, target/release/{server,bot-service,rules-worker})");
