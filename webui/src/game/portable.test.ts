// Portable `.bdrec` format (`docs/REPLAY.md` §10). Run with:
//   node --test webui/src/game/portable.test.ts
// Pure logic only -- node's zstd stands in for the engine codec, so this runs
// without wasm. The engine's own codec is cross-checked in
// `tools/test-portable.mjs` and `crates/game-core/tests/record_codec.rs`.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { zstdCompressSync, zstdDecompressSync } from "node:zlib";

import {
  buildPortable,
  CONTAINER_MAGIC,
  crc32,
  FORMAT_VERSION,
  hasPortableTrailer,
  isSafePath,
  loaderPolicyFrom,
  MAX_BLOB_RAW,
  MAX_FILES,
  PortableError,
  sha256Hex,
  splitPortable,
  TRAILER_LEN,
  TRAILER_MAGIC,
  untrustedLoaderMessage,
  verifyEmbedded,
  type ZstdCodec,
} from "./portable.ts";

const codec: ZstdCodec = {
  compress: (b) => new Uint8Array(zstdCompressSync(b)),
  decompress: (b, maxOut) => {
    const out = zstdDecompressSync(b);
    if (out.length > maxOut) throw new Error(`output ${out.length} exceeds ${maxOut}`);
    return new Uint8Array(out);
  },
};

/** A stand-in for the record frame: the real one is `encode_record_zst`, but
 *  the portable container only cares that this is opaque leading bytes. */
const record = (text = "record-json") => new TextEncoder().encode(text);

const file = (path: string, text: string) => ({ path, bytes: new TextEncoder().encode(text) });

const glueSha = "a".repeat(64);
const info = { bundle: "b".repeat(64), glueSha256: glueSha, api: 1 };

test("build -> split -> verify round-trips the files and keeps the record", async () => {
  const files = [file("glue.js", "js"), file("glue_bg.wasm", "wasm"), file("data/board.json", "{}")];
  const built = await buildPortable(record("PLAIN"), files, info, codec);
  assert.equal(built.plainLen, 5);
  assert.ok(built.portableLen > built.plainLen);
  assert.ok(built.blobLen < built.portableLen, "blob is the compressed part");

  const split = splitPortable(built.bytes);
  assert.ok(split, "trailer found");
  assert.equal(new TextDecoder().decode(split.record), "PLAIN", "record part is untouched");
  assert.equal(split.manifest.bundle, info.bundle);
  assert.equal(split.manifest.glueSha256, glueSha);
  assert.equal(split.manifest.files.length, 3);
  assert.deepEqual(
    split.manifest.files.map((f) => f.path),
    ["glue.js", "glue_bg.wasm", "data/board.json"],
    "paths preserved in order",
  );

  const embedded = await verifyEmbedded(split, codec);
  assert.equal(embedded.files.size, 3);
  assert.equal(new TextDecoder().decode(embedded.files.get("glue.js")!), "js");
  assert.equal(new TextDecoder().decode(embedded.files.get("glue_bg.wasm")!), "wasm");
  assert.equal(new TextDecoder().decode(embedded.files.get("data/board.json")!), "{}");
});

test("a plain record has no portable tail", () => {
  assert.equal(hasPortableTrailer(record()), false);
  assert.equal(splitPortable(record()), null);
  assert.equal(hasPortableTrailer(new Uint8Array(TRAILER_LEN)), false);
});

test("refuses to wrap something already portable", async () => {
  const built = await buildPortable(record(), [file("glue.js", "x")], info, codec);
  await assert.rejects(
    () => buildPortable(built.bytes, [file("glue.js", "x")], info, codec),
    /already portable/,
  );
});

test("tampered blob hash is refused", async () => {
  const built = await buildPortable(record(), [file("glue.js", "hello")], info, codec);
  const split = splitPortable(built.bytes)!;
  // `blob` is a subarray of `built.bytes`; flip a byte through the parent.
  assert.equal(split.blob.buffer, built.bytes.buffer, "split views the same buffer");
  built.bytes[split.blob.byteOffset] ^= 0xff;
  await assert.rejects(() => verifyEmbedded(split, codec), /blob hash mismatch|does not decompress/);
});

test("tampered file hash is refused", async () => {
  const built = await buildPortable(record(), [file("glue.js", "hello")], info, codec);
  const split = splitPortable(built.bytes)!;
  // Rebuild the section with a manifest whose file hash is wrong: the blob is
  // honest but the manifest lies about it.
  const manifest = { ...split.manifest, files: [{ ...split.manifest.files[0], sha256: "0".repeat(64) }] };
  const forged = await forge(built.bytes, manifest, split.blob);
  const s2 = splitPortable(forged)!;
  await assert.rejects(() => verifyEmbedded(s2, codec), /hash mismatch/);
});

test("tampered blob sha in the manifest is refused", async () => {
  const built = await buildPortable(record(), [file("glue.js", "hello")], info, codec);
  const split = splitPortable(built.bytes)!;
  const manifest = { ...split.manifest, blobSha256: "0".repeat(64) };
  const forged = await forge(built.bytes, manifest, split.blob);
  const s2 = splitPortable(forged)!;
  await assert.rejects(() => verifyEmbedded(s2, codec), /blob hash mismatch/);
});

