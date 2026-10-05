#!/usr/bin/env node
// Build the web-served Live2D assets for every model.
//
//   live2d/<id>.cxx3             Quadrism OMG3 archive (input)
//     tools/live2d/unpack.py  -> live2d-src/<id>/{main.xml,imageFileBuf*.png}
//     tools/live2d/convert.py -> webui/public/assets/live2d/<id>/{model.json,texture_<n>.png}
//     StreamingAssets copy    -> webui/public/assets/live2d/<id>/{idle.mtn,*.mtn,physics.json}
//
// Every output directory holds ONLY model.json + texture_<n>.png and the
// motion/physics files the stands play (`*.mtn`, `physics.json`) -- never
// main.xml (the raw editor XML is ~5 MB per model and must not ship) or
// model.moc. Models whose outputs are newer than their inputs are skipped, so
// the webui build can run this every time (webui/package.json `prebuild`).
//
// Runs on any platform Node runs on (no shell). The heavy conversions
// (unpack/convert/selfcheck) are Python and run ONLY when a model is actually
// stale -- a fresh `npm run build` needs no Python at all.
//
//   node tools/live2d/build.mjs
//   FORCE=1 CHECK=1 node tools/live2d/build.mjs
//
// Environment:
//   FORCE=1              rebuild every model regardless of timestamps
//   CHECK=1              self-check every model instead of only rebuilt ones
//   GAME_LIVE2D=<path>   StreamingAssets/BandoriLive2D override (default: next
//                        to this repo); the .mtn / physics.json copies come
//                        from there and are what catalog.json indexes per id.

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, unlinkSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const OUT = join(ROOT, "webui", "public", "assets", "live2d");
const SRC = join(ROOT, "live2d-src");
const GAME_LIVE2D = resolve(ROOT, process.env.GAME_LIVE2D
  ?? "../BandoriMonopoly_Data/StreamingAssets/BandoriLive2D");

const py = (...args) => {
  console.log("+", "python", args.join(" "));
  execFileSync("python", args, { cwd: ROOT, stdio: "inherit" });
};

/** Unpack is due when the archive is newer than its raw extract; convert when
 *  the raw extract is newer than the served model.json (or a texture named by
 *  model.json is missing). A model that needs unpacking also needs converting. */
function stale() {
  const force = !!process.env.FORCE;
  const unpack = [], convert = [];
  const cxx3s = readdirSync(join(ROOT, "live2d"))
    .filter((f) => f.endsWith(".cxx3"))
    .sort();
  for (const f of cxx3s) {
    const mid = f.slice(0, -".cxx3".length);
    const cxx3 = join(ROOT, "live2d", f);
    const xml = join(SRC, mid, "main.xml");
    const model = join(OUT, mid, "model.json");
    if (force || !existsSync(xml) || mtime(cxx3) > mtime(xml)) {
      unpack.push(mid);
      convert.push(mid);
      continue;
    }
    if (force || !existsSync(model) || mtime(xml) > mtime(model) || mtime(cxx3) > mtime(model)) {
      convert.push(mid);
      continue;
    }
    let tex;
    try {
      tex = JSON.parse(readFileSync(model, "utf8")).textures ?? [];
    } catch {
      convert.push(mid);
      continue;
    }
    if (tex.some((t) => !existsSync(join(OUT, mid, t)) || statSync(join(OUT, mid, t)).size === 0)) {
      convert.push(mid);
    }
  }
  return { unpack, convert };
}

const mtime = (p) => (existsSync(p) ? statSync(p).mtimeMs : 0);

/** The .mtn curves and physics.json the stands play (catalog.json indexes them
 *  per id). Skipped quietly when the game tree is not present -- the models
 *  then just hold their default pose. */
function copyMotions() {
  if (!existsSync(GAME_LIVE2D)) {
    console.error(`live2d: no game tree at ${GAME_LIVE2D} -- skipping motion/physics copy`);
    return;
  }
  for (const dir of readdirSync(OUT, { withFileTypes: true })) {
    if (!dir.isDirectory()) continue;
    const src = join(GAME_LIVE2D, dir.name);
    if (!existsSync(src)) continue;
    const files = readdirSync(src).filter((f) => f.endsWith(".mtn") || f === "physics.json");
    for (const f of files) {
      const from = join(src, f), to = join(OUT, dir.name, f);
      if (!existsSync(to) || mtime(from) > mtime(to)) copyFileSync(from, to);
    }
  }
}

/** The served tree never carries main.xml / model.moc. */
function sweep() {
  for (const dir of readdirSync(OUT, { withFileTypes: true })) {
    if (!dir.isDirectory()) continue;
    for (const f of readdirSync(join(OUT, dir.name))) {
      const ok = f === "model.json" || f === "physics.json" || f.endsWith(".mtn") || f.startsWith("texture_");
      if (ok) continue;
      rmSync(join(OUT, dir.name, f), { recursive: true, force: true });
    }
  }
}

const { unpack, convert } = stale();
mkdirSync(SRC, { recursive: true });
mkdirSync(OUT, { recursive: true });

for (const mid of unpack) py("tools/live2d/unpack.py", mid);
if (convert.length) py("tools/live2d/convert.py", ...convert);

copyMotions();
sweep();

if (process.env.CHECK) py("tools/live2d/selfcheck.py");
else if (convert.length) py("tools/live2d/selfcheck.py", ...convert);

const files = [];
(function walk(d) {
  for (const e of readdirSync(d, { withFileTypes: true })) {
    const p = join(d, e.name);
    if (e.isDirectory()) walk(p);
    else files.push(p);
  }
})(OUT);
const count = (pred) => files.filter(pred).length;
const total = files.reduce((n, f) => n + statSync(f).size, 0);
console.log(`live2d assets in ${OUT}:`);
console.log(`  ${files.length} files, ${(total / 1e6).toFixed(1)} MB`);
console.log(`  model.json ${count((f) => f.endsWith("model.json"))}, `
  + `textures ${count((f) => /texture_/.test(f))}`);
console.log(`  motions ${count((f) => f.endsWith(".mtn"))}, `
  + `physics ${count((f) => f.endsWith("physics.json"))}`);