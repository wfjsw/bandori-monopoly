// Replay playback: a read-only `GameSession` that drives the unchanged Board
// through `ReplayMatch`. The record itself travels as raw `.bdrec` bytes here
// (the IndexedDB side is `game/record.ts`); the engine decodes any framing --
// zstd today, gzip / plain JSON from older recordings. The player scene owns
// the compat dialog and the transport controls.

import { rules } from "../core/data";
import type { Command, MatchEvent, MatchView } from "../core/types";
import type { Msg } from "../i18n/msg";
import { t as tr } from "../i18n/t";
import { GameSession } from "./session";
import type {
  EngineStamp,
  IndexStatus,
  Mismatch,
  RecordHeader,
  ReplayStatus,
  TurnMark,
} from "./record";

type ReplayMatch = InstanceType<typeof rules.ReplayMatch>;

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

/** What would block a replay right now, from a `ReplayMatch` built with `force`. */
export function compatOf(m: ReplayMatch): Mismatch[] {
  try {
    const list: Mismatch[] = JSON.parse(m.compat());
    return Array.isArray(list) ? list : [];
  } catch {
    return [];
  }
}

/**
 * The same table as `game_core::record::compat` / `ReplayMatch::compat`, but
 * over the current [`engine_stamp`] and a record header only -- so the replay
 * list can badge a row without building a `ReplayMatch` for it. The player
 * still gates playback on the engine's own `compat()`.
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
 * Build a `ReplayMatch` from `.bdrec` bytes. `force` plays through a stamp
 * mismatch; the caller is expected to have shown [`compatOf`] first and to
 * refuse the `fatal` rows itself. The engine accepts zstd (current), gzip and
 * plain JSON here.
 */
export function openReplayMatch(bytes: Uint8Array, force: boolean): ReplayMatch {
  return rules.ReplayMatch.from_record_bytes(bytes, force);
}

/**
 * A match played back from a record. Read-only (`readOnly` locks every input
 * surface the 托管 toggle would), `recorded` so it pays no profile rewards.
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
  indexStatus: IndexStatus | null = null;

  private m: ReplayMatch;
  private timer: number;
  private lastAt = performance.now();
  private acc = 0;
  private lastEventId = 0;
  private idleRun = 0;
  private closed = false;
  private indexing = false;

  constructor(m: ReplayMatch, id = "") {
    super();
    this.m = m;
    this.id = id || "replay";
    this.header = JSON.parse(m.header());
    this.turns = (JSON.parse(m.turns()) as TurnMark[]).map((t) => ({ round: t.round, turn: t.turn, tick: Number(t.tick) }));
    this.totalTicks = m.total_ticks();
    this.perspective = (this.header.seats.find((x) => !x.bot) ?? this.header.seats[0])?.member ?? 0;
    this.you = this.perspective;
    this.timer = window.setInterval(() => this.tick(), 50);
    this.runIndex();
    this.pump();
  }

  /** The `Status` without advancing (the scrub bar reads this). */
  status(): ReplayStatus {
    if (this.closed) return { tick: "0", ended: true, diverged: this.divergence != null };
    return JSON.parse(this.m.status()) as ReplayStatus;
  }

  /** Current tick, as a number (the wire value is a u64 string). */
  now(): number {
    return Number(this.status().tick) || 0;
  }

  private tick(): void {
    if (this.closed || document.hidden) {
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
    const st = JSON.parse(this.m.step(k * this.speed * (idle ? 10 : 1))) as ReplayStatus;
    this.pump();
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
  }

  private pump(): void {
    const evs: MatchEvent[] = JSON.parse(this.m.events_since(this.lastEventId));
    for (const e of evs) {
      this.lastEventId = e.id;
      this.emitEvent(e);
    }
    // The engine produces nothing while a bot just thinks -- that is the idle
    // stretch skip-idle jumps over.
    if (evs.length || this.view?.state.busy) this.idleRun = 0;
    else this.idleRun++;
    if (this.m.take_changed() || !this.view) {
      this.emitView(JSON.parse(this.m.view(this.perspective)) as MatchView);
    }
  }

  /** Keyframes for `seek`, built in `setTimeout` chunks so the UI stays up. */
  private runIndex(): void {
    if (this.indexing) return;
    this.indexing = true;
    const step = () => {
      if (this.closed) return;
      try {
        this.indexStatus = JSON.parse(this.m.index(250)) as IndexStatus;
        this.emitOther();
        if (!this.indexStatus.done) window.setTimeout(step, 16);
        else this.indexing = false;
      } catch (e) {
        console.warn("replay index failed:", e);
        this.indexing = false;
      }
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
    this.emitView(JSON.parse(this.m.view(member)) as MatchView);
    this.emitOther();
  }

  /** Jump to `tick` (clamped). Bumps `epoch` so the Board remounts. */
  seek(tick: number): void {
    if (this.closed) return;
    try {
      JSON.parse(this.m.seek(Math.max(0, Math.round(tick))));
    } catch (e) {
      console.warn("replay seek failed:", e);
    }
    // The Board's event log is re-seeded from the state's tail on remount, so
    // the collected-stream cursor moves to that tail's last id.
    const v = JSON.parse(this.m.view(this.perspective)) as MatchView;
    const tail = v.state.events ?? [];
    this.lastEventId = tail.length ? tail[tail.length - 1].id : 0;
    this.idleRun = 0;
    this.epoch++;
    this.emitView(v);
    this.emitOther();
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
    try {
      this.m.free();
    } catch {
      /* already freed */
    }
  }
}

/** The playback session on screen, if any. Held here rather than freed on
 *  React unmount: StrictMode unmounts and remounts once in development, and a
 *  `free()` on that first unmount would kill the match under the second mount. */
let current: ReplaySession | null = null;

export function startReplay(m: ReplayMatch, id = ""): ReplaySession {
  endReplay();
  current = new ReplaySession(m, id);
  return current;
}

export function endReplay(): void {
  const old = current;
  current = null;
  if (pending) opened.delete(pending);
  // Free on the next tick: the exiting board still reads `status()` / `view()`
  // during its last render, and a wasm trap there would take the app down.
  if (old) window.setTimeout(() => old.leave(), 0);
}

/** What [`openPending`] produced. */
export type Opened =
  | { phase: "error"; message: string }
  /** Non-fatal stamp mismatch: the UI offers "continue anyway". */
  | { phase: "warn"; mismatches: Mismatch[]; m: ReplayMatch }
  | { phase: "ready"; rs: ReplaySession };

const opened = new WeakMap<{ bytes: Uint8Array; id: string }, Opened>();

/**
 * Build the replay for the queued record: `force` so `compat()` can name any
 * stamp differences, then refuse the `fatal` ones. Memoized on the pending
 * entry, so StrictMode's double-invoked initializer gets the same instance.
 */
export function openPending(): Opened | null {
  const p = pending;
  if (!p) return null;
  const hit = opened.get(p);
  if (hit) return hit;
  let out: Opened;
  try {
    const m = openReplayMatch(p.bytes, true);
    const mis = compatOf(m);
    const fatal = mis.filter((x) => x.fatal);
    if (fatal.length) {
      m.free();
      out = { phase: "error", message: `${tr("replay.compatFatalText")} (${fatal.map((x) => x.field).join(", ")})` };
    } else if (mis.length) {
      out = { phase: "warn", mismatches: mis, m };
    } else {
      out = { phase: "ready", rs: startReplay(m, p.id) };
    }
  } catch (e) {
    out = { phase: "error", message: e instanceof Error ? e.message : String(e) };
  }
  opened.set(p, out);
  return out;
}