// **Portable** `.bdrec`: a plain record plus an embedded engine bundle
// (`docs/REPLAY.md` §10). The plain part is byte-identical to what
// `record_zst` writes -- every old reader keeps working and simply ignores the
// trailing extension -- and the extension carries the exact engine the record
// needs so it can be replayed offline, on a deployment that never hosted that
// bundle.
//
// SECURITY: a portable file is untrusted input. The embedded `glue.js` is
// **never executed** -- it is only hashed, and the hash must be on the
// deployment's loader allow-list before a *shipped* copy of that loader is
// imported. The embedded wasm is instantiated inside a worker whose import
// surface is the glue's (Error / throw / externref table only -- see the audit
// in `docs/REPLAY.md` §10.4). Every manifest hash is verified before anything
// is handed to the engine, and every length is bounded (decompression bomb
// limits). This module does no I/O and no evaluation: the browser and
// `tools/bdrec-portable.mjs` share it as-is under node.

/** One file inside the embedded bundle. `path` is bundle-relative and always
 *  slash-separated: `glue.js`, `glue_bg.wasm`, `data/board.json`,
 *  `rules/index.json`, `rules/conds-<sha>.bin`, `modules/<sha>.wasm`,
 *  `bundle.json`. */
export interface PortableFile {
  path: string;
  /** sha256 hex of the file's bytes. */
  sha256: string;
  /** Byte length of the file. */
  len: number;
}

/** The manifest that rides with the blob (UTF-8 JSON). */
export interface PortableManifest {
  kind: "bdrec-portable";
  version: number;
  /** The engine bundle this file embeds (`EngineStamp.bundle`). */
  bundle: string;
  /** sha256 of the bundle's `glue.js` || `glue_bg.wasm` -- the loader identity
   *  that must be on the allow-list before anything runs. */
  glueSha256: string;
  /** The bundle's `replay_api_version` (1 when the glue predates the call). */
  api: number;
  /** Only `"zstd"` is written; readers refuse anything else. */
  compression: "zstd";
  /** Length of the (compressed) blob that follows the manifest. */
  blobLen: number;
  /** sha256 hex of that compressed blob. */
  blobSha256: string;
  /** Files, in the order their bytes are concatenated into the blob. */
  files: PortableFile[];
}

/** How to compress / decompress the blob. The browser uses the engine's
 *  `zst_compress` / `zst_decompress` (one shared codec with the record
 *  framing); the CLI and node tests use `node:zlib`'s zstd. Both emit and
 *  accept standard zstd frames. */
export interface ZstdCodec {
  compress(bytes: Uint8Array): Uint8Array;
  /** Bounded decompression: `maxOut` is the hard cap on the result. */
  decompress(bytes: Uint8Array, maxOut: number): Uint8Array;
}

// ---------------------------------------------------------------- layout

/**
 * A portable file is three parts back to back:
 *
 * ```text
 * [0, R)          the plain `.bdrec` -- one zstd frame of RecordFile JSON,
 *                 exactly what `record_zst` writes today
 * [R, R+S)        the portable section: one **zstd skippable frame**
 *                 (magic 184D2A50) wrapping the engine-bundle container
 * [R+S, R+S+32)   the trailer locating the section
 * ```
 *
 * Old readers ignore everything after the first zstd frame (verified for every
 * archived bundle's `expand_record`), so a portable file is just a record to
 * them. `zstd -d` also skips the skippable frame and prints the record JSON.
 *
 * Trailer (32 bytes):
 *
 * ```text
 *  0  "BDRECEND"   magic
 *  8  u8  version  1
 *  9  u8  kind     1 = engine bundle
 * 10  u16 flags    0
 * 12  u64le section_off   (= R)
 * 20  u64le section_len   (= S)
 * 28  u32le crc32         of trailer[0..28)
 * ```
 *
 * Engine-bundle container (inside the skippable frame's payload):
 *
 * ```text
 *  0  "BDRECENG"   magic
 *  8  u8  version       1
 *  9  u8  compression   1 = zstd blob
 * 10  u16 reserved      0
 * 12  u32le manifest_len
 * 16  manifest          UTF-8 JSON
 * ..  blob              zstd frame over concat(files[i].bytes)
 * ```
 */
