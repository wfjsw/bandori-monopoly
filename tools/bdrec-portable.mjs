#!/usr/bin/env node
// Pack / unpack / verify a **portable** `.bdrec` -- a plain record with the
// engine bundle it needs embedded behind it (`docs/REPLAY.md` §10). The format
// and its limits live in `webui/src/game/portable.ts` (shared with the browser);
// this tool only does the file I/O and resolves bundle bytes from the engine
// archive store (`data/engine-archive/`, read-only here -- it never writes).
//
//   node tools/bdrec-portable.mjs pack   <in.bdrec> [--out <f>] [--bundle <id>]
//                                      [--store <dir>] [--archive <dir>]
//   node tools/bdrec-portable.mjs unpack <in.bdrec> [--out-dir <dir>]
//   node tools/bdrec-portable.mjs verify <in.bdrec> [--store <dir>] [--archive <dir>]
//
// pack    embeds the engine for the record's bundle (or `--bundle`), reports
//         the plain and portable sizes, and writes the portable file next to
//         the input as `<name>.portable.bdrec` unless `--out` says otherwise.
// unpack  writes the plain record to `<out-dir>/record.bdrec` and the embedded
//         files under `<out-dir>/bundle/`.
// verify  checks every manifest hash, the compression bounds and -- against
//         `archive/engine/index.json` -- that the embedded loader is one this
//         deployment trusts. Exits non-zero on any refusal.
//
// The store is looked up relative to the repo root (the tool's own `..`), so it
// works from a worktree that has no `data/engine-archive` of its own if you
// point `--store` at the main tree. Nothing here ever executes embedded
// JavaScript: `glue.js` is hashed and checked, never imported.

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { zstdCompressSync, zstdDecompressSync } from "node:zlib";

import {
  buildPortable,
  isSafePath,
  loaderPolicyFrom,
  MAX_BLOB_RAW,
  PortableError,
  sha256Hex,
  splitPortable,
  untrustedLoaderMessage,
  verifyEmbedded,
} from "../webui/src/game/portable.ts";
import { serveIdFor } from "../webui/src/game/engineBundle.ts";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const codec = {
  compress: (b) => new Uint8Array(zstdCompressSync(b)),
  decompress: (b, maxOut) => {
    const out = zstdDecompressSync(b);
    if (out.length > maxOut) throw new Error(`output ${out.length} exceeds ${maxOut}`);
    return new Uint8Array(out);
  },
};

// ---------------------------------------------------------------- args

function parseArgs(argv) {
  const out = { _: [], flags: {} };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith("--")) {
      const key = a.slice(2);
      const next = argv[i + 1];
      if (next === undefined || next.startsWith("--")) out.flags[key] = true;
      else {
        out.flags[key] = next;
        i++;
      }
    } else out._.push(a);
  }
  return out;
}

const fmt = (n) => n.toLocaleString("en-US");
const kb = (n) => `${fmt(n)} B (${(n / 1024 / 1024).toFixed(2)} MiB)`;

// ---------------------------------------------------------------- store

function storeRoot(flags) {
  return resolve(flags.store ?? process.env.BD_ENGINE_STORE ?? join(ROOT, "data", "engine-archive"));
}
function archiveRoot(flags) {
  return resolve(flags.archive ?? process.env.BD_ENGINE_ARCHIVE ?? join(ROOT, "archive", "engine"));
}

function readJson(p) {
  return JSON.parse(readFileSync(p, "utf8"));
}

/** The archive index, or null when there is none. */
function readIndex(flags) {
  const p = join(archiveRoot(flags), "index.json");
  return existsSync(p) ? readJson(p) : null;
}

/**
 * The bytes of one engine bundle, as the replay worker loads them. Paths are
 * the portable/bundle-relative ones (`glue.js`, `data/board.json`,
 * `modules/<sha>.wasm`, ...).
 */
