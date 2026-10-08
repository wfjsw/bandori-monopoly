#!/usr/bin/env node
// Rebuild a frozen engine bundle from its source commit (`docs/REPLAY.md` §9).
//
// The git tree records only (bundle id -> commit + `source_id` + toolchain)
// in `archive/engine/index.json`; the wasm itself is never committed. This
// tool recreates the bundle from that commit and decides whether the result
// is the same *engine* -- by source identity and behaviour, not by bytes.
//
// Usage:
//
//   node tools/rebuild-engine.mjs <commit|bundle-id> [--out DIR] [--dist DIR | --no-dist]
//   node tools/rebuild-engine.mjs --verify          # check every rebuildable entry
//
// What makes a rebuild acceptable (`docs/REPLAY.md` §9.5.1):
//
//   1. `source_id` matches the index entry -- the behaviour-relevant inputs
//      (engine + rules + data trees, lockfiles, format/save/abi) are the same.
//      Compilers, wasm-bindgen versions, rust-src presence and paths are NOT
//      part of this and may differ freely.
//   2. The entry's **reference record** (a short seeded match sealed at
//      archive time under `archive/engine/refs/`) replays clean through the
//      rebuilt glue: every checkpoint hash matches, no divergence.
//
// When the rebuilt bytes hash to a different bundle id than the seal (the
// usual case across rustc installs), the rebuilt bytes are kept under their
// own byte-id in the cache and the index entry gains
// `aliases: { <seal id>: <rebuilt id> }` + `rebuilt: {...}`. Records stamped
// with the original id still resolve to the rebuilt bundle
// (`webui/src/game/engineBundle.ts` walks `aliases`).
//
// The commit's own `tools/build-glue.mjs` and `tools/build-ruleset.mjs` run
// inside a temporary `git worktree` in `os.tmpdir()` (outside the repo, so no
// parent `rust-toolchain.toml` / `.cargo/config.toml` can leak into the
// build). The worktree is removed on the way out.
//
// Output goes to `--out` (default `data/engine-cache/`, outside git), then the
// bundle is copied into the served site (`--dist`, default `webui/dist`).
//
// Bundles marked `rebuild: false` were sealed from an uncommitted tree and
// cannot be regenerated -- their bytes live in the persistent store
// (`data/engine-archive/`, see tools/archive-engine.mjs).

import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { bundleId, glueSha256 } from "./engine-bundle.mjs";
import {
  loadGlue,
  sourceId,
  sourceRowsFromGit,
  verifyReferenceRecord,
} from "./engine-source.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const INDEX = join(ROOT, "archive", "engine", "index.json");

function log(...a) {
  console.log(...a);
}

function fail(msg) {
  console.error(`rebuild-engine: ${msg}`);
  process.exit(1);
}

function run(cmd, args, opts = {}) {
  log("+", cmd, args.join(" "));
  execFileSync(cmd, args, { stdio: "inherit", cwd: ROOT, ...opts });
}

function runOut(cmd, args, opts = {}) {
  return execFileSync(cmd, args, { encoding: "utf8", cwd: ROOT, ...opts }).trim();
}

function sha256File(p) {
  return createHash("sha256").update(readFileSync(p)).digest("hex");
}

// ---------------------------------------------------------------- args

function parseArgs(argv) {
  const out = {
    target: null,
    out: join(ROOT, "data", "engine-cache"),
    dist: join(ROOT, "webui", "dist"),
    verify: false,
    keepWorktree: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const next = () => {
      const v = argv[++i];
      if (v == null) fail(`${a}: missing value`);
      return v;
    };
    if (a === "--out") out.out = resolve(next());
    else if (a === "--dist") out.dist = resolve(next());
    else if (a === "--no-dist") out.dist = null;
    else if (a === "--verify") out.verify = true;
    else if (a === "--keep-worktree") out.keepWorktree = true;
    else if (a === "--help" || a === "-h") {
      log(`usage: node tools/rebuild-engine.mjs <commit|bundle-id> [--out DIR] [--dist DIR | --no-dist]
       node tools/rebuild-engine.mjs --verify [--out DIR] [--dist DIR | --no-dist]`);
      process.exit(0);
    } else if (out.target == null && !a.startsWith("--")) out.target = a;
    else fail(`unknown argument ${a}`);
  }
  return out;
}

// ---------------------------------------------------------------- index

function readIndex() {
  if (!existsSync(INDEX)) fail(`no archive index at ${INDEX}`);
  return JSON.parse(readFileSync(INDEX, "utf8"));
}

