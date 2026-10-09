// Opening and exporting a **portable** `.bdrec` (`docs/REPLAY.md` §10) on the
// page. The format and its bounds live in `portable.ts` (pure, node-safe);
// this module is the I/O and the security policy around it:
//
//   * an embedded `glue.js` is **never executed** -- only hashed, and only a
//     loader this deployment ships (allow-list) is imported, from our own
//     `assets/engine/loaders/<sha>/glue.js`, never from the file;
//   * every manifest hash is checked before anything reaches an engine;
//   * the embedded engine runs in the replay worker (no DOM), which is
//     locked down against fetch / sockets once the engine is in;
//   * the hosted archive is preferred when it holds the same bytes.

import { rules } from "../core/data";
import engineId from "../wasm/engine_id.json";
import {
  bundleBase,
  archiveIndex,
  serveIdFor,
  type ArchiveIndex,
  type BundleEntry,
} from "./engineBundle";
import {
  isSha256Hex,
  loaderPolicyFrom,
  PortableError,
  sha256Hex,
  splitPortable,
  trustedLoaderUrl,
  untrustedLoaderMessage,
  verifyEmbedded,
  type BuildFile,
  type EmbeddedBundle,
  type PortableManifest,
  type ZstdCodec,
} from "./portable";
import {
  currentStamp,
  openOnOwnEngine,
  PageReplayHandle,
  WorkerReplayHandle,
  type ReplayHandle,
} from "./replayEngine";
import type { RecordHeader } from "./record";

// ---------------------------------------------------------------- codec

/**
 * The zstd codec portable packing needs. It is the engine's own
 * (`zst_compress` / `zst_decompress`, the same frames `record_zst` writes), so
 * there is one codec across the record and the extension. Not part of the
 * frozen replay API -- only the page's engine is asked for it.
 */
export function engineCodec(): ZstdCodec {
  const glue = rules as unknown as {
    zst_compress?: (b: Uint8Array) => Uint8Array;
    zst_decompress?: (b: Uint8Array, max: number) => Uint8Array;
  };
  const enc = glue.zst_compress;
  const dec = glue.zst_decompress;
  if (typeof enc !== "function" || typeof dec !== "function") {
    throw new PortableError(
      "this build's engine has no zstd codec (zst_compress / zst_decompress) — rebuild the glue (tools/build-glue.mjs)",
    );
  }
  return {
    compress: (b) => enc(b),
    decompress: (b, maxOut) => {
      try {
        return dec(b, maxOut);
      } catch (e) {
        throw new PortableError(`portable blob does not decompress: ${e instanceof Error ? e.message : e}`);
      }
    },
  };
}

// ---------------------------------------------------------------- prepare

/** A record ready to play: the plain part, plus its verified embedded engine
 *  when the file carried one. */
export interface PreparedRecord {
  /** The plain `.bdrec` bytes -- what `ReplayMatch.from_record_bytes` eats. */
  record: Uint8Array;
  /** The embedded engine, verified and decompressed. Null on a plain record. */
  embedded: EmbeddedBundle | null;
  /** The manifest of `embedded`, when there is one. */
  manifest: PortableManifest | null;
}

/**
 * Split and verify a record file. A plain record comes straight through; a
 * portable one is fully hash-checked and decompressed here (bounded), so
 * nothing unverified reaches an engine later.
 */
export async function prepareRecord(bytes: Uint8Array): Promise<PreparedRecord> {
  let split;
  try {
    split = splitPortable(bytes);
  } catch (e) {
    if (e instanceof PortableError) throw e;
    throw new PortableError(String(e instanceof Error ? e.message : e));
  }
  if (!split) return { record: bytes, embedded: null, manifest: null };
  const embedded = await verifyEmbedded(split, engineCodec());
  return { record: split.record, embedded, manifest: embedded.manifest };
}

// ---------------------------------------------------------------- loader policy

/**
 * The embedded loader allow-list: every `glueSha256` the archive index knows,
 * plus the running build's own glue identity (`webui/src/wasm/engine_id.json`,
 * which is what `set_glue_sha` is called with at boot). An embedded `glue.js`
 * whose hash is not here is refused -- we never execute the file's own copy.
 */
