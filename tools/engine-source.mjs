// SOURCE-based engine-bundle identity (`docs/REPLAY.md` §9.2).
//
// The byte-level bundle id (`tools/engine-bundle.mjs`) covers the *compiled*
// artifacts. That id deliberately changes when something that does not affect
// game behaviour changes -- rustc version, wasm-bindgen version, whether the
// `rust-src` component is installed, absolute paths in panic strings. None of
// those change how a record replays.
//
// `source_id` is the behaviour-level identity: a hash over the inputs that
// *do* change how the game plays, and nothing else. Two builds with the same
// `source_id` are the same engine, and a rebuild that differs in bytes is
// still accepted once the stored reference record replays clean through it.
//
//   clean commit:  git tree/blob object ids of the input paths (cheap, exact)
//   dirty tree:    sha256 of the same file set's contents (never rebuildable)
//
// plus `format` / `save_version` / `abi` -- the three version numbers the
// record schema itself depends on.
//
// Input set (behaviour-relevant only):
//
//   crates/game-core, crates/game-rules, crates/web-glue, rules/cond,
//   third_party/cel-rust,            (the whole trees)
//   rules/**,                        (the guest workspace: card-sdk, cards,
//                                     skills, tiles, events, fixtures, locks)
//   Cargo.lock, rules/Cargo.lock,    (dependency resolution)
//   Cargo.toml, rules/Cargo.toml,    (the build graph: members, the cel patch)
//   the DATA_FILES the bundle hashes (the game tables)
//
// NOT in the set, on purpose: rustc / wasm-bindgen / cargo versions, build
// flags and recipes (tools/*.mjs), paths, timestamps, webui, docs, bots.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";

/** Version tag baked into every source_id. */
export const SOURCE_ID_VERSION = "bdre-source-v1";

/** Trees whose contents define behaviour. Missing ones are skipped (a commit
 *  from before `third_party/cel-rust` landed simply does not have it). */
export const SOURCE_TREES = [
  "crates/game-core",
  "crates/game-rules",
  "crates/web-glue",
  "rules/cond",
  "third_party/cel-rust",
  "rules",
];

/** Lockfiles and manifests: the resolved dependency graph and the build graph. */
export const SOURCE_BLOBS = ["Cargo.lock", "Cargo.toml", "rules/Cargo.lock", "rules/Cargo.toml"];

/** Directory names never walked when hashing a dirty tree. */
const SKIP_DIRS = new Set(["target", "node_modules", ".git", "dist", "__pycache__", "locales"]);

// ---------------------------------------------------------------- git rows

function gitOut(repo, args) {
  return execFileSync("git", args, {
    cwd: repo,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  }).trim();
}

/** `[{path, oid}]` for a clean `commit` -- git tree/blob object ids. */
export function sourceRowsFromGit(repo, commit) {
  const rows = [];
  for (const p of SOURCE_TREES) {
    let oid;
    try {
      oid = gitOut(repo, ["rev-parse", "--verify", `${commit}:${p}`]);
    } catch {
      continue; // path absent at this commit
    }
    rows.push({ path: p, oid });
  }
  for (const p of SOURCE_BLOBS) {
    let oid;
    try {
      oid = gitOut(repo, ["rev-parse", "--verify", `${commit}:${p}`]);
    } catch {
      continue;
    }
    rows.push({ path: p, oid });
  }
  return rows;
}

// ---------------------------------------------------------------- dirty rows

function walkFiles(root, rel, out) {
  const dir = join(root, rel);
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const e of entries) {
    if (e.isDirectory()) {
      if (SKIP_DIRS.has(e.name)) continue;
      walkFiles(root, rel ? `${rel}/${e.name}` : e.name, out);
    } else if (e.isFile()) {
      out.push(rel ? `${rel}/${e.name}` : e.name);
    }
  }
}

/** `[{path, sha256}]` over the same file set, for a dirty tree (or a snapshot
 *  with no git at all). Every file under each input path -- untracked ones
 *  change the build too, so they count. */
export function sourceRowsFromFiles(root, dataFiles) {
  const rows = [];
  const addDir = (rel) => {
    const files = [];
    walkFiles(root, rel, files);
    files.sort();
    for (const f of files) {
      const p = join(root, ...f.split("/"));
      rows.push({ path: f, oid: createHash("sha256").update(readFileSync(p)).digest("hex") });
    }
  };
  const addFile = (rel) => {
    const p = join(root, ...rel.split("/"));
    if (existsSync(p)) {
      rows.push({ path: rel, oid: createHash("sha256").update(readFileSync(p)).digest("hex") });
    }
  };
  for (const t of SOURCE_TREES) addDir(t);
  for (const b of SOURCE_BLOBS) addFile(b);
  for (const n of dataFiles ?? []) addFile(`data/${n}`);
  return rows;
}

