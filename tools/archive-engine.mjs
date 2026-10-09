#!/usr/bin/env node
// Freeze the current replay engine into the versioned engine archive
// (`docs/REPLAY.md` §9). Run this at the end of every build/deploy, BEFORE
// the new dist goes live: every `.bdrec` names the bundle that wrote it, and
// the replay player loads that bundle instead of re-simulating with whatever
// engine is current.
//
// Usage:
//
//   node tools/archive-engine.mjs                       # this repo's build
//   node tools/archive-engine.mjs --from <deployed-dir> # a snapshot (glue/ + data/ + dist/assets/rules/)
//   node tools/archive-engine.mjs --check               # verify the store covers every indexed bundle
//   node tools/archive-engine.mjs --backup <dir>        # copy the persistent store aside (no upload)
//
// Source layout (auto-detected, first hit wins; override with flags):
//
//   glue   --glue DIR   <from>/glue.js + glue_bg.wasm
//                       <from>/webui/src/wasm/          (a repo root)
//                       webui/src/wasm/
//   data   --data DIR   <from>/data/    (a snapshot) or data/
//   rules  --rules DIR  <from>/dist/assets/rules/ or <from>/assets/rules/
//                       or webui/public/assets/rules/
//
// Two directories, deliberately split so the wasm never lands in git:
//
//   --archive DIR   (default `archive/engine/`)   index.json only -- versioned
//   --store DIR     (default `data/engine-archive/`)  bundle bytes + modules/
//                                                   persistent, outside git
//   --cache DIR     (default `data/engine-cache/`)   tools/rebuild-engine.mjs output
//
// When `--archive` is passed explicitly and `--store` is not, the store defaults
// to the archive dir -- the self-contained layout tools/test-replay-archive.mjs
// (and older checkouts) expect.
//
// Each index entry records how it can be reproduced:
//
//   commit / toolchain / rebuild     -- clean tree: `tools/rebuild-engine.mjs <id>`
//   dirty: true, rebuild: false      -- sealed from a dirty or unknown tree;
//                                       the stored bytes are the only copy
//   files: {relpath: sha256}         -- per-file hashes, verified by --check
//
// `--dist DIR` (default `webui/dist`) copies the store + index into
// `DIR/assets/engine/`, hardlinking when it can. Nothing under dist is
// deleted -- the live site only ever gains files here.
//
// Keeps only what replay needs: no audio, no live2d, no UI. A bundle is
// ~6 MB + the shared modules pool (one copy of the ruleset per archive).

import {
  copyFileSync,
  existsSync,
  linkSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { bundleId, glueSha256, stampKey } from "./engine-bundle.mjs";
import {
  makeReferenceRecord,
  sourceId,
  sourceRowsFromFiles,
  sourceRowsFromGit,
} from "./engine-source.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

// ---------------------------------------------------------------- args

function parseArgs(argv) {
  const out = {
    from: null,
    glue: null,
    data: null,
    rules: null,
    archive: join(ROOT, "archive", "engine"),
    store: join(ROOT, "data", "engine-archive"),
    storeExplicit: false,
    cache: join(ROOT, "data", "engine-cache"),
    dist: join(ROOT, "webui", "dist"),
    current: true,
    check: false,
    backup: null,
    forceDirty: false,
  };
  let archiveExplicit = false;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const next = () => {
      const v = argv[++i];
      if (v == null) throw new Error(`${a}: missing value`);
      return v;
    };
    if (a === "--from") out.from = resolve(next());
    else if (a === "--glue") out.glue = resolve(next());
    else if (a === "--data") out.data = resolve(next());
    else if (a === "--rules") out.rules = resolve(next());
    else if (a === "--archive") {
      out.archive = resolve(next());
      archiveExplicit = true;
    } else if (a === "--store") {
      out.store = resolve(next());
      out.storeExplicit = true;
    } else if (a === "--cache") out.cache = resolve(next());
    else if (a === "--dist") out.dist = resolve(next());
    else if (a === "--no-dist") out.dist = null;
    else if (a === "--not-current") out.current = false;
    else if (a === "--check") out.check = true;
    else if (a === "--backup") out.backup = resolve(next());
    else if (a === "--force-dirty") out.forceDirty = true;
    else if (a === "--help" || a === "-h") {
      console.log(`usage: node tools/archive-engine.mjs [--from DIR] [--glue DIR] [--data DIR]
           [--rules DIR] [--archive DIR] [--store DIR] [--cache DIR]
           [--dist DIR | --no-dist] [--not-current] [--check] [--backup DIR] [--force-dirty]`);
      process.exit(0);
    } else throw new Error(`unknown argument ${a}`);
  }
  // Self-contained mode: an explicit --archive with no --store keeps bytes
  // beside the index (the layout the replay-archive test pins).
  if (archiveExplicit && !out.storeExplicit) out.store = out.archive;
  return out;
}

