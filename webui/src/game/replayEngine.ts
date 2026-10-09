// The stable replay API (`docs/REPLAY.md` §9): one handle shape for every
// engine that can play a `.bdrec`, whether it is the wasm instance the page
// already runs or an archived bundle loaded in a worker. The frozen surface
// is `ReplayMatch` v1 (see `replay_api_version` in web-glue) -- the UI may
// only use what is declared here, so an old bundle keeps driving it.
//
// Match frames cross the boundary as JSON and are normalized on the way in
// (`normalizeMatchView`): a frame from an older bundle renders in the current
// UI, with defaults for fields that postdate it.

import type { MatchEvent, MatchView } from "../core/types";
import { rules } from "../core/data";
import {
  bundleBase,
  bundleNotDeployedMessage,
  type ArchiveIndex,
  type BundleChoice,
  missingBundleMessage,
} from "./engineBundle";
import type {
  EngineStamp,
  IndexStatus,
  Mismatch,
  RecordHeader,
  ReplayStatus,
  TurnMark,
} from "./record";

/** The running engine's stamp, or null when the glue cannot say. Lives here
 *  (not in `engineBundle`) so that module stays importable from node tests
 *  without loading the wasm. */
export function currentStamp(): EngineStamp | null {
  try {
    return JSON.parse(rules.engine_stamp()) as EngineStamp;
  } catch {
    return null;
  }
}

/** The running engine's bundle id ("" when this build has no glue identity). */
export function currentBundleId(): string {
  return currentStamp()?.bundle ?? "";
}

/** The frozen driver surface. Async: the archived path lives in a worker. */
export interface ReplayHandle {
  /** The bundle's `replay_api_version()` (1 when the glue predates the call). */
  readonly apiVersion: number;
  /** The bundle this handle runs, "" when the build has no identity. */
  readonly bundleId: string;
  /** "page" (the running engine) or "worker" (an archived bundle). */
  readonly kind: "page" | "worker";
  header(): Promise<RecordHeader>;
  compat(): Promise<Mismatch[]>;
  step(n: number): Promise<ReplayStatus>;
  seek(tick: number): Promise<ReplayStatus>;
  view(member: number): Promise<MatchView>;
  eventsSince(lastId: number): Promise<MatchEvent[]>;
  takeChanged(): Promise<boolean>;
  turns(): Promise<TurnMark[]>;
  totalTicks(): Promise<number>;
  status(): Promise<ReplayStatus>;
  index(budget: number): Promise<IndexStatus>;
  /** The commit-reveal fairness check (`docs/FAIRNESS.md`), run by this
   *  handle's engine -- the bundle that wrote the record. Returns the
   *  `VerifyReport` JSON. */
  verifyFair(bytes: Uint8Array): Promise<string>;
  free(): Promise<void>;
}

// ---------------------------------------------------------------- view tolerance

/**
 * Older bundles emit older `MatchState` frames. Keep the UI rendering them:
 * fill the fields that have grown since with their documented defaults.
 * Policy (`docs/REPLAY.md` §9): MatchState changes are **additive**, or they
 * land here keyed by bundle id.
 */
export function normalizeMatchView(raw: unknown): MatchView {
  const v = (raw ?? {}) as Record<string, any>;
  const st = (v.state ?? {}) as Record<string, any>;
  const def = <T>(x: T | undefined | null, d: T): T => (x === undefined || x === null ? d : x);
  st.players = def(st.players, []).map((p: any) => ({
    ...p,
    state: def(p?.state, {}),
    tokens: def(p?.tokens, []),
    field: def(p?.field, []).map((f: any) => ({ crystals: 0, cp: 0, faceDown: false, ...f })),
    actions: def(p?.actions, []),
    discard: def(p?.discard, []),
  }));
  st.events = def(st.events, []);
  st.marks = def(st.marks, []).map((m: any) => ({ category: "", count: 0, owner: -1, ...m }));
  st.houses = def(st.houses, []);
  st.mortgaged = def(st.mortgaged, []);
  st.embers = def(st.embers, []);
  st.owners = def(st.owners, []);
  st.bans = def(st.bans, []);
  st.eventActive = def(st.eventActive, []);
  st.eventTop = def(st.eventTop, []);
  st.eventDiscard = def(st.eventDiscard, []);
  st.plan = def(st.plan, { playerId: -1, from: -1, steps: 0, started: false, reach: [], canBuild: false });
  st.prompt = st.prompt
    ? { price: -1, prices: [], items: [], options: [], players: [], answers: [], ...st.prompt }
    : st.prompt;
  st.vote = st.vote ? { players: [], answers: [], ...st.vote } : st.vote;
  st.buyPrice = def(st.buyPrice, -1);
  st.buildCost = def(st.buildCost, -1);
  st.timeLeft = def(st.timeLeft, 0);
  st.shield = def(st.shield, 0);
  st.endReason = def(st.endReason, "");
  v.state = st;
  v.hand = def(v.hand, []);
  v.handNotes = def(v.handNotes, []);
  v.draw = def(v.draw, []);
  v.you = def(v.you, 0);
  v.playerId = def(v.playerId, -1);
  return v as unknown as MatchView;
}

