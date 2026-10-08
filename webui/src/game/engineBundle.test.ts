// Engine-bundle routing (`docs/REPLAY.md` §9). Run with:
//   node --test webui/src/game/engineBundle.test.ts
// Pure logic only -- no wasm, no fetch.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { resolveBundle, stampKey, type ArchiveIndex, type BundleEntry } from "./engineBundle.ts";
import type { EngineStamp, RecordHeader } from "./record.ts";

const stamp = (over: Partial<EngineStamp> = {}): EngineStamp => ({
  format: 1,
  save_version: 4,
  abi: 40,
  ruleset_sha256: "rules-a",
  data_sha256: "data-a",
  engine: "game-core",
  build: "0.1.0",
  bundle: "",
  ...over,
});

const header = (over: Partial<EngineStamp> = {}): RecordHeader =>
  ({
    engine: stamp(over),
    mode: 0,
    step: 0.05,
    origin: "Solo",
    created: "",
    seats: [],
    partial: false,
    ended: true,
    reason: "",
    rounds: 1,
    total_ticks: "10",
    gaps: false,
  }) as RecordHeader;

const entry = (id: string, s: EngineStamp): BundleEntry => ({
  id,
  stamp: s,
  key: stampKey(s),
});

const index = (bundles: BundleEntry[], current: string | null = bundles[0]?.id ?? null): ArchiveIndex => ({
  version: 1,
  current,
  bundles,
});

test("a record naming the running bundle plays in-page", () => {
  const now = stamp({ bundle: "aaa" });
  const c = resolveBundle(header({ bundle: "aaa" }), index([entry("aaa", now)]), now);
  assert.equal(c.kind, "current");
});

test("a record naming another bundle loads that bundle", () => {
  const now = stamp({ bundle: "bbb", data_sha256: "data-b" });
  const other = entry("aaa", stamp({ bundle: "aaa" }));
  const c = resolveBundle(header({ bundle: "aaa" }), index([other, entry("bbb", now)]), now);
  assert.equal(c.kind, "archived");
  assert.equal(c.kind === "archived" && c.bundle, "aaa");
});

test("a rebuild whose bytes hashed differently is reached through aliases", () => {
  // The seal is "aaa"; a rebuild from the same source produced byte id "rrr"
  // (different rustc install). Records stamped "aaa" must still resolve.
  const now = stamp({ bundle: "bbb", data_sha256: "data-b" });
  const rebuilt = entry("aaa", stamp({ bundle: "aaa" }));
  rebuilt.aliases = { aaa: "rrr", rrr: "rrr" };
  rebuilt.rebuilt = { byte_id: "rrr", source_id: "src", verified: true };
  const c = resolveBundle(header({ bundle: "aaa" }), index([rebuilt, entry("bbb", now)]), now);
  assert.equal(c.kind, "archived");
  assert.equal(c.kind === "archived" && c.bundle, "rrr", "serves the rebuilt bytes");
  // And a record the rebuilt engine wrote (stamped "rrr") resolves to the same.
  const c2 = resolveBundle(header({ bundle: "rrr" }), index([rebuilt, entry("bbb", now)]), now);
  assert.equal(c2.kind, "archived");
  assert.equal(c2.kind === "archived" && c2.bundle, "rrr");
});

test("a bundle the archive has lost is an error naming the id", () => {
  const now = stamp({ bundle: "bbb" });
  const c = resolveBundle(header({ bundle: "zzz" }), index([entry("bbb", now)]), now);
  assert.equal(c.kind, "missing");
  assert.equal(c.kind === "missing" && c.bundle, "zzz");
});

test("a legacy record matches the running engine when the stamps agree", () => {
  const now = stamp({ bundle: "aaa" });
  const c = resolveBundle(header({}), index([]), now);
  assert.equal(c.kind, "current");
});

test("a legacy record falls back to the archived build with the same stamp", () => {
  const now = stamp({ bundle: "bbb", data_sha256: "data-b" });
  const old = entry("aaa", stamp({ bundle: "aaa", data_sha256: "data-a" }));
  const c = resolveBundle(header({}), index([old]), now);
  assert.equal(c.kind, "archived");
  assert.equal(c.kind === "archived" && c.bundle, "aaa");
});

test("a legacy record from an unarchived build is not reproducible", () => {
  const now = stamp({ bundle: "bbb", data_sha256: "data-b" });
  const c = resolveBundle(header({ data_sha256: "data-ancient" }), index([entry("aaa", stamp())]), now);
  assert.equal(c.kind, "unknown");
  assert.match(c.kind === "unknown" ? c.reason : "", /never archived/);
});

test("stampKey covers the identity fields only", () => {
  assert.equal(stampKey(stamp({ bundle: "x", build: "y", engine: "z" })), "1|4|40|rules-a|data-a");
});