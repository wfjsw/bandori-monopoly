// Replay playback: a read-only `GameSession` that drives the unchanged Board
// through the engine that **wrote** the record (`docs/REPLAY.md` §9). A record
// names its engine bundle; the loader plays it with the page's own wasm when
// that is the same build, and otherwise boots the archived bundle in a worker
// (`game/replayEngine.ts`). Stamp mismatch is no longer "warn and diverge" --
// it is "load the other bundle"; a bundle the archive does not hold is an
// error naming the id, never a silent re-simulation with a different engine.
//
// The record itself travels as raw `.bdrec` bytes here (the IndexedDB side is
// `game/record.ts`); the engine decodes any framing -- zstd today, gzip /
// plain JSON from older recordings.

import type { Command, MatchEvent } from "../core/types";
import type { Msg } from "../i18n/msg";
import { t as tr } from "../i18n/t";
import { GameSession } from "./session";
import { rules } from "../core/data";
import { type ReplayHandle } from "./replayEngine";
import { openRecordOnEngine } from "./portableEngine";
import type {
  EngineStamp,
  IndexStatus,
  Mismatch,
  RecordHeader,
  ReplayStatus,
  TurnMark,
} from "./record";

/** `TurnMark` with the u64 tick already converted to a number. */
export interface Turn {
  round: number;
  turn: number;
  tick: number;
}

/** One record handed to `/replay/view` (list row, file, or Results). */
let pending: { bytes: Uint8Array; id: string } | null = null;

/** Queue a `.bdrec`'s bytes for the player scene, then navigate there. */
export function setPendingReplay(bytes: Uint8Array, id = ""): void {
  pending = { bytes, id };
}

/** The queued record. Stays until [`clearPendingReplay`], so a re-render of
 *  the player scene can pick it up again. */
export function peekPendingReplay(): { bytes: Uint8Array; id: string } | null {
  return pending;
}

export function clearPendingReplay(): void {
  pending = null;
}

/** Queue `.bdrec` bytes (zstd, gzip or plain JSON) for the player. Throws when
 *  the bytes are not a record the engine can parse, so a caller can say "bad
 *  file" and stay put. */
export function queueReplayBytes(bytes: Uint8Array, id = ""): void {
  rules.record_header_bytes(bytes); // parse check, any framing
  setPendingReplay(bytes, id);
}

/**
 * The same table as `game_core::record::compat`, but over the current
 * [`engine_stamp`] and a record header only -- so the replay list can badge a
 * row without opening anything. A non-empty list means the record was written
 * by a different build; the player then loads that build's bundle instead of
 * forcing this one to approximate it. `fatal` still means "refuse outright"
 * (a newer format, or a different card ABI).
 */
export function stampMismatches(header: RecordHeader): Mismatch[] {
  let now: EngineStamp;
  try {
    now = JSON.parse(rules.engine_stamp()) as EngineStamp;
  } catch {
    return [];
  }
  const want = header.engine;
  const out: Mismatch[] = [];
  const num = (field: string, x: number, y: number, fatal: boolean) => {
    if (x && y && x !== y) out.push({ field, want: String(x), got: String(y), fatal });
  };
  const text = (field: string, x: string, y: string, fatal: boolean) => {
    if (x && y && x !== y) out.push({ field, want: x, got: y, fatal });
  };
  num("format", want.format, now.format, true);
  num("save_version", want.save_version, now.save_version, false);
  num("abi", want.abi, now.abi, true);
  text("ruleset_sha256", want.ruleset_sha256, now.ruleset_sha256, false);
  text("data_sha256", want.data_sha256, now.data_sha256, false);
  return out;
}

/**
 * A match played back from a record. Read-only (`readOnly` locks every input
 * surface the 托管 toggle would), `recorded` so it pays no profile rewards.
 * The engine lives behind {@link ReplayHandle}: the page's own wasm for a
 * record made by this build, or a worker running the archived bundle that
 * wrote it.
 */
