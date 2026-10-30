// An in-memory IndexedDB fake for the node tests (`record.test.ts`,
// `soloSave.test.ts`, `customRules.test.ts`). Not a bundled module -- tests
// import it, the app never does.

type Row = { id: string } & Record<string, unknown>;

/** Just enough of the IndexedDB surface for `core/idb.ts`'s consumers: open /
 *  upgrade, a transaction, and put / get / getAll / delete on object stores.
 *  Returns the table map so a test can inspect or seed rows. */
export function installFakeIdb(): Map<string, Map<string, Row>> {
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
    const listeners: { success?: () => void; upgrade?: () => void } = {};
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
      set onerror(_f: () => void) {
        /* unused by the consumers */
      },
      set onupgradeneeded(f: () => void) {
        listeners.upgrade = f;
      },
    };
    return r as unknown as IDBOpenDBRequest;
  };

  (globalThis as { indexedDB?: unknown }).indexedDB = { open };
  return tables;
}

/** A minimal `localStorage` for the same tests. */
export function installFakeStorage(): Storage {
  const map = new Map<string, string>();
  const s = {
    get length() {
      return map.size;
    },
    clear: () => map.clear(),
    getItem: (k: string) => map.get(k) ?? null,
    key: (i: number) => [...map.keys()][i] ?? null,
    removeItem: (k: string) => void map.delete(k),
    setItem: (k: string, v: string) => void map.set(k, String(v)),
  };
  (globalThis as { localStorage?: unknown }).localStorage = s;
  return s as unknown as Storage;
}