// ---------------------------------------------------------------- source

function firstDir(candidates, what) {
  for (const c of candidates) {
    if (c && existsSync(c)) return c;
  }
  throw new Error(`${what} not found (tried: ${candidates.filter(Boolean).join(", ")})`);
}

function resolveSource(a) {
  const from = a.from;
  const glue = a.glue ?? firstDir(
    [
      from && existsSync(join(from, "glue.js")) ? from : null,
      from && existsSync(join(from, "webui", "src", "wasm", "glue.js")) ? join(from, "webui", "src", "wasm") : null,
      join(ROOT, "webui", "src", "wasm"),
    ],
    "glue",
  );
  const data = a.data ?? firstDir(
    [from && join(from, "data"), join(ROOT, "data")],
    "game data",
  );
  const rules = a.rules ?? firstDir(
    [
      from && join(from, "dist", "assets", "rules"),
      from && join(from, "assets", "rules"),
      from && join(from, "webui", "public", "assets", "rules"),
      join(ROOT, "webui", "public", "assets", "rules"),
    ],
    "ruleset modules",
  );
  for (const [what, dir, file] of [
    ["glue.js", glue, "glue.js"],
    ["glue_bg.wasm", glue, "glue_bg.wasm"],
    ["data", data, "board.json"],
    ["rules/index.json", rules, "index.json"],
  ]) {
    if (!existsSync(join(dir, file))) throw new Error(`${what}: no ${file} in ${dir}`);
  }
  return { glue, data, rules };
}

// ---------------------------------------------------------------- helpers

function dirBytes(dir) {
  let n = 0;
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    n += e.isDirectory() ? dirBytes(p) : statSync(p).size;
  }
  return n;
}

function mib(n) {
  return `${(n / (1024 * 1024)).toFixed(1)} MiB`;
}

function sha256File(p) {
  return createHash("sha256").update(readFileSync(p)).digest("hex");
}

/**
 * Copy `src` to `dst`. Hardlinks only between frozen trees (store <-> dist).
 * NEVER hardlink a build output (`webui/src/wasm/`, a worktree's `webui/…`)
 * into the store: a later `build-glue.mjs` writes `glue_bg.wasm` in place, and
 * on NTFS that write goes through the hardlink and destroys the archived
 * bytes. (This happened to bundle `e4e20956…` on 2026-10-08; the seal's wasm
 * is unrecoverable from this machine.) When in doubt, copy.
 */
function copyOrLink(src, dst) {
  // Always a real copy. Hardlinks were tried twice and both times a live file
  // (webui/src/wasm/glue*, data/*.json) ended up sharing an inode with an
  // archived bundle, so an in-place rebuild destroyed archived bytes
  // (e4e20956…'s glue_bg.wasm, then its glue.js, 2026-10-08). The archive is
  // small; the disk cost of copies is worth never aliasing a frozen file.
  mkdirSync(dirname(dst), { recursive: true });
  if (existsSync(dst)) return;
  copyFileSync(src, dst);
}