export const TRAILER_MAGIC = "BDRECEND";
export const SECTION_MAGIC = [0x50, 0x2a, 0x4d, 0x18] as const; // zstd skippable frame
export const CONTAINER_MAGIC = "BDRECENG";
export const FORMAT_VERSION = 1;
export const KIND_ENGINE = 1;
export const COMPRESSION_ZSTD = 1;

/** Fixed trailer size. */
export const TRAILER_LEN = 32;
/** Skippable-frame header (magic + u32le size). */
export const SECTION_HEAD_LEN = 8;
/** Container header (magic + version + compression + reserved + u32le len). */
export const CONTAINER_HEAD_LEN = 16;

// ---------------------------------------------------------------- limits
// A portable file is untrusted input. These bounds bound a decompression bomb
// and a pathological manifest before any allocation grows past them.

/** Whole portable section (skippable frame) cap: 24 MiB. */
export const MAX_SECTION = 24 * 1024 * 1024;
/** Cap on the *decompressed* blob: 32 MiB (a real bundle is ~7.5 MiB). */
export const MAX_BLOB_RAW = 32 * 1024 * 1024;
/** Cap on the compressed blob (also the skippable payload). */
export const MAX_BLOB_ZST = 24 * 1024 * 1024;
/** Cap on the manifest JSON. */
export const MAX_MANIFEST = 256 * 1024;
/** Cap on the number of embedded files. */
export const MAX_FILES = 256;
/** Cap on one path's length. */
export const MAX_PATH = 200;
/** Cap on one file's uncompressed length. */
export const MAX_FILE = 24 * 1024 * 1024;

// ---------------------------------------------------------------- hashing

/** sha256 hex. `crypto.subtle` in the browser and in node >= 19. */
export async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const d = await crypto.subtle.digest("SHA-256", bytes.slice().buffer as ArrayBuffer);
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** True for a 64-char lowercase hex string. */
export function isSha256Hex(s: unknown): s is string {
  return typeof s === "string" && /^[0-9a-f]{64}$/.test(s);
}

// ---------------------------------------------------------------- crc32

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

/** CRC-32 (IEEE) -- trailer integrity only; the manifest carries sha256s. */
export function crc32(bytes: Uint8Array): number {
  let c = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) c = CRC_TABLE[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

// ---------------------------------------------------------------- paths

const PATH_RE = /^[A-Za-z0-9._/-]+$/;

/** A bundle-relative path is safe when it is slash-separated, relative, free
 *  of `..` / `.` segments, and inside the size bounds. Rejecting anything else
 *  here keeps an embedded manifest from naming `../glue.js` or `C:/...`. */
export function isSafePath(p: unknown): p is string {
  if (typeof p !== "string" || !p || p.length > MAX_PATH) return false;
  if (!PATH_RE.test(p)) return false;
  if (p.startsWith("/") || p.endsWith("/") || p.includes("//") || p.includes("\\")) return false;
  for (const seg of p.split("/")) {
    if (seg === "" || seg === "." || seg === "..") return false;
  }
  return true;
}

// ---------------------------------------------------------------- errors

/** Why a portable file was refused. `message` is user-facing. */
export class PortableError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PortableError";
  }
}

// ---------------------------------------------------------------- split

/** The pieces `splitPortable` hands back. `blob` is still compressed. */
export interface PortableSplit {
  record: Uint8Array;
  manifest: PortableManifest;
  /** The compressed blob (zstd frame over concat(file bytes)). */
  blob: Uint8Array;
  /** Absolute offset of the portable section inside the original bytes. */
  sectionOff: number;
}

