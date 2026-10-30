// One interface for a match whether it runs on the server (online) or in the
// browser's wasm engine (solo). Scenes only talk to `session`.
//
// Refresh recovery: a solo match is saved (engine snapshot, `soloSave.ts`) and
// resumed on load; an online player is re-attached through the server session
// (the token lives in sessionStorage) and `GET /api/rooms/{id}/state`.

import { D, currentSources, customRules, customRulesActive, customRulesBytes, rules, subscribeData, switchSource } from "../core/data";
import type { BotMentality, Command, MatchEvent, MatchView, RoomInfo, RoomMember, ScoreWeights } from "../core/types";
import { api, ensureSession, openStream } from "../net/api";
import { absolute, buildId } from "../net/server";
import { startClockSync, clockSync, resyncLimiter } from "./clockSync";
import type { Msg } from "../i18n/msg";
import { isAuto, plan, type AutoMode, type AutopilotCtx, type PolicyName } from "./autopilot";
import { SOLO_CAP_MS } from "./botBudget";
import { botSeats, driveSeat, decisionAt, ponderUpcoming, type DriveHooks } from "./botDrive";
import { emitOnlineView, OnlineExtras } from "./viewExtras";
import { putReplay, recordFilename, type RecordHeader } from "./record";
import { SoloEngine } from "./soloEngine";
import type { SaveSnap, SoloBootOpts, SoloPush } from "./soloProtocol";
import { clearSoloSave, hasSoloSave, loadSoloSave, saveSoloSave } from "./soloSave.ts";
import { kv } from "../core/kv.ts";

const prefs = kv("prefs");

type ViewCb = (v: MatchView) => void;
type EventCb = (e: MatchEvent) => void;

/**
 * 托管 / 混沌 driver: on every view change, wait a human beat (0.4–1.0 s, like
 * `tick_live`), then send one candidate through the ordinary `act` path. The
 * policy (`bot` / `chaos`) is pluggable; the driver is shared.
 *
 * Never two commands in flight. Each distinct command is sent at most once per
 * `state.seq`, and a refused one is marked tried so the next candidate is
 * attempted instead of looping.
 */
/** After 托管 / 混沌 is switched on (or switched between the two), the
 *  autopilot waits this long before its first command, so a misclick can be
 *  undone -- or the mode cycled on -- before anything is sent. */
export const AUTO_COOLDOWN_MS = 3000;

/** The autopilot re-checks the view this often when nothing is scheduled, so a
 *  missed wake-up (a view that arrived while a command was in flight, a
 *  prompt that appeared without a seq change) cannot leave it idle. */
const AUTO_HEARTBEAT_MS = 1500;

class Autopilot {
  private timer = 0;
  private heartbeat = 0;
  /** `performance.now()` before which no command is sent (the cooldown). */
  armedAt = 0;
  private inFlight = false;
  private seq = -1;
  private tried = new Set<string>();
  private played = 0;
  private turnKey = "";
  private off: (() => void) | null = null;
  /** Set while the 进阶 search is running for this seat (the thinking pill). */
  private searching = false;

  constructor(private sess: GameSession) {}

  start(): void {
    this.rearm();
    if (!this.heartbeat) {
      this.heartbeat = window.setInterval(() => {
        if (!this.timer && !this.inFlight && isAuto(this.sess.autoMode)) this.schedule();
      }, AUTO_HEARTBEAT_MS);
    }
    if (this.off) return;
    this.off = this.sess.subscribe((v) => this.onView(v));
    if (this.sess.view) this.onView(this.sess.view);
  }

  stop(): void {
    clearTimeout(this.timer);
    this.timer = 0;
    clearInterval(this.heartbeat);
    this.heartbeat = 0;
    this.inFlight = false;
    this.seq = -1;
    this.tried.clear();
    this.setSearching(false);
    this.off?.();
    this.off = null;
  }

  /** Restart the cooldown (a mode switch while running counts too). */
  rearm(): void {
    this.armedAt = performance.now() + AUTO_COOLDOWN_MS;
    if (this.off) this.schedule();
  }

  private delay(): number {
    // The 进阶 search needs no human beat -- it is the one that "thinks".
    if (this.sess.autoMode === "advanced") {
      return Math.max(0, this.armedAt - performance.now());
    }
    return Math.max(400 + Math.random() * 600, this.armedAt - performance.now());
  }

  private schedule(): void {
    clearTimeout(this.timer);
    this.timer = window.setTimeout(() => this.step(), this.delay());
  }

  private onView(v: MatchView): void {
    if (!isAuto(this.sess.autoMode)) return;
    const key = `${v.state.round}:${v.state.turn}`;
    if (key !== this.turnKey) {
      this.turnKey = key;
      this.played = 0;
    }
    if (v.state.seq * 100000 + Math.max(0, v.state.prompt?.id ?? 0) === this.seq && this.tried.size > 0) return;
    this.schedule();
  }