export class ReplaySession extends GameSession {
  readonly kind = "replay" as const;
  readonly id: string;
  readOnly = true;
  recorded = true;
  /** 1 / 2 / 4 -- how many tick quanta per real 50 ms. */
  speed = 1;
  playing = true;
  /** Blow through stretches of ticks that produce no events. */
  skipIdle = false;
  /** Seat `member` id, or 0 for spectator (no hand, no per-viewer extras). */
  perspective = 0;
  /** Bumped on every seek, so the Board remounts with `key={epoch}`. */
  epoch = 0;
  header: RecordHeader;
  turns: Turn[] = [];
  totalTicks = 0;
  /** Where the first checkpoint mismatch landed; auto-pauses the transport. */
  divergence: { round: number; turn: number } | null = null;
  /** Stamp differences vs the **current** engine. Empty when the replay runs
   *  on the bundle that wrote it (the normal case); carried only when the
   *  record is being forced through a foreign engine. */
  stampMismatches: Mismatch[] = [];
  indexStatus: IndexStatus | null = null;
  /** Which engine is playing: the page's own wasm, or an archived bundle. */
  readonly engineKind: "page" | "worker";
  readonly engineBundle: string;
  /** How the engine was obtained: the page, the hosted archive, or the copy
   *  the record itself carries (`docs/REPLAY.md` §10). `"embedded"` is the
   *  only one the user is told about. */
  engineSource: "page" | "hosted" | "embedded" = "page";

  private h: ReplayHandle;
  /** Serializes engine calls (a worker answers one at a time anyway). */
  private q: Promise<unknown> = Promise.resolve();
  private lastStatus: ReplayStatus = { tick: "0", ended: false, diverged: false };
  private timer: number;
  private lastAt = performance.now();
  private acc = 0;
  private lastEventId = 0;
  private idleRun = 0;
  private closed = false;
  private busy = false;
  private indexing = false;

  private constructor(h: ReplayHandle, header: RecordHeader, turns: Turn[], totalTicks: number, id: string) {
    super();
    this.h = h;
    this.id = id || "replay";
    this.header = header;
    this.turns = turns;
    this.totalTicks = totalTicks;
    this.engineKind = h.kind;
    this.engineBundle = h.bundleId;
    this.perspective = (header.seats.find((x) => !x.bot) ?? header.seats[0])?.member ?? 0;
    this.you = this.perspective;
    this.timer = window.setInterval(() => void this.tick(), 50);
    this.runIndex();
    void this.pump();
  }

  /** Build the session on an opened handle (whatever engine runs it). */
  static async create(h: ReplayHandle, id = ""): Promise<ReplaySession> {
    const header = await h.header();
    const turns = (await h.turns()).map((t: TurnMark) => ({ round: t.round, turn: t.turn, tick: Number(t.tick) }));
    const totalTicks = await h.totalTicks();
    const s = new ReplaySession(h, header, turns, totalTicks, id);
    s.lastStatus = await h.status();
    return s;
  }

  /** One engine call, in order. */
  private run<T>(f: (h: ReplayHandle) => Promise<T>): Promise<T> {
    const p = this.q.then(() => (this.closed ? Promise.reject(new Error("replay closed")) : f(this.h)));
    this.q = p.catch(() => undefined);
    return p;
  }

  /** The `Status` without advancing (the scrub bar reads this). It is the
   *  last status the engine returned -- the session is the only writer. */
  status(): ReplayStatus {
    return this.lastStatus;
  }

  /** Current tick, as a number (the wire value is a u64 string). */
  now(): number {
    return Number(this.status().tick) || 0;
  }

  private async tick(): Promise<void> {
    if (this.closed || document.hidden || this.busy) {
      this.lastAt = performance.now();
      return;
    }
    const t = performance.now();
    const dt = Math.min(0.5, (t - this.lastAt) / 1000);
    this.lastAt = t;
    if (!this.playing) return;
    this.acc += dt;
    let k = Math.floor(this.acc / 0.05);
    if (k > 10) k = 10;
    if (k <= 0) return;
    this.acc -= k * 0.05;
    // `step` advances tick quanta and applies the non-tick inputs it meets for
    // free. Skip-idle multiplies the stride while nothing is happening, so a
    // bot's thinking time does not eat the wall clock.
    const idle = this.skipIdle && this.idleRun > 4;
    this.busy = true;
    try {
      const st = await this.run((h) => h.step(k * this.speed * (idle ? 10 : 1)));
      this.lastStatus = st;
      await this.pump();
      if (st.diverged && !this.divergence) {
        const v = this.view;
        this.divergence = { round: v?.state.round ?? 0, turn: v?.state.turn ?? 0 };
        this.playing = false;
        this.emitOther();
        return;
      }
      if (st.ended) {
        this.playing = false;
        this.emitOther();
      }
    } catch (e) {
      console.warn("replay step failed:", e);
      this.playing = false;
      this.emitOther();
    } finally {
      this.busy = false;
    }
  }