export async function trustedLoaderPolicy(index: ArchiveIndex | null): Promise<ReturnType<typeof loaderPolicyFrom>> {
  const shas: string[] = [];
  for (const b of index?.bundles ?? []) if (b.glueSha256) shas.push(b.glueSha256);
  const self = (engineId as { glueSha256?: string } | undefined)?.glueSha256;
  if (self) shas.push(self);
  return loaderPolicyFrom(shas);
}

/** Where our trusted copy of a loader lives, in preference order. */
function loaderCandidates(manifest: PortableManifest, bundle: string): string[] {
  return [trustedLoaderUrl(manifest.glueSha256), `${bundleBase(bundle)}glue.js`];
}

/**
 * Fetch **our** copy of the loader named by `manifest` and check it is the
 * same bytes the record claims. The embedded `glue.js` is only hashed; this
 * function's return value is the URL of our copy, which the worker imports.
 */
export async function resolveTrustedLoader(
  manifest: PortableManifest,
  bundle: string,
  embeddedGlueJs: Uint8Array,
): Promise<string> {
  const want = await sha256Hex(embeddedGlueJs);
  for (const url of loaderCandidates(manifest, bundle)) {
    try {
      const r = await fetch(url);
      if (!r.ok) continue;
      const bytes = new Uint8Array(await r.arrayBuffer());
      const got = await sha256Hex(bytes);
      if (got === want) return url;
      // A shipped loader whose bytes differ is a deployment bug -- keep
      // looking rather than importing the wrong interface.
      console.warn(`portable: loader at ${url} hashes ${got}, want ${want}`);
    } catch {
      /* try the next candidate */
    }
  }
  throw new PortableError(
    `no trusted copy of engine loader ${manifest.glueSha256} is deployed ` +
      `(expected ${trustedLoaderUrl(manifest.glueSha256)}) — refusing to run the embedded engine`,
  );
}

// ---------------------------------------------------------------- open

/** Which engine ended up playing the record, and what to tell the user. */
export interface OpenedEngine {
  handle: ReplayHandle;
  source: "page" | "hosted" | "embedded";
  bundle: string;
  /** User-facing note (the record carries its own engine). */
  note: string | null;
}

/** Does the archived entry hold exactly the bytes the record embedded? */
function hostedMatches(entry: BundleEntry | null | undefined, manifest: PortableManifest): boolean {
  if (!entry?.files) return false; // unverifiable -> do not prefer it
  for (const f of manifest.files) {
    const keys = [f.path, `${entry.id}/${f.path}`];
    const hit = keys.map((k) => entry.files![k]).find((h) => typeof h === "string");
    if (hit !== f.sha256) return false;
  }
  return true;
}

/**
 * Open a record on the engine that wrote it.
 *
 * Order (`docs/REPLAY.md` §10.3): the page's own engine when the record names
 * this build; else the hosted archive bundle when it is known locally and
 * carries the same bytes (verified against the portable manifest); else the
 * record's embedded engine. A plain record without an embedded engine keeps
 * the old behaviour exactly.
 */