  private ctx(): AutopilotCtx {
    return {
      tiles: D.tiles,
      personas: D.personas,
      playedThisTurn: this.played,
      deckPreset: (c) => JSON.parse(rules.deck_preset(c)),
      // Deck book entry for the public table, else the preset
      // (`docs/BOT.md` §3.7); the lookup itself lives in game-core.
      deckSuggest: (c, seat, opponents) =>
        JSON.parse(rules.deck_suggest(c, seat, JSON.stringify(opponents))),
      // Strategy book entry for the public table, else the defaults (today's
      // constants) (`docs/BOT.md` §3.8) -- one source of truth with the engine.
      strategyFor: (c, seat, opponents) =>
        JSON.parse(rules.strategy_for(c, seat, JSON.stringify(opponents))),
      deckRandom: (c) => {
        const pool: string[] = JSON.parse(rules.deck_pool(c));
        for (let i = pool.length - 1; i > 0; i--) {
          const j = Math.floor(Math.random() * (i + 1));
          [pool[i], pool[j]] = [pool[j], pool[i]];
        }
        return JSON.parse(rules.deck_clean(c, JSON.stringify(pool)));
      },
      random: Math.random,
    };
  }

  private static key(cmd: Command): string {
    return JSON.stringify(cmd);
  }

  private setSearching(on: boolean): void {
    if (this.searching === on) return;
    this.searching = on;
    this.sess.setBotThinking(this.sess.you, on);
  }

  private step(): void {
    this.timer = 0;
    const s = this.sess;
    const mode = s.autoMode;
    if (!isAuto(mode) || this.inFlight) return;
    if (performance.now() < this.armedAt) return this.schedule();
    const v = s.view;
    if (!v) return;
    // A new state, or a new prompt on the same state, resets what was tried.
    const at = v.state.seq * 100000 + Math.max(0, v.state.prompt?.id ?? 0);
    if (at !== this.seq) {
      this.seq = at;
      this.tried.clear();
    }
    void this.stepDrive(v, at, mode);
  }

  /**
   * 托管, all three modes (`docs/BOT-DRIVE.md`): every decision comes from the
   * worker's policy -- `act` for 托管 / 混沌 (standard and chaos mentality
   * over the same code the server bot runs), `decide` for 进阶 (`docs/BOT.md`
   * B6 search). Setup / trivial surfaces still go through `plan`. On any
   * worker failure the policy table answers instead -- a match never stalls
   * (§1).
   */
  private stepDrive(v: MatchView, at: number, mode: PolicyName): void {
    const s = this.sess;
    // Setup / discard / vote: the search does not cover them (and the server
    // keeps a bot's setup engine-side). Same policy as the mode's table.
    if (decisionAt(v.state, v.playerId) == null) {
      const cmd = plan(v, this.ctx(), mode).find((c) => !this.tried.has(Autopilot.key(c)));
      if (!cmd) return;
      this.tried.add(Autopilot.key(cmd));
      this.inFlight = true;
      Promise.resolve(s.act(cmd))
        .then((err) => {
          if (err) {
            console.warn("advanced: command refused", cmd, err);
            this.schedule();
          } else if (cmd.act === "play") this.played++;
        })
        .finally(() => {
          this.inFlight = false;
          const now = s.view;
          if (isAuto(s.autoMode) && now && now.state.seq * 100000 + Math.max(0, now.state.prompt?.id ?? 0) !== at) this.schedule();
        });
      return;
    }
    // A searched surface: one ask per state, through the pool.
    const key = "search";
    if (this.tried.has(key)) return;
    this.tried.add(key);
    this.inFlight = true;
    const hooks: DriveHooks = {
      ctx: this.ctx(),
      room: s.id,
      timed: s.kind !== "solo",
      soloCapMs: botSoloCapMs(),
      decisionsThisTurn: this.played,
      search: mode === "advanced",
      mentality: mode === "chaos" ? "chaos" : mode === "advanced" ? "advanced" : "standard",
      onThinking: (_m, on) => this.setSearching(on),
    };
    void driveSeat(
      (_member, cmd) => {
        if (cmd.act === "play") this.played++;
        return s.act(cmd);
      },
      () => s.view,
      s.you,
      hooks,
    )
      .catch((e) => console.warn(`${mode}: drive failed`, e))
      .finally(() => {
        this.inFlight = false;
        this.setSearching(false);
        const now = s.view;
        if (isAuto(s.autoMode) && now && now.state.seq * 100000 + Math.max(0, now.state.prompt?.id ?? 0) !== at) this.schedule();
      });
  }
}

/** Solo's per-decision search cap (`docs/BOT.md` §3.6, user ruling). Kept in
 *  the settings store so it survives a refresh; the solo setup screen edits it. */
const SOLO_BOT_CAP_KEY = "botSoloCapMs";

export function botSoloCapMs(): number {
  const raw = Number(prefs.get(SOLO_BOT_CAP_KEY));
  return Number.isFinite(raw) && raw >= 200 ? Math.min(12_000, raw) : SOLO_CAP_MS;
}

export function setBotSoloCapMs(ms: number): void {
  prefs.set(SOLO_BOT_CAP_KEY, String(Math.round(ms)));
}