  private async pump(): Promise<void> {
    const evs = await this.run((h) => h.eventsSince(this.lastEventId));
    for (const e of evs as MatchEvent[]) {
      this.lastEventId = e.id;
      this.emitEvent(e);
    }
    // The engine produces nothing while a bot just thinks -- that is the idle
    // stretch skip-idle jumps over.
    if (evs.length || this.view?.state.busy) this.idleRun = 0;
    else this.idleRun++;
    const changed = await this.run((h) => h.takeChanged());
    if (changed || !this.view) {
      this.emitView(await this.run((h) => h.view(this.perspective)));
    }
  }

  /** Keyframes for `seek`, built in `setTimeout` chunks so the UI stays up. */
  private runIndex(): void {
    if (this.indexing) return;
    this.indexing = true;
    const step = () => {
      if (this.closed) return;
      void this.run((h) => h.index(250))
        .then((st) => {
          this.indexStatus = st;
          this.emitOther();
          if (!st.done && !this.closed) window.setTimeout(step, 16);
          else this.indexing = false;
        })
        .catch((e) => {
          console.warn("replay index failed:", e);
          this.indexing = false;
        });
    };
    window.setTimeout(step, 0);
  }

  setPlaying(on: boolean): void {
    if (this.playing === on) return;
    this.playing = on;
    this.lastAt = performance.now();
    this.emitOther();
  }

  /** Dismiss the divergence banner: keep playing (inaccurate) or stay paused. */
  ackDivergence(keepPlaying: boolean): void {
    this.divergence = null;
    this.playing = keepPlaying;
    this.lastAt = performance.now();
    this.emitOther();
  }

  setSpeed(v: number): void {
    const s = v >= 4 ? 4 : v >= 2 ? 2 : 1;
    if (this.speed === s) return;
    this.speed = s;
    this.animSpeed = s;
    this.emitOther();
  }

  setSkipIdle(on: boolean): void {
    if (this.skipIdle === on) return;
    this.skipIdle = on;
    this.emitOther();
  }

  /** 0 = spectator; otherwise a seat's `member` id. */
  setPerspective(member: number): void {
    if (this.closed || this.perspective === member) return;
    this.perspective = member;
    this.you = member;
    void this.run((h) => h.view(member))
      .then((v) => this.emitView(v))
      .catch((e) => console.warn("replay view failed:", e));
    this.emitOther();
  }

  /** Jump to `tick` (clamped). Bumps `epoch` so the Board remounts. */
  seek(tick: number): void {
    if (this.closed) return;
    void this.run(async (h) => {
      this.lastStatus = await h.seek(Math.max(0, Math.round(tick)));
      // The Board's event log is re-seeded from the state's tail on remount,
      // so the collected-stream cursor moves to that tail's last id.
      const v = await h.view(this.perspective);
      const tail = v.state.events ?? [];
      this.lastEventId = tail.length ? tail[tail.length - 1].id : 0;
      this.idleRun = 0;
      this.epoch++;
      this.emitView(v);
      this.emitOther();
    }).catch((e) => console.warn("replay seek failed:", e));
  }

  /** The mark at or after `tick` (next) / before it (previous). */
  private markAt(tick: number, dir: 1 | -1): number | null {
    const marks = this.turns;
    if (!marks.length) return null;
    if (dir > 0) {
      const next = marks.find((m) => m.tick > tick + 0.5);
      return next ? next.tick : null;
    }
    const prev = [...marks].reverse().find((m) => m.tick < tick - 0.5);
    // One step back from the very start of a turn lands on the turn before it;
    // from mid-turn it lands on the current turn's start.
    if (prev) return prev.tick;
    return marks[0].tick < tick ? marks[0].tick : null;
  }