function writeIndex(index) {
  writeFileSync(INDEX, JSON.stringify(index, null, 1) + "\n");
}

/** Resolve the CLI target to `{ commit, expectId, entry }`. */
function resolveTarget(target, index) {
  if (/^[0-9a-f]{64}$/i.test(target)) {
    const entry = index.bundles.find((b) => b.id === target.toLowerCase());
    if (!entry) fail(`bundle ${target} is not in ${INDEX}`);
    if (entry.rebuild === false) {
      fail(
        `bundle ${target} is marked rebuild:false -- it was sealed from an uncommitted tree\n` +
          `and cannot be regenerated. Its bytes live in the persistent store (data/engine-archive/)\n` +
          `or the deploy's /assets/engine/${target}/. Copy them; do not rebuild.`,
      );
    }
    if (!entry.commit) {
      fail(`bundle ${target} has no source commit recorded; cannot rebuild`);
    }
    return { commit: entry.commit, expectId: entry.id, entry };
  }
  let commit;
  try {
    commit = runOut("git", ["rev-parse", "--verify", `${target}^{commit}`]);
  } catch {
    fail(`not a bundle id or commit: ${target}`);
  }
  const entry = index.bundles.find((b) => b.commit === commit && b.rebuild !== false) ?? null;
  return { commit, expectId: entry?.id ?? null, entry };
}

// ---------------------------------------------------------------- assemble

/** Load the glue in node far enough to ask for its stamp and DATA_FILES. */
function probeGlue(glueDir) {
  const script = `
    import { pathToFileURL } from "node:url";
    import { readFileSync } from "node:fs";
    const dir = process.argv[1];
    const g = await import(pathToFileURL(dir + "/glue.js").href + "?rebuild");
    await g.default({ module_or_path: new Uint8Array(readFileSync(dir + "/glue_bg.wasm")) });
    const stamp = JSON.parse(g.engine_stamp());
    const dataFiles = JSON.parse(g.data_files());
    const api = typeof g.replay_api_version === "function" ? g.replay_api_version() : 1;
    process.stdout.write(JSON.stringify({ stamp, dataFiles, api }));
  `;
  return JSON.parse(
    execFileSync(process.execPath, ["--input-type=module", "-e", script, glueDir], {
      encoding: "utf8",
      cwd: ROOT,
    }),
  );
}

function toolchain() {
  let wasmBindgen = "missing";
  try {
    wasmBindgen = runOut("wasm-bindgen", ["--version"]);
  } catch {
    fail("wasm-bindgen CLI not on PATH (the version is pinned in crates/web-glue)");
  }
  return { rustc: runOut("rustc", ["-V"]), cargo: runOut("cargo", ["-V"]), wasm_bindgen: wasmBindgen };
}

/** The recipe lives in the commit: run *its* build scripts inside the worktree. */
function buildInWorktree(worktree) {
  for (const s of ["tools/build-glue.mjs", "tools/build-ruleset.mjs"]) {
    if (!existsSync(join(worktree, s))) {
      fail(`commit has no ${s} -- too old to rebuild with the pinned recipe`);
    }
  }
  run(process.execPath, ["tools/build-glue.mjs"], { cwd: worktree });
  run(process.execPath, ["tools/build-ruleset.mjs"], { cwd: worktree });
}

/**
 * Assemble the bundle the way `archive-engine.mjs` does: glue + DATA_FILES +
 * ruleset index, hashed by the same recipe. Writes `<out>/<id>/` and
 * `<out>/modules/`; returns the id and stamp.
 */