// ---------------------------------------------------------------- page driver

/** The engine the page already runs (`rules.ReplayMatch`). */
export class PageReplayHandle implements ReplayHandle {
  readonly kind = "page" as const;
  readonly apiVersion: number;
  readonly bundleId: string;
  private m: InstanceType<typeof rules.ReplayMatch>;
  private done = false;

  constructor(m: InstanceType<typeof rules.ReplayMatch>, apiVersion = 1, bundleId = "") {
    this.m = m;
    this.apiVersion = apiVersion;
    this.bundleId = bundleId;
  }

  header(): Promise<RecordHeader> {
    return Promise.resolve(JSON.parse(this.m.header()) as RecordHeader);
  }
  compat(): Promise<Mismatch[]> {
    return Promise.resolve(JSON.parse(this.m.compat()) as Mismatch[]);
  }
  step(n: number): Promise<ReplayStatus> {
    return Promise.resolve(JSON.parse(this.m.step(n)) as ReplayStatus);
  }
  seek(tick: number): Promise<ReplayStatus> {
    return Promise.resolve(JSON.parse(this.m.seek(tick)) as ReplayStatus);
  }
  view(member: number): Promise<MatchView> {
    return Promise.resolve(normalizeMatchView(JSON.parse(this.m.view(member))));
  }
  eventsSince(lastId: number): Promise<MatchEvent[]> {
    return Promise.resolve(JSON.parse(this.m.events_since(lastId)) as MatchEvent[]);
  }
  takeChanged(): Promise<boolean> {
    return Promise.resolve(this.m.take_changed());
  }
  turns(): Promise<TurnMark[]> {
    return Promise.resolve(JSON.parse(this.m.turns()) as TurnMark[]);
  }
  totalTicks(): Promise<number> {
    return Promise.resolve(this.m.total_ticks());
  }
  status(): Promise<ReplayStatus> {
    return Promise.resolve(JSON.parse(this.m.status()) as ReplayStatus);
  }
  index(budget: number): Promise<IndexStatus> {
    return Promise.resolve(JSON.parse(this.m.index(budget)) as IndexStatus);
  }
  verifyFair(bytes: Uint8Array): Promise<string> {
    // The page's own glue. `verify_fair` arrived with the fairness scheme;
    // older builds do not have it.
    const f = (rules as unknown as { verify_fair?: (b: Uint8Array) => string }).verify_fair;
    if (typeof f !== "function") {
      return Promise.resolve(
        JSON.stringify({
          ok: false,
          present: false,
          steps: [{ step: "commit", ok: false, note: "engine predates fairness verification" }],
        }),
      );
    }
    return Promise.resolve(rules.verify_fair(bytes));
  }
  free(): Promise<void> {
    if (!this.done) {
      this.done = true;
      try {
        this.m.free();
      } catch {
        /* already freed */
      }
    }
    return Promise.resolve();
  }
}

// ---------------------------------------------------------------- worker driver

type WorkerRes = { id: number; ok: true; value?: unknown } | { id: number; ok: false; error: string };

/** One archived bundle driven over the worker protocol. */
export class WorkerReplayHandle implements ReplayHandle {
  readonly kind = "worker" as const;
  readonly apiVersion: number;
  readonly bundleId: string;
  private w: Worker;
  private seq = 0;
  private pending = new Map<number, { ok: (v: unknown) => void; err: (e: Error) => void }>();
  private done = false;

  private constructor(w: Worker, apiVersion: number, bundleId: string) {
    this.w = w;
    this.apiVersion = apiVersion;
    this.bundleId = bundleId;
    w.onmessage = (e: MessageEvent<WorkerRes>) => {
      const r = e.data;
      const p = this.pending.get(r.id);
      if (!p) return;
      this.pending.delete(r.id);
      if (r.ok) p.ok(r.value);
      else p.err(new Error(r.error));
    };
    w.onerror = (e) => {
      const err = new Error(`replay worker: ${e.message || "failed"}`);
      for (const p of this.pending.values()) p.err(err);
      this.pending.clear();
    };
  }

