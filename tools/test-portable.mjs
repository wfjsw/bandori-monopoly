// Portable `.bdrec` end-to-end checks (`docs/REPLAY.md` §10). Run with:
//
//   node --test tools/test-portable.mjs
//
// The archive's reference records (`archive/engine/refs/*.bdrec`) are packed
// with the CLI, verified, and replayed **twice** -- as the plain record and as
// the portable file -- on the very bundle that wrote them, and must end in a
// byte-equal state. The security refusals (tampered hashes, an unknown loader,
// an oversized section) are checked against the real format.
//
// Needs the engine-archive store for the bundle bytes:
//   data/engine-archive/          (default)
//   BD_ENGINE_STORE / --store     (a worktree without one)
// and a built glue (`node tools/build-glue.mjs`) for the codec cross-check.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { zstdCompressSync, zstdDecompressSync } from "node:zlib";

import {
  buildPortable,
  crc32,
  FORMAT_VERSION,
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
const TMP = join(ROOT, "target", "scratch", "test-portable");
const STORE = resolve(process.env.BD_ENGINE_STORE ?? join(ROOT, "data", "engine-archive"));
const ARCHIVE = resolve(process.env.BD_ENGINE_ARCHIVE ?? join(ROOT, "archive", "engine"));
const WGLUE = join(ROOT, "webui", "src", "wasm", "glue.js");

const codec = {
  compress: (b) => new Uint8Array(zstdCompressSync(b)),
  decompress: (b, maxOut) => {
    const out = zstdDecompressSync(b);
    if (out.length > maxOut) throw new Error(`output ${out.length} exceeds ${maxOut}`);
    return new Uint8Array(out);
  },
};

// ---------------------------------------------------------------- helpers

/** Load an archived bundle's engine the way `replay-worker.js` does. */
async function loadBundleGlue(bundleId) {
  const index = JSON.parse(readFileSync(join(ARCHIVE, "index.json"), "utf8"));
  const entry = index.bundles.find((b) => b.id === bundleId);
  assert.ok(entry, `bundle ${bundleId} is in the archive index`);
  const serveId = serveIdFor(entry, bundleId);
  const dir = join(STORE, serveId);
  assert.ok(existsSync(join(dir, "bundle.json")), `bundle bytes at ${dir}`);
  const manifest = JSON.parse(readFileSync(join(dir, "bundle.json"), "utf8"));
  const L = manifest.layout;
  const g = await import(pathToFileURL(join(dir, L.glueJs)).href + `?${bundleId.slice(0, 8)}`);
  await g.default({ module_or_path: new Uint8Array(readFileSync(join(dir, L.glueWasm))) });
  const names = JSON.parse(g.data_files());
  const files = {};
  for (const n of names) {
    const p = join(dir, L.dataDir, n);
    if (existsSync(p)) files[n] = readFileSync(p, "utf8");
  }
  g.load_data(JSON.stringify(files));
  const rulesIndex = JSON.parse(readFileSync(join(dir, L.rulesIndex), "utf8"));
  for (const m of rulesIndex.modules ?? []) {
    const pool = join(STORE, "modules", m.file);
    const beside = join(dir, L.modulesDir, m.file);
    const abs = existsSync(pool) ? pool : beside;
    g.ruleset_add(new Uint8Array(readFileSync(abs)));
  }
  if (rulesIndex.conds?.file) {
    const c = join(dirname(join(dir, L.rulesIndex)), rulesIndex.conds.file);
    assert.ok(existsSync(c), `conds blob ${c}`);
    g.ruleset_precompiled(new Uint8Array(readFileSync(c)));
  }
  g.ruleset_build();
  return { g, entry, dir, manifest };
}

/** Step a record to the end and return the end state as JSON text. */
function replayToEnd(g, recordBytes) {
  const m = g.ReplayMatch.from_record_bytes(recordBytes, true);
  try {
    for (let i = 0; i < 100_000; i++) {
      const st = JSON.parse(m.status());
      if (st.ended) break;
      m.step(1000);
    }
    const st = JSON.parse(m.status());
    assert.equal(st.diverged, false, "replay did not diverge");
    assert.equal(st.ended, true, "replay reached the end");
    return { view: m.view(0), status: st, header: m.header() };
  } finally {
    m.free();
  }
}

/** Every ref record in `archive/engine/refs/`. */
function refs() {
  const dir = join(ARCHIVE, "refs");
  return existsSync(dir) ? readdirSync(dir).filter((n) => n.endsWith(".bdrec")).sort() : [];
}

// ---------------------------------------------------------------- tests

test("the store and a built glue are present", () => {
  assert.ok(existsSync(STORE), `engine-archive store at ${STORE} (set BD_ENGINE_STORE)`);
  assert.ok(existsSync(WGLUE), `built glue at ${WGLUE} (run node tools/build-glue.mjs)`);
  assert.ok(refs().length > 0, "archive/engine/refs/ has reference records");
});

test("engine and node zstd codecs agree (frames are standard)", async () => {
  const g = await import(pathToFileURL(WGLUE).href + "?codec");
  await g.default({ module_or_path: new Uint8Array(readFileSync(join(ROOT, "webui", "src", "wasm", "glue_bg.wasm"))) });
  const payload = new TextEncoder().encode(`portable codec ${"x".repeat(4096)}`);
  const a = g.zst_compress(payload);
  const b = zstdCompressSync(Buffer.from(payload));
  assert.equal(new TextDecoder().decode(g.zst_decompress(new Uint8Array(b), 1 << 20)), new TextDecoder().decode(payload));
  assert.equal(zstdDecompressSync(Buffer.from(a)).toString("utf8"), new TextDecoder().decode(payload));
  // The decompression-bomb cap is real.
  assert.throws(() => g.zst_decompress(new Uint8Array(b), 8), /exceeds/);
});

test("record_header_bytes reads a portable file exactly like the plain one", async () => {
  const g = await import(pathToFileURL(WGLUE).href + "?hdr");
  await g.default({ module_or_path: new Uint8Array(readFileSync(join(ROOT, "webui", "src", "wasm", "glue_bg.wasm"))) });
  const ref = new Uint8Array(readFileSync(join(ARCHIVE, "refs", refs()[0])));
  const plain = g.record_header_bytes(ref);
  const built = await buildPortable(ref, [{ path: "glue.js", bytes: new TextEncoder().encode("js") }], {
    bundle: "b".repeat(64),
    glueSha256: "a".repeat(64),
    api: 1,
  }, codec);
  const portable = g.record_header_bytes(built.bytes);
  assert.equal(portable, plain, "header is identical with the engine section appended");
});

test("every archive reference record replays identically plain and portable", async () => {
  mkdirSync(TMP, { recursive: true });
  for (const name of refs()) {
    const bundleId = name.replace(/\.bdrec$/, "");
    const plainPath = join(ARCHIVE, "refs", name);
    const plain = new Uint8Array(readFileSync(plainPath));

    // Pack with the CLI (the gate says "packed via the CLI"). The ref's name
    // is its bundle id; pass it explicitly so a pre-bundle record (one whose
    // stamp has no `bundle` field) packs too.
    const outPath = join(TMP, `${bundleId}.portable.bdrec`);
    execFileSync(process.execPath, [
      join(ROOT, "tools", "bdrec-portable.mjs"), "pack", plainPath,
      "--out", outPath, "--bundle", bundleId, "--store", STORE, "--archive", ARCHIVE,
    ], { stdio: "pipe" });
    const portable = new Uint8Array(readFileSync(outPath));
    assert.ok(portable.length > plain.length, `${name}: portable is bigger`);

    // The record part must be byte-identical: old readers see the same file.
    const split = splitPortable(portable);
    assert.ok(split, `${name}: has a portable tail`);
    assert.deepEqual([...split.record], [...plain], `${name}: record bytes unchanged`);

    // verify exits 0 and reports a trusted loader.
    const v = execFileSync(process.execPath, [
      join(ROOT, "tools", "bdrec-portable.mjs"), "verify", outPath,
      "--store", STORE, "--archive", ARCHIVE,
    ], { encoding: "utf8" });
    assert.match(v, /loader\s+trusted/, `${name}: loader allow-list`);
    assert.match(v, /ok\s*$/, `${name}: verify ok`);

    // Replay both forms on the bundle that wrote them.
    const { g } = await loadBundleGlue(bundleId);
    const a = replayToEnd(g, plain);
    const b = replayToEnd(g, portable); // the full portable file: the tail must be ignored
    const c = replayToEnd(g, split.record);
    assert.equal(b.view, a.view, `${name}: portable file ends in the same state as the plain record`);
    assert.equal(c.view, a.view, `${name}: the record part alone ends in the same state`);
    assert.deepEqual(b.status, a.status, `${name}: same end status`);
  }
});

test("tampered manifest hashes are refused", async () => {
  const ref = new Uint8Array(readFileSync(join(ARCHIVE, "refs", refs()[0])));
  const built = await buildPortable(ref, [
    { path: "glue.js", bytes: new TextEncoder().encode("js") },
    { path: "glue_bg.wasm", bytes: new TextEncoder().encode("wasm") },
  ], { bundle: "b".repeat(64), glueSha256: "a".repeat(64), api: 1 }, codec);

  // 1. A flipped byte in the blob (the compressed engine).
  {
    const split = splitPortable(built.bytes);
    built.bytes[split.blob.byteOffset] ^= 0xff;
    await assert.rejects(() => verifyEmbedded(split, codec), /hash mismatch|does not decompress/);
  }
  // 2. A manifest that lies about one file's hash.
  const clean = await buildPortable(ref, [{ path: "glue.js", bytes: new TextEncoder().encode("js") }], {
    bundle: "b".repeat(64), glueSha256: "a".repeat(64), api: 1,
  }, codec);
  const split = splitPortable(clean.bytes);
  const manifest = JSON.parse(JSON.stringify(split.manifest));
  manifest.files[0].sha256 = "0".repeat(64);
  const forged = await wrap(ref, manifest, split.blob);
  const s2 = splitPortable(forged);
  await assert.rejects(() => verifyEmbedded(s2, codec), /hash mismatch/);
});

test("an unknown loader is refused, not run", async () => {
  const index = JSON.parse(readFileSync(join(ARCHIVE, "index.json"), "utf8"));
  const known = new Set(index.bundles.map((b) => b.glueSha256).filter(Boolean));
  const policy = loaderPolicyFrom(known);
  assert.ok(known.size > 0, "the archive index lists glue loaders");
  const stranger = "c".repeat(64);
  assert.equal(policy.trusted.has(stranger), false);
  const msg = untrustedLoaderMessage(stranger);
  assert.match(msg, /not on this deployment's trusted loader list/);
  assert.match(msg, /refusing to run it/);
  // The refs' own loaders ARE on the list (they are the archived bundles).
  const ref = new Uint8Array(readFileSync(join(ARCHIVE, "refs", refs()[0])));
  const { entry } = await loadBundleGlue(refs()[0].replace(/\.bdrec$/, ""));
  assert.ok(policy.trusted.has(entry.glueSha256), "the ref's loader is trusted");
});

test("an oversized section is refused before it is decompressed", async () => {
  const ref = new Uint8Array(readFileSync(join(ARCHIVE, "refs", refs()[0])));
  // No trailer at all: plain, not a broken portable file.
  const plain = new Uint8Array(32);
  assert.equal(splitPortable(plain), null, "no BDRECEND trailer: plain");

  // A trailer that claims a section far past the cap, sitting where it says.
  const t = new Uint8Array(32);
  for (let i = 0; i < 8; i++) t[i] = "BDRECEND".charCodeAt(i);
  t[8] = FORMAT_VERSION;
  t[9] = 1;
  const file = new Uint8Array(ref.length + 16 + t.length);
  file.set(ref, 0);
  file.set(t, file.length - 32);
  const dv = new DataView(file.buffer);
  dv.setBigUint64(file.length - 32 + 12, BigInt(ref.length), true);
  dv.setBigUint64(file.length - 32 + 20, BigInt(16 + 32), true); // lies about the size
  // First: a section length that does not match the file -> refused.
  dv.setUint32(file.length - 32 + 28, crc32(file.subarray(file.length - 32, file.length - 4)), true);
  assert.throws(() => splitPortable(file), /outside|too large/);

  // Second: a tiny file whose trailer claims a section over the cap. The
  // length is checked before any arithmetic or allocation.
  const huge = new Uint8Array(64);
  const ht = huge.subarray(huge.length - 32);
  for (let i = 0; i < 8; i++) ht[i] = "BDRECEND".charCodeAt(i);
  ht[8] = FORMAT_VERSION;
  ht[9] = 1;
  const hdv = new DataView(huge.buffer, huge.length - 32);
  hdv.setBigUint64(12, 1n, true);
  hdv.setBigUint64(20, BigInt(MAX_BLOB_RAW + 1), true);
  hdv.setUint32(28, crc32(ht.subarray(0, 28)), true);
  assert.throws(() => splitPortable(huge), /too large/);
});

test("a portable file is refused rather than misread when the container is broken", async () => {
  const ref = new Uint8Array(readFileSync(join(ARCHIVE, "refs", refs()[0])));
  const built = await buildPortable(ref, [{ path: "glue.js", bytes: new TextEncoder().encode("js") }], {
    bundle: "b".repeat(64), glueSha256: "a".repeat(64), api: 1,
  }, codec);
  // Break the skippable-frame magic in the middle of the file.
  built.bytes[ref.length] ^= 0xff;
  assert.throws(() => splitPortable(built.bytes), PortableError);
});

// ---------------------------------------------------------------- helpers

async function wrap(record, manifest, blob) {
  const m = new TextEncoder().encode(JSON.stringify(manifest));
  const payload = new Uint8Array(16 + m.length + blob.length);
  for (let i = 0; i < 8; i++) payload[i] = "BDRECENG".charCodeAt(i);
  payload[8] = FORMAT_VERSION;
  payload[9] = 1;
  new DataView(payload.buffer).setUint32(12, m.length, true);
  payload.set(m, 16);
  payload.set(blob, 16 + m.length);
  const section = new Uint8Array(8 + payload.length);
  section.set([0x50, 0x2a, 0x4d, 0x18], 0);
  new DataView(section.buffer).setUint32(4, payload.length, true);
  section.set(payload, 8);
  const trailer = new Uint8Array(32);
  for (let i = 0; i < 8; i++) trailer[i] = "BDRECEND".charCodeAt(i);
  trailer[8] = FORMAT_VERSION;
  trailer[9] = 1;
  const tdv = new DataView(trailer.buffer);
  tdv.setBigUint64(12, BigInt(record.length), true);
  tdv.setBigUint64(20, BigInt(section.length), true);
  tdv.setUint32(28, crc32(trailer.subarray(0, 28)), true);
  const out = new Uint8Array(record.length + section.length + 32);
  out.set(record, 0);
  out.set(section, record.length);
  out.set(trailer, record.length + section.length);
  return out;
}