// ---------------------------------------------------------------- id

function finish(rows, s, dirty) {
  const parts = [
    SOURCE_ID_VERSION + (dirty ? "-dirty" : ""),
    ...[...rows]
      .sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0))
      .map((r) => `${r.path}=${r.oid}`),
    `format=${s.format ?? 0}`,
    `save=${s.save_version ?? 0}`,
    `abi=${s.abi ?? 0}`,
    "",
  ];
  return createHash("sha256").update(parts.join("\n"), "utf8").digest("hex");
}

/** The behaviour-level identity. `rows` from `sourceRowsFromGit` or
 *  `sourceRowsFromFiles`; `s` is an `EngineStamp`-shaped object. */
export function sourceId(rows, s, dirty) {
  return finish(rows, s, !!dirty);
}

// ---------------------------------------------------------------- reference record

/**
 * A short seeded match, sealed at archive time beside the index
 * (`archive/engine/refs/<id>.bdrec`) and replayed through every rebuild as the
 * behaviour check. Small (a few kB): two bots, a fixed seed, and no event
 * bundle.
 *
 * `g` is an initialised glue (data loaded, ruleset built, `set_glue_sha` done).
 */
export function makeReferenceRecord(g, seed = 7) {
  const members = [
    { id: 1, player: "P1", bot: true },
    { id: 2, player: "P2", bot: true },
  ];
  const m = new g.SoloMatch(JSON.stringify(members), seedHex(seed), 1, "");
  try {
    m.quick_start();
    for (let i = 0; i < 400; i++) {
      m.tick_steps(10);
      if (m.ended()) break;
    }
    return m.record_zst("2026-10-08 00:00");
  } finally {
    m.free();
  }
}

/**
 * Replay `bytes` through `g` and check every checkpoint. Returns
 * `{ok, ticks, reason}`. `force` is required whenever the rebuilt glue's stamp
 * differs from the record's -- which it always does once the bytes changed.
 *
 * A checkpoint mismatch is a behaviour difference: the rebuild is rejected.
 */
export function verifyReferenceRecord(g, bytes, force = true) {
  let m;
  try {
    m = g.ReplayMatch.from_record_bytes(bytes, force);
  } catch (e) {
    return { ok: false, ticks: 0, reason: `from_record_bytes: ${e}` };
  }
  try {
    let st;
    let ticks = 0;
    for (let i = 0; i < 20000; i++) {
      st = JSON.parse(m.step(10));
      ticks = st.tick ?? ticks;
      if (st.diverged) {
        return {
          ok: false,
          ticks,
          reason: `checkpoint diverged at tick ${ticks} (round ${st.round ?? "?"}, turn ${st.turn ?? "?"})`,
        };
      }
      if (st.ended) return { ok: true, ticks, reason: "replayed clean" };
    }
    return { ok: false, ticks, reason: "did not end within 20000 steps" };
  } catch (e) {
    return { ok: false, ticks: 0, reason: `step: ${e}` };
  } finally {
    try {
      m.free();
    } catch {
      /* already gone */
    }
  }
}

// ---------------------------------------------------------------- glue loading

/**
 * Load a glue in node the way `webui/public/assets/engine/replay-worker.js`
 * does. A fresh URL query key per call, so a process can load two builds.
 * `glueDir` has glue.js + glue_bg.wasm; `dataDir` has the tables; `rules` is
 * optional `{index, modulesDir}`.
 */
/** `SoloMatch::new` wants the 256-bit seed as hex (`docs/FAIRNESS.md`); tools
 *  just need a deterministic one. */
const seedHex = (n) => (n >>> 0).toString(16).padStart(64, "0");

export async function loadGlue({ glueDir, dataDir, rules, glueSha, tag }) {
  const { pathToFileURL } = await import("node:url");
  const g = await import(pathToFileURL(join(glueDir, "glue.js")).href + `?${tag ?? Math.random().toString(36).slice(2)}`);
  await g.default({ module_or_path: new Uint8Array(readFileSync(join(glueDir, "glue_bg.wasm"))) });
  const names = JSON.parse(g.data_files());
  const files = {};
  for (const n of names) {
    const p = join(dataDir, n);
    if (existsSync(p)) files[n] = readFileSync(p, "utf8");
  }
  g.load_data(JSON.stringify(files));
  if (rules) {
    const index = JSON.parse(readFileSync(rules.index, "utf8"));
    for (const m of index.modules ?? []) {
      g.ruleset_add(new Uint8Array(readFileSync(join(rules.modulesDir, m.file))));
    }
    g.ruleset_build();
  }
  if (glueSha && typeof g.set_glue_sha === "function") g.set_glue_sha(glueSha);
  return g;
}