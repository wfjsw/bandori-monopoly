// The page side of the solo engine worker (`soloWorker.ts` is the other end;
// `soloProtocol.ts` holds the shared contract). One module worker owns one
// `SoloMatch`: its 50 ms tick loop, the engine's own standard bots, and the
// save / record export. The page sends commands and receives only what the UI
// needs -- events, and a fresh view frame when the match (or that seat's frame)
// actually changed.
//
// Persistence: the worker cannot write `localStorage`, and `pagehide` cannot
// await it -- so the worker pushes a `SoloSave`-shaped snapshot (the engine
// strings) every second it is dirty and after every command, and the session
// keeps the newest one in memory to write synchronously when the page goes
// away.

import type { Command, MatchView } from "../core/types.ts";
import type { Msg } from "../i18n/msg.ts";
import type { EngineBoot, SaveSnap, SoloOpen, SoloPush, SoloRestore, SoloWorkerLike } from "./soloProtocol.ts";

/**
 * The page-side handle to one engine worker. Requests resolve in order; every
 * push lands through `onPush` before the request that produced it resolves
 * (the worker pumps -- and posts -- before it answers), so `session.view` is
 * already the post-command frame when `act` resolves.
 */
export class SoloEngine {
  /** Unsolicited frames from the worker. */
  onPush: (p: SoloPush) => void = () => {};
  private w: SoloWorkerLike;
  private seq = 0;
  private pending = new Map<number, { ok: (v: unknown) => void; err: (e: Error) => void }>();
  private closed = false;

  constructor(opts: { workerFactory?: () => SoloWorkerLike } = {}) {
    // The `new Worker(new URL(...))` shape is what the bundler recognizes and
    // turns into a worker chunk -- keep it literal (tests inject the factory).
    this.w = opts.workerFactory
      ? opts.workerFactory()
      : (new Worker(new URL("./soloWorker.ts", import.meta.url), {
          type: "module",
          name: "solo-engine",
        }) as unknown as SoloWorkerLike);
    this.w.onmessage = (e: { data: unknown }) => this.onMessage(e.data);
    this.w.onerror = (e: unknown) => {
      const err = new Error(`solo engine worker: ${String((e as { message?: string })?.message ?? e)}`);
      for (const p of this.pending.values()) p.err(err);
      this.pending.clear();
    };
  }

  private onMessage(data: unknown): void {
    const m = (data ?? {}) as Record<string, unknown>;
    if (typeof m.push === "string") {
      this.onPush(m as unknown as SoloPush);
      return;
    }
    const r = data as { id: number; ok: boolean; value?: unknown; error?: string };
    const p = this.pending.get(r.id);
    if (!p) return;
    this.pending.delete(r.id);
    if (r.ok) p.ok(r.value);
    else p.err(new Error(r.error ?? "solo engine error"));
  }

  private rpc<T>(op: string, rest: Record<string, unknown> = {}): Promise<T> {
    if (this.closed) return Promise.reject(new Error("solo engine closed"));
    const id = ++this.seq;
    return new Promise<T>((ok, err) => {
      this.pending.set(id, { ok: ok as (v: unknown) => void, err });
      this.w.postMessage({ id, op, ...rest });
    });
  }

  /** Load the glue, the game tables and the card ruleset in the worker. */
  boot(): Promise<EngineBoot> {
    return this.rpc<EngineBoot>("init");
  }

  start(spec: SoloOpen): Promise<true> {
    return this.rpc<true>("start", { ...spec });
  }

  restore(spec: SoloRestore): Promise<true> {
    return this.rpc<true>("restore", { ...spec });
  }

  /** A command as `member`. Resolves to the engine's `Msg`, or null on success. */
  act(member: number, cmd: Command): Promise<Msg | null> {
    return this.rpc<Msg | null>("act", { member, cmd });
  }

  /** Skip ban / pick / deck with random characters and preset decks. */
  quickStart(): Promise<true> {
    return this.rpc<true>("quick_start");
  }

  /** One seat's frame (the driver's view cache is fed from pushes; this is the
   *  explicit read for tests and for a seat that has not been pushed yet). */
  view(member: number): Promise<MatchView> {
    return this.rpc<MatchView>("view", { member });
  }

  /** A fresh engine snapshot right now (pagehide / tab-hide). */
  save(): Promise<SaveSnap> {
    return this.rpc<SaveSnap>("save");
  }

  /** Seal the `.bdrec` (`SoloMatch::record_zst`); the bytes transfer back. */
  export(created: string): Promise<Uint8Array> {
    return this.rpc<Uint8Array>("export", { created }).then((b) =>
      b instanceof Uint8Array ? b : new Uint8Array(b as ArrayLike<number>),
    );
  }

  /** Forward `document.visibilityState` so the tick loop keeps the page's
   *  pacing while the tab is hidden (see the worker's `setInterval`). */
  setVisibility(hidden: boolean): void {
    if (this.closed) return;
    void this.rpc<true>("visibility", { hidden }).catch(() => undefined);
  }

  /** Stop the worker. The match goes with it -- `leave` tears a match down. */
  close(): void {
    if (this.closed) return;
    this.closed = true;
    for (const p of this.pending.values()) p.err(new Error("solo engine closed"));
    this.pending.clear();
    this.w.terminate();
  }
}