function u32(b: Uint8Array, o: number): number {
  return (b[o] | (b[o + 1] << 8) | (b[o + 2] << 16) | (b[o + 3] << 24)) >>> 0;
}
function u64(b: Uint8Array, o: number): number {
  // Safe: every offset we accept is bounded by the file length (<= 2^32 in
  // practice, and `MAX_SECTION` long before that).
  return u32(b, o) + u32(b, o + 4) * 0x1_0000_0000;
}
function putU32(out: Uint8Array, o: number, v: number): void {
  out[o] = v & 0xff;
  out[o + 1] = (v >>> 8) & 0xff;
  out[o + 2] = (v >>> 16) & 0xff;
  out[o + 3] = (v >>> 24) & 0xff;
}
function putU64(out: Uint8Array, o: number, v: number): void {
  putU32(out, o, v >>> 0);
  putU32(out, o + 4, Math.floor(v / 0x1_0000_0000) >>> 0);
}

/** Does this byte string end with a portable trailer? Cheap, synchronous. */
export function hasPortableTrailer(bytes: Uint8Array): boolean {
  if (bytes.length < TRAILER_LEN) return false;
  const t = bytes.subarray(bytes.length - TRAILER_LEN);
  for (let i = 0; i < 8; i++) if (t[i] !== TRAILER_MAGIC.charCodeAt(i)) return false;
  return true;
}

/**
 * Split a (possibly) portable `.bdrec` into its plain record and its embedded
 * engine bundle.
 *
 * Returns `null` for a plain record -- no trailer, nothing appended. When a
 * trailer *is* present the file claims to be portable, so any malformation is
 * an error: a corrupt extension is never silently dropped (the user asked for
 * that engine and must know it did not load).
 */
export function splitPortable(bytes: Uint8Array): PortableSplit | null {
  if (!hasPortableTrailer(bytes)) return null;
  const t = bytes.subarray(bytes.length - TRAILER_LEN);
  const version = t[8];
  const kind = t[9];
  const flags = (t[10] | (t[11] << 8)) >>> 0;
  const off = u64(t, 12);
  const len = u64(t, 20);
  const wantCrc = u32(t, 28);
  const gotCrc = crc32(t.subarray(0, 28));
  if (version !== FORMAT_VERSION) {
    throw new PortableError(`portable format version ${version} is not supported (want ${FORMAT_VERSION})`);
  }
  if (kind !== KIND_ENGINE) throw new PortableError(`portable section kind ${kind} is not supported`);
  if (flags !== 0) throw new PortableError(`portable section flags ${flags} are not supported`);
  if (wantCrc !== gotCrc) throw new PortableError("portable trailer is corrupt (crc)");
  // Bounds first: a trailer that claims a huge section is refused before any
  // arithmetic or allocation looks at it.
  if (len > MAX_SECTION) throw new PortableError(`portable section is too large (${len} > ${MAX_SECTION})`);
  if (off < 1 || len < SECTION_HEAD_LEN || off + len + TRAILER_LEN !== bytes.length) {
    throw new PortableError("portable trailer points outside the file");
  }

  const section = bytes.subarray(off, off + len);
  for (let i = 0; i < 4; i++) {
    if (section[i] !== SECTION_MAGIC[i]) throw new PortableError("portable section is not a zstd skippable frame");
  }
  const frameSize = u32(section, 4);
  if (frameSize !== len - SECTION_HEAD_LEN) throw new PortableError("portable skippable frame length mismatch");

  const payload = section.subarray(SECTION_HEAD_LEN);
  for (let i = 0; i < 8; i++) {
    if (payload[i] !== CONTAINER_MAGIC.charCodeAt(i)) throw new PortableError("portable container magic mismatch");
  }
  const cVersion = payload[8];
  const compression = payload[9];
  const reserved = (payload[10] | (payload[11] << 8)) >>> 0;
  const manifestLen = u32(payload, 12);
  if (cVersion !== FORMAT_VERSION) {
    throw new PortableError(`portable container version ${cVersion} is not supported (want ${FORMAT_VERSION})`);
  }
  if (compression !== COMPRESSION_ZSTD) throw new PortableError(`portable compression ${compression} is not supported`);
  if (reserved !== 0) throw new PortableError(`portable container reserved ${reserved} is not supported`);
  if (manifestLen < 2 || manifestLen > MAX_MANIFEST) {
    throw new PortableError(`portable manifest length ${manifestLen} is out of range`);
  }
  const manifestEnd = CONTAINER_HEAD_LEN + manifestLen;
  if (manifestEnd > payload.length) throw new PortableError("portable manifest runs past the section");

  let manifest: PortableManifest;
  try {
    manifest = JSON.parse(new TextDecoder().decode(payload.subarray(CONTAINER_HEAD_LEN, manifestEnd))) as PortableManifest;
  } catch (e) {
    throw new PortableError(`portable manifest is not JSON: ${e instanceof Error ? e.message : e}`);
  }
  const blob = payload.subarray(manifestEnd);
  validateManifest(manifest, blob.length);
  return {
    record: bytes.subarray(0, off),
    manifest,
    blob,
    sectionOff: off,
  };
}

