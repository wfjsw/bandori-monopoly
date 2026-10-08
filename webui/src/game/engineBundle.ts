// The engine archive (`docs/REPLAY.md` §9): which build wrote a record, and
// where that build's frozen engine lives. A record is replayed by the bundle
// its `EngineStamp.bundle` names -- never by "whatever engine is current".
//
// Layout under `/assets/engine/` (produced by `tools/archive-engine.mjs`):
//
//   index.json                 bundles -> stamps, plus the `current` pointer
//   modules/<sha256>.wasm      content-addressed card modules, shared
//   <bundle>/bundle.json       identity + layout of the frozen engine
//   <bundle>/glue.js|.wasm     the engine
//   <bundle>/data/*.json       the tables the stamp hashed
//   <bundle>/rules/index.json  the ruleset index (bytes live in ../modules)

import type { EngineStamp, RecordHeader } from "./record";

/** One archived bundle (`index.json`'s entry). */
export interface BundleEntry {
  id: string;
  created?: string;
  glueSha256?: string;
  bytes?: number;
  stamp: EngineStamp;
  /** `stampKey` of `stamp`, as the archive tool wrote it. */
  key?: string;
  /** Behaviour-level identity (`tools/engine-source.mjs`): the inputs that
   *  change how a record replays, and nothing else. Two builds with the same
   *  `source_id` are the same engine even when their bytes differ. */
  source_id?: string;
  /** Reference record sealed at archive time (`archive/engine/refs/…`). */
  ref?: string;
  ref_sha256?: string;
  /** Source commit; null when sealed from a dirty or unknown tree. */
  commit?: string | null;
  /** True when the inputs differed from `commit` at seal time. */
  dirty?: boolean;
  /** False for bundles that cannot be regenerated (`tools/rebuild-engine.mjs`). */
  rebuild?: boolean;
  /** rustc / cargo / wasm-bindgen versions the seal recorded (informational). */
  toolchain?: { rustc?: string; cargo?: string; wasm_bindgen?: string };
  /** Per-file sha256, relative to the store root (what `--check` verifies). */
  files?: Record<string, string>;
  /** Set when the stored bytes are known to be wrong (e.g. a hardlink
   *  write-through destroyed them). The entry is kept so records still name
   *  it; `tools/archive-engine.mjs --check` refuses the deploy until the
   *  bytes are restored and this flag is cleared. */
  damaged?: boolean;
  damaged_note?: string;
  damaged_at?: string;
  /** Byte-id -> serve-id. When a rebuild's bytes hash to a different bundle
   *  id (different rustc install, rust-src present/absent, …) the rebuilt
   *  bytes are kept under their own id and this maps the seal's id onto them.
   *  Records stamped with the seal's id still resolve. */
  aliases?: Record<string, string>;
  /** Set by `tools/rebuild-engine.mjs` when bytes were regenerated and the
   *  reference record replayed clean through the new build. */
  rebuilt?: {
    byte_id: string;
    source_id?: string;
    verified?: boolean;
    ref?: string;
    verified_at?: string;
    toolchain?: { rustc?: string; cargo?: string; wasm_bindgen?: string };
  };
}

/** Where the bytes for `want` actually live: an alias target, else the
 *  rebuilt byte id, else the entry id itself. */
export function serveIdFor(entry: BundleEntry, want: string): string {
  return entry.aliases?.[want] ?? entry.rebuilt?.byte_id ?? entry.id;
}

export interface ArchiveIndex {
  version: number;
  /** The bundle of the build currently deployed. */
  current: string | null;
  bundles: BundleEntry[];
}

/** `bundle.json`: how to load a bundle's engine and its inputs. */
export interface BundleManifest {
  id: string;
  api: number;
  created?: string;
  glueSha256?: string;
  stamp: EngineStamp;
  layout: {
    glueJs: string;
    glueWasm: string;
    dataDir: string;
    dataFiles: string[];
    rulesIndex: string;
    /** Relative to the bundle directory (the shared modules pool). */
    modulesDir: string;
  };
}

/** The stamp fields that identify a build when the record predates the
 *  `bundle` field: same recipe as `tools/engine-bundle.mjs`'s `stampKey`. */
export function stampKey(s: EngineStamp): string {
  return [s.format, s.save_version, s.abi, s.ruleset_sha256, s.data_sha256].join("|");
}

let indexCache: Promise<ArchiveIndex | null> | null = null;

