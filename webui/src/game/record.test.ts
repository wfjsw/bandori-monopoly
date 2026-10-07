// Unit tests for the `.bdrec` helpers. Run with:
//   node --test webui/src/game/record.test.ts
// (Node's built-in runner; `CompressionStream` needs no polyfill on Node 24,
// and node's `zlib` is the reference zstd used to build / check fixtures.)

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { zstdCompressSync, zstdDecompressSync } from "node:zlib";
import {
  decodeRecord,
  deleteReplay,
  getReplay,
  gzip,
  gunzipText,
  isGzipped,
  isZstd,
  listReplays,
  putReplay,
  REPLAY_KEEP,
  recordFilename,
  sniffRecord,
  type RecordHeader,
} from "./record.ts";

// ---------------------------------------------------------------- fixtures

const JSON_TEXT = JSON.stringify({ magic: "bdrec", header: { created: "2026-10-07 12:00" }, body: {}, check: "0" });

function header(over: Partial<RecordHeader> = {}): RecordHeader {
  return {
    engine: {
      format: 1,
      save_version: 1,
      abi: 1,
      ruleset_sha256: "stub",
      data_sha256: "stub",
      engine: "game-core",
      build: "test",
    },
    mode: 0,
    step: 0.05,
    origin: "Solo",
    created: "2026-10-07 12:00",
    seats: [],
    partial: false,
    ended: true,
    reason: "settle",
    rounds: 3,
    total_ticks: "120",
    ...over,
  };
}

// ------------------------------------------------- in-memory IndexedDB fake

type Row = { id: string } & Record<string, unknown>;

/** Just enough of the IndexedDB surface for `record.ts`: open / upgrade,
 *  a transaction, and put / get / getAll / delete on one object store. */
function installFakeIdb(): Map<string, Row> {
  const tables = new Map<string, Map<string, Row>>();
  const later = (fn: () => void) => queueMicrotask(fn);

  const request = <T>(get: () => T): IDBRequest<T> => {
    const listeners: { success?: () => void; error?: () => void } = {};
    const r = {
      result: undefined as unknown as T,
      error: null as DOMException | null,
      set onsuccess(f: () => void) {
        listeners.success = f;
        later(() => {
          try {
            r.result = get();
          } catch (e) {
            r.error = e as DOMException;
            listeners.error?.();
            return;
          }
          listeners.success?.();
        });
      },
      set onerror(f: () => void) {
        listeners.error = f;
      },
    };
    return r as unknown as IDBRequest<T>;
  };

  const storeOf = (table: Map<string, Row>): IDBObjectStore =>
    ({
      put(row: Row) {
        request(() => {
          table.set(row.id, row);
          return row.id;
        }).onsuccess = () => undefined;
      },
      get(id: string) {
        return request(() => table.get(id)) as IDBRequest<Row | undefined>;
      },
      getAll() {
        return request(() => [...table.values()]) as IDBRequest<Row[]>;
      },
      delete(id: string) {
        request(() => {
          table.delete(id);
          return undefined;
        }).onsuccess = () => undefined;
      },
    }) as unknown as IDBObjectStore;

  const open = (name: string, _version: number): IDBOpenDBRequest => {
    let table = tables.get(name);
    const upgraders: (() => void)[] = [];
    const db = {
      objectStoreNames: { contains: (s: string) => tables.has(`${name}/${s}`) },
      createObjectStore(key: string) {
        tables.set(`${name}/${key}`, new Map());
        return null;
      },
      transaction(_store: string, _mode: string) {
        const key = `${name}/${_store}`;
        const t = tables.get(key) ?? new Map();
        tables.set(key, t);
        const txListeners: { complete?: () => void; error?: () => void; abort?: () => void } = {};
        const tx = {
          objectStore: () => storeOf(t),
          set oncomplete(f: () => void) {
            txListeners.complete = f;
            later(() => txListeners.complete?.());
          },
          set onerror(f: () => void) {
            txListeners.error = f;
          },
          set onabort(f: () => void) {
            txListeners.abort = f;
          },
        };
        return tx as unknown as IDBTransaction;
      },
      close() {
        return undefined;
      },
    };
    const listeners: { success?: () => void; error?: () => void; upgrade?: () => void } = {};
    const r = {
      result: db as unknown as IDBDatabase,
      error: null as DOMException | null,
      set onsuccess(f: () => void) {
        listeners.success = f;
        later(() => {
          if (!table) {
            table = new Map();
            tables.set(name, table);
            listeners.upgrade?.();
          }
          listeners.success?.();
        });
      },
      set onerror(f: () => void) {
        listeners.error = f;
      },
      set onupgradeneeded(f: () => void) {
        listeners.upgrade = f;
        upgraders.push(f);
      },
    };
    void upgraders;
    return r as unknown as IDBOpenDBRequest;
  };

  (globalThis as { indexedDB?: unknown }).indexedDB = { open };
  return tables;
}