/** Structural checks on a manifest (bounds and shape only -- hashes are
 *  verified against the blob and the files, see {@link verifyEmbedded}). */
export function validateManifest(m: PortableManifest, blobLen: number): void {
  if (!m || typeof m !== "object") throw new PortableError("portable manifest is not an object");
  if (m.kind !== "bdrec-portable") throw new PortableError(`portable manifest kind ${JSON.stringify(m.kind)} is not supported`);
  if (m.version !== FORMAT_VERSION) {
    throw new PortableError(`portable manifest version ${m.version} is not supported (want ${FORMAT_VERSION})`);
  }
  if (m.compression !== "zstd") throw new PortableError(`portable compression ${JSON.stringify(m.compression)} is not supported`);
  if (typeof m.bundle !== "string") throw new PortableError("portable manifest has no bundle id");
  if (!isSha256Hex(m.glueSha256)) throw new PortableError("portable manifest has no glue sha256");
  if (typeof m.api !== "number" || !Number.isInteger(m.api) || m.api < 1 || m.api > 64) {
    throw new PortableError(`portable manifest api ${JSON.stringify(m.api)} is out of range`);
  }
  if (typeof m.blobLen !== "number" || m.blobLen !== blobLen) {
    throw new PortableError(`portable manifest blobLen ${m.blobLen} != ${blobLen}`);
  }
  if (!isSha256Hex(m.blobSha256)) throw new PortableError("portable manifest has no blob sha256");
  if (!Array.isArray(m.files) || m.files.length === 0) throw new PortableError("portable manifest has no files");
  if (m.files.length > MAX_FILES) throw new PortableError(`portable manifest has too many files (${m.files.length})`);
  let total = 0;
  const seen = new Set<string>();
  for (const f of m.files) {
    if (!f || typeof f !== "object") throw new PortableError("portable manifest file entry is not an object");
    if (!isSafePath(f.path)) throw new PortableError(`portable file path ${JSON.stringify(f.path)} is not safe`);
    if (seen.has(f.path)) throw new PortableError(`portable file path ${f.path} is duplicated`);
    seen.add(f.path);
    if (!isSha256Hex(f.sha256)) throw new PortableError(`portable file ${f.path} has no sha256`);
    if (typeof f.len !== "number" || !Number.isInteger(f.len) || f.len < 0 || f.len > MAX_FILE) {
      throw new PortableError(`portable file ${f.path} length ${f.len} is out of range`);
    }
    total += f.len;
  }
  if (total > MAX_BLOB_RAW) throw new PortableError(`portable files total ${total} is over the ${MAX_BLOB_RAW} cap`);
}

// ---------------------------------------------------------------- verify

/** An embedded bundle, fully verified and decompressed. */
export interface EmbeddedBundle {
  manifest: PortableManifest;
  /** Path -> bytes. Every entry is hash-verified. */
  files: Map<string, Uint8Array>;
}