export abstract class GameSession {
  abstract readonly kind: "solo" | "online" | "replay";
  /** Route id: "solo" or the room id. */
  abstract readonly id: string;
  /** The rules / tables source this session's engine or stream was booted
   *  from. A live switch to another source ends the session (below). */
  readonly bootSources = currentSources();
  view: MatchView | null = null;
  room: RoomInfo | null = null;
  you = 0;
  connected = true;
  /** Set when the match result has been recorded on the profile. */
  recorded = false;
  /** Input is locked regardless of `autoMode` (a replay is read-only). */
  readOnly = false;
  /** Animation speed the board's `Animator` follows (1 / 2 / 4). */
  animSpeed = 1;
  /** Only local presentation changes; the match and its answer clocks do not. */
  setAnimSpeed(v: number): void {
    const speed = v >= 4 ? 4 : v >= 2 ? 2 : 1;
    if (this.animSpeed === speed) return;
    this.animSpeed = speed;
    this.emitOther();
  }
  /**
   * Auto-play mode for this seat: `off` (you), `bot` (托管), `chaos` (混沌),
   * `advanced` (进阶 -- the worker-pool search, `docs/BOT.md` B6).
   * Per-session, not persisted. Any non-`off` value locks the user's inputs.
   */
  autoMode: AutoMode = "off";
  /** Seats whose 进阶 search is running right now (the "thinking…" pill). */
  private thinking = new Set<number>();
  private thinkCbs = new Set<() => void>();
  private autoCbs = new Set<() => void>();
  private autopilot: Autopilot | null = null;
  protected views = new Set<ViewCb>();
  protected events = new Set<EventCb>();
  protected others = new Set<() => void>();

  /** Subscribe to state, events and room/connection changes. Returns unsubscribe. */
  subscribe(onView: ViewCb, onEvent?: EventCb, onOther?: () => void): () => void {
    this.views.add(onView);
    if (onEvent) this.events.add(onEvent);
    if (onOther) this.others.add(onOther);
    if (this.view) onView(this.view);
    return () => {
      this.views.delete(onView);
      if (onEvent) this.events.delete(onEvent);
      if (onOther) this.others.delete(onOther);
    };
  }

  /** Set 托管 / 混沌 / 进阶 / off. Off takes effect immediately (mid-command
   *  included); turning a mode on, or switching modes, starts the
   *  [`AUTO_COOLDOWN_MS`] cooldown before the first command. */
  setAutoMode(mode: AutoMode): void {
    if (this.autoMode === mode) return;
    this.autoMode = mode;
    if (isAuto(mode)) {
      this.autopilot ??= new Autopilot(this);
      this.autopilot.start();
    } else {
      this.autopilot?.stop();
    }
    this.autoCbs.forEach((cb) => cb());
  }

  /** Milliseconds until the autopilot may act (0 when off or already live). */
  autoCooldownLeft(): number {
    if (!isAuto(this.autoMode) || !this.autopilot) return 0;
    return Math.max(0, this.autopilot.armedAt - performance.now());
  }

  /** Re-render when the auto mode flips. Returns unsubscribe. */
  subscribeAutoMode(cb: () => void): () => void {
    this.autoCbs.add(cb);
    return () => this.autoCbs.delete(cb);
  }

  /** Is the 进阶 search running for `member`? (the "thinking…" indicator). */
  isThinking(member: number): boolean {
    return this.thinking.has(member);
  }

  /** Re-render when a search starts / stops. Returns unsubscribe. */
  subscribeThinking(cb: () => void): () => void {
    this.thinkCbs.add(cb);
    return () => this.thinkCbs.delete(cb);
  }

  /** Mark `member` as searching / done (called by the drivers). */
  setBotThinking(member: number, on: boolean): void {
    const had = this.thinking.has(member);
    if (on) this.thinking.add(member);
    else this.thinking.delete(member);
    if (had !== on) this.thinkCbs.forEach((cb) => cb());
  }

  protected emitView(v: MatchView): void {
    this.view = v;
    this.views.forEach((cb) => cb(v));
  }

  protected emitEvent(e: MatchEvent): void {
    this.events.forEach((cb) => cb(e));
  }

  protected emitOther(): void {
    this.others.forEach((cb) => cb());
  }

  /** Send a command. Resolves to the error message, or null on success. */
  abstract act(cmd: Command): Promise<Msg | null>;
  /**
   * The Animator queue has drained and the view is current for `seq`.
   * Online sessions release the server's turn-clock hold; solo / replay
   * have no hold and ignore this.
   */
  animDone(_seq: number): Promise<void> {
    return Promise.resolve();
  }
  /**
   * This seat has the rolled turn order on screen. Online matches end the
   * order phase on every seat's ack; solo goes straight from the roll to the
   * deck and replay has no session to tell.
   */
  orderReady(): Promise<void> {
    return Promise.resolve();
  }
  abstract leave(): void;
}

export let session: GameSession | null = null;

// Dev-only seam for browser checks (mirrors `__bmDebug` in `core/data.ts`).
if (import.meta.env?.DEV) {
  (globalThis as { __bmSession?: unknown }).__bmSession = () => session;
}

export function endSession(): void {
  session?.setAutoMode("off");
  session?.leave();
  session = null;
}

// ------------------------------------------------------------------ solo

/** An engine worker booted ahead of a solo start, if any. */
let warmEngine: SoloEngine | null = null;
/** The source the warm worker was booted from (server / build / custom). */
let warmKey = "";
/** Bumped when a source switch replaces the page's data, so a warm worker
 *  keyed on identical-looking settings (same name and byte length) is still
 *  not reused for the new bytes. */
let sourceEpoch = 0;

/**
 * Where a solo worker boots from: the selected game server / build (absolute
 * URLs -- the worker has no page-relative base) and, only when that is the
 * local bundled path, the user's custom rules module. The page's `D` and the
 * worker therefore come from the same source.
 */
