// The page's one IndexedDB database: everything durable lives here -- prefs,
// the profile and sound settings, per-server session tokens, the custom
// ruleset, the in-progress solo save (gzip-framed) and finished `.bdrec`
// replays. A long solo match alone overruns `localStorage`'s 5 MB quota, and a
// replay is binary, so nothing of size stays there. Only per-tab state
// (`sessionStorage`: the tab's token, the install hint) is outside this DB.

const DB_NAME = "bm";
/** v2 added `saves` / `custom`; v3 moved every `localStorage` key here
 *  (`prefs` / `account` / `session`). */
const DB_VERSION = 3;
const STORES = ["replays", "saves", "custom", "prefs", "account", "session"] as const;

export type StoreName = (typeof STORES)[number];

export function req<T>(r: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error ?? new Error("indexedDB request failed"));
  });
}

export function committed(t: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    t.oncomplete = () => resolve();
    t.onerror = () => reject(t.error ?? new Error("indexedDB transaction failed"));
    t.onabort = () => reject(t.error ?? new Error("indexedDB transaction aborted"));
  });
}

/** True when this browser exposes a usable `indexedDB` at all. Blocked
 *  third-party storage and very old engines report nothing; the callers keep
 *  working on their in-memory copies, they just do not persist. */
export function idbAvailable(): boolean {
  try {
    return typeof indexedDB !== "undefined" && indexedDB !== null;
  } catch {
    return false;
  }
}

/** Open (and on first use create / upgrade) the shared database. */
export function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const open = indexedDB.open(DB_NAME, DB_VERSION);
    open.onupgradeneeded = () => {
      const db = open.result;
      for (const name of STORES) {
        if (!db.objectStoreNames.contains(name)) db.createObjectStore(name, { keyPath: "id" });
      }
    };
    open.onsuccess = () => resolve(open.result);
    open.onerror = () => reject(open.error ?? new Error("indexedDB.open failed"));
  });
}