/**
 * Verify and unpack an embedded bundle: blob hash, bounded zstd decode, then
 * every file's length and sha256. Nothing is handed to an engine before this
 * returns.
 */
export async function verifyEmbedded(split: PortableSplit, codec: ZstdCodec): Promise<EmbeddedBundle> {
  const { manifest, blob } = split;
  const blobHash = await sha256Hex(blob);
  if (blobHash !== manifest.blobSha256) {
    throw new PortableError("portable blob hash mismatch (the file was modified)");
  }
  let raw: Uint8Array;
  try {
    raw = codec.decompress(blob, MAX_BLOB_RAW);
  } catch (e) {
    throw new PortableError(`portable blob does not decompress: ${e instanceof Error ? e.message : e}`);
  }
  if (raw.length > MAX_BLOB_RAW) throw new PortableError(`portable blob decompresses past the ${MAX_BLOB_RAW} cap`);
  let total = 0;
  for (const f of manifest.files) total += f.len;
  if (raw.length !== total) {
    throw new PortableError(`portable blob decompressed to ${raw.length} bytes, manifest says ${total}`);
  }
  const files = new Map<string, Uint8Array>();
  let at = 0;
  for (const f of manifest.files) {
    const bytes = raw.subarray(at, at + f.len);
    at += f.len;
    const h = await sha256Hex(bytes);
    if (h !== f.sha256) throw new PortableError(`portable file ${f.path} hash mismatch (the file was modified)`);
    files.set(f.path, bytes);
  }
  return { manifest, files };
}

// ---------------------------------------------------------------- build

/** One input file for {@link buildPortable}. */
export interface BuildFile {
  path: string;
  bytes: Uint8Array;
}

/** Identity the manifest records. */
export interface BuildInfo {
  bundle: string;
  glueSha256: string;
  api: number;
}

/**
 * Append an embedded engine bundle to a plain `.bdrec`.
 *
 * `record` must be a plain record (one zstd frame of `RecordFile` JSON); the
 * function refuses to wrap something that is already portable. Every file is
 * hashed, the concatenation is zstd-compressed, and the whole section is
 * wrapped in a skippable frame plus the locating trailer. Returns the portable
 * bytes and the sizes involved.
 */
