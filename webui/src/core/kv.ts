// Small string-keyed tables over IndexedDB, mirrored in memory.
//
// The page reads most of its settings synchronously -- at module scope, in a
// render, in a component's state initializer -- while IndexedDB is async. Each
// table therefore loads once into a `Map` during boot (`hydrateAppStorage` in
// `core/storage.ts`) and every `get` after that is synchronous; writes update
// the map immediately and reach the database in the background.

import { committed, idbAvailable, openDb, req } from "./idb.ts";

const STORE_NAMES = ["prefs", "account", "session"] as const;
export type KvName = (typeof STORE_NAMES)[number];
export const KV_STORES: readonly KvName[] = STORE_NAMES;

interface Row {
  id: string;
  value: string;
}

const caches = new Map<KvName, Map<string, string>>();
const warned = new Set<KvName>();

function table(name: KvName): Map<string, string> {
  let t = caches.get(name);
  if (!t) {
    t = new Map();
    caches.set(name, t);
  }
  return t;
}

function warn(name: KvName, e: unknown): void {
  if (warned.has(name)) return;
  warned.add(name);
  console.warn(`could not write the ${name} store:`, e);
}

/** Load one table into memory. Idempotent. */
export function hydrateKv(name: KvName): Promise<void> {
  const t = table(name);
  if (!idbAvailable()) return Promise.resolve();
  return (async () => {
    const db = await openDb();
    try {
      const tx = db.transaction(name, "readonly");
      const rows = (await req(tx.objectStore(name).getAll() as IDBRequest<Row[]>)) ?? [];
      await committed(tx);
      for (const r of rows) t.set(r.id, r.value);
    } finally {
      db.close();
    }
  })().catch((e) => warn(name, e));
}

async function put(name: KvName, id: string, value: string): Promise<void> {
  if (!idbAvailable()) return;
  const db = await openDb();
  try {
    const tx = db.transaction(name, "readwrite");
    tx.objectStore(name).put({ id, value } satisfies Row);
    await committed(tx);
  } finally {
    db.close();
  }
}

async function drop(name: KvName, id: string): Promise<void> {
  if (!idbAvailable()) return;
  const db = await openDb();
  try {
    const tx = db.transaction(name, "readwrite");
    tx.objectStore(name).delete(id);
    await committed(tx);
  } finally {
    db.close();
  }
}

export interface Kv {
  get(key: string): string;
  set(key: string, value: string): void;
  remove(key: string): void;
}

/** One table's synchronous view. */
export function kv(name: KvName): Kv {
  const t = table(name);
  return {
    get: (key) => t.get(key) ?? "",
    set: (key, value) => {
      t.set(key, value);
      void put(name, key, value).catch((e) => warn(name, e));
    },
    remove: (key) => {
      t.delete(key);
      void drop(name, key).catch((e) => warn(name, e));
    },
  };
}
