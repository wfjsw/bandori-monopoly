// Choosing the engine a portable export embeds (`docs/REPLAY.md` §9/§10).
// Run with:
//   node --test webui/src/game/portableExport.test.ts
//
// The regression this pins: an online record is sealed by the server, whose
// stamp names no bundle when the deploy did not ship the glue identity. The
// export must then resolve the bundle from the stamp exactly as opening the
// record does -- never report "nothing to embed".

import { strict as assert } from "node:assert";
import { afterEach, test } from "node:test";
import { resetArchiveIndex, stampKey, type ArchiveIndex, type BundleEntry } from "./engineBundle.ts";
import { collectBundleForExport, resolveExportBundle } from "./portableExport.ts";
import { PortableError } from "./portable.ts";
import type { EmbeddedBundle } from "./portable.ts";
import type { EngineStamp, RecordHeader } from "./record.ts";

const stamp = (over: Partial<EngineStamp> = {}): EngineStamp => ({
  format: 4,
  save_version: 17,
  abi: 115,
  ruleset_sha256: "rules-a",
  data_sha256: "",
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
    origin: "Server",
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
  glueSha256: "a".repeat(64),
});

const index = (bundles: BundleEntry[], current: string | null = null): ArchiveIndex => ({
  version: 1,
  current,
  bundles,
});

function embedded(bundle: string): EmbeddedBundle {
  return {
    manifest: {
      kind: "bdrec-portable",
      version: 1,
      bundle,
      glueSha256: "b".repeat(64),
      api: 1,
      compression: "zstd",
      blobLen: 0,
      blobSha256: "c".repeat(64),
      files: [],
    },
    files: new Map([["bundle.json", new Uint8Array([1])]]),
  };
}

afterEach(() => resetArchiveIndex());

test("an empty bundle resolves to the archived build with the same stamp", () => {
  const arch = entry("arch", stamp({ bundle: "arch" }));
  const picked = resolveExportBundle(header(), null, index([arch]), null);
  assert.equal(picked.bundle, "arch");
  assert.equal(picked.entry?.id, "arch");
  assert.equal(picked.origin, "hosted");
});

test("an empty bundle prefers the running engine when its stamp matches", () => {
  const now = stamp({ bundle: "cur" });
  const here = entry("cur", now);
  const other = entry("arch", stamp({ bundle: "arch" }));
  const picked = resolveExportBundle(header(), null, index([other, here]), now);
  assert.equal(picked.bundle, "cur");
});

test("an empty bundle the archive cannot match says why", () => {
  const other = entry("arch", stamp({ bundle: "arch", ruleset_sha256: "rules-b" }));
  assert.throws(
    () => resolveExportBundle(header(), null, index([other]), null),
    (e: unknown) => e instanceof PortableError && /never archived/.test(e.message),
  );
});

test("a record with no stamp says it cannot be identified", () => {
  const bare = { ...header(), engine: undefined } as unknown as RecordHeader;
  assert.throws(
    () => resolveExportBundle(bare, null, index([]), null),
    (e: unknown) => e instanceof PortableError && /no engine stamp/.test(e.message),
  );
});

test("a record that carries its engine re-exports without the archive", () => {
  const picked = resolveExportBundle(header(), embedded("emb"), index([]), null);
  assert.equal(picked.bundle, "emb");
  assert.equal(picked.origin, "embedded");
});

test("a named bundle resolves through the archive's alias", () => {
  const arch = entry("arch", stamp({ bundle: "arch" }));
  arch.aliases = { arch: "rebuilt", rebuilt: "rebuilt" };
  const picked = resolveExportBundle(header({ bundle: "arch" }), null, index([arch]), null);
  assert.equal(picked.bundle, "arch");
  assert.equal(picked.entry?.id, "arch");
});

test("collect fetches a legacy record's matching bundle", async () => {
  const arch = entry("arch", stamp({ bundle: "arch" }));
  const urls: string[] = [];
  globalThis.fetch = (async (input: RequestInfo | URL) => {
    const url = String(input);
    urls.push(url);
    if (url.endsWith("/index.json")) {
      return new Response(JSON.stringify(index([arch])), { status: 200 });
    }
    if (url.endsWith("/bundle.json")) {
      return new Response(
        JSON.stringify({ id: "arch", api: 1, glueSha256: "a".repeat(64), layout: { glueJs: "glue.js", glueWasm: "glue_bg.wasm" } }),
        { status: 200 },
      );
    }
    if (url.endsWith("/glue.js")) return new Response(new Uint8Array([1]), { status: 200 });
    if (url.endsWith("/glue_bg.wasm")) return new Response(new Uint8Array([2]), { status: 200 });
    return new Response("", { status: 404 });
  }) as typeof fetch;

  const got = await collectBundleForExport(header(), null, null, "http://x.test");
  assert.equal(got.bundle, "arch");
  assert.equal(got.origin, "hosted");
  assert.equal(got.api, 1);
  assert.deepEqual(got.files.map((f) => f.path).sort(), ["bundle.json", "glue.js", "glue_bg.wasm"]);
  assert.ok(urls.some((u) => u.includes("/assets/engine/arch/glue.js")), urls.join("\n"));
});

test("collect names the bundle when the deployment cannot serve it", async () => {
  globalThis.fetch = (async () => new Response("", { status: 404 })) as typeof fetch;
  await assert.rejects(
    () => collectBundleForExport(header({ bundle: "gone" }), null, null, "http://x.test"),
    (e: unknown) => e instanceof PortableError && /no copy of engine bundle gone/.test(e.message),
  );
});