export async function openRecordOnEngine(bytes: Uint8Array, force = false): Promise<OpenedEngine> {
  const prep = await prepareRecord(bytes);
  const header = JSON.parse(rules.record_header_bytes(prep.record)) as RecordHeader;
  const index = await archiveIndex();
  const now = currentStamp();
  const want = header.engine?.bundle ?? prep.manifest?.bundle ?? "";

  // Prefer the hosted bundle when it is known and byte-identical to what the
  // record embedded. Otherwise run the embedded engine -- that is the whole
  // point of carrying it.
  if (prep.embedded) {
    const policy = await trustedLoaderPolicy(index);
    const entry =
      index?.bundles.find((b) => b.id === want) ??
      index?.bundles.find((b) => b.aliases && want in b.aliases) ??
      index?.bundles.find((b) => b.rebuilt?.byte_id === want);
    const canPreferHosted =
      !!entry &&
      (want === now?.bundle || hostedMatches(entry, prep.manifest!)) &&
      policy.trusted.has(prep.manifest!.glueSha256);
    if (want === now?.bundle && now?.bundle) {
      // Same engine as the page: play here, no worker, no note.
      const m = rules.ReplayMatch.from_record_bytes(prep.record, force);
      return {
        handle: new PageReplayHandle(m, typeof rules.replay_api_version === "function" ? rules.replay_api_version() : 1, want),
        source: "page",
        bundle: want,
        note: null,
      };
    }
    if (canPreferHosted) {
      try {
        const h = await openOnOwnEngine(
          { kind: "archived", bundle: serveIdFor(entry!, want), entry: entry! },
          prep.record,
          force,
        );
        return { handle: h, source: "hosted", bundle: want, note: null };
      } catch (e) {
        console.warn("portable: hosted bundle failed, falling back to the embedded engine:", e);
      }
    }
    return openEmbedded(prep.embedded, prep.record, header, force, policy);
  }

  // Plain record: unchanged routing.
  const h = await openPlain(header, index, now, prep.record, force);
  return { handle: h, source: h.kind === "page" ? "page" : "hosted", bundle: want, note: null };
}

/** The pre-portable routing table (`resolveBundle` + `openOnOwnEngine`). */
async function openPlain(
  header: RecordHeader,
  index: ArchiveIndex | null,
  now: ReturnType<typeof currentStamp>,
  record: Uint8Array,
  force: boolean,
): Promise<ReplayHandle> {
  // Imported lazily to keep this module's import graph free of cycles in tests.
  const { resolveBundle } = await import("./engineBundle");
  const choice = resolveBundle(header, index, now);
  return openOnOwnEngine(choice, record, force);
}

/** Boot the record's own embedded engine in a worker. */
async function openEmbedded(
  embedded: EmbeddedBundle,
  record: Uint8Array,
  header: RecordHeader,
  force: boolean,
  policy: ReturnType<typeof loaderPolicyFrom>,
): Promise<OpenedEngine> {
  const m = embedded.manifest;
  const glueJs = embedded.files.get(m.files.find((f) => f.path.endsWith("glue.js"))?.path ?? "glue.js");
  if (!glueJs) throw new PortableError("embedded bundle has no glue.js");

  // 1. The loader must be one we trust (allow-list) ...
  if (!policy.trusted.has(m.glueSha256)) throw new PortableError(untrustedLoaderMessage(m.glueSha256));
  // 2. ... and we load OUR copy of it, never the file's bytes.
  const loaderUrl = await resolveTrustedLoader(m, m.bundle, glueJs);

  // Rebuild the bundle.json layout from the manifest: the worker's embedded
  // boot does not read a layout, it is handed the files directly.
  const rawFiles = new Map<string, Uint8Array>();
  const modules: Uint8Array[] = [];
  let conds: Uint8Array | null = null;
  for (const [path, bytes] of embedded.files) {
    if (path.startsWith("data/")) rawFiles.set(path.slice("data/".length), bytes);
    else if (path.startsWith("modules/")) modules.push(bytes);
    else if (/^rules\/conds-.*\.bin$/.test(path)) conds = bytes;
  }
  // The data file *names* the engine expects come from the embedded bundle.json
  // when there is one, else from the glue itself (what the worker does today).
  let names: string[] | null = null;
  const bundleJson = embedded.files.get("bundle.json");
  if (bundleJson) {
    try {
      const b = JSON.parse(new TextDecoder().decode(bundleJson)) as { layout?: { dataFiles?: string[] } };
      names = b.layout?.dataFiles ?? null;
    } catch {
      names = null;
    }
  }
  const data = await assembleDataFor(rawFiles, names ?? [...rawFiles.keys()], header);

  const h = await WorkerReplayHandle.openEmbedded({
    loaderUrl,
    wasm: embedded.files.get("glue_bg.wasm") ?? new Uint8Array(0),
    data,
    modules,
    conds,
    api: m.api,
    bundle: m.bundle,
    record,
    force,
  });
  return {
    handle: h,
    source: "embedded",
    bundle: m.bundle,
    note: "embedded",
  };
}

