// The advanced-bot worker pool (`docs/BOT.md` B6).
//
// N Web Workers, each its own wasm instance of `bot-glue` (game-core +
// game-rules wasmi + bot-core) and its own determinization stream. One seat's
// view goes out; per-action root statistics come back; the page merges them
// (`search_root_parallel`'s `merge_stats`) and picks the action to send
// through the ordinary `act` path.
//
// Lazy: nothing downloads until an advanced seat / advanced 托管 is in use
// ([`ensureBotPool`]). The main page's glue is untouched.
//
// Information boundary (`docs/BOT.md` §1): the worker receives ONE seat's view
// -- the same frame the page renders for that seat -- never the World, a match
// seed, or another seat's hidden information. The seed is the **search** RNG's
// ([`decisionSeed`] + [`seedForThread`]), never the match's. Recordings are
// unchanged because the answers are what get recorded.

import type { Command, MatchView } from "../core/types.ts";
import { outerDeadlineMs, seedForThread } from "./botBudget.ts";

// ---------------------------------------------------------------- merge

/** One root action's statistics, as `bot-glue` returns them (`rootStats`). */
export interface RootStat {
  /** The public command the page would send for this action. */
  answer: Command;
  visits: number;
  valueSum: number;
  mean: number;
  evalMax: number;
  evalSum: number;
  evalVisits: number;
}

/**
 * Merge per-worker root statistics into one list, sorted best-first (visits
 * desc, mean desc, stable command order). Mirrors `bot_core::merge_stats`
 * exactly -- `botPool.test.ts` pins the two together. Value sums are added,
 * `evalMax` takes the max.
 */
export function mergeRootStats(parts: readonly RootStat[][]): RootStat[] {
  const by = new Map<string, RootStat>();
  for (const part of parts) {
    for (const s of part) {
      const k = JSON.stringify(s.answer);
      const e = by.get(k);
      if (e) {
        e.visits += s.visits;
        e.valueSum += s.valueSum;
        e.evalMax = Math.max(e.evalMax, s.evalMax);
        e.evalSum += s.evalSum;
        e.evalVisits += s.evalVisits;
      } else {
        by.set(k, {
          answer: s.answer,
          visits: s.visits,
          valueSum: s.valueSum,
          mean: s.mean,
          evalMax: s.evalMax,
          evalSum: s.evalSum,
          evalVisits: s.evalVisits,
        });
      }
    }
  }
  const out = [...by.values()];
  for (const s of out) {
    if (!Number.isFinite(s.evalMax)) s.evalMax = 0;
    if (s.visits > 0) s.mean = s.valueSum / s.visits;
    else s.mean = 0;
  }
  out.sort((x, y) => {
    if (y.visits !== x.visits) return y.visits - x.visits;
    if (y.mean !== x.mean) return y.mean - x.mean;
    // Stable total order so the merged pick is deterministic on ties.
    return JSON.stringify(x.answer) < JSON.stringify(y.answer) ? -1 : 1;
  });
  return out;
}

/** The action the merged statistics pick (best-first). */
export function pickAction(merged: readonly RootStat[]): Command | null {
  return merged.length ? merged[0].answer : null;
}

// ---------------------------------------------------------------- worker

export interface BotDecideResult {
  answer: Command;
  iterations: number;
  elapsedMs: number;
  heuristic: boolean;
  reused: boolean;
  decisionKey: string;
  /** Merged across the pool (empty when the heuristic answered). */
  rootStats: RootStat[];
  /** How many workers contributed (1 = single search). */
  workers: number;
  /** True when this was the merged pick rather than a single worker's answer. */
  merged: boolean;
}

export interface BotPoolOpts {
  /** Worker count. Defaults to [`defaultWorkerCount`]. */
  workers?: number;
  /** Absolute URL of the bot-glue bundle directory (trailing slash). */
  baseUrl?: string;
  /** Injected for tests; defaults to `new Worker(...)`. */
  workerFactory?: (url: URL, index: number) => BotWorkerLike;
}

/** The slice of `Worker` the pool uses (so tests can inject a fake). */
export interface BotWorkerLike {
  postMessage(msg: unknown): void;
  terminate(): void;
  onmessage: ((e: { data: unknown }) => void) | null;
  onerror: ((e: unknown) => void) | null;
}

interface WorkerReply {
  id: number;
  ok: boolean;
  value?: unknown;
  error?: string;
}

/**
 * How many workers this device should get:
 * `clamp(navigator.hardwareConcurrency − 1, 1, 4)`, fewer on phones /
 * low-memory (`navigator.deviceMemory`).
 */