function assemble(worktree, out) {
  const glueDir = join(worktree, "webui", "src", "wasm");
  const dataDir = join(worktree, "data");
  const rulesDir = join(worktree, "webui", "public", "assets", "rules");
  for (const [what, dir, file] of [
    ["glue", glueDir, "glue.js"],
    ["glue", glueDir, "glue_bg.wasm"],
    ["data", dataDir, "board.json"],
    ["rules", rulesDir, "index.json"],
  ]) {
    if (!existsSync(join(dir, file))) fail(`${what}: no ${file} in ${dir}`);
  }

  const probe = probeGlue(glueDir);
  const rulesIndex = JSON.parse(readFileSync(join(rulesDir, "index.json"), "utf8"));
  const glueJs = readFileSync(join(glueDir, "glue.js"));
  const glueWasm = readFileSync(join(glueDir, "glue_bg.wasm"));
  const gsha = glueSha256(glueJs, glueWasm);

  const hasher = createHash("sha256");
  for (const name of probe.dataFiles) {
    const p = join(dataDir, name);
    if (existsSync(p)) hasher.update(readFileSync(p));
  }
  const dsha = hasher.digest("hex");
  const stamp = {
    format: probe.stamp.format,
    save_version: probe.stamp.save_version,
    abi: probe.stamp.abi,
    ruleset_sha256: String(rulesIndex.ruleset_sha256 || probe.stamp.ruleset_sha256 || "stub"),
    data_sha256: dsha,
    engine: probe.stamp.engine || "game-core",
    build: probe.stamp.build || "",
  };
  const id = bundleId(gsha, stamp);
  if (!id) fail("empty glue sha");
  stamp.bundle = id;

  const modulesDir = join(out, "modules");
  mkdirSync(modulesDir, { recursive: true });
  for (const m of rulesIndex.modules ?? []) {
    const src = join(rulesDir, m.file);
    if (!existsSync(src)) fail(`ruleset module missing: ${src}`);
    copyFileSync(src, join(modulesDir, m.file));
  }

  const bundleDir = join(out, id);
  rmSync(bundleDir, { recursive: true, force: true });
  mkdirSync(join(bundleDir, "data"), { recursive: true });
  mkdirSync(join(bundleDir, "rules"), { recursive: true });
  copyFileSync(join(glueDir, "glue.js"), join(bundleDir, "glue.js"));
  copyFileSync(join(glueDir, "glue_bg.wasm"), join(bundleDir, "glue_bg.wasm"));
  for (const name of probe.dataFiles) {
    const p = join(dataDir, name);
    if (existsSync(p)) copyFileSync(p, join(bundleDir, "data", name));
  }
  writeFileSync(join(bundleDir, "rules", "index.json"), JSON.stringify(rulesIndex));
  writeFileSync(
    join(bundleDir, "bundle.json"),
    JSON.stringify(
      {
        id,
        api: probe.api,
        created: new Date().toISOString(),
        glueSha256: gsha,
        stamp,
        layout: {
          glueJs: "glue.js",
          glueWasm: "glue_bg.wasm",
          dataDir: "data",
          dataFiles: probe.dataFiles,
          rulesIndex: "rules/index.json",
          modulesDir: "../modules",
        },
      },
      null,
      1,
    ) + "\n",
  );
  return {
    id,
    stamp,
    gsha,
    dsha,
    api: probe.api,
    dataFiles: probe.dataFiles,
    bundleDir,
    modulesDir,
    glueDir,
    dataDir,
    rulesDir,
    rulesIndex,
  };
}

/** Per-file hashes of an assembled bundle, store-relative. */
function hashBundle(bundleDir, id, modulesDir, rulesIndex) {
  const files = {};
  const walk = (dir, prefix) => {
    for (const e of readdirSync(dir, { withFileTypes: true })) {
      const p = join(dir, e.name);
      const rel = prefix ? `${prefix}/${e.name}` : e.name;
      if (e.isDirectory()) walk(p, rel);
      else files[rel] = sha256File(p);
    }
  };
  walk(bundleDir, id);
  for (const m of rulesIndex.modules ?? []) files[`modules/${m.file}`] = sha256File(join(modulesDir, m.file));
  return files;
}

// ---------------------------------------------------------------- verify

/**
 * The two checks that make a byte-different rebuild acceptable:
 * `source_id` equality, and a clean replay of the stored reference record.
 */
