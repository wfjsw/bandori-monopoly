#!/usr/bin/env node
// Explicit, optional preparation of Quadrism archives from a Unity build.
// Not invoked by build.mjs or the npm prebuild; existing archives are kept.
// GAME_LIVE2D=<path> QUADRISM=/path/to/quadexec node tools/live2d/prepare.mjs
// FORCE=1 opts into replacing existing archives.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, renameSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const GAME = resolve(ROOT, process.env.GAME_LIVE2D
  ?? "../BandoriMonopoly_Data/StreamingAssets/BandoriLive2D");
const ARCHIVES = join(ROOT, "live2d");

if (!existsSync(GAME)) throw new Error(`Live2D source directory does not exist: ${GAME}`);
mkdirSync(ARCHIVES, { recursive: true });
for (const dir of readdirSync(GAME, { withFileTypes: true })) {
  if (!dir.isDirectory()) continue;
  const src = join(GAME, dir.name);
  const moc = join(src, "model.moc");
  if (!existsSync(moc)) continue;
  const archive = join(ARCHIVES, `${dir.name}.cxx3`);
  if (existsSync(archive) && !process.env.FORCE) continue;
  const quad = process.env.QUADRISM;
  if (!quad) throw new Error(`live2d ${dir.name}: set QUADRISM=/path/to/quadexec (see docs/LIVE2D.md)`);
  const textures = readdirSync(src).filter((f) => /^texture_\d+\.png$/.test(f))
    .sort((a, b) => a.localeCompare(b, "en", { numeric: true }))
    .map((f) => join(src, f));
  const tempDir = mkdtempSync(join(ARCHIVES, ".prepare-"));
  const temp = join(tempDir, `${dir.name}.cxx3`);
  try {
    console.log(`+ Quadrism ${dir.name}`);
    execFileSync(quad, ["conv", moc, temp, "--tex", "atlas", ...textures.flatMap((t) => ["--texture", t])],
      { cwd: ROOT, stdio: "inherit" });
    renameSync(temp, archive);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}