  nextTurn(): void {
    const to = this.markAt(this.now(), 1);
    if (to != null) this.seek(to);
  }

  prevTurn(): void {
    const now = this.now();
    const marks = this.turns;
    const started = marks.find((m) => Math.abs(m.tick - now) <= 0.5);
    // Already sitting on a mark: go to the one before it, not to the same one.
    const to = started ? this.markAt(started.tick - 0.5, -1) : this.markAt(now, -1);
    if (to != null) this.seek(to);
    else this.seek(0);
  }

  /** No commands -- a replay is read-only. */
  act(_cmd: Command): Promise<Msg | null> {
    return Promise.resolve({ k: "err.replay" });
  }

  /** The 托管 toggle does not exist here; keep the method harmless. */
  setAutoMode(): void {
    /* read-only */
  }

  leave(): void {
    this.closed = true;
    clearInterval(this.timer);
    void this.h.free().catch(() => undefined);
  }
}

/** The playback session on screen, if any. Held here rather than freed on
 *  React unmount: StrictMode unmounts and remounts once in development, and a
 *  `free()` on that first unmount would kill the match under the second mount. */
let current: ReplaySession | null = null;

export function startReplay(rs: ReplaySession): ReplaySession {
  endReplay();
  current = rs;
  return current;
}

export function endReplay(): void {
  const old = current;
  current = null;
  // Drop the memoized open only when it produced this session -- a mid-boot
  // unmount (StrictMode, or the user backing out while a worker starts) must
  // not orphan the in-flight open and start a second engine on remount.
  if (old) opened.delete(pending as { bytes: Uint8Array; id: string });
  // Free on the next tick: the exiting board still reads `status()` / `view()`
  // during its last render, and a wasm trap there would take the app down.
  if (old) window.setTimeout(() => old.leave(), 0);
}

/** What [`openPending`] produced. */
export type Opened =
  | { phase: "error"; message: string }
  | { phase: "ready"; rs: ReplaySession };

/**
 * Open the queued record on the engine that wrote it. Memoized on the pending
 * entry (StrictMode double-invoked initializers share the one open), and
 * async because an archived bundle boots in a worker.
 */
export function openPending(): Promise<Opened> | null {
  const p = pending;
  if (!p) return null;
  let hit = opened.get(p);
  if (!hit) {
    hit = openRecord(p);
    opened.set(p, hit);
  }
  return hit;
}

const opened = new WeakMap<{ bytes: Uint8Array; id: string }, Promise<Opened>>();

/** Bundle resolution + engine boot + the fatal compat gate.
 *
 *  A portable record (`docs/REPLAY.md` §10) is routed by
 *  `openRecordOnEngine`: the hosted archive when it holds the same bytes,
 *  otherwise the engine the record itself carries. `force` is false: the
 *  engine is chosen to match the record, so a stamp difference here is a bug
 *  in the archive, not something to play through. */
async function openRecord(p: { bytes: Uint8Array; id: string }): Promise<Opened> {
  try {
    const opened = await openRecordOnEngine(p.bytes, false);
    const h = opened.handle;
    try {
      if (pending !== p) {
        await h.free();
        return { phase: "error", message: tr("replay.noReplay") };
      }
      const mis = await h.compat();
      const fatal = mis.filter((x) => x.fatal);
      if (fatal.length) {
        await h.free();
        return {
          phase: "error",
          message: `${tr("replay.compatFatalText")} (${fatal.map((x) => x.field).join(", ")})`,
        };
      }
      const rs = await ReplaySession.create(h, p.id);
      rs.stampMismatches = mis;
      rs.engineSource = opened.source;
      startReplay(rs);
      return { phase: "ready", rs };
    } catch (e) {
      await h.free().catch(() => undefined);
      throw e;
    }
  } catch (e) {
    return { phase: "error", message: e instanceof Error ? e.message : String(e) };
  }
}