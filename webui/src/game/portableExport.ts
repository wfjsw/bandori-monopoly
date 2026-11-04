// Collecting the engine bundle a portable export embeds (`docs/REPLAY.md`
// §10). Split from `portableEngine` so this half is node-safe (no wasm, no
// worker): the bundle choice is pure archive data and is unit-tested against
// the same routing table that opens a record.
//
// A record's `EngineStamp.bundle` may be **empty**: a host that cannot see its
// own glue identity (the server's in-process fallback, or a deploy that did not
// ship the identity file) stamps `""` -- "unknown bundle". Opening such a
// record already falls back to matching the stamp's identity fields
// (`resolveBundle`); the export must do the same, or every online record
// refuses "include engine" even though the archive holds the exact bundle.

import {
  archiveIndex,
  bundleBase,
  entryFor,
  missingBundleMessage,
  resolveBundle,
  serveIdFor,
  type ArchiveIndex,
  type BundleEntry,
} from "./engineBundle.ts";
import { isSha256Hex, PortableError, sha256Hex, type BuildFile, type EmbeddedBundle } from "./portable.ts";
import type { EngineStamp, RecordHeader } from "./record";

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
 * Which archived bundle an export should embed: the one the record names, or
 * -- for a stamp with no bundle -- the one its identity fields match, exactly
 * as [`resolveBundle`] would open it. Throws a `PortableError` when nothing
 * matches.
 */
export function resolveExportBundle(
  header: RecordHeader,
  embedded: EmbeddedBundle | null,
  index: ArchiveIndex | null,
  now: EngineStamp | null,
): { bundle: string; entry?: BundleEntry; origin: "hosted" | "embedded" } {
  const named = header.engine?.bundle ?? "";
  if (embedded && (embedded.manifest.bundle === named || !named)) {
    return { bundle: embedded.manifest.bundle, origin: "embedded" };
  }
  if (named) return { bundle: named, entry: entryFor(index, named), origin: "hosted" };
  if (!header.engine) {
    throw new PortableError("this record has no engine stamp — its engine cannot be identified");
  }

  const choice = resolveBundle(header, index, now);
  if (choice.kind === "missing") throw new PortableError(missingBundleMessage(choice.bundle));
  if (choice.kind === "unknown") throw new PortableError(choice.reason);
  const bundle = choice.bundle || now?.bundle || "";
  if (!bundle) {
    throw new PortableError(
      "this record names no engine bundle and no archived build matches its stamp — " +
        "it can still be replayed locally, but not embedded",
    );
  }
  return {
    bundle,
    entry: choice.kind === "archived" ? choice.entry : entryFor(index, bundle),
    origin: "hosted",
  };
}

/**
 * Collect the engine bundle a record needs for a portable export.
 *
 * Prefers the record's own embedded copy (re-exporting a portable record is
 * free), otherwise fetches the hosted archive bundle and checks it against
 * the index's recorded hashes. Throws a `PortableError` (a user-facing
 * message) when this deployment has no copy of that engine.
 *
 * `now` is the running engine's stamp (`currentStamp()`), used only for a
 * record whose stamp names no bundle; `origin` is the page origin (a test
 * passes it explicitly -- node has no `location`).
 */
export async function collectBundleForExport(
  header: RecordHeader,
  embedded: EmbeddedBundle | null,
  now: EngineStamp | null,
  origin: string = typeof location === "undefined" ? "" : location.origin,
): Promise<BundleForExport> {
  const { bundle: want, entry, origin: from } = resolveExportBundle(header, embedded, await archiveIndex(), now);
  if (from === "embedded" && embedded) {
    return {
      files: [...embedded.files].map(([path, bytes]) => ({ path, bytes })),
      glueSha256: embedded.manifest.glueSha256,
      api: embedded.manifest.api,
      bundle: want,
      origin: "embedded",
    };
  }
  const serveId = entry ? serveIdFor(entry, want) : want;
  // `URL` needs an absolute base: `/assets/engine/<id>/` alone throws.
  const base = new URL(bundleBase(serveId), origin);
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
    // no-store: a bundle archived by a deploy after this page loaded must not
    // lose to a cached 404 for its directory.
    const r = await fetch(new URL("bundle.json", base), { cache: "no-store" });
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    manifest = await r.json();
  } catch {
    throw new PortableError(
      `this deployment has no copy of engine bundle ${want} to embed — ` +
        `run \`node tools/archive-engine.mjs\` (or open a record that carries its engine)`,
    );
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
      await push(`modules/${m.file}`, new URL(`modules/${m.file}`, new URL("/assets/engine/", origin)));
    }
  }
  // Identity: the index's glueSha256, else hash the pair ourselves.
  const glueJs = files.find((f) => f.path === (L.glueJs ?? "glue.js"));
  const glueWasm = files.find((f) => f.path === (L.glueWasm ?? "glue_bg.wasm"));
  const glueSha256 =
    entry?.glueSha256 ??
    manifest.glueSha256 ??
    (glueJs && glueWasm ? await sha256Hex(concat(glueJs.bytes, glueWasm.bytes)) : "");
  if (!isSha256Hex(glueSha256)) {
    throw new PortableError(`engine bundle ${want} has no glue identity`);
  }

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
