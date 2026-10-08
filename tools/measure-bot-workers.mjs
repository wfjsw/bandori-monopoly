#!/usr/bin/env node
// Measure the advanced-bot worker bundle (`docs/BOT.md` B6): iterations per
// decision at 1 vs 4 workers, on the real built ruleset, plus the bundle size.
//
//   node tools/measure-bot-workers.mjs [--budgets 1000,3000] [--workers 1,4]
//
// Each "worker" is a node worker_thread running its own `bot-glue` wasm
// instance (the same module the browser Worker imports) with its own
// determinization stream (`seed_for_thread(seed, i)`), exactly the page's
// root-parallel shape. Views come from a live solo match (web-glue).

import { Worker, isMainThread, parentPort, workerData } from "node:worker_threads";
import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const WEBUI = join(ROOT, "webui");
const BOT_DIR = join(WEBUI, "public", "assets", "engine", "bot-glue");
const WEB_GLUE_DIR = join(WEBUI, "src", "wasm");
const DATA_DIR = join(ROOT, "data");

// ---------------------------------------------------------------- worker

if (!isMainThread) {
  const { glueJs, glueWasm, dataDir, rulesDir, view, budgetMs, seed } = workerData;
  const glue = (await import(pathToFileURL(glueJs).href + "?mw" + seed));
  await glue.default({ module_or_path: new Uint8Array(readFileSync(glueWasm)) });
  const names = JSON.parse(glue.data_files());
  const files = {};
  for (const n of names) {
    const p = join(dataDir, n);
    if (existsSync(p)) files[n] = readFileSync(p, "utf8");
  }
  glue.load_data(JSON.stringify(files));
  const index = JSON.parse(readFileSync(join(rulesDir, "index.json"), "utf8"));
  for (const m of index.modules) glue.ruleset_add(new Uint8Array(readFileSync(join(rulesDir, m.file))));
  if (index.conds?.file) glue.ruleset_precompiled(new Uint8Array(readFileSync(join(rulesDir, index.conds.file))));
  glue.ruleset_build();
  const r = JSON.parse(glue.decide(JSON.stringify(view), budgetMs, seed));
  parentPort.postMessage(r);
  process.exit(0);
}

// ---------------------------------------------------------------- helpers

function rulesDir() {
  for (const d of [join(WEBUI, "public", "assets", "rules"), join(ROOT, "dist", "cards")]) {
    if (existsSync(join(d, "index.json"))) return d;
  }
  throw new Error("no built ruleset -- run tools/build-ruleset.mjs");
}

function splitmix64(x) {
  const M = 0xffffffffffffffffn;
  const z0 = (x + 0x9e3779b97f4a7c15n) & M;
  const z1 = ((z0 ^ (z0 >> 30n)) * 0xbf58476d1ce4e5b9n) & M;
  const z2 = ((z1 ^ (z1 >> 27n)) * 0x94d049bb133111ebn) & M;
  return (z2 ^ (z2 >> 31n)) & M;
}
function seedForThread(seed, thread) {
  if (thread === 0) return seed >>> 0;
  const M = 0xffffffffffffffffn;
  const mix = (0x9e3779b97f4a7c15n * BigInt(thread + 1)) & M;
  return Number(splitmix64((BigInt(seed >>> 0) ^ mix) & M) & 0xffffffffn);
}

function decisionAt(state, playerId) {
  if (state.phase !== "play") return null;
  if (state.prompt.id > 0) {
    const k = state.prompt.players.indexOf(playerId);
    return k >= 0 && (state.prompt.answers[k] ?? -1) < 0 ? state.prompt.id : null;
  }
  if (state.busy || state.turn !== playerId) return null;
  return state.step === 2 || state.step === 4 ? 0 : null;
}

async function loadWebGlue() {
  const glue = (await import(pathToFileURL(join(WEB_GLUE_DIR, "glue.js")).href + "?meas"));
  await glue.default({ module_or_path: new Uint8Array(readFileSync(join(WEB_GLUE_DIR, "glue_bg.wasm"))) });
  const names = JSON.parse(glue.data_files());
  const files = {};
  for (const n of names) {
    const p = join(DATA_DIR, n);
    if (existsSync(p)) files[n] = readFileSync(p, "utf8");
  }
  glue.load_data(JSON.stringify(files));
  const dir = rulesDir();
  const index = JSON.parse(readFileSync(join(dir, "index.json"), "utf8"));
  for (const m of index.modules) glue.ruleset_add(new Uint8Array(readFileSync(join(dir, m.file))));
  if (index.conds?.file) glue.ruleset_precompiled(new Uint8Array(readFileSync(join(dir, index.conds.file))));
  glue.ruleset_build();
  return glue;
}