function soloBootOpts(): SoloBootOpts {
  const from = currentSources();
  return {
    rulesBase: absolute(from.rules),
    dataBase: absolute(from.data),
    custom: customRulesActive() ? customRulesBytes() : null,
  };
}

/** A cheap identity for `soloBootOpts()`, so a worker booted for one source is
 *  never reused for another. The epoch covers a switch that re-uses the same
 *  custom name and byte length with different bytes. */
function soloSourceKey(): string {
  const opts = soloBootOpts();
  const c = customRulesActive() ? customRules() : null;
  return `${sourceEpoch}\u0000${opts.rulesBase}\u0000${opts.dataBase}\u0000${c ? `${c.name}:${c.bytes.length}` : "-"}`;
}

/** Drop the warmed worker after a live source switch and warm the new source.
 *  `takeSoloEngine` would reject it anyway, but this frees the old worker now
 *  instead of at the next start. */
export function invalidateSoloEngine(): void {
  sourceEpoch++;
  warmSoloEngine();
}

/**
 * Boot a solo engine worker in the background (glue, data, ruleset) so the
 * next `startSolo` / `resumeSolo` skips that cost. Idempotent for one source;
 * a worker booted for a different server / build / custom rules is discarded
 * and replaced. The setup screen calls it when it opens. A failed warm-up is
 * dropped and the start falls back to a cold worker (and reports its own
 * error).
 */
export function warmSoloEngine(): void {
  const key = soloSourceKey();
  if (warmEngine && warmKey === key) return;
  if (warmEngine) {
    warmEngine.close();
    warmEngine = null;
  }
  const e = new SoloEngine();
  warmEngine = e;
  warmKey = key;
  e.boot(soloBootOpts()).catch(() => {
    if (warmEngine === e) {
      warmEngine = null;
      warmKey = "";
    }
    e.close();
  });
}

/** The warmed worker (handed over once) when it matches the current source,
 *  else a fresh one. */
function takeSoloEngine(): SoloEngine {
  const key = soloSourceKey();
  let e = warmEngine;
  const warmedFor = warmKey;
  warmEngine = null;
  warmKey = "";
  if (e && warmedFor !== key) {
    e.close();
    e = null;
  }
  return e ?? new SoloEngine();
}

/**
 * One solo match. The `SoloMatch` (wasm), its 50 ms fixed-step tick loop and
 * the engine's standard bots run in a module worker (`soloWorker.ts`); this
 * class is the page-side half: it caches the frames the worker pushes, drives
 * the 进阶 seats through the bot pool, and keeps a save fresh (`soloSave.ts`).
 *
 * `ready` resolves once the worker has booted (glue + data + ruleset) and
 * opened the match. Until then `view` is null and the UI waits -- the same
 * shape as `OnlineSession`, whose first frame arrives over the stream.
 */
export class SoloSession extends GameSession {
  readonly kind = "solo";
  readonly id = "solo";
  readonly weights: ScoreWeights;
  /** Resolves when the engine worker is running the match; rejects when it
   *  could not (the save is dropped, the UI backs out to the menu). */
  readonly ready: Promise<void>;
  /** The `.bdrec` exported when the match ended (zstd-framed), kept for Results. */
  replayBytes: Uint8Array | null = null;
  replayId: string | null = null;
  replayName = "bdrec-replay.bdrec";
  private engine: SoloEngine;
  /** Newest engine snapshot the worker pushed. `pagehide` writes this as-is --
   *  the page cannot await a round-trip as it goes away (`docs` note in
   *  `soloProtocol.ts`). */
  private snap: SaveSnap = { match: "", last: 0 };
  private closed = false;
  private exportStarted = false;
  /** Seats the 进阶 driver is currently searching for (one ask at a time). */
  private botInFlight = new Set<number>();
  /** Per-seat decisions already spent this turn (the budget spread). */
  private botTurnKey = "";
  private botPlayed = new Map<number, number>();
  /** Speculative `ponder` bookkeeping: one in flight per seat, rate-limited
   *  (`docs/BOT.md` §3.5 -- the same shape as the server's `spawn_ponder`). */
  private botPonderInFlight = new Set<number>();
  private botPonderAt = new Map<number, number>();

  /** A bot seat's mentality, read off its own frame (the human frame carries
   *  every seat, but the seat frame is authoritative for its mentality). */
  private botMentality(member: number): "standard" | "chaos" | "advanced" {
    const v = this.botViews.get(member) ?? this.view;
    const m = v?.state.players[v.playerId]?.mentality;
    return m === "chaos" || m === "advanced" ? m : "standard";
  }
  /** The 进阶 seats' frames the worker pushes (the driver's views). */
  private botViews = new Map<number, MatchView>();
  /** Re-checks the speculative ponder surface while idle (the worker only
   *  pushes on change; a ponder is worth firing without a state change). */
  private ponderTimer: number;
  /** Write the cached snapshot now (pagehide cannot await), ask the worker for
   *  a fresher one while the page can still hear the answer (tab switch), and
   *  forward visibility so the tick loop keeps the page's pacing while hidden. */
  private onHide = () => {
    this.engine.setVisibility(document.hidden);
    this.persist();
    void this.engine
      .save()
      .then((s) => {
        this.snap = s;
        this.persist();
      })
      .catch(() => undefined);
  };