async function verifyBehaviour({ worktree, commit, built, entry }) {
  // 1. source_id: the behaviour-relevant inputs. The worktree is a clean
  //    checkout of `commit`, so this is the cheap git-tree-id form.
  const rows = sourceRowsFromGit(worktree, commit);
  const sid = sourceId(rows, built.stamp, false);
  log(`  source id    ${sid.slice(0, 16)}…`);
  if (entry?.source_id) {
    if (sid !== entry.source_id) {
      return {
        ok: false,
        source_id: sid,
        reason: `source_id mismatch\n    sealed   ${entry.source_id}\n    rebuilt  ${sid}\n  the behaviour-relevant inputs differ; this is not the same engine`,
      };
    }
    log(`  source id    MATCHES the seal`);
  } else {
    log(`  source id    (no sealed source_id to compare against)`);
  }

  // 2. reference record: a short seeded match sealed at archive time. Replay
  //    it through the REBUILT glue and require every checkpoint to match.
  //    `force` is required: the rebuilt glue's stamp names its own bundle id.
  const refRel = entry?.ref ?? `refs/${entry?.id ?? built.id}.bdrec`;
  const refPath = join(ROOT, "archive", "engine", refRel);
  if (!entry) {
    log(`  ref          (no index entry; skipping behaviour check)`);
    return { ok: true, source_id: sid, ref: null };
  }
  if (!existsSync(refPath)) {
    return {
      ok: false,
      source_id: sid,
      reason: `missing reference record ${refRel} -- cannot verify behaviour`,
    };
  }
  const refBytes = readFileSync(refPath);
  if (entry.ref_sha256 && sha256File(refPath) !== entry.ref_sha256) {
    return {
      ok: false,
      source_id: sid,
      reason: `reference record ${refRel} hash mismatch (record damaged, not a rebuild problem)`,
    };
  }
  log(`  ref          ${refRel}  ${statSync(refPath).size} bytes`);

  const g = await loadGlue({
    glueDir: built.glueDir,
    dataDir: built.dataDir,
    rules: { index: join(built.bundleDir, "rules", "index.json"), modulesDir: built.modulesDir },
    glueSha: built.gsha,
    tag: `verify-${built.id.slice(0, 8)}`,
  });
  const res = verifyReferenceRecord(g, refBytes, true);
  if (!res.ok) {
    return {
      ok: false,
      source_id: sid,
      reason: `reference record diverged: ${res.reason}\n  a checkpoint mismatch means the rebuilt engine behaves differently`,
    };
  }
  log(`  ref          replays clean (${res.ticks} ticks, every checkpoint matched)`);
  return { ok: true, source_id: sid, ref: refRel };
}

// ---------------------------------------------------------------- dist

/** Copy a finished bundle (and its modules) into the served site. */
function copyToDist(bundleDir, modulesDir, dist, alsoAs) {
  const into = join(dist, "assets", "engine");
  mkdirSync(into, { recursive: true });
  const copyTree = (src, dst) => {
    mkdirSync(dst, { recursive: true });
    for (const e of readdirSync(src, { withFileTypes: true })) {
      const s = join(src, e.name);
      const d = join(dst, e.name);
      if (e.isDirectory()) copyTree(s, d);
      else if (!existsSync(d)) copyFileSync(s, d);
    }
  };
  const id = bundleDir.split(/[/\\]/).pop();
  copyTree(bundleDir, join(into, id));
  if (alsoAs && alsoAs !== id) copyTree(bundleDir, join(into, alsoAs));
  copyTree(modulesDir, join(into, "modules"));
  if (existsSync(INDEX)) copyFileSync(INDEX, join(into, "index.json"));
}

// ---------------------------------------------------------------- main