export async function buildPortable(
  record: Uint8Array,
  files: BuildFile[],
  info: BuildInfo,
  codec: ZstdCodec,
): Promise<{ bytes: Uint8Array; manifest: PortableManifest; plainLen: number; portableLen: number; blobLen: number }> {
  if (hasPortableTrailer(record)) throw new PortableError("record is already portable");
  if (!files.length) throw new PortableError("no engine files to embed");
  if (files.length > MAX_FILES) throw new PortableError(`too many engine files (${files.length})`);
  if (!isSha256Hex(info.glueSha256)) throw new PortableError("glueSha256 must be 64 hex chars");

  const order: BuildFile[] = [];
  const seen = new Set<string>();
  let total = 0;
  for (const f of files) {
    if (!isSafePath(f.path)) throw new PortableError(`engine file path ${JSON.stringify(f.path)} is not safe`);
    if (seen.has(f.path)) throw new PortableError(`engine file path ${f.path} is duplicated`);
    seen.add(f.path);
    if (f.bytes.length > MAX_FILE) throw new PortableError(`engine file ${f.path} is too large`);
    total += f.bytes.length;
    order.push(f);
  }
  if (total > MAX_BLOB_RAW) throw new PortableError(`engine files total ${total} is over the ${MAX_BLOB_RAW} cap`);

  const plain = new Uint8Array(total);
  {
    let at = 0;
    for (const f of order) {
      plain.set(f.bytes, at);
      at += f.bytes.length;
    }
  }
  const blob = codec.compress(plain);
  if (blob.length > MAX_BLOB_ZST) throw new PortableError(`compressed engine blob is too large (${blob.length})`);

  const manifest: PortableManifest = {
    kind: "bdrec-portable",
    version: FORMAT_VERSION,
    bundle: info.bundle,
    glueSha256: info.glueSha256,
    api: info.api,
    compression: "zstd",
    blobLen: blob.length,
    blobSha256: await sha256Hex(blob),
    files: await Promise.all(
      order.map(async (f) => ({ path: f.path, sha256: await sha256Hex(f.bytes), len: f.bytes.length })),
    ),
  };
  const manifestBytes = new TextEncoder().encode(JSON.stringify(manifest));
  if (manifestBytes.length > MAX_MANIFEST) throw new PortableError("portable manifest is too large");

  const payloadLen = CONTAINER_HEAD_LEN + manifestBytes.length + blob.length;
  const sectionLen = SECTION_HEAD_LEN + payloadLen;
  if (sectionLen > MAX_SECTION) throw new PortableError(`portable section is too large (${sectionLen})`);

  const section = new Uint8Array(sectionLen);
  section.set(SECTION_MAGIC, 0);
  putU32(section, 4, payloadLen);
  const payload = section.subarray(SECTION_HEAD_LEN);
  for (let i = 0; i < 8; i++) payload[i] = CONTAINER_MAGIC.charCodeAt(i);
  payload[8] = FORMAT_VERSION;
  payload[9] = COMPRESSION_ZSTD;
  putU32(payload, 12, manifestBytes.length);
  payload.set(manifestBytes, CONTAINER_HEAD_LEN);
  payload.set(blob, CONTAINER_HEAD_LEN + manifestBytes.length);

  const trailer = new Uint8Array(TRAILER_LEN);
  for (let i = 0; i < 8; i++) trailer[i] = TRAILER_MAGIC.charCodeAt(i);
  trailer[8] = FORMAT_VERSION;
  trailer[9] = KIND_ENGINE;
  putU64(trailer, 12, record.length);
  putU64(trailer, 20, sectionLen);
  putU32(trailer, 28, crc32(trailer.subarray(0, 28)));

  const out = new Uint8Array(record.length + sectionLen + TRAILER_LEN);
  out.set(record, 0);
  out.set(section, record.length);
  out.set(trailer, record.length + sectionLen);
  return { bytes: out, manifest, plainLen: record.length, portableLen: out.length, blobLen: blob.length };
}

// ---------------------------------------------------------------- loaders

/**
 * The embedded `glue.js` is never executed. Its sha256 must name a loader the
 * deployment ships, and we import **our** copy of it. The allow-list is the
 * set of `glueSha256`s the deployment knows (`archive/engine/index.json`, plus
 * the running build's own glue identity).
 */
export interface LoaderPolicy {
  /** glueSha256 -> where our trusted copy of that loader lives. */
  trusted: Map<string, string>;
}

/** The URL our copy of a loader lives at (deployed by
 *  `tools/archive-engine.mjs` alongside the archive). */
export function trustedLoaderUrl(glueSha256: string): string {
  return `/assets/engine/loaders/${glueSha256}/glue.js`;
}

/** Build the policy from the archive index's `glueSha256` set (and anything
 *  else the caller knows, e.g. the running build's). */
export function loaderPolicyFrom(glueShas: Iterable<string>): LoaderPolicy {
  const trusted = new Map<string, string>();
  for (const s of glueShas) {
    if (isSha256Hex(s)) trusted.set(s, trustedLoaderUrl(s));
  }
  return { trusted };
}

/** Refusal message when an embedded loader is not on the allow-list. */
export function untrustedLoaderMessage(glueSha256: string): string {
  return (
    `this record embeds engine loader ${glueSha256}, which is not on this deployment's trusted loader list — ` +
    `refusing to run it. Re-export the record from a known engine, or deploy the loader ` +
    `(assets/engine/loaders/${glueSha256}/glue.js) after adding it to archive/engine/index.json.`
  );
}