function copyTree(src, dst) {
  mkdirSync(dst, { recursive: true });
  for (const e of readdirSync(src, { withFileTypes: true })) {
    const s = join(src, e.name);
    const d = join(dst, e.name);
    if (e.isDirectory()) copyTree(s, d);
    else copyOrLink(s, d);
  }
}

// ---------------------------------------------------------------- git / toolchain

/** How this bundle can be reproduced -- `docs/REPLAY.md` §9.
 *  `sourceRoot` is the tree the bytes were built from (a `--from` worktree, or
 *  this repo). Provenance and `source_id` are about THAT tree, not about
 *  whatever the main checkout has been doing since.
 *
 *  Prefers `engine_id.json` (written by tools/build-glue.mjs at *build* time)
 *  over a live git probe: the glue may have been built earlier and the tree
 *  moved on since, so only the build-time stamp is authoritative. */
function provenance(a, glueDir, gsha, sourceRoot, dataFiles) {
  const toolchain = { rustc: "unknown", cargo: "unknown", wasm_bindgen: "unknown" };
  const out = (cmd, args, cwd) => {
    try {
      return execFileSync(cmd, args, {
        encoding: "utf8",
        cwd: cwd ?? sourceRoot,
        stdio: ["ignore", "pipe", "ignore"],
      }).trim();
    } catch {
      return null;
    }
  };
  toolchain.rustc = out("rustc", ["-V"], ROOT) ?? "unknown";
  toolchain.cargo = out("cargo", ["-V"], ROOT) ?? "unknown";
  toolchain.wasm_bindgen = out("wasm-bindgen", ["--version"], ROOT) ?? "unknown";

  // Build-time stamp from tools/build-glue.mjs, when it matches these bytes.
  const idPath = join(glueDir, "engine_id.json");
  if (existsSync(idPath) && !a.forceDirty) {
    try {
      const eid = JSON.parse(readFileSync(idPath, "utf8"));
      if (eid.glueSha256 === gsha && eid.toolchain) {
        return {
          commit: eid.commit ?? null,
          dirty: eid.dirty ?? true,
          rebuild: !eid.dirty && !!eid.commit,
          toolchain: eid.toolchain,
          dirtyPaths: eid.dirty ? ["(dirty at build time; see engine_id.json)"] : [],
          sourceRoot,
          dataFiles,
        };
      }
    } catch {
      /* fall through to the live probe */
    }
  }

  let commit = null;
  let dirty = true;
  let dirtyPaths = [];
  if (!a.forceDirty) {
    commit = out("git", ["rev-parse", "HEAD"]);
    if (commit) {
      // Inputs that define behaviour (`tools/engine-source.mjs`): engine
      // crates, the guest rules, the cel fork, the lockfiles / manifests and
      // the game tables. Anything else (webui, docs, bots, build recipes) does
      // not change how a record replays.
      const paths = [
        "Cargo.toml",
        "Cargo.lock",
        "crates/game-core",
        "crates/game-rules",
        "crates/web-glue",
        "crates/rules-cond",
        "third_party/cel-rust",
        "rules",
        "data",
      ];
      const st = out("git", ["status", "--porcelain", "--", ...paths]) ?? "";
      dirtyPaths = st.split("\n").filter(Boolean);
      dirty = dirtyPaths.length > 0;
    }
  }
  // A clean tree is the only thing `tools/rebuild-engine.mjs` can regenerate:
  // it checks the commit out and runs *that* commit's build scripts.
  const rebuild = !dirty && !!commit;
  return { commit, dirty, rebuild, toolchain, dirtyPaths, sourceRoot, dataFiles };
}

/** `source_id` for the tree the bytes came from. Clean: git tree/blob ids of
 *  `commit`. Dirty: sha256 of the same file set's contents. */