  private constructor(engine: SoloEngine, weights: ScoreWeights, open: () => Promise<void>, replayId: string | null = null) {
    super();
    this.you = 1;
    this.engine = engine;
    this.weights = weights;
    // The store id this match's record goes under (kept from the save, so a
    // re-export on resume overwrites the same row and the history row stays
    // linked to it).
    this.replayId = replayId;
    engine.onPush = (p) => this.onPush(p);
    this.ready = open();
    // Mark the rejection handled here too -- the setup path never awaits
    // `ready`; Play's effect is what backs out of a failed open.
    void this.ready.catch((e) => {
      console.warn("solo engine could not open the match:", e);
      if (this.closed) return;
      clearSoloSave();
    });
    this.ponderTimer = window.setInterval(() => {
      if (!this.closed) this.driveBots();
    }, 350);
    window.addEventListener("pagehide", this.onHide);
    document.addEventListener("visibilitychange", this.onHide);
  }

  static start(player: string, playerCharacter: string, bots: SoloBot[], weights: ScoreWeights): SoloSession {
    // The solo setup screen's picks, its `""` (random) seats resolved here
    // among the free characters -- everything not explicitly picked. The
    // engine sees a character on every seat and starts past ban / pick.
    const chars = resolveSoloCharacters([playerCharacter, ...bots.map((b) => b.persona)]);
    const seat = (id: number, name: string, persona: string, bot: boolean, mentality: BotMentality): RoomMember => {
      const c = D.persona(persona);
      return {
        id,
        player: name,
        persona,
        cnId: c ? D.artId(c) : "",
        ready: true,
        host: id === 1,
        bot,
        away: false,
        mentality,
      };
    };
    const members: RoomMember[] = [
      seat(1, player, chars[0], false, "standard"),
      ...bots.map((b, i) => seat(i + 2, b.name, chars[i + 1], true, b.mentality)),
    ];
    // Commit-reveal runs inside the solo engine worker (`soloWorker.ts`,
    // `docs/FAIRNESS.md` "solo"): the same recipe an online match runs, so the
    // exported record carries the openings and verifies. Solo proves little --
    // the player is both committer and contributor -- so the UI does not show
    // the commitment.
    const engine = takeSoloEngine();
    return new SoloSession(engine, weights, async () => {
      await engine.boot(soloBootOpts());
      await engine.start({ members, mode: 0, weights, you: 1 });
      engine.setVisibility(document.hidden);
    });
  }

  /** The match saved before a refresh, if any. The save's engine strings go to
   *  the worker; a restore failure (a corrupt save) rejects `ready` and Play
   *  drops the session. */
  static async resume(): Promise<SoloSession | null> {
    const s = await loadSoloSave();
    if (!s) return null;
    const engine = takeSoloEngine();
    const out = new SoloSession(
      engine,
      s.weights,
      async () => {
        await engine.boot(soloBootOpts());
        await engine.restore({ save: s.match, rec: s.rec, last: s.last, you: 1 });
        engine.setVisibility(document.hidden);
      },
      s.replayId ?? null,
    );
    out.recorded = s.recorded;
    return out;
  }

  static hasSave(): boolean {
    return hasSoloSave();
  }

  /** Worker pushes: events + frames (the UI's view), engine snapshots (the
   *  save), and the end-of-match trigger for the `.bdrec` export. Order is
   *  the worker's post order, so a view is already fresh when `act` resolves. */
  private onPush(p: SoloPush): void {
    if (this.closed) return;
    if (p.push === "save") {
      this.snap = { match: p.match, rec: p.rec, last: p.last };
      this.persist();
      return;
    }
    for (const e of p.events) this.emitEvent(e);
    if (p.view) this.emitView(p.view);
    for (const b of p.bots ?? []) this.botViews.set(b.member, b.view);
    if (p.ended) this.exportReplay();
    else this.driveBots();
  }