// ---------------------------------------------------------------- framing

test("sniffing sees zstd, gzip and plain JSON", () => {
  const plain = new TextEncoder().encode(JSON_TEXT);
  const gz = new Uint8Array([0x1f, 0x8b, 8, 0]);
  const zst = new Uint8Array(zstdCompressSync(Buffer.from(JSON_TEXT)));
  assert.deepEqual([...zst.slice(0, 4)], [0x28, 0xb5, 0x2f, 0xfd], "zstd magic 28 B5 2F FD");
  assert.deepEqual([...gz.slice(0, 2)], [0x1f, 0x8b], "gzip magic 1F 8B");
  assert.equal(sniffRecord(zst), "zstd");
  assert.equal(sniffRecord(gz), "gzip");
  assert.equal(sniffRecord(plain), "json");
  assert.ok(isZstd(zst) && !isGzipped(zst));
  assert.ok(isGzipped(gz) && !isZstd(gz));
  assert.ok(!isZstd(plain) && !isGzipped(plain));
});

test("gzip round-trips a record body", async () => {
  const z = await gzip(JSON_TEXT);
  assert.ok(z.length > 0);
  assert.ok(isGzipped(z), "gzip output carries the 1f 8b magic");
  assert.equal(await gunzipText(z), JSON_TEXT);
});

test("zstd round-trips a record body (the reference decoder reads our fixture)", () => {
  const zst = new Uint8Array(zstdCompressSync(Buffer.from(JSON_TEXT)));
  assert.equal(sniffRecord(zst), "zstd");
  assert.equal(zstdDecompressSync(Buffer.from(zst)).toString(), JSON_TEXT);
});

test("decodeRecord reads gzip and plain JSON and refuses zstd", async () => {
  const plain = new TextEncoder().encode(JSON_TEXT);
  assert.equal(await decodeRecord(plain), JSON_TEXT);

  const z = await gzip(JSON_TEXT);
  assert.equal(await decodeRecord(z), JSON_TEXT);

  // A zstd body is the engine's job (`from_record_bytes`); say so rather than
  // hand back binary garbage.
  const zst = new Uint8Array(zstdCompressSync(Buffer.from(JSON_TEXT)));
  await assert.rejects(decodeRecord(zst), /engine/i);
});

test("recordFilename stamps .bdrec", () => {
  assert.equal(recordFilename("2026-10-07 12:00"), "bdrec-2026-10-07-12-00.bdrec");
  assert.equal(recordFilename(""), "bdrec-replay.bdrec");
});

// ---------------------------------------------------------------- store

test("zstd .bdrec bytes round-trip through the store", async () => {
  installFakeIdb();
  const zst = new Uint8Array(zstdCompressSync(Buffer.from(JSON_TEXT)));
  const id = await putReplay(header(), zst);
  const got = await getReplay(id);
  assert.ok(got);
  assert.deepEqual([...got], [...zst], "the store keeps the bytes as written");
  assert.equal(sniffRecord(got), "zstd");
  assert.equal(zstdDecompressSync(Buffer.from(got)).toString(), JSON_TEXT);
});

test("indexedDB store keeps the newest N records", async () => {
  installFakeIdb();
  const bytes = new Uint8Array(zstdCompressSync(Buffer.from(JSON_TEXT)));
  const ids: string[] = [];
  for (let i = 0; i < REPLAY_KEEP + 5; i++) {
    // One id per put, oldest first: `savedAt` is Date.now(), so give the rows
    // distinct times through the id order (the fake stores what we pass).
    ids.push(await putReplay(header({ created: `c${i}`, rounds: i }), bytes, `id${i}`));
    // Keep Date.now() monotonic across the loop for the sort.
    await new Promise((r) => setTimeout(r, 2));
  }
  const list = await listReplays();
  assert.equal(list.length, REPLAY_KEEP, `kept ${REPLAY_KEEP} of ${ids.length}`);
  // The newest ids survive; the oldest were dropped.
  const kept = new Set(list.map((e) => e.id));
  for (const id of ids.slice(-REPLAY_KEEP)) assert.ok(kept.has(id), `${id} kept`);
  for (const id of ids.slice(0, ids.length - REPLAY_KEEP)) assert.ok(!kept.has(id), `${id} dropped`);
  // Newest first.
  assert.deepEqual(list.map((e) => e.id), ids.slice(-REPLAY_KEEP).reverse());

  const got = await getReplay(ids[ids.length - 1]);
  assert.ok(got);
  assert.equal(sniffRecord(got), "zstd");

  await deleteReplay(ids[ids.length - 1]);
  assert.equal((await listReplays()).length, REPLAY_KEEP - 1);
  assert.equal(await getReplay(ids[ids.length - 1]), null);
});