/**
 * Decode the embedded data tables into the strings `load_data` hashes.
 *
 * `load_data` stamps `sha256` over the **strings** it is given, and a UTF-8
 * BOM at the head of a table survives a byte-preserving decode but not
 * `TextDecoder`'s default (which strips it). Records sealed by the tools and
 * the server therefore carry the BOM-inclusive hash; ones sealed by a page
 * that loaded the tables with `Response.text()` carry the stripped hash. Both
 * are real, so pick whichever reproduces the record's own `data_sha256` --
 * that is the only way the embedded engine can agree with the record.
 */
async function assembleDataFor(
  rawFiles: Map<string, Uint8Array>,
  names: string[],
  header: RecordHeader,
): Promise<Record<string, string>> {
  const wanted = header.engine?.data_sha256 ?? "";
  const build = (keepBom: boolean): Record<string, string> => {
    // `ignoreBOM: true` means "leave the BOM alone" (the naming is inverted).
    const dec = new TextDecoder("utf-8", { ignoreBOM: keepBom });
    const out: Record<string, string> = {};
    for (const n of names) {
      const b = rawFiles.get(n);
      if (b) out[n] = dec.decode(b);
    }
    return out;
  };
  const hashOf = async (data: Record<string, string>): Promise<string> => {
    const parts = names.filter((n) => n in data).map((n) => new TextEncoder().encode(data[n]));
    const all = new Uint8Array(parts.reduce((a, p) => a + p.length, 0));
    let at = 0;
    for (const p of parts) {
      all.set(p, at);
      at += p.length;
    }
    return sha256Hex(all);
  };
  for (const keepBom of [true, false]) {
    const data = build(keepBom);
    if (!wanted || (await hashOf(data)) === wanted) return data;
  }
  // Neither mode matched: keep the raw bytes (BOM-inclusive), which is what
  // the file actually says, and let `compat` report the difference.
  return build(true);
}

// ---------------------------------------------------------------- export

/** Engine files gathered for packing. */
export interface BundleForExport {
  files: BuildFile[];
  glueSha256: string;
  api: number;
  bundle: string;
  /** Where the bytes came from: the hosted archive, or a record's own tail. */
  origin: "hosted" | "embedded";
}

/**
 * Collect the engine bundle a record needs for a portable export.
 *
 * Prefers the record's own embedded copy (re-exporting a portable record is
 * free), otherwise fetches the hosted archive bundle and checks it against
 * the index's recorded hashes. Returns null when this deployment has no copy
 * of that engine -- the UI disables the option with that reason.
 */