  /**
   * Keep every bot seat moving (`docs/BOT-DRIVE.md`). The engine holds their
   * `ai` off, so this driver is the one applying answers through the normal
   * `act` path -- standard / chaos answer with the worker's policy alone,
   * `advanced` searches (`docs/BOT.md` B6). Setup (ban / pick / deck) stays
   * engine-side (`MatchPlayer::auto_setup`). Fallback to the policy table on
   * any worker failure so a match never stalls (§1).
   *
   * The seats' frames come from the worker's pushes (they change together with
   * the state); the driver runs on every sync and on a light idle timer, which
   * is what keeps the speculative `ponder` (`docs/BOT.md` §3.5) firing while
   * the match is quiet.
   */
  private driveBots(): void {
    const v0 = this.view;
    // Setup has no move surface, but it does carry prompts (初始重抽): a bot
    // answers those through the same channel, so only a setup frame with no
    // open prompt is idle.
    if (!v0 || (v0.state.phase !== "play" && !v0.state.prompt?.id)) return;
    // One turn key per (round, turn) for the budget spread.
    const turnKey = `${v0.state.round}:${v0.state.turn}`;
    if (turnKey !== this.botTurnKey) {
      this.botTurnKey = turnKey;
      this.botPlayed.clear();
    }
    for (const member of botSeats(v0)) {
      if (this.botInFlight.has(member)) continue;
      const v = this.botViews.get(member);
      if (!v) continue;
      const mentality = this.botMentality(member);
      if (decisionAt(v.state, v.playerId) == null) {
        // Nothing to answer right now. Speculate on the seat's **next own
        // decision** when it is near (BOT-RESEARCH #5, `docs/BOT.md` §3.5) --
        // rate-limited like the server's idle probe, one in flight per seat.
        // An idle view has no searchable surface; `ponderUpcoming` gates on
        // `nextTurnNear` and the worker predicts the turn-start 运营 view.
        // Only a searching (进阶) seat speculates -- the policy alone has no
        // tree to fill.
        const now = performance.now();
        if (
          mentality === "advanced" &&
          !this.botPonderInFlight.has(member) &&
          now >= (this.botPonderAt.get(member) ?? 0)
        ) {
          this.botPonderInFlight.add(member);
          this.botPonderAt.set(member, now + 200);
          ponderUpcoming(
            (m) => this.botViews.get(m) ?? null,
            member,
            {
              room: this.id,
              timed: false,
              soloCapMs: botSoloCapMs(),
            },
          );
          // `ponderUpcoming` is fire-and-forget; release the slot after the
          // worker's own outer deadline so the next probe can speculate again.
          const release = window.setTimeout(() => {
            this.botPonderInFlight.delete(member);
          }, 3_500);
          void release;
        }
        continue;
      }
      // A live decision supersedes any speculative search in flight.
      this.botPonderInFlight.delete(member);
      this.botInFlight.add(member);
      const played = this.botPlayed.get(member) ?? 0;
      void driveSeat(
        (m, cmd) => this.actAs(m, cmd),
        (m) => this.botViews.get(m) ?? null,
        member,
        {
          ctx: this.autopilotCtx(),
          room: this.id,
          // Solo prompts have no deadline (`docs/BOT.md` §3.6 user ruling):
          // a generous configurable cap instead of the room clock.
          timed: false,
          soloCapMs: botSoloCapMs(),
          decisionsThisTurn: played,
          search: mentality === "advanced",
          mentality,
          onThinking: (m, on) => this.setBotThinking(m, on),
        },
      ).finally(() => {
        this.botInFlight.delete(member);
        // A search can span several ticks; catch up on the next decision.
        if (!this.closed) this.driveBots();
      });
    }
  }

  /** `act` as any member (the 进阶 driver's seat, not just the human's). */
  private actAs(member: number, cmd: Command): Promise<Msg | null> {
    return this.engine.act(member, cmd).then((err) => {
      if (!err && cmd.act === "play") this.botPlayed.set(member, (this.botPlayed.get(member) ?? 0) + 1);
      return err;
    });
  }

  /** The shared fallback-policy inputs (the same context `Autopilot` builds). */
  private autopilotCtx(): AutopilotCtx {
    return {
      tiles: D.tiles,
      personas: D.personas,
      playedThisTurn: 0,
      deckPreset: (c) => JSON.parse(rules.deck_preset(c)),
      deckSuggest: (c, seat, opponents) =>
        JSON.parse(rules.deck_suggest(c, seat, JSON.stringify(opponents))),
      // Strategy book entry for the public table, else the defaults (today's
      // constants) (`docs/BOT.md` §3.8) -- one source of truth with the engine.
      strategyFor: (c, seat, opponents) =>
        JSON.parse(rules.strategy_for(c, seat, JSON.stringify(opponents))),
      deckRandom: (c) => {
        const pool: string[] = JSON.parse(rules.deck_pool(c));
        for (let i = pool.length - 1; i > 0; i--) {
          const j = Math.floor(Math.random() * (i + 1));
          [pool[i], pool[j]] = [pool[j], pool[i]];
        }
        return JSON.parse(rules.deck_clean(c, JSON.stringify(pool)));
      },
      random: Math.random,
    };
  }

  /** Hand the cached engine snapshot to the save store (the envelope fields --
   *  weights / recorded / replayId -- are the page's). Called on the worker's
   *  save pushes (at most one a second while dirty) and on the way out
   *  (`pagehide`); see `soloProtocol.ts` for why the snapshot is cached. */
  private persist(): void {
    if (this.closed || !this.snap.match) return;
    saveSoloSave({
      v: 1,
      match: this.snap.match,
      weights: this.weights,
      recorded: this.recorded,
      // The cursor that belongs with these engine strings -- not a later one,
      // or a resume would skip events the snapshot does not have.
      last: this.snap.last,
      rec: this.snap.rec,
      replayId: this.replayId ?? undefined,
    });
  }