export function defaultWorkerCount(): number {
  const hc = typeof navigator !== "undefined" ? navigator.hardwareConcurrency || 2 : 2;
  const mem = typeof navigator !== "undefined" ? (navigator as { deviceMemory?: number }).deviceMemory : undefined;
  let n = Math.min(4, Math.max(1, hc - 1));
  // Phones / low-memory: 8 GB and under gets one fewer, never below 1.
  if (mem != null && mem <= 4) n = 1;
  else if (mem != null && mem <= 8) n = Math.min(n, 2);
  return n;
}

/** Absolute URL of the bot-glue bundle (the worker imports it from there). */
export function botBaseUrl(): URL {
  // Resolved against the page, like `/assets/engine/bot-glue/`. The worker
  // script itself lives beside it (`bot-worker.js`).
  const base = typeof document !== "undefined" ? document.baseURI : "http://localhost/";
  return new URL("assets/engine/bot-glue/", base);
}

class WorkerSlot {
  readonly index: number;
  private readonly base: URL;
  private readonly factory: (url: URL, index: number) => BotWorkerLike;
  private w: BotWorkerLike | null = null;
  private nextId = 1;
  private pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void; timer: number }>();
  ready = false;

  constructor(base: URL, factory: (url: URL, index: number) => BotWorkerLike, index = 0) {
    this.base = base;
    this.factory = factory;
    this.index = index;
  }

  private ensure(): BotWorkerLike {
    if (this.w) return this.w;
    const w = this.factory(new URL("bot-worker.js", this.base), this.index);
    w.onmessage = (e) => {
      const r = e.data as WorkerReply;
      const p = this.pending.get(r.id);
      if (!p) return;
      this.pending.delete(r.id);
      clearTimeout(p.timer);
      if (r.ok) p.resolve(r.value);
      else p.reject(new Error(r.error ?? "bot worker error"));
    };
    w.onerror = (e) => {
      const err = new Error(String((e as { message?: string })?.message ?? e));
      for (const [, p] of this.pending) {
        clearTimeout(p.timer);
        p.reject(err);
      }
      this.pending.clear();
    };
    this.w = w;
    return w;
  }

  /** One request. Rejects on timeout (the caller falls back). */
  call(op: string, payload: Record<string, unknown>, timeoutMs: number): Promise<unknown> {
    const w = this.ensure();
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`bot worker ${op} timed out after ${timeoutMs} ms`));
      }, timeoutMs) as unknown as number;
      this.pending.set(id, { resolve, reject, timer });
      w.postMessage({ id, op, ...payload });
    });
  }

  dispose(): void {
    for (const [, p] of this.pending) clearTimeout(p.timer);
    this.pending.clear();
    this.w?.terminate();
    this.w = null;
    this.ready = false;
  }
}

/**
 * The pool. One instance per page; [`ensureBotPool`] hands out the shared one.
 * Each worker keeps its own tree-reuse cache per seat (root-parallel across
 * workers merges only the root statistics, as `search_root_parallel` does).
 */
export class BotPool {
  readonly workers: number;
  private readonly slots: WorkerSlot[] = [];
  private readonly base: URL;
  private readonly factory: (url: URL, index: number) => BotWorkerLike;
  private loaded: Promise<void> | null = null;
  private disposed = false;

  constructor(opts: BotPoolOpts = {}) {
    this.workers = Math.max(1, opts.workers ?? defaultWorkerCount());
    this.base = opts.baseUrl ? new URL(opts.baseUrl) : botBaseUrl();
    this.factory =
      opts.workerFactory ??
      ((url) => new Worker(url, { type: "module" }) as unknown as BotWorkerLike);
    for (let i = 0; i < this.workers; i++) {
      this.slots.push(new WorkerSlot(this.base, this.factory, i));
    }
  }

  /** Load the bundle into every worker (idempotent). */
  ensureLoaded(): Promise<void> {
    if (this.disposed) return Promise.reject(new Error("bot pool disposed"));
    this.loaded ??= (async () => {
      // Sequential: the wasm download is shared HTTP cache; parallel init just
      // spikes the main thread with fetch bookkeeping.
      for (const s of this.slots) {
        await s.call("init", { base: this.base.href }, 60_000);
        s.ready = true;
      }
    })();
    return this.loaded;
  }