function bundleFiles(store, serveId, wantId) {
  const dir = join(store, serveId);
  const manifestPath = join(dir, "bundle.json");
  if (!existsSync(manifestPath)) {
    throw new PortableError(
      `engine bundle ${wantId} is not in the store (expected ${manifestPath}) — ` +
        `run \`node tools/archive-engine.mjs\` first, or pass --store`,
    );
  }
  const manifest = readJson(manifestPath);
  const L = manifest.layout ?? {};
  const out = [];
  const push = (rel, abs) => {
    if (!existsSync(abs)) throw new PortableError(`bundle file missing: ${abs}`);
    out.push({ path: rel, bytes: new Uint8Array(readFileSync(abs)) });
  };
  push("bundle.json", manifestPath);
  if (L.glueJs) push(L.glueJs, join(dir, L.glueJs));
  if (L.glueWasm) push(L.glueWasm, join(dir, L.glueWasm));
  for (const name of L.dataFiles ?? []) push(`${L.dataDir ?? "data"}/${name}`, join(dir, L.dataDir ?? "data", name));
  if (L.rulesIndex) push(L.rulesIndex, join(dir, L.rulesIndex));
  // Precompiled guard conditions ride beside the rules index (docs/GUARDS.md).
  const index = L.rulesIndex ? readJson(join(dir, L.rulesIndex)) : null;
  if (index?.conds?.file) {
    const rel = `${dirname(L.rulesIndex).replace(/\\/g, "/")}/${index.conds.file}`;
    push(rel, join(dir, dirname(L.rulesIndex), index.conds.file));
  }
  // Content-addressed modules live in the shared pool beside the bundle dirs.
  for (const m of index?.modules ?? []) {
    const pool = join(store, "modules", m.file);
    const beside = join(dir, L.modulesDir ?? "../modules", m.file);
    const abs = existsSync(pool) ? pool : beside;
    push(`modules/${m.file}`, abs);
  }
  for (const f of out) {
    if (!isSafePath(f.path)) throw new PortableError(`bundle file path ${f.path} is not safe`);
  }
  return { files: out, manifest };
}

/** The record JSON body from a plain (or portable) `.bdrec`. */
function recordJson(bytes) {
  const split = splitPortable(bytes);
  const record = split ? split.record : bytes;
  return JSON.parse(Buffer.from(zstdDecompressSync(record)).toString("utf8"));
}

function splitOrPlain(bytes) {
  try {
    return splitPortable(bytes);
  } catch (e) {
    if (e instanceof PortableError) throw e;
    throw new PortableError(String(e?.message ?? e));
  }
}

// ---------------------------------------------------------------- commands

async function cmdPack(flags, input) {
  const bytes = new Uint8Array(readFileSync(input));
  if (splitOrPlain(bytes)) {
    throw new PortableError(`${input} is already portable (unpack it first to repack)`);
  }
  const body = recordJson(bytes);
  const want = flags.bundle ?? body?.header?.engine?.bundle ?? "";
  if (!want) {
    throw new PortableError(
      "the record names no engine bundle (written before bundles existed) — pass --bundle <id>",
    );
  }
  const index = readIndex(flags);
  const entry = index?.bundles?.find((b) => b.id === want || (b.aliases && want in b.aliases) || b.rebuilt?.byte_id === want);
  const serveId = entry ? serveIdFor(entry, want) : want;
  const { files, manifest } = bundleFiles(storeRoot(flags), serveId, want);

  // Identity the manifest records. `glueSha256` is the loader identity: the
  // sha256 of `glue.js` || `glue_bg.wasm` (tools/engine-bundle.mjs).
  const glueJs = files.find((f) => f.path === (manifest.layout?.glueJs ?? "glue.js"));
  const glueWasm = files.find((f) => f.path === (manifest.layout?.glueWasm ?? "glue_bg.wasm"));
  const glueSha256 =
    manifest.glueSha256 ??
    (glueJs && glueWasm
      ? createHash("sha256").update(glueJs.bytes).update(glueWasm.bytes).digest("hex")
      : "");
  if (!glueSha256) throw new PortableError("bundle has no glue identity (glue.js / glue_bg.wasm missing)");

  const built = await buildPortable(bytes, files, { bundle: want, glueSha256, api: manifest.api ?? 1 }, codec);
  const outPath = resolve(flags.out ?? input.replace(/(\.bdrec)?$/, ".portable.bdrec"));
  writeFileSync(outPath, built.bytes);
  console.log(`packed ${input}`);
  console.log(`  bundle   ${want}`);
  console.log(`  files    ${built.manifest.files.length}`);
  console.log(`  plain    ${kb(built.plainLen)}`);
  console.log(`  portable ${kb(built.portableLen)}  (engine blob ${kb(built.blobLen)} compressed)`);
  console.log(`  wrote    ${outPath}`);
  return outPath;
}