test("unsafe paths are refused", async () => {
  for (const bad of ["../glue.js", "/glue.js", "a//b", "a\\b", "C:/glue.js", "glue.js/", "./x", "a/../b", ""]) {
    await assert.rejects(
      () => buildPortable(record(), [file(bad, "x")], info, codec),
      PortableError,
      `path ${JSON.stringify(bad)}`,
    );
    assert.equal(isSafePath(bad), false, `isSafePath ${JSON.stringify(bad)}`);
  }
  assert.equal(isSafePath("glue.js"), true);
  assert.equal(isSafePath("rules/conds-abc.bin"), true);
  assert.equal(isSafePath("modules/abc.wasm"), true);
});

test("oversized section / file count are refused", async () => {
  await assert.rejects(
    () => buildPortable(record(), Array.from({ length: MAX_FILES + 1 }, (_, i) => file(`f${i}`, "x")), info, codec),
    /too many engine files/,
  );
  // A trailer that claims a section larger than the cap is refused before any
  // allocation of that size -- the file is only 32 bytes of trailer here.
  const big = new Uint8Array(TRAILER_LEN);
  const t = big;
  for (let i = 0; i < 8; i++) t[i] = TRAILER_MAGIC.charCodeAt(i);
  t[8] = FORMAT_VERSION;
  t[9] = 1;
  const dv = new DataView(t.buffer, t.byteOffset);
  dv.setBigUint64(12, 1n, true);
  dv.setBigUint64(20, BigInt(MAX_BLOB_RAW), true); // absurd section length
  dv.setUint32(28, crc32(t.subarray(0, 28)), true);
  assert.throws(() => splitPortable(big), /too large|outside/);
});

test("corrupt trailer is refused, not silently treated as plain", async () => {
  const built = await buildPortable(record(), [file("glue.js", "x")], info, codec);
  built.bytes[built.bytes.length - 1] ^= 0xff; // break the crc
  assert.throws(() => splitPortable(built.bytes), /trailer is corrupt/);
});

test("unknown loader is refused with a clear message", () => {
  const policy = loaderPolicyFrom([glueSha, "not-a-sha"]);
  assert.equal(policy.trusted.size, 1, "only real sha256s are trusted");
  assert.equal(policy.trusted.has(glueSha), true);
  assert.equal(policy.trusted.has("c".repeat(64)), false);
  const msg = untrustedLoaderMessage("c".repeat(64));
  assert.match(msg, /not on this deployment's trusted loader list/);
  assert.match(msg, /refusing to run it/);
});

test("manifest shape is validated", async () => {
  const built = await buildPortable(record(), [file("glue.js", "x")], info, codec);
  const split = splitPortable(built.bytes)!;
  for (const bad of [
    { ...split.manifest, kind: "other" },
    { ...split.manifest, version: 99 },
    { ...split.manifest, compression: "gzip" as const },
    { ...split.manifest, files: [] },
    { ...split.manifest, blobLen: 1 },
    { ...split.manifest, blobSha256: "zz" },
    { ...split.manifest, glueSha256: "short" },
  ]) {
    const forged = await forge(built.bytes, bad as typeof split.manifest, split.blob);
    assert.throws(() => splitPortable(forged), PortableError, JSON.stringify(bad).slice(0, 60));
  }
});

// ---------------------------------------------------------------- helpers

/** Re-wrap `bytes`'s record part with a different manifest and blob. */
async function forge(bytes: Uint8Array, manifest: unknown, blob: Uint8Array): Promise<Uint8Array> {
  const split = splitPortable(bytes)!;
  const m = new TextEncoder().encode(JSON.stringify(manifest));
  const payload = new Uint8Array(16 + m.length + blob.length);
  for (let i = 0; i < 8; i++) payload[i] = CONTAINER_MAGIC.charCodeAt(i);
  payload[8] = FORMAT_VERSION;
  payload[9] = 1;
  const dv = new DataView(payload.buffer);
  dv.setUint32(12, m.length, true);
  payload.set(m, 16);
  payload.set(blob, 16 + m.length);
  const section = new Uint8Array(8 + payload.length);
  section.set([0x50, 0x2a, 0x4d, 0x18], 0);
  new DataView(section.buffer).setUint32(4, payload.length, true);
  section.set(payload, 8);
  const trailer = new Uint8Array(TRAILER_LEN);
  for (let i = 0; i < 8; i++) trailer[i] = TRAILER_MAGIC.charCodeAt(i);
  trailer[8] = FORMAT_VERSION;
  trailer[9] = 1;
  const tdv = new DataView(trailer.buffer);
  tdv.setBigUint64(12, BigInt(split.record.length), true);
  tdv.setBigUint64(20, BigInt(section.length), true);
  tdv.setUint32(28, crc32(trailer.subarray(0, 28)), true);
  const out = new Uint8Array(split.record.length + section.length + TRAILER_LEN);
  out.set(split.record, 0);
  out.set(section, split.record.length);
  out.set(trailer, split.record.length + section.length);
  return out;
}