  /**
   * One decision. Spawns `this.workers` searches in parallel (each with its
   * own `seedForThread(seed, i)`), merges the root statistics, and returns the
   * pick. Falls back to a single worker's answer when only one is alive.
   *
   * `budgetMs` is the **search** budget each worker gets (they run in
   * parallel); the outer wait is [`outerDeadlineMs`] and then the caller's
   * heuristic fallback.
   */
  async decide(view: MatchView, budgetMs: number, seed: number): Promise<BotDecideResult> {
    await this.ensureLoaded();
    const deadline = outerDeadlineMs(budgetMs);
    const started = Date.now();
    const jobs = this.slots.map((s, i) =>
      s
        .call("decide", { view, budgetMs, seed: seedForThread(seed, i) }, deadline)
        .then((v) => v as DecideReply)
        .catch((e: unknown) => {
          console.warn(`bot worker ${i}:`, e);
          return null;
        }),
    );
    // All workers share the same wall budget; wait for them together, but
    // never past the outer deadline (the timeout fallback is the heuristic).
    const settled = await Promise.all(jobs);
    const oks = settled.filter((r): r is DecideReply => !!r && r.ok !== false);
    if (!oks.length) {
      throw new Error("every bot worker failed");
    }
    const parts = oks.map((r) => r.rootStats ?? []);
    const merged = mergeRootStats(parts);
    const multi = oks.length > 1 && merged.length > 0;
    // Heuristic-delegated (or a single worker): take that worker's answer.
    // Otherwise the merged pick, which may differ from any worker's own top.
    const chosen = multi ? pickAction(merged) : oks[0].answer;
    if (!chosen) throw new Error("bot workers returned no answer");
    const iterations = oks.reduce((n, r) => n + (r.iterations ?? 0), 0);
    return {
      answer: chosen,
      iterations,
      elapsedMs: Date.now() - started,
      heuristic: oks.every((r) => !!r.heuristic),
      reused: oks.some((r) => !!r.reused),
      decisionKey: oks[0].decisionKey ?? "",
      rootStats: merged,
      workers: oks.length,
      merged: multi,
    };
  }

  /** Speculative search on the seat's current view (BOT-RESEARCH #5). */
  async ponder(view: MatchView, budgetMs: number, seed: number): Promise<void> {
    if (!this.slots.length) return;
    try {
      await this.ensureLoaded();
    } catch {
      return;
    }
    const deadline = outerDeadlineMs(budgetMs);
    // One worker only: the cache lives inside each worker, and a decide that
    // fans out will hit whichever worker pondered only 1/N of the time. Fire
    // the ponder at every worker so any of them can answer from cache.
    await Promise.all(
      this.slots.map((s, i) =>
        s
          .call("ponder", { view, budgetMs, seed: seedForThread(seed, i) }, deadline)
          .catch(() => undefined),
      ),
    );
  }

  /**
   * Drop the cached answer for one decision key after the engine refused it
   * (`docs/BOT.md` §5 B6), so a refused answer is never replayed. Fires at
   * every worker (each keeps its own cache); best-effort.
   */
  async invalidate(decisionKey: string): Promise<void> {
    if (!decisionKey || !this.slots.length) return;
    await Promise.all(
      this.slots.map((s) => s.call("invalidate", { decisionKey }, 500).catch(() => undefined)),
    );
  }

  dispose(): void {
    this.disposed = true;
    for (const s of this.slots) s.dispose();
    this.slots.length = 0;
    this.loaded = null;
  }
}

interface DecideReply {
  ok?: boolean;
  answer: Command;
  iterations?: number;
  elapsedMs?: number;
  heuristic?: boolean;
  reused?: boolean;
  decisionKey?: string;
  rootStats?: RootStat[];
}

// ---------------------------------------------------------------- shared

let shared: BotPool | null = null;

/**
 * The page's pool, created (and loading) on first use. Pass `opts` only on the
 * first call -- later calls return the same instance.
 */
export function ensureBotPool(opts?: BotPoolOpts): BotPool {
  if (shared) return shared;
  shared = new BotPool(opts);
  return shared;
}

/** Drop the shared pool (tests / a full page reset). */
export function resetBotPool(): void {
  shared?.dispose();
  shared = null;
}

/**
 * Node-side loader used by the tests: import the built `bot-glue` module
 * directly (no Worker) so the API can be asserted against the real ruleset.
 * Mirrors `webui/src/game/ruleset.test.ts`'s `loadGlue`.
 */
export interface DirectBotGlue {
  load_data(files: string): void;
  data_files(): string;
  ruleset_add(bytes: Uint8Array): void;
  ruleset_precompiled(bytes: Uint8Array): void;
  ruleset_build(): number;
  decide(view: string, budgetMs: number, seed: number): string;
  ponder(view: string, budgetMs: number, seed: number): string;
  invalidate(decisionKey: string): boolean;
  info(): string;
  set_search(opts: string): void;
  reset_cache(): void;
  seed_for_thread(seed: number, thread: number): number;
  default(opts?: { module_or_path: Uint8Array }): Promise<unknown>;
}