function computeSourceId(prov, stamp) {
  const dirty = prov.dirty || !prov.commit;
  const rows = dirty
    ? sourceRowsFromFiles(prov.sourceRoot, prov.dataFiles)
    : sourceRowsFromGit(prov.sourceRoot, prov.commit);
  return sourceId(rows, stamp, dirty);
}

// ---------------------------------------------------------------- glue probe

/** Load the glue in node far enough to ask it for its own stamp and the
 *  `DATA_FILES` list -- the archive must hash exactly the fields the engine
 *  stamps, and those live in the binary. `wantEngine` also loads the data and
 *  ruleset, so the caller can seal a reference record with a real match. */
async function probeGlue(glueDir, dataDir, rulesDir, gsha, wantEngine) {
  const g = await import(pathToFileURL(join(glueDir, "glue.js")).href);
  await g.default({ module_or_path: new Uint8Array(readFileSync(join(glueDir, "glue_bg.wasm"))) });
  const stamp = JSON.parse(g.engine_stamp());
  const dataFiles = JSON.parse(g.data_files());
  const api = typeof g.replay_api_version === "function" ? g.replay_api_version() : 1;
  if (!wantEngine) return { stamp, dataFiles, api, g: null };

  const files = {};
  for (const n of dataFiles) {
    const p = join(dataDir, n);
    if (existsSync(p)) files[n] = readFileSync(p, "utf8");
  }
  g.load_data(JSON.stringify(files));
  if (rulesDir) {
    const rulesIndex = JSON.parse(readFileSync(join(rulesDir, "index.json"), "utf8"));
    for (const m of rulesIndex.modules ?? []) {
      const mp = join(rulesDir, m.file);
      if (existsSync(mp)) g.ruleset_add(new Uint8Array(readFileSync(mp)));
      else {
        const pool = join(rulesDir, "..", "..", "modules", m.file);
        if (existsSync(pool)) g.ruleset_add(new Uint8Array(readFileSync(pool)));
      }
    }
    // Precompiled guard conditions (docs/GUARDS.md §8.2), beside the index.
    // Absent = a set with no `pre` (older bundles): nothing to load.
    if (rulesIndex.conds?.file) {
      const cp = join(rulesDir, rulesIndex.conds.file);
      if (!existsSync(cp)) throw new Error(`precompiled conds blob missing: ${cp}`);
      if (typeof g.ruleset_precompiled !== "function") {
        throw new Error("ruleset ships precompiled conditions but this glue cannot load them");
      }
      g.ruleset_precompiled(new Uint8Array(readFileSync(cp)));
    }
    g.ruleset_build();
  }
  if (typeof g.set_glue_sha === "function" && gsha) g.set_glue_sha(gsha);
  return { stamp, dataFiles, api, g };
}

/** sha256 over the `DATA_FILES` contents in order -- the engine's own recipe
 *  (`load_data` / `Ctx::load`). Missing files are skipped, as there. */
function dataSha256Hex(dataDir, dataFiles) {
  const hasher = createHash("sha256");
  for (const name of dataFiles) {
    const p = join(dataDir, name);
    if (existsSync(p)) hasher.update(readFileSync(p));
  }
  return hasher.digest("hex");
}

// ---------------------------------------------------------------- check / backup

/** Every indexed bundle must be in the store with the recorded hashes -- or,
 *  when `rebuild`, we only warn (tools/rebuild-engine.mjs can fill it in).
 *  `files` keys are store-relative (`<id>/glue.js`, `modules/<sha>.wasm`).
 *  `archive` is the index's own directory: the reference record is written
 *  beside it (`<archive>/refs/<id>.bdrec`) and looked up there too -- a
 *  `--archive` override must check the archive it just wrote, not the live
 *  `archive/engine/` default. */