async function rebuildOne(a, commit, expectId, entry) {
  const tc = toolchain();
  log(`rebuild: ${expectId ?? "(new)"} from ${commit}`);
  log(`  rustc        ${tc.rustc}`);
  log(`  wasm-bindgen ${tc.wasm_bindgen}`);
  if (entry?.toolchain) {
    for (const [k, want] of Object.entries(entry.toolchain)) {
      const got = tc[k];
      if (want && got && want !== got) {
        log(`  WARN toolchain ${k}: this build has ${got}`);
        log(`                  the seal recorded  ${want}`);
        log(`       (informational only -- bytes may differ; behaviour is what is verified)`);
      }
    }
  }

  // The worktree goes OUTSIDE the repo: a `rust-toolchain.toml` or
  // `.cargo/config.toml` in any parent of a worktree under `target/` would
  // change the active toolchain / flags for the rebuild (docs/REPLAY.md §9.5.1).
  const wt = join(tmpdir(), `bdre-rebuild-${commit.slice(0, 12)}-${process.pid}`);
  rmSync(wt, { recursive: true, force: true });
  mkdirSync(wt, { recursive: true });
  run("git", ["worktree", "add", "--detach", wt, commit]);
  try {
    buildInWorktree(wt);
    const built = assemble(wt, a.out);

    // -- behaviour verification (source_id + reference record) -------------
    const v = await verifyBehaviour({ worktree: wt, commit, built, entry });
    if (!v.ok) fail(v.reason);

    // -- byte id: same, or aliased ---------------------------------------
    const byteMatch = !expectId || built.id === expectId;
    if (expectId && !byteMatch) {
      log(`  bytes        DIFFER (rebuilt byte id ${built.id.slice(0, 12)}…, seal was ${expectId.slice(0, 12)}…)`);
      log(`               accepted: source_id matches and the reference record replays clean`);
    } else if (expectId) {
      log(`  bytes        identical to the seal (${built.id.slice(0, 12)}…)`);
    }

    // Where the bytes live: always under their true byte id. When that is not
    // the seal's id, also install them at the seal's id so a plain
    // `/assets/engine/<seal>/` fetch works, and record the mapping in
    // `aliases` so the loader resolves the seal id to the rebuilt bundle.
    const serveAs = expectId && !byteMatch ? expectId : null;
    if (serveAs) {
      const aliasDir = join(a.out, serveAs);
      rmSync(aliasDir, { recursive: true, force: true });
      const copyTree = (src, dst) => {
        mkdirSync(dst, { recursive: true });
        for (const e of readdirSync(src, { withFileTypes: true })) {
          const s = join(src, e.name);
          const d = join(dst, e.name);
          if (e.isDirectory()) copyTree(s, d);
          else copyFileSync(s, d);
        }
      };
      copyTree(built.bundleDir, aliasDir);
      const files = hashBundle(built.bundleDir, built.id, built.modulesDir, built.rulesIndex);
      const index = readIndex();
      const at = index.bundles.findIndex((b) => b.id === expectId);
      if (at >= 0) {
        const e = index.bundles[at];
        e.aliases = { ...(e.aliases ?? {}), [expectId]: built.id, [built.id]: built.id };
        e.rebuilt = {
          byte_id: built.id,
          source_id: v.source_id,
          verified: true,
          ref: v.ref,
          verified_at: new Date().toISOString(),
          toolchain: tc,
        };
        e.files = Object.fromEntries(
          Object.entries(files).map(([k, val]) => [
            k.startsWith("modules/") ? k : k.replace(built.id, serveAs),
            val,
          ]),
        );
        writeIndex(index);
        log(`  aliases      ${expectId.slice(0, 12)}… -> ${built.id.slice(0, 12)}…  (written to the index)`);
      }
    } else if (expectId && v.source_id) {
      const index = readIndex();
      const at = index.bundles.findIndex((b) => b.id === expectId);
      if (at >= 0 && !index.bundles[at].source_id) {
        index.bundles[at].source_id = v.source_id;
        writeIndex(index);
      }
    }

    log(`bundle   ${built.id}`);
    log(`  glue sha     ${built.gsha.slice(0, 16)}…`);
    log(`  source id    ${(v.source_id ?? "").slice(0, 16)}…`);
    log(`  ruleset      ${built.stamp.ruleset_sha256.slice(0, 16)}…`);
    log(`  data sha     ${built.dsha.slice(0, 16)}…`);
    log(`  out          ${built.bundleDir}`);
    if (a.dist) {
      copyToDist(built.bundleDir, built.modulesDir, a.dist, serveAs);
      log(`  dist         ${join(a.dist, "assets", "engine", built.id)}` + (serveAs ? ` (+ ${serveAs.slice(0, 12)}…)` : ""));
    }
    return built;
  } finally {
    if (a.keepWorktree) {
      log(`  worktree kept at ${wt}`);
    } else {
      try {
        run("git", ["worktree", "remove", "--force", wt]);
      } catch {
        rmSync(wt, { recursive: true, force: true });
        run("git", ["worktree", "prune"], {});
      }
    }
  }
}

async function main() {
  const a = parseArgs(process.argv.slice(2));
  const index = readIndex();
  if (a.verify) {
    let ok = 0;
    let bad = 0;
    for (const e of index.bundles) {
      if (e.rebuild === false) {
        log(`skip  ${e.id.slice(0, 12)}…  rebuild:false (stored bytes only)`);
        continue;
      }
      if (!e.commit) {
        log(`skip  ${e.id.slice(0, 12)}…  no commit recorded`);
        continue;
      }
      try {
        await rebuildOne({ ...a, dist: null }, e.commit, e.id, e);
        ok++;
      } catch (err) {
        console.error(String(err?.message ?? err));
        bad++;
      }
    }
    log(`verify: ${ok} rebuilt and behaviour-verified, ${bad} failed`);
    if (bad) process.exit(1);
    return;
  }
  if (!a.target) fail("need a commit or bundle id (or --verify)");
  const { commit, expectId, entry } = resolveTarget(a.target, index);
  await rebuildOne(a, commit, expectId, entry);
}

main().catch((e) => {
  console.error(e?.stack ?? e);
  process.exit(1);
});