export async function collectBundleForExport(
  header: RecordHeader,
  embedded: EmbeddedBundle | null,
): Promise<BundleForExport | null> {
  const want = header.engine?.bundle ?? embedded?.manifest.bundle ?? "";
  if (!want) return null;
  if (embedded && (embedded.manifest.bundle === want || !header.engine?.bundle)) {
    return {
      files: [...embedded.files].map(([path, bytes]) => ({ path, bytes })),
      glueSha256: embedded.manifest.glueSha256,
      api: embedded.manifest.api,
      bundle: want,
      origin: "embedded",
    };
  }
  const index = await archiveIndex();
  const entry =
    index?.bundles.find((b) => b.id === want) ??
    index?.bundles.find((b) => b.aliases && want in b.aliases) ??
    index?.bundles.find((b) => b.rebuilt?.byte_id === want);
  const serveId = entry ? serveIdFor(entry, want) : want;
  const base = bundleBase(serveId);
  let manifest: {
    id?: string;
    api?: number;
    glueSha256?: string;
    layout?: {
      glueJs?: string;
      glueWasm?: string;
      dataDir?: string;
      dataFiles?: string[];
      rulesIndex?: string;
      modulesDir?: string;
    };
  };
  try {
    const r = await fetch(new URL("bundle.json", base));
    if (!r.ok) return null;
    manifest = await r.json();
  } catch {
    return null;
  }
  const L = manifest.layout ?? {};
  const files: BuildFile[] = [];
  const get = async (path: string, url: string | URL): Promise<BuildFile | null> => {
    try {
      const r = await fetch(url);
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      return { path, bytes: new Uint8Array(await r.arrayBuffer()) };
    } catch (e) {
      console.warn(`portable export: cannot fetch ${url}:`, e);
      return null;
    }
  };
  const push = async (path: string, url: string | URL) => {
    const f = await get(path, url);
    if (!f) throw new PortableError(`engine bundle ${want} is incomplete (missing ${path})`);
    files.push(f);
  };
  await push("bundle.json", new URL("bundle.json", base));
  if (L.glueJs) await push(L.glueJs, new URL(L.glueJs, base));
  if (L.glueWasm) await push(L.glueWasm, new URL(L.glueWasm, base));
  for (const n of L.dataFiles ?? []) await push(`${L.dataDir ?? "data"}/${n}`, new URL(`${L.dataDir ?? "data"}/${n}`, base));
  if (L.rulesIndex) {
    await push(L.rulesIndex, new URL(L.rulesIndex, base));
    const idx = JSON.parse(new TextDecoder().decode(files.find((f) => f.path === L.rulesIndex)!.bytes)) as {
      modules?: { file: string }[];
      conds?: { file?: string };
    };
    if (idx.conds?.file) {
      const rel = `${L.rulesIndex.replace(/\/[^/]*$/, "")}/${idx.conds.file}`;
      await push(rel, new URL(idx.conds.file, new URL(L.rulesIndex, base)));
    }
    for (const m of idx.modules ?? []) {
      await push(`modules/${m.file}`, new URL(`modules/${m.file}`, "/assets/engine/"));
    }
  }
  // Identity: the index's glueSha256, else hash the pair ourselves.
  const glueJs = files.find((f) => f.path === (L.glueJs ?? "glue.js"));
  const glueWasm = files.find((f) => f.path === (L.glueWasm ?? "glue_bg.wasm"));
  const glueSha256 =
    entry?.glueSha256 ??
    manifest.glueSha256 ??
    (glueJs && glueWasm ? await sha256Hex(concat(glueJs.bytes, glueWasm.bytes)) : "");
  if (!isSha256Hex(glueSha256)) return null;

  // When the index records per-file hashes, the fetched bytes must match --
  // "same bytes, verified" before they go into a new file.
  if (entry?.files) {
    for (const f of files) {
      const claimed = entry.files[f.path] ?? entry.files[`${entry.id}/${f.path}`];
      if (typeof claimed === "string") {
        const got = await sha256Hex(f.bytes);
        if (got !== claimed) {
          throw new PortableError(`hosted ${f.path} does not match the archive index (got ${got})`);
        }
      }
    }
  }
  return { files, glueSha256, api: manifest.api ?? 1, bundle: want, origin: "hosted" };
}

function concat(a: Uint8Array, b: Uint8Array): Uint8Array {
  const out = new Uint8Array(a.length + b.length);
  out.set(a, 0);
  out.set(b, a.length);
  return out;
}

/**
 * Make a portable copy of `bytes`: the plain record untouched, plus the engine
 * bundle it needs. Throws a `PortableError` (with a user-facing message) when
 * this deployment has no copy of that engine.
 */
export async function exportPortable(
  bytes: Uint8Array,
  header: RecordHeader,
): Promise<{ bytes: Uint8Array; plainLen: number; portableLen: number }> {
  const prep = await prepareRecord(bytes);
  const bundle = await collectBundleForExport(header, prep.embedded);
  if (!bundle) {
    throw new PortableError(
      `this deployment has no copy of engine bundle ${header.engine?.bundle || "?"} to embed — ` +
        `run \`node tools/archive-engine.mjs\` (or open a record that carries its engine)`,
    );
  }
  const { buildPortable } = await import("./portable");
  const built = await buildPortable(prep.record, bundle.files, {
    bundle: bundle.bundle,
    glueSha256: bundle.glueSha256,
    api: bundle.api,
  }, engineCodec());
  return { bytes: built.bytes, plainLen: built.plainLen, portableLen: built.portableLen };
}