/** The archive index (cached). Null when the site ships no archive. */
export function archiveIndex(): Promise<ArchiveIndex | null> {
  indexCache ??= (async () => {
    try {
      const r = await fetch("/assets/engine/index.json");
      if (!r.ok) return null;
      const text = await r.text();
      // An SPA answers unknown paths with index.html and HTTP 200.
      if (text.trimStart().startsWith("<")) return null;
      const v = JSON.parse(text) as ArchiveIndex;
      return Array.isArray(v.bundles) ? v : null;
    } catch {
      return null;
    }
  })();
  return indexCache;
}

/** Forget the cached index (tests). */
export function resetArchiveIndex(): void {
  indexCache = null;
}

/** How the player should open a record. */
export type BundleChoice =
  /** The record names a bundle and this page *is* that bundle. */
  | { kind: "current"; bundle: string }
  /** The record names a bundle the archive still holds (or a legacy record
   *  whose stamp matches exactly one archived build). */
  | { kind: "archived"; bundle: string; entry: BundleEntry }
  /** The record names a bundle the archive does not hold. */
  | { kind: "missing"; bundle: string }
  /** A pre-`bundle` record whose stamp matches nothing -- not reproducible. */
  | { kind: "unknown"; reason: string };

/**
 * Pick the engine that must play `header`. Exact match on `bundle` wins;
 * an alias hit (a rebuild whose bytes hashed to a different id) counts as an
 * exact match on the rebuilt bundle; a record from before the `bundle` field
 * falls back to matching the stamp's identity fields against the archive
 * (and the running engine).
 *
 * `now` is the running engine's stamp (`currentStamp()`), or null when the
 * page has no engine up. Pure -- the archive lookup is data, not side effects.
 */
export function resolveBundle(
  header: RecordHeader,
  index: ArchiveIndex | null,
  now: EngineStamp | null,
): BundleChoice {
  const want = header.engine?.bundle ?? "";
  if (want) {
    if (want === now?.bundle) return { kind: "current", bundle: want };
    const entry =
      index?.bundles.find((b) => b.id === want) ??
      index?.bundles.find((b) => b.aliases && want in b.aliases) ??
      index?.bundles.find((b) => b.rebuilt?.byte_id === want);
    if (entry) return { kind: "archived", bundle: serveIdFor(entry, want), entry };
    return { kind: "missing", bundle: want };
  }
  // Legacy record: no bundle id. The stamp's identity fields are the best
  // key available; the running engine is the only candidate not in the index.
  const key = stampKey(header.engine);
  const candidates: { id: string; stamp: EngineStamp; entry: BundleEntry | null }[] = (index?.bundles ?? []).map((b) => ({
    id: b.id,
    stamp: b.stamp,
    entry: b,
  }));
  if (now) candidates.push({ id: now.bundle || "(current)", stamp: now, entry: null });
  const hits = candidates.filter((c) => stampKey(c.stamp) === key);
  if (!hits.length) {
    return {
      kind: "unknown",
      reason:
        `no engine bundle matches this record (stamp ${key}) — ` +
        `it was written by a build that was never archived and cannot be reproduced exactly`,
    };
  }
  // Prefer the running engine when it matches: it is already compiled and
  // its stamp is identical by definition of the match.
  const cur = hits.find((c) => c.entry == null);
  if (cur) return { kind: "current", bundle: now?.bundle ?? "" };
  return { kind: "archived", bundle: hits[0].id, entry: hits[0].entry! };
}

/** Base URL of an archived bundle (`/assets/engine/<id>/`). */
export function bundleBase(id: string): string {
  return `/assets/engine/${id}/`;
}

/** A missing bundle's message: name the id, where it would live, and the one
 *  command that fixes it when the bundle is rebuildable from git. */
export function missingBundleMessage(id: string): string {
  return (
    `engine bundle ${id} is not in the local archive (expected /assets/engine/${id}/) — ` +
    `this record can only be replayed by that exact build. ` +
    `If the bundle is still in git, run \`node tools/rebuild-engine.mjs ${id}\` and reload; ` +
    `otherwise copy it from the engine store (data/engine-archive/${id}/).`
  );
}

/** The index has the bundle but the site does not serve its bytes -- the
 *  usual case after a dist wipe and before `tools/rebuild-engine.mjs`. */
export function bundleNotDeployedMessage(id: string, commit?: string | null): string {
  const from = commit ? ` from ${commit.slice(0, 12)}…` : "";
  return (
    `engine bundle ${id} is indexed but not deployed (expected /assets/engine/${id}/) — ` +
    `run \`node tools/rebuild-engine.mjs ${id}\` to rebuild it${from} and reload`
  );
}