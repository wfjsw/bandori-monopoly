// `.bdrec` file handling and the local replay store. A `.bdrec` is a
// **zstd-framed** record JSON (`docs/REPLAY.md` "Format"): the compression runs
// in the wasm engine (`SoloMatch::record_zst`), and playback decompresses there
// too (`ReplayMatch::from_record_bytes`), which is why this module only sniffs
// the framing and stores opaque bytes. Older recordings are gzip (the browser
// used `CompressionStream("gzip")`) or plain JSON; the engine decodes all
// three, and the gzip helpers below stay for the legacy paths and tests.
// Nothing here talks to the engine -- the callers pass bytes and the header in
// -- so the module stays importable from node's test runner.

/** Who built the engine that wrote the record (`EngineStamp`). */
export interface EngineStamp {
  format: number;
  save_version: number;
  abi: number;
  ruleset_sha256: string;
  data_sha256: string;
  engine: string;
  build: string;
  /** The engine bundle that can replay this record (`docs/REPLAY.md` §9).
   *  Empty on records written before bundles existed -- the loader then
   *  matches the stamp's identity fields against the archive. */
  bundle?: string;
}

export interface SeatInfo {
  member: number;
  player: string;
  bot: boolean;
  mentality: "standard" | "chaos" | "advanced";
  character: string;
  rank: number;
  score: number;
}

/** Commit-reveal openings sealed into the header (`docs/FAIRNESS.md`). */
export interface Fairness {
  /** Scheme version of the recipe that produced these openings (1 or 2). */
  v: number;
  /** The commitment shown from the room's creation (hex). */
  commit: string;
  /** Server secret 256-bit seed (hex), revealed after the match. */
  seed: string;
  /** Server secret 256-bit salt (hex). */
  salt: string;
  /** Nonces of the members who sat in the match, ascending member id. */
  nonces: { member: number; nonce: string }[];
  /** Canonical room settings string; bound by the header, not by the v2 commit. */
  settings: string;
}

/** `RecordHeader`. `total_ticks` travels as a JSON string (u64). */
export interface RecordHeader {
  engine: EngineStamp;
  mode: number;
  step: number;
  origin: "Solo" | { Online: { room: string } };
  created: string;
  seats: SeatInfo[];
  partial: boolean;
  ended: boolean;
  reason: string;
  rounds: number;
  total_ticks: string;
  /** Server-side recording lost buffered ticks (a restart mid-match). */
  gaps?: boolean;
  /** Commit-reveal material (absent on records from before the scheme). */
  fair?: Fairness;
}

/** One checked step of the fairness verify (`docs/FAIRNESS.md`). */
export interface VerifyStep {
  step: string;
  ok: boolean;
  note: string;
}

/** What `verify_fair` reported about one record. */
export interface VerifyReport {
  ok: boolean;
  /** False when the record carries no fairness material (pre-scheme). */
  present: boolean;
  steps: VerifyStep[];
}

/** One field of `EngineStamp` that differs between record and engine. */
export interface Mismatch {
  field: string;
  want: string;
  got: string;
  fatal: boolean;
}

/** A scrub-bar / prev-turn mark (`TurnMark`); `tick` is a u64 string. */
export interface TurnMark {
  round: number;
  turn: number;
  tick: string;
}

/** `Replayer`'s `Status`; `tick` is a u64 string. */
export interface ReplayStatus {
  tick: string;
  ended: boolean;
  diverged: boolean;
}

/** `Replayer`'s `IndexStatus`; `bytes` is a u64 string. */
export interface IndexStatus {
  done: boolean;
  inputs: number;
  total: number;
  keyframes: number;
  bytes: string;
}

// ---------------------------------------------------------------- framing

/** The zstd magic `28 B5 2F FD`. */
export function isZstd(bytes: Uint8Array): boolean {
  return bytes.length >= 4 && bytes[0] === 0x28 && bytes[1] === 0xb5 && bytes[2] === 0x2f && bytes[3] === 0xfd;
}

/** The gzip magic `1f 8b`. */
export function isGzipped(bytes: Uint8Array): boolean {
  return bytes.length >= 2 && bytes[0] === 0x1f && bytes[1] === 0x8b;
}

/** What framing a `.bdrec` byte string carries. */
export type RecordEncoding = "zstd" | "gzip" | "json";

/** Sniff the framing: zstd (current), gzip (legacy) or plain JSON. */
export function sniffRecord(bytes: Uint8Array): RecordEncoding {
  return isZstd(bytes) ? "zstd" : isGzipped(bytes) ? "gzip" : "json";
}

// ---------------------------------------------------------------- gzip (legacy)

/** `CompressionStream` / `DecompressionStream` over a byte buffer. The DOM
 *  typings disagree with themselves about `BufferSource` vs `Uint8Array`, so
 *  the stream type is taken loosely here. */
async function pipe(src: Uint8Array | string, stream: CompressionStream | DecompressionStream): Promise<Uint8Array> {
  const bytes = typeof src === "string" ? new TextEncoder().encode(src) : src;
  const blob = new Blob([bytes.slice().buffer as ArrayBuffer]);
  const out = blob.stream().pipeThrough(stream as unknown as TransformStream<Uint8Array, Uint8Array>);
  return new Uint8Array(await new Response(out).arrayBuffer());
}

/** gzip a UTF-8 string. Legacy framing; new records are zstd via the engine. */
export function gzip(text: string): Promise<Uint8Array> {
  return pipe(text, new CompressionStream("gzip"));
}