  /** Export the `.bdrec` once, when the match ends: zstd-frame it into
   *  IndexedDB (keep 10) and hold the bytes for the Results buttons. The store
   *  id is allocated **synchronously** and kept across a resume, so
   *  `showResults` can write it into the history row the moment the match ends
   *  (`applyMatch`), before the async export lands; a re-export (a refresh
   *  before the first one finished) then overwrites the same id. The
   *  compression runs in the worker (`record_zst`). */
  private exportReplay(): void {
    if (this.exportStarted) return;
    this.exportStarted = true;
    this.replayId ??= `r${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
    const created = new Date().toISOString();
    void (async () => {
      try {
        const bytes = await this.engine.export(created);
        const header = JSON.parse(rules.record_header_bytes(bytes)) as RecordHeader;
        await putReplay(header, bytes, this.replayId ?? undefined);
        this.replayBytes = bytes;
        this.replayName = recordFilename(created);
        this.persist();
      } catch (e) {
        console.warn("could not export the replay:", e);
      } finally {
        this.emitOther();
      }
    })();
  }

  markRecorded(): void {
    this.recorded = true;
    this.persist();
  }

  act(cmd: Command): Promise<Msg | null> {
    return this.engine.act(this.you, cmd);
  }

  /** Skip the character pick with a random one and preset decks. */
  quickStart(): void {
    void this.engine.quickStart();
  }

  leave(): void {
    this.detach();
    clearSoloSave();
  }

  /** Close the worker but keep the save: a source switch drops the live
   *  session and the remounted board resumes this match from it. */
  detach(): void {
    this.persist();
    this.closed = true;
    clearInterval(this.ponderTimer);
    window.removeEventListener("pagehide", this.onHide);
    document.removeEventListener("visibilitychange", this.onHide);
    this.engine.close();
  }
}

/** One bot seat in a solo match: its name, character and decision policy. */
export interface SoloBot {
  name: string;
  mentality: BotMentality;
  /** Explicit character name, or `""` for a random free one at start. */
  persona: string;
}

/**
 * Resolve `""` (random) seats to distinct characters from the free pool
 * (everything not explicitly picked). Explicit picks are kept as they are,
 * so "random" never steals a character somebody named.
 */
export function resolveSoloCharacters(want: readonly string[]): string[] {
  const taken = new Set(want.filter(Boolean));
  const free = D.personas.map((c) => c.name).filter((n) => !taken.has(n));
  return want.map((w) => {
    if (w) return w;
    if (!free.length) return "";
    return free.splice(Math.floor(Math.random() * free.length), 1)[0];
  });
}

export function startSolo(player: string, playerCharacter: string, bots: SoloBot[], weights: ScoreWeights): SoloSession {
  endSession();
  const s = SoloSession.start(player, playerCharacter, bots, weights);
  session = s;
  return s;
}

/** Abandon the running / saved solo match. */
export function discardSolo(): void {
  if (session instanceof SoloSession) endSession();
  clearSoloSave();
}

/** The running solo match, or the one saved before a refresh. */
export async function resumeSolo(): Promise<SoloSession | null> {
  if (session instanceof SoloSession) return session;
  const s = await SoloSession.resume();
  if (s) {
    endSession();
    session = s;
  }
  return s;
}

// ------------------------------------------------------------------ online

export class OnlineSession extends GameSession {
  readonly kind = "online";
  readonly id: string;
  /**
   * Watching, not seated: no hand, `view.playerId === -1`, every input locked.
   * A spectator is anyone whose member id is not in `room.members`. Flips to
   * `false` if they later take a free seat (`join` upgrades in place).
   */
  spectator: boolean;
  private close: () => void;
  dissolved: Msg | null = null;
  /**
   * Online extras cache (`playable` / `estCost` / `skills` / `aiAnswer`).
   * Solo does **not** use this: the local engine already ships those fields
   * in its frame (`web-glue`'s `Match::view_extra`) and is the authoritative
   * engine there -- exact and free (no determinization, no sampling). Online
   * computes them from the seat view through the bot worker because the
   * server no longer ships them and the client must not ask for hidden state.
   * The two paths stay separate because the results are not identical
   * (online samples hidden zones; solo reads the live match).
   */
  private extras: OnlineExtras;
  private stopClock: () => void;

  constructor(room: RoomInfo, you: number, view: MatchView | null = null) {
    super();
    this.id = room.id;
    this.room = room;
    this.you = you;
    this.spectator = !room.members.some((m) => m.id === you);
    // A watcher has no seat and no secrets to compute extras from.
    if (this.spectator) this.readOnly = true;
    this.extras = new OnlineExtras(room.id, you);
    // Route the initial frame through `emitView` too, so extras kick off
    // before the first subscriber attaches (they get the merged view).
    if (view) this.emitView(view);
    this.close = openStream(room.id, {
      room: (r) => {
        this.room = r;
        // A spectator who took a free seat is now a player: unlock the inputs
        // and let the next frame derive extras from the seat view.
        const seated = r.members.some((m) => m.id === this.you);
        if (this.spectator && seated) {
          this.spectator = false;
          this.readOnly = false;
        }
        this.emitOther();
      },
      match: (v) => this.emitView(v),
      event: (e) => this.emitEvent(e),
      dissolve: (reason) => {
        this.dissolved = reason;
        this.emitOther();
      },
      status: (c) => {
        this.connected = c;
        this.emitOther();
        // Re-sync on reconnect: hidden-tab timers throttle, and a dropped
        // stream may have missed background refreshes.
        if (c) this.resyncClock();
      },
    });
    // NTP-style offset onto the server's monotonic clock (turn deadlines).
    this.stopClock = startClockSync(async (t1) => {
      const r = await api.clock(t1);
      return r.ok && r.data ? r.data : null;
    });
    // Re-sync when the tab becomes visible again (timers throttle while hidden).
    if (typeof document !== "undefined") {
      document.addEventListener("visibilitychange", this.onVisible);
    }
  }

  private onVisible = () => {
    if (document.visibilityState === "visible") this.resyncClock();
  };

  /** A forced refresh (token-bucket rate-limited): a few samples, so a server
   *  that restarted while we were away (its clock epoch changed) is confirmed
   *  by two agreeing samples at once instead of waiting for the next refresh. */
  resyncClock(): void {
    if (!resyncLimiter.take()) return;
    void (async () => {
      for (let i = 0; i < 3; i++) {
        const t1 = performance.now();
        const r = await api.clock(t1);
        if (r.ok && r.data) clockSync.refresh({ t1, t2: r.data.t2, t3: r.data.t3, t4: performance.now() });
        if (i < 2) await new Promise((res) => setTimeout(res, 30));
      }
    })();
  }

  /**
   * Merge cached extras, emit, and kick off the seat-view computation when
   * they are not in yet (`emitOnlineView`). Until they arrive the fields stay
   * absent (consumers already tolerate that); when they land the view is
   * re-emitted so the UI updates. A failed / slow `extras` never blocks the
   * frame -- greying may be coarse and 托管 falls back to its policy table.
   */
  protected emitView(v: MatchView): void {
    // A spectator's frame is already the public view; there is no seat to
    // derive `playable` / `estCost` / `skills` / `aiAnswer` from.
    if (this.spectator) return super.emitView(v);
    emitOnlineView(this.extras, v, (m) => super.emitView(m), () => this.view);
  }

  async act(cmd: Command): Promise<Msg | null> {
    if (this.readOnly) return { k: "err.not_a_player" };
    const r = await api.act(this.id, cmd);
    return r.ok ? null : (r.error ?? { k: "err.unknown_act" });
  }

  /**
   * The Animator queue has drained and the view is current for `seq`.
   * Releases the server's turn-clock hold. Debounced / once per state
   * version by the caller (Board); no-op on a stale ack.
   */
  async animDone(seq: number): Promise<void> {
    // A watcher has no seat whose turn-clock hold to release; don't spam the
    // endpoint (it would answer `err.not_a_player`) on every frame.
    if (this.readOnly) return;
    await api.animDone(this.id, seq);
  }

  /**
   * This seat has the rolled order on screen. A watcher has no seat to ack
   * for, so it stays silent.
   */
  async orderReady(): Promise<void> {
    if (this.readOnly) return;
    await api.orderReady(this.id);
  }

  leave(): void {
    this.close();
    this.stopClock();
    if (typeof document !== "undefined") {
      document.removeEventListener("visibilitychange", this.onVisible);
    }
    if (!this.dissolved) void api.leave(this.id);
  }

  /**
   * Apply a join reply that took a free seat: the SSE `room` frame is the
   * usual path, but a reconnecting stream must not leave the client locked in
   * watcher mode after the server has already seated us.
   */
  adoptSeat(room: RoomInfo, you: number): void {
    this.room = room;
    this.you = you;
    if (this.spectator) {
      this.spectator = false;
      this.readOnly = false;
    }
    this.emitOther();
  }
}

export function startOnline(room: RoomInfo, you: number, view: MatchView | null = null): OnlineSession {
  if (session instanceof OnlineSession && session.id === room.id) return session;
  endSession();
  const s = new OnlineSession(room, you, view);
  session = s;
  return s;
}

/**
 * Re-attach to room `id` after a refresh: the tab's server session still
 * holds the player (the presence timeout hands it to the AI only after a while).
 */
export async function resumeOnline(id: string, player: string, persona: string, cnId: string): Promise<OnlineSession | Msg> {
  if (session instanceof OnlineSession && session.id === id && !session.dissolved) return session;
  if (!(await ensureSession(player, persona, cnId))) return { k: "err.offline" };
  const r = await api.roomState(id);
  if (!r.ok || !r.data) return r.error ?? { k: "err.room.gone" };
  // The room's build is the server's truth. Another tab may have selected a
  // different build; switch to the room's before rendering it (`Lobby.enter`
  // does the same on the click path). The switch remounts the scene, whose
  // fresh `useOnline` then sees the matching build.
  const room = r.data.room;
  if (room.build && room.build !== buildId()) {
    try {
      await switchSource({ build: room.build });
    } catch {
      // The switch reverted and toasted. Online matches run on the server, so
      // the room stays playable on the previous (local) roster instead of
      // bouncing the player out; the room is re-entered on a fresh navigation.
    }
  }
  return startOnline(room, r.data.you, r.data.game);
}

/**
 * The live session's ruleset / tables (and, online, its SSE stream and REST
 * base) are fixed when it starts. A successful source switch replaces all of
 * them, so the old session cannot continue: drop it. Screens that remain
 * (Room, Lobby) re-attach through `resumeOnline` on the new source; a solo
 * board remounts from its save. A failed switch restores the old source
 * before bumping, so its session survives.
 */
subscribeData(() => {
  const s = session;
  if (!s) return;
  const now = currentSources();
  if (now.rules === s.bootSources.rules && now.data === s.bootSources.data) return;
  s.setAutoMode("off");
  if (s instanceof SoloSession) {
    // Its save is the resume point; detaching keeps it.
    s.detach();
    session = null;
  } else {
    endSession();
  }
});

/** Which part of the match screen the phase belongs in. */
export function matchScene(v: MatchView | null): "select" | "deck" | "board" | null {
  if (!v || !v.state.phase) return null;
  if (v.state.phase === "deck") return "deck";
  return ["order", "ban", "pick"].includes(v.state.phase) ? "select" : "board";
}
