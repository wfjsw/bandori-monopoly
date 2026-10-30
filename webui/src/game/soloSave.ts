// The one in-progress solo match, persisted so a refresh can resume it.
//
// The payload is the engine's whole-match snapshot plus its recorder log; both
// are JSON that only grows, and together they overran `localStorage`'s 5 MB
// origin quota on a long game -- which failed silently and, worse, threw the
// same warning on every one-second save. Both now live in IndexedDB (the shared
// `bm` database, `core/idb.ts`) as gzip-framed bytes.
//
// `hasSave()` is read during render, so row existence is mirrored by an
// in-memory flag hydrated once at boot (`hydrateSoloSave`), not by a probe per
// call. Writes coalesce -- the worker can push faster than a large transaction
// settles -- and only one warning is printed until a write succeeds again.
//
// Only snapshots the worker took at a save point reach here (`soloProtocol.ts`
// `safeSavePoint`): the match was waiting for a command, so a resume lands on
// a surface that can be driven rather than mid-effect.

import type { ScoreWeights } from "../core/types";
import { committed, openDb, req } from "../core/idb.ts";

const ROW_ID = "solo";
const WARN = "could not save the solo match:";

/** The persisted match. `match` is `SoloMatch::save()` and `rec`
 *  `record_state()`; `last` is the event cursor that belongs to them. */
export interface SoloSave {
  v: number;
  match: string;
  weights: ScoreWeights;
  recorded: boolean;
  /** The event cursor the snapshot's strings were taken at. */
  last: number;
  /** The recorder beside `match` (absent on saves from before records). */
  rec?: string;
  /** IndexedDB id of the `.bdrec` exported when the match ended. */
  replayId?: string;
}

interface SaveRow {
  id: string;
  v: number;
  /** True when `match` / `rec` are gzip-framed. */
  z: boolean;
  match: Uint8Array;
  rec?: Uint8Array;
  weights: ScoreWeights;
  recorded: boolean;
  last: number;
  replayId?: string;
}

let exists = false;

// ------------------------------------------------------------ compression

const canCompress = typeof CompressionStream !== "undefined";

async function squash(s: string): Promise<Uint8Array> {
  const bytes = new TextEncoder().encode(s);
  if (!canCompress) return bytes;
  const stream = new Blob([bytes as BlobPart]).stream().pipeThrough(new CompressionStream("gzip"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

async function unsquash(b: Uint8Array, z: boolean): Promise<string> {
  if (!z) return new TextDecoder().decode(b);
  const stream = new Blob([b as BlobPart]).stream().pipeThrough(new DecompressionStream("gzip"));
  return new Response(stream).text();
}

// ---------------------------------------------------------------- read

async function readRow(): Promise<SaveRow | null> {
  const db = await openDb();
  try {
    const t = db.transaction("saves", "readonly");
    const row = await req(t.objectStore("saves").get(ROW_ID) as IDBRequest<SaveRow | undefined>);
    await committed(t);
    return row ?? null;
  } finally {
    db.close();
  }
}

/** Note whether a save exists, for `hasSoloSave()` (once, at boot). */
export async function hydrateSoloSave(): Promise<void> {
  try {
    exists = !!(await readRow());
  } catch {
    exists = false;
  }
}

/** True when a resumable save exists. */
export function hasSoloSave(): boolean {
  return exists;
}

/** The saved match, or null. */
export async function loadSoloSave(): Promise<SoloSave | null> {
  try {
    const row = await readRow();
    if (!row) {
      exists = false;
      return null;
    }
    const match = await unsquash(row.match, row.z);
    const rec = row.rec ? await unsquash(row.rec, row.z) : undefined;
    return {
      v: row.v,
      match,
      weights: row.weights,
      recorded: row.recorded,
      last: row.last,
      ...(rec ? { rec } : {}),
      ...(row.replayId ? { replayId: row.replayId } : {}),
    };
  } catch (e) {
    console.warn("could not read the solo save:", e);
    return null;
  }
}

// ---------------------------------------------------------------- write

let chain: Promise<void> = Promise.resolve();
let queued: SoloSave | null = null;
let writing = false;

function warn(e: unknown): void {
  if (warned) return;
  warned = true;
  console.warn(WARN, e);
}

async function dropRow(): Promise<void> {
  const db = await openDb();
  try {
    const t = db.transaction("saves", "readwrite");
    t.objectStore("saves").delete(ROW_ID);
    await committed(t);
  } finally {
    db.close();
  }
}

async function writeRow(s: SoloSave): Promise<void> {
  const z = canCompress;
  const row: SaveRow = {
    id: ROW_ID,
    v: s.v,
    z,
    match: await squash(s.match),
    weights: s.weights,
    recorded: s.recorded,
    last: s.last,
    ...(s.rec ? { rec: await squash(s.rec) } : {}),
    ...(s.replayId ? { replayId: s.replayId } : {}),
  };
  // A newer snapshot arrived while this one compressed: let the queue's next
  // iteration write that one instead.
  if (queued) return;
  const db = await openDb();
  try {
    const t = db.transaction("saves", "readwrite");
    t.objectStore("saves").put(row);
    await committed(t);
  } finally {
    db.close();
  }
}

function pump(): void {
  if (writing || !queued) return;
  const next = queued;
  queued = null;
  writing = true;
  chain = chain
    .then(() => writeRow(next))
    .then(() => {
      warned = false;
    })
    .catch(warn)
    .finally(() => {
      writing = false;
      pump();
    });
}

/** Drop the save. */
export function clearSoloSave(): void {
  exists = false;
  lastSaved = null;
  queued = null;
  // Serialized behind any write already queued, so a late put cannot bring the
  // row back after the delete.
  chain = chain.then(dropRow).catch(() => undefined);
}

/**
 * Save the match. The payload is written in the background, coalesced; an
 * identical consecutive snapshot is skipped.
 */
export function saveSoloSave(s: SoloSave): void {
  const last = lastSaved;
  if (
    last &&
    last.match === s.match &&
    last.rec === s.rec &&
    last.weights === s.weights &&
    last.recorded === s.recorded &&
    last.replayId === s.replayId &&
    last.last === s.last
  ) {
    return;
  }
  lastSaved = s;
  exists = true;
  queued = s;
  pump();
}

let lastSaved: SoloSave | null = null;
let warned = false;