/** gzip bytes. */
export function gzipBytes(bytes: Uint8Array): Promise<Uint8Array> {
  return pipe(bytes, new CompressionStream("gzip"));
}

/** gunzip to bytes. */
export function gunzip(bytes: Uint8Array): Promise<Uint8Array> {
  return pipe(bytes, new DecompressionStream("gzip"));
}

/** gunzip to text. */
export async function gunzipText(bytes: Uint8Array): Promise<string> {
  return new TextDecoder().decode(await gunzip(bytes));
}

/** Record JSON text from a legacy framing: gunzip a gzip member, decode plain
 *  JSON. A **zstd** body is decoded by the engine
 *  (`ReplayMatch::from_record_bytes` / `record_header_bytes`) -- the browser
 *  has no reliable zstd, so this throws rather than hand back garbage. */
export async function decodeRecord(bytes: Uint8Array): Promise<string> {
  const kind = sniffRecord(bytes);
  if (kind === "zstd") throw new Error("zstd .bdrec decodes in the engine (from_record_bytes)");
  return kind === "gzip" ? gunzipText(bytes) : new TextDecoder().decode(bytes);
}

// ---------------------------------------------------------------- files

/** The `.bdrec` name stamped with the record's own `created` when we have it. */
export function recordFilename(created: string, fallback = "replay"): string {
  const stamp = (created || "").replace(/[^0-9A-Za-z_-]+/g, "-").replace(/^-+|-+$/g, "") || fallback;
  return `bdrec-${stamp}.bdrec`;
}

/** Save `bytes` (a zstd-framed `.bdrec`) through the browser's download slot. */
export function downloadRecord(bytes: Uint8Array, filename: string): void {
  const url = URL.createObjectURL(new Blob([bytes.slice().buffer as ArrayBuffer], { type: "application/zstd" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = filename.endsWith(".bdrec") ? filename : `${filename}.bdrec`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

/** Read an opened file to its raw bytes -- zstd, gzip or plain JSON, left as
 *  they are for the engine to decode. */
export function readRecordFile(file: File): Promise<Uint8Array> {
  return file.arrayBuffer().then((b) => new Uint8Array(b));
}

// ---------------------------------------------------------------- store

const DB_NAME = "bm";
const DB_VERSION = 1;
const STORE = "replays";
/** How many solo records the store keeps (the newest by `savedAt`). */
export const REPLAY_KEEP = 10;

export interface ReplayEntry {
  id: string;
  header: RecordHeader;
  savedAt: number;
}

interface ReplayRow extends ReplayEntry {
  bytes: Uint8Array;
}

function req<T>(r: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error ?? new Error("indexedDB request failed"));
  });
}

function committed(t: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    t.oncomplete = () => resolve();
    t.onerror = () => reject(t.error ?? new Error("indexedDB transaction failed"));
    t.onabort = () => reject(t.error ?? new Error("indexedDB transaction aborted"));
  });
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const open = indexedDB.open(DB_NAME, DB_VERSION);
    open.onupgradeneeded = () => {
      const db = open.result;
      if (!db.objectStoreNames.contains(STORE)) db.createObjectStore(STORE, { keyPath: "id" });
    };
    open.onsuccess = () => resolve(open.result);
    open.onerror = () => reject(open.error ?? new Error("indexedDB.open failed"));
  });
}

/** Headers only, newest first. */
export async function listReplays(): Promise<ReplayEntry[]> {
  const db = await openDb();
  try {
    const t = db.transaction(STORE, "readonly");
    const rows = (await req(t.objectStore(STORE).getAll() as IDBRequest<ReplayRow[]>)) ?? [];
    await committed(t);
    return rows
      .map(({ id, header, savedAt }) => ({ id, header, savedAt }))
      .sort((a, b) => b.savedAt - a.savedAt);
  } finally {
    db.close();
  }
}

/** Store one record's bytes (a zstd-framed `.bdrec`), dropping everything past
 *  [`REPLAY_KEEP`]. Returns its id. */
export async function putReplay(header: RecordHeader, bytes: Uint8Array, id?: string): Promise<string> {
  const row: ReplayRow = {
    id: id ?? `r${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`,
    header,
    savedAt: Date.now(),
    bytes,
  };
  const db = await openDb();
  try {
    const t = db.transaction(STORE, "readwrite");
    const st = t.objectStore(STORE);
    st.put(row);
    const rows = (await req(st.getAll() as IDBRequest<ReplayRow[]>)) ?? [];
    const sorted = [...rows].sort((a, b) => b.savedAt - a.savedAt);
    for (const old of sorted.slice(REPLAY_KEEP)) st.delete(old.id);
    await committed(t);
  } finally {
    db.close();
  }
  return row.id;
}

/** The stored `.bdrec` bytes as written (zstd today, gzip for older rows), or null. */
export async function getReplay(id: string): Promise<Uint8Array | null> {
  const db = await openDb();
  try {
    const t = db.transaction(STORE, "readonly");
    const row = await req(t.objectStore(STORE).get(id) as IDBRequest<ReplayRow | undefined>);
    await committed(t);
    return row?.bytes ?? null;
  } finally {
    db.close();
  }
}

export async function deleteReplay(id: string): Promise<void> {
  const db = await openDb();
  try {
    const t = db.transaction(STORE, "readwrite");
    t.objectStore(STORE).delete(id);
    await committed(t);
  } finally {
    db.close();
  }
}