function checkStore(index, archive, store, cache) {
  let missing = 0;
  let corrupt = 0;
  let rebuildable = 0;
  for (const e of index.bundles) {
    // Reference record: the behaviour check every rebuild must pass. Small
    // and versioned beside the index (`archive/engine/refs/`).
    if (e.ref) {
      const rp = join(archive, e.ref);
      if (!existsSync(rp)) {
        console.error(`FAIL  ${e.id.slice(0, 12)}…  missing reference record ${e.ref}`);
        missing++;
      } else if (e.ref_sha256 && sha256File(rp) !== e.ref_sha256) {
        console.error(`FAIL  ${e.id.slice(0, 12)}…  ${e.ref} hash mismatch`);
        corrupt++;
      }
    }
    const present = existsSync(join(store, e.id, "bundle.json"));
    const inCache = existsSync(join(cache, e.id, "bundle.json"));
    if (!present && !inCache) {
      if (e.rebuild) {
        rebuildable++;
        console.warn(`warn  ${e.id.slice(0, 12)}…  not in the store; rebuild with tools/rebuild-engine.mjs ${e.id}`);
      } else {
        console.error(`FAIL  ${e.id.slice(0, 12)}…  not in the store and not rebuildable (rebuild:false)`);
        missing++;
      }
      continue;
    }
    // Prefer the store; fall back to the rebuild cache. Keys are relative to
    // that root, which is what `files` records.
    const root = present ? store : cache;
    const files = e.files ?? {};
    let bad = 0;
    for (const [rel, want] of Object.entries(files)) {
      const p = join(root, rel);
      const p2 = join(root === store ? cache : store, rel);
      const use = existsSync(p) ? p : existsSync(p2) ? p2 : null;
      if (!use) {
        console.error(`FAIL  ${e.id.slice(0, 12)}…  missing ${rel}`);
        bad++;
        continue;
      }
      const got = sha256File(use);
      if (got !== want) {
        console.error(`FAIL  ${e.id.slice(0, 12)}…  ${rel} hash ${got.slice(0, 12)}… != ${want.slice(0, 12)}…`);
        bad++;
      }
    }
    if (bad) {
      corrupt += bad;
      if (e.damaged) {
        console.error(`      entry is flagged damaged: ${e.damaged_note ?? ""}`);
      }
    } else if (!present && inCache) {
      rebuildable++;
      console.warn(`warn  ${e.id.slice(0, 12)}…  only in the cache; run tools/rebuild-engine.mjs ${e.id} (or --backup) to persist`);
    } else if (e.damaged) {
      console.warn(`warn  ${e.id.slice(0, 12)}…  hashes match but the entry is flagged damaged: ${e.damaged_note ?? ""}`);
    } else {
      console.log(
        `ok    ${e.id.slice(0, 12)}…  ${Object.keys(files).length} files` +
          (e.rebuild ? `  (rebuild: ${e.commit.slice(0, 12)}…)` : "  (rebuild: false)"),
      );
    }
  }
  console.log(`check: ${index.bundles.length} bundles, ${missing} missing, ${corrupt} bad hashes, ${rebuildable} need a rebuild`);
  return missing + corrupt;
}

function backupStore(store, dest) {
  if (!existsSync(store)) throw new Error(`no store at ${store}`);
  mkdirSync(dest, { recursive: true });
  copyTree(store, dest);
  const n = dirBytes(dest);
  console.log(`backup  ${dest}  (${mib(n)})`);
  console.log(`        copy this somewhere safe -- it is the only home of rebuild:false bundles`);
  console.log(`        (this command never uploads; hand the directory to your backup target yourself)`);
}

// ---------------------------------------------------------------- main