  /** Boot a worker on an archived bundle, then open `bytes` in it. */
  static async open(base: string, bundleId: string, bytes: Uint8Array, force: boolean): Promise<WorkerReplayHandle> {
    const w = new Worker("/assets/engine/replay-worker.js", { type: "module", name: `replay-${bundleId.slice(0, 8)}` });
    const h = new WorkerReplayHandle(w, 1, bundleId);
    let boot: { api: number; stamp: EngineStamp; id: string };
    try {
      boot = await h.rpc<{ api: number; stamp: EngineStamp; id: string }>("init", { base });
    } catch (e) {
      await h.free().catch(() => {});
      // The index has this bundle; the site just does not serve its bytes
      // (dist wipe, missing store, not yet rebuilt).
      throw new Error(bundleNotDeployedMessage(bundleId) + ` (${e instanceof Error ? e.message : e})`);
    }
    (h as { apiVersion: number }).apiVersion = boot.api ?? 1;
    await h.rpc("open", { bytes, force });
    return h;
  }

  private rpc<T>(op: string, rest: Record<string, unknown> = {}): Promise<T> {
    if (this.done) return Promise.reject(new Error("replay worker closed"));
    const id = ++this.seq;
    return new Promise<T>((ok, err) => {
      this.pending.set(id, { ok: ok as (v: unknown) => void, err });
      this.w.postMessage({ id, op, ...rest });
    });
  }

  private call<T>(method: string, ...args: unknown[]): Promise<T> {
    return this.rpc<T>("call", { method, args });
  }

  header(): Promise<RecordHeader> {
    return this.call("header");
  }
  compat(): Promise<Mismatch[]> {
    return this.call("compat");
  }
  step(n: number): Promise<ReplayStatus> {
    return this.call("step", n);
  }
  seek(tick: number): Promise<ReplayStatus> {
    return this.call("seek", tick);
  }
  async view(member: number): Promise<MatchView> {
    return normalizeMatchView(await this.call("view", member));
  }
  eventsSince(lastId: number): Promise<MatchEvent[]> {
    return this.call("events_since", lastId);
  }
  takeChanged(): Promise<boolean> {
    return this.call("take_changed");
  }
  turns(): Promise<TurnMark[]> {
    return this.call("turns");
  }
  totalTicks(): Promise<number> {
    return this.call("total_ticks");
  }
  status(): Promise<ReplayStatus> {
    return this.call("status");
  }
  index(budget: number): Promise<IndexStatus> {
    return this.call("index", budget);
  }
  verifyFair(bytes: Uint8Array): Promise<string> {
    return this.rpc("verify", { bytes });
  }
  async free(): Promise<void> {
    if (this.done) return;
    this.done = true;
    try {
      await this.rpc("close");
    } catch {
      /* worker may already be gone */
    }
    this.w.terminate();
    this.pending.clear();
  }
}

// ---------------------------------------------------------------- open

/**
 * Open a record on the engine that wrote it: the running instance when the
 * record names this page's bundle, otherwise an archived bundle in a worker.
 * Throws a message naming the bundle id when it cannot be reproduced.
 */
export async function openOnOwnEngine(
  choice: BundleChoice,
  bytes: Uint8Array,
  force = false,
): Promise<ReplayHandle> {
  if (choice.kind === "missing") throw new Error(missingBundleMessage(choice.bundle));
  if (choice.kind === "unknown") throw new Error(choice.reason);
  if (choice.kind === "current") {
    const m = rules.ReplayMatch.from_record_bytes(bytes, force);
    return new PageReplayHandle(m, typeof rules.replay_api_version === "function" ? rules.replay_api_version() : 1, choice.bundle);
  }
  return WorkerReplayHandle.open(bundleBase(choice.bundle), choice.bundle, bytes, force);
}

/** The in-page engine as a handle without a record (diagnostics / tests). */
export function pageApiVersion(): number {
  return typeof rules.replay_api_version === "function" ? rules.replay_api_version() : 1;
}

/** `ArchiveIndex` is re-exported so the player can fetch it once and hand it
 *  to {@link openOnOwnEngine} via {@link resolveBundle}. */
export type { ArchiveIndex };