/** Searched decision views from a short live match (only `legal_actions` non-empty). */
async function searchedViews(limit) {
  const glue = await loadWebGlue();
  // bot-glue's `decide` decides whether a surface is searched; cheap to ask.
  const bot = (await import(pathToFileURL(join(BOT_DIR, "glue.js")).href + "?views"));
  await bot.default({ module_or_path: new Uint8Array(readFileSync(join(BOT_DIR, "glue_bg.wasm"))) });
  const names = JSON.parse(bot.data_files());
  const files = {};
  for (const n of names) {
    const p = join(DATA_DIR, n);
    if (existsSync(p)) files[n] = readFileSync(p, "utf8");
  }
  bot.load_data(JSON.stringify(files));
  const dir = rulesDir();
  const index = JSON.parse(readFileSync(join(dir, "index.json"), "utf8"));
  for (const m of index.modules) bot.ruleset_add(new Uint8Array(readFileSync(join(dir, m.file))));
  if (index.conds?.file) bot.ruleset_precompiled(new Uint8Array(readFileSync(join(dir, index.conds.file))));
  bot.ruleset_build();

  const seats = [
    { id: 1, player: "P", character: "户山香澄", cnId: "", ready: true, host: true, bot: false, away: false, mentality: "standard" },
    { id: 2, player: "B1", character: "美竹兰", cnId: "", ready: true, host: false, bot: true, away: false, mentality: "standard" },
    { id: 3, player: "B2", character: "仓田真白", cnId: "", ready: true, host: false, bot: true, away: false, mentality: "standard" },
    { id: 4, player: "B3", character: "花园多惠", cnId: "", ready: true, host: false, bot: true, away: false, mentality: "standard" },
  ];
  const out = [];
  const seen = new Set();
  for (const seed of [20261008, 42, 999]) {
    if (out.length >= limit) break;
    const m = new glue.SoloMatch(JSON.stringify(seats), seed, 0, "");
    try {
      m.quick_start();
      for (let step = 0; step < 300 && out.length < limit; step++) {
        m.tick_steps(10);
        for (let member = 1; member <= 4; member++) {
          const v = JSON.parse(m.view(member));
          if (decisionAt(v.state, v.playerId) == null) continue;
          const key = `${v.state.seq}:${v.state.prompt?.id ?? 0}:${member}`;
          if (seen.has(key)) continue;
          seen.add(key);
          // Only surfaces the search would branch on.
          bot.reset_cache();
          const probe = JSON.parse(bot.decide(JSON.stringify(v), 1, 1));
          if (probe.heuristic) continue;
          out.push(v);
        }
        if (m.ended()) break;
      }
    } finally {
      m.free?.();
    }
  }
  return out;
}

function spawnWorker(view, budgetMs, seed) {
  return new Promise((resolve, reject) => {
    const w = new Worker(fileURLToPath(import.meta.url), {
      workerData: {
        glueJs: join(BOT_DIR, "glue.js"),
        glueWasm: join(BOT_DIR, "glue_bg.wasm"),
        dataDir: DATA_DIR,
        rulesDir: rulesDir(),
        view,
        budgetMs,
        seed,
      },
    });
    w.once("message", (r) => {
      w.terminate();
      resolve(r);
    });
    w.once("error", reject);
  });
}

// ---------------------------------------------------------------- main

const args = process.argv.slice(2);
const budgets = (args.find((a) => a.startsWith("--budgets="))?.split("=")[1] ?? "1000,3000")
  .split(",")
  .map(Number);
const workerCounts = (args.find((a) => a.startsWith("--workers="))?.split("=")[1] ?? "1,4")
  .split(",")
  .map(Number);

const wasmBytes = statSync(join(BOT_DIR, "glue_bg.wasm")).size;
const jsBytes = statSync(join(BOT_DIR, "glue.js")).size;
console.log(`bot-glue bundle: glue_bg.wasm ${wasmBytes} bytes (${(wasmBytes / 1024 / 1024).toFixed(2)} MiB), glue.js ${jsBytes} bytes`);
console.log(`ruleset: ${rulesDir()}`);
console.log(`budgets: ${budgets.join(", ")} ms   workers: ${workerCounts.join(", ")}\n`);

const views = await searchedViews(6);
console.log(`searched decision views: ${views.length}\n`);
if (!views.length) {
  console.error("no searched surfaces in the sample -- cannot measure");
  process.exit(1);
}

console.log("| workers | budget | views | iters/decision | wall ms (mean) | scale-up |");
console.log("|---|---|---|---|---|---|");
const byKey = new Map();
for (const budgetMs of budgets) {
  let baseIters = 0;
  let baseWall = 0;
  for (const n of workerCounts) {
    let iters = 0;
    let wall = 0;
    for (const v of views) {
      const seed = 12345;
      const t0 = Date.now();
      const rs = await Promise.all(
        Array.from({ length: n }, (_, i) => spawnWorker(v, budgetMs, seedForThread(seed, i))),
      );
      wall += Date.now() - t0;
      iters += rs.reduce((a, r) => a + (r.iterations ?? 0), 0);
    }
    const meanIters = iters / views.length;
    const meanWall = wall / views.length;
    if (n === workerCounts[0]) {
      baseIters = meanIters;
      baseWall = meanWall;
    }
    const scale = baseIters > 0 ? (meanIters / baseIters).toFixed(2) + "x" : "-";
    byKey.set(`${n}@${budgetMs}`, { meanIters, meanWall });
    console.log(
      `| ${n} | ${budgetMs} | ${views.length} | ${meanIters.toFixed(1)} | ${meanWall.toFixed(0)} | ${scale} |`,
    );
  }
}
console.log(`\nbundle: ${(wasmBytes / 1024 / 1024).toFixed(2)} MiB wasm + ${(jsBytes / 1024).toFixed(1)} KiB js`);