async function cmdVerify(flags, input) {
  const bytes = new Uint8Array(readFileSync(input));
  const split = splitOrPlain(bytes);
  if (!split) {
    console.log(`${input}: plain .bdrec (no embedded engine) — nothing to verify`);
    return;
  }
  const embedded = await verifyEmbedded(split, codec);
  const m = embedded.manifest;
  console.log(`${input}: portable .bdrec`);
  console.log(`  record    ${kb(split.record.length)}`);
  console.log(`  section   ${kb(split.sectionOff === 0 ? 0 : bytes.length - split.record.length - 32)}`);
  console.log(`  bundle    ${m.bundle}`);
  console.log(`  glueSha   ${m.glueSha256}`);
  console.log(`  files     ${m.files.length} (${kb(m.files.reduce((a, f) => a + f.len, 0))} uncompressed)`);

  // 1. Every file hash is already verified by `verifyEmbedded`.
  // 2. The loader must be one this deployment trusts.
  const index = readIndex(flags);
  const known = new Set((index?.bundles ?? []).map((b) => b.glueSha256).filter(Boolean));
  const policy = loaderPolicyFrom(known);
  if (!policy.trusted.has(m.glueSha256)) {
    throw new PortableError(untrustedLoaderMessage(m.glueSha256));
  }
  console.log(`  loader    trusted (in ${archiveRoot(flags)}/index.json)`);

  // 3. Cross-check against the store when it is here: the embedded bytes must
  //    be the archived bytes for that bundle.
  const store = storeRoot(flags);
  const entry = index?.bundles?.find(
    (b) => b.id === m.bundle || (b.aliases && m.bundle in b.aliases) || b.rebuilt?.byte_id === m.bundle,
  );
  const serveId = entry ? serveIdFor(entry, m.bundle) : m.bundle;
  const dir = join(store, serveId);
  if (existsSync(join(dir, "bundle.json"))) {
    let checked = 0;
    for (const f of m.files) {
      const abs = f.path.startsWith("modules/") ? join(store, "modules", f.path.slice("modules/".length)) : join(dir, f.path);
      if (!existsSync(abs)) continue;
      const h = await sha256Hex(new Uint8Array(readFileSync(abs)));
      if (h !== f.sha256) {
        throw new PortableError(`embedded ${f.path} differs from the archived bundle (${h} != ${f.sha256})`);
      }
      checked++;
    }
    console.log(`  store     ${checked}/${m.files.length} files match ${dir}`);
  } else {
    console.log(`  store     (bundle not in ${store} — skipping the cross-check)`);
  }
  console.log("ok");
}

async function cmdUnpack(flags, input) {
  const bytes = new Uint8Array(readFileSync(input));
  const split = splitOrPlain(bytes);
  const outDir = resolve(flags["out-dir"] ?? input.replace(/(\.bdrec)?$/, "") + ".unpacked");
  mkdirSync(outDir, { recursive: true });
  writeFileSync(join(outDir, "record.bdrec"), split ? split.record : bytes);
  if (!split) {
    console.log(`${input}: plain .bdrec — wrote ${join(outDir, "record.bdrec")}`);
    return outDir;
  }
  const embedded = await verifyEmbedded(split, codec);
  const bundleDir = join(outDir, "bundle");
  for (const [path, fileBytes] of embedded.files) {
    const abs = join(bundleDir, ...path.split("/"));
    mkdirSync(dirname(abs), { recursive: true });
    writeFileSync(abs, fileBytes);
  }
  writeFileSync(join(outDir, "manifest.json"), JSON.stringify(embedded.manifest, null, 2) + "\n");
  console.log(`${input}: unpacked ${embedded.files.size} engine files to ${bundleDir}`);
  console.log(`  record    ${join(outDir, "record.bdrec")}`);
  console.log(`  manifest  ${join(outDir, "manifest.json")}`);
  return outDir;
}

// ---------------------------------------------------------------- main

async function main() {
  const { _, flags } = parseArgs(process.argv.slice(2));
  const cmd = _[0];
  const input = _[1];
  if (!cmd || !input || ["pack", "unpack", "verify"].includes(cmd) === false) {
    console.error(
      "usage: node tools/bdrec-portable.mjs pack|unpack|verify <in.bdrec> [--out <f>] [--out-dir <d>] [--bundle <id>] [--store <dir>] [--archive <dir>]",
    );
    return 2;
  }
  try {
    if (cmd === "pack") await cmdPack(flags, input);
    else if (cmd === "unpack") await cmdUnpack(flags, input);
    else await cmdVerify(flags, input);
    return 0;
  } catch (e) {
    console.error(`${cmd}: ${e instanceof Error ? e.message : e}`);
    return 1;
  }
}

// Only run when invoked directly (tests import the pieces above).
if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  main().then((code) => {
    process.exitCode = code;
  });
}

export { bundleFiles, recordJson, storeRoot, archiveRoot, readIndex };