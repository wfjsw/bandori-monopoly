// The user's custom solo ruleset: one wasm module stored as bytes.
//
// It used to be a base64 string in `localStorage`, which inflated it by a third
// and competed with the match save for the 5 MB quota. The bytes now live in
// the shared `bm` IndexedDB (`core/idb.ts`); a synchronous in-memory copy
// (`currentCustomRules`) keeps every boot path that reads it -- `soloBootOpts`,
// the replay worker, the source snapshot -- synchronous. `hydrateCustomRules`
// loads that copy once at boot, before the first ruleset load.

import { committed, idbAvailable, openDb, req } from "./idb.ts";

export interface CustomRules {
  name: string;
  bytes: Uint8Array;
}

const ROW_ID = "custom";

interface CustomRow {
  id: string;
  name: string;
  bytes: Uint8Array;
}

let current: CustomRules | null = null;
let hydration: Promise<void> | null = null;

/** The custom module in force, or null. */
export function currentCustomRules(): CustomRules | null {
  return current;
}

/** A cheap identity for the module (FNV-1a over the name and bytes), so a
 *  source snapshot can tell two same-length rulesets apart. */
export function customRulesHash(c: CustomRules | null): string {
  if (!c) return "";
  let h = 0x811c9dc5;
  const add = (b: number) => {
    h ^= b;
    h = Math.imul(h, 0x01000193) >>> 0;
  };
  for (let i = 0; i < c.name.length; i++) add(c.name.charCodeAt(i));
  add(0);
  for (let i = 0; i < c.bytes.length; i++) add(c.bytes[i]);
  return `${c.bytes.length.toString(36)}-${h.toString(36)}`;
}

async function readRow(): Promise<CustomRow | null> {
  const db = await openDb();
  try {
    const t = db.transaction("custom", "readonly");
    const row = await req(t.objectStore("custom").get(ROW_ID) as IDBRequest<CustomRow | undefined>);
    await committed(t);
    return row ?? null;
  } finally {
    db.close();
  }
}

/** Load the stored module into memory (idempotent, once at boot). */
export function hydrateCustomRules(): Promise<void> {
  hydration ??= (async () => {
    if (!idbAvailable()) return;
    try {
      const row = await readRow();
      if (row) current = { name: row.name, bytes: row.bytes };
    } catch (e) {
      console.warn("could not read the custom ruleset:", e);
    }
  })();
  return hydration;
}

/** Store (and make live) the module. False when the store refuses it. */
export async function storeCustomRules(bytes: Uint8Array, name: string): Promise<boolean> {
  current = { name, bytes };
  if (!idbAvailable()) return false;
  try {
    const row: CustomRow = { id: ROW_ID, name, bytes };
    const db = await openDb();
    try {
      const t = db.transaction("custom", "readwrite");
      t.objectStore("custom").put(row);
      await committed(t);
    } finally {
      db.close();
    }
    return true;
  } catch (e) {
    console.warn("could not store the custom ruleset:", e);
    return false;
  }
}

/** Forget the module (memory and store). */
export async function forgetCustomRules(): Promise<void> {
  current = null;
  if (!idbAvailable()) return;
  try {
    const db = await openDb();
    try {
      const t = db.transaction("custom", "readwrite");
      t.objectStore("custom").delete(ROW_ID);
      await committed(t);
    } finally {
      db.close();
    }
  } catch {
    /* the in-memory value is already gone */
  }
}