async function main() {
  const a = parseArgs(process.argv.slice(2));

  if (a.backup) {
    backupStore(a.store, a.backup);
    return;
  }

  const indexPath = join(a.archive, "index.json");
  const index = existsSync(indexPath)
    ? JSON.parse(readFileSync(indexPath, "utf8"))
    : { version: 1, current: null, bundles: [] };
  index.version = 1;

  if (a.check) {
    const bad = checkStore(index, a.archive, a.store, a.cache);
    if (bad) process.exit(1);
    return;
  }

  const src = resolveSource(a);
  console.log(`source:\n  glue  ${src.glue}\n  data  ${src.data}\n  rules ${src.rules}`);

  const glueJs = readFileSync(join(src.glue, "glue.js"));
  const glueWasm = readFileSync(join(src.glue, "glue_bg.wasm"));
  const gsha = glueSha256(glueJs, glueWasm);

  // The tree the bytes came from: a `--from` worktree when given, else this
  // repo. Provenance and `source_id` are about THAT tree, not about whatever
  // the main checkout has been doing since.
  const sourceRoot = a.from ?? ROOT;

  const { stamp: probe, dataFiles, api, g } = await probeGlue(src.glue, src.data, src.rules, null, true);
  const rulesIndex = JSON.parse(readFileSync(join(src.rules, "index.json"), "utf8"));
  const dsha = dataSha256Hex(src.data, dataFiles);

  // A debug engine (cfg(debug_assertions) -- the console cheats) must never
  // become a replay bundle: it is larger/slower, and its records may carry
  // cheat inputs a release engine cannot re-simulate (`docs/SERVER.md`).
  // `cheats_enabled()` is the engine's own answer; `engine_id.json`'s
  // `profile` is the belt-and-braces check from tools/build-glue.mjs. (A glue
  // that predates both was built with the old hardcoded `--release`.)
  const debugGlue = typeof g.cheats_enabled === "function" ? !!g.cheats_enabled() : false;
  let idProfile = null;
  try {
    idProfile = JSON.parse(readFileSync(join(src.glue, "engine_id.json"), "utf8")).profile ?? null;
  } catch {
    /* no stamp: fall back to the glue's own answer */
  }
  if (debugGlue || idProfile === "debug") {
    throw new Error(
      `archive-engine: refusing to archive a debug engine (cheats_enabled=${debugGlue}, ` +
        `engine_id.json profile=${idProfile ?? "unknown"}). Rebuild with ` +
        `NODE_ENV=production node tools/build-glue.mjs`,
    );
  }

  // The stamp the engine would seal a record with here: its own format /
  // save / abi / engine / build, plus the ruleset and data hashes computed
  // from the files this bundle freezes.
  const stamp = {
    format: probe.format,
    save_version: probe.save_version,
    abi: probe.abi,
    ruleset_sha256: String(rulesIndex.ruleset_sha256 || probe.ruleset_sha256 || "stub"),
    data_sha256: dsha,
    engine: probe.engine || "game-core",
    build: probe.build || "",
  };
  if (rulesIndex.abi != null && Number(rulesIndex.abi) !== Number(probe.abi)) {
    throw new Error(`ruleset index abi ${rulesIndex.abi} != engine abi ${probe.abi}`);
  }
  const id = bundleId(gsha, stamp);
  if (!id) throw new Error("empty glue sha");
  stamp.bundle = id;

  // -- provenance + source_id + reference record --------------------------
  // `source_id` is the behaviour-level identity (`tools/engine-source.mjs`):
  // the inputs that change how a record replays, and nothing else. The
  // reference record is a short seeded match sealed beside the index; every
  // rebuild replays it through the rebuilt glue and must hit every checkpoint.
  const prov = provenance(a, src.glue, gsha, sourceRoot, dataFiles);
  const srcId = computeSourceId(prov, stamp);
  if (typeof g.set_glue_sha === "function") g.set_glue_sha(gsha);
  const refName = `refs/${id}.bdrec`;
  const refPath = join(a.archive, refName);
  mkdirSync(dirname(refPath), { recursive: true });
  writeFileSync(refPath, makeReferenceRecord(g));
  const refSha = sha256File(refPath);
  console.log(`source   ${srcId.slice(0, 16)}…${prov.dirty ? " (dirty tree: content hash)" : ` (commit ${prov.commit.slice(0, 12)}…)`}`);
  console.log(`ref      ${refName}  ${statSync(refPath).size} bytes  sha ${refSha.slice(0, 12)}…`);

  const store = a.store;
  const bundleDir = join(store, id);
  const modulesDir = join(store, "modules");

  // -- modules pool (content-addressed; one copy per store) ----------------
  let linked = 0;
  for (const m of rulesIndex.modules ?? []) {
    const srcP = join(src.rules, m.file);
    const dstP = join(modulesDir, m.file);
    if (!existsSync(srcP)) throw new Error(`ruleset module missing: ${srcP}`);
    const before = existsSync(dstP);
    copyOrLink(srcP, dstP);
    if (!before) linked++;
  }

  // -- bundle ------------------------------------------------------------
  mkdirSync(bundleDir, { recursive: true });
  copyOrLink(join(src.glue, "glue.js"), join(bundleDir, "glue.js"));
  copyOrLink(join(src.glue, "glue_bg.wasm"), join(bundleDir, "glue_bg.wasm"));
  for (const name of dataFiles) {
    const p = join(src.data, name);
    if (existsSync(p)) copyOrLink(p, join(bundleDir, "data", name));
  }
  mkdirSync(join(bundleDir, "rules"), { recursive: true });
  // The index only *names* the modules; the bytes live in ../modules. Keep a
  // verbatim copy of the index so a bundle lists exactly the modules it ran.
  writeFileSync(join(bundleDir, "rules", "index.json"), JSON.stringify(rulesIndex));
  // The precompiled guard conditions ride in the bundle's `rules/` dir (they
  // are small and per-set, not worth a shared pool). Bundles from before
  // conditions existed have no `conds` entry -- nothing to copy.
  if (rulesIndex.conds?.file) {
    const srcP = join(src.rules, rulesIndex.conds.file);
    if (!existsSync(srcP)) throw new Error(`precompiled conds blob missing: ${srcP}`);
    copyOrLink(srcP, join(bundleDir, "rules", rulesIndex.conds.file));
  }

  const bundleJson = {
    id,
    api,
    created: new Date().toISOString(),
    glueSha256: gsha,
    stamp,
    layout: {
      glueJs: "glue.js",
      glueWasm: "glue_bg.wasm",
      dataDir: "data",
      dataFiles,
      rulesIndex: "rules/index.json",
      // Relative to the bundle directory. The store keeps every module
      // once, under `modules/` (filenames are their sha256).
      modulesDir: "../modules",
    },
  };
  writeFileSync(join(bundleDir, "bundle.json"), JSON.stringify(bundleJson, null, 1));

  // Per-file hashes: what `--check` verifies, and the reason a store copy is
  // auditable even when `rebuild: false`.
  const files = {};
  const hashTree = (dir, prefix) => {
    for (const e of readdirSync(dir, { withFileTypes: true })) {
      const p = join(dir, e.name);
      const rel = prefix ? `${prefix}/${e.name}` : e.name;
      if (e.isDirectory()) hashTree(p, rel);
      else files[rel] = sha256File(p);
    }
  };
  hashTree(bundleDir, id);
  for (const m of rulesIndex.modules ?? []) {
    const rel = `modules/${m.file}`;
    files[rel] = sha256File(join(modulesDir, m.file));
  }

  // -- index -------------------------------------------------------------
  mkdirSync(a.archive, { recursive: true });
  const entry = {
    id,
    created: bundleJson.created,
    glueSha256: gsha,
    bytes: dirBytes(bundleDir),
    stamp,
    key: stampKey(stamp),
    source_id: srcId,
    ref: refName,
    ref_sha256: refSha,
    commit: prov.commit,
    dirty: prov.dirty,
    rebuild: prov.rebuild,
    toolchain: prov.toolchain,
    files,
  };
  if (prov.dirty && prov.dirtyPaths.length) {
    console.log(`dirty  inputs differ from HEAD (${prov.dirtyPaths.length} paths) -- bundle is rebuild:false`);
    for (const p of prov.dirtyPaths.slice(0, 8)) console.log(`       ${p}`);
    if (prov.dirtyPaths.length > 8) console.log(`       … and ${prov.dirtyPaths.length - 8} more`);
    console.log(`       committing the tree is what makes tools/rebuild-engine.mjs work`);
  }
  const at = index.bundles.findIndex((b) => b.id === id);
  if (at >= 0) index.bundles[at] = entry;
  else index.bundles.unshift(entry);
  if (a.current) index.current = id;
  writeFileSync(indexPath, JSON.stringify(index, null, 1));

  // -- dist copy ---------------------------------------------------------
  if (a.dist) {
    const into = join(a.dist, "assets", "engine");
    mkdirSync(into, { recursive: true });
    copyOrLink(indexPath, join(into, "index.json"));
    // Every stored bundle, not just this one: a UI build wipes dist, and
    // older records must still find their engine after each deploy.
    for (const b of index.bundles) {
      const dir = join(store, b.id);
      if (existsSync(dir)) copyTree(dir, join(into, b.id));
      else {
        const c = join(a.cache, b.id);
        if (existsSync(c)) copyTree(c, join(into, b.id));
      }
    }
    if (existsSync(modulesDir)) copyTree(modulesDir, join(into, "modules"));
    const cacheMods = join(a.cache, "modules");
    if (existsSync(cacheMods)) copyTree(cacheMods, join(into, "modules"));
    // The worker driver rides along: the frozen API talks to any bundle, and
    // dist must serve it even before the next UI build copies webui/public.
    const worker = join(ROOT, "webui", "public", "assets", "engine", "replay-worker.js");
    if (existsSync(worker)) copyOrLink(worker, join(into, "replay-worker.js"));
  }

  // -- report ------------------------------------------------------------
  const bundleBytes = dirBytes(bundleDir);
  const moduleBytes = existsSync(modulesDir) ? dirBytes(modulesDir) : 0;
  console.log(`bundle   ${id}`);
  console.log(`  glue sha     ${gsha.slice(0, 16)}…`);
  console.log(`  source id    ${srcId.slice(0, 16)}…  (${prov.dirty ? "dirty: content hash" : `commit ${prov.commit.slice(0, 12)}…`})`);
  console.log(`  ref          ${refName}`);
  console.log(`  ruleset      ${stamp.ruleset_sha256.slice(0, 16)}… (${(rulesIndex.modules ?? []).length} modules)`);
  console.log(`  data sha     ${dsha.slice(0, 16)}… (${dataFiles.length} files)`);
  console.log(`  stamp        format=${stamp.format} save=${stamp.save_version} abi=${stamp.abi} build=${stamp.build}`);
  console.log(`  api          ${api}`);
  console.log(`  rebuild      ${prov.rebuild ? `yes (${prov.commit.slice(0, 12)}…)` : "no (store the files; see --backup)"}`);
  console.log(`  toolchain    ${prov.toolchain.rustc} / ${prov.toolchain.wasm_bindgen}`);
  console.log(`  size         bundle ${mib(bundleBytes)} + modules ${mib(moduleBytes)} (shared, ${linked} new) = ${mib(bundleBytes + moduleBytes)} on first sight`);
  console.log(`  store        ${store}`);
  console.log(`  index        ${indexPath}`);
  if (a.dist) console.log(`  dist         ${join(a.dist, "assets", "engine")}`);

  // Deploy gate: every previously indexed bundle must still be servable.
  const bad = checkStore(index, a.archive, store, a.cache);
  if (bad) {
    console.error(`archive-engine: ${bad} problem(s) in the store -- fix before going live`);
    process.exit(1);
  }
}

main().catch((e) => {
  console.error(e.message ?? e);
  process.exit(1);
});