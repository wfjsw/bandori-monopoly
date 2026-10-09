// One interface for a match whether it runs on the server (online) or in the
// browser's wasm engine (solo). Scenes only talk to `session`.
//
// Refresh recovery: a solo match is saved to localStorage (engine snapshot) and
// resumed on load; an online player is re-attached through the server session
// (the token lives in sessionStorage) and `GET /api/rooms/{id}/state`.

import { D, rules } from "../core/data";
import type { BotMentality, Command, MatchEvent, MatchView, RoomInfo, RoomMember, ScoreWeights } from "../core/types";
import { api, ensureSession, openStream } from "../net/api";
import type { Msg } from "../i18n/msg";
import { isAuto, plan, type AutoMode, type AutopilotCtx } from "./autopilot";
import { SOLO_CAP_MS } from "./botBudget";
import { advancedSeats, driveSeat, decisionAt, ponderUpcoming, type DriveHooks } from "./botDrive";
import { putReplay, recordFilename, type RecordHeader } from "./record";
import { SoloEngine } from "./soloEngine";
import type { SaveSnap, SoloPush } from "./soloProtocol";

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
      characters: D.characters,
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
    if (mode === "advanced") return void this.stepAdvanced(v, at);
    const cmd = plan(v, this.ctx(), mode).find((c) => !this.tried.has(Autopilot.key(c)));
    if (!cmd) return;
    this.tried.add(Autopilot.key(cmd));
    this.inFlight = true;
    Promise.resolve(s.act(cmd))
      .then((err) => {
        if (err) {
          // Logged, marked tried, and the next candidate is attempted after
          // the usual human beat -- never a tight retry loop.
          console.warn(`${mode}: command refused`, cmd, err);
          this.schedule();
        } else if (cmd.act === "play") {
          this.played++;
        }
      })
      .finally(() => {
        this.inFlight = false;
        // Solo's `pump` emits the next view inside `act`; catch up if one
        // already landed while we were in flight.
        const now = s.view;
        if (isAuto(s.autoMode) && now && now.state.seq * 100000 + Math.max(0, now.state.prompt?.id ?? 0) !== this.seq) this.schedule();
      });
  }

  /**
   * 进阶 托管 (`docs/BOT.md` B6): the worker pool answers play-phase decisions;
   * setup / trivial surfaces still go through the standard policy below. On
   * any search failure the existing bot policy answers instead -- a match
   * never stalls (§1).
   */
  private stepAdvanced(v: MatchView, at: number): void {
    const s = this.sess;
    // Setup / discard / vote: the search does not cover them (and the server
    // keeps an Advanced seat's setup engine-side). Same policy as `bot`.
    if (decisionAt(v.state, v.playerId) == null) {
      const cmd = plan(v, this.ctx(), "advanced").find((c) => !this.tried.has(Autopilot.key(c)));
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
      .catch((e) => console.warn("advanced: drive failed", e))
      .finally(() => {
        this.inFlight = false;
        this.setSearching(false);
        const now = s.view;
        if (isAuto(s.autoMode) && now && now.state.seq * 100000 + Math.max(0, now.state.prompt?.id ?? 0) !== at) this.schedule();
      });
  }
}

/** Solo's per-decision search cap (`docs/BOT.md` §3.6, user ruling). Kept in
 *  localStorage so it survives a refresh; the solo setup screen edits it. */
const SOLO_BOT_CAP_KEY = "bm.botSoloCapMs";

export function botSoloCapMs(): number {
  const raw = Number(localStorage.getItem(SOLO_BOT_CAP_KEY));
  return Number.isFinite(raw) && raw >= 200 ? Math.min(12_000, raw) : SOLO_CAP_MS;
}

export function setBotSoloCapMs(ms: number): void {
  localStorage.setItem(SOLO_BOT_CAP_KEY, String(Math.round(ms)));
}

export abstract class GameSession {
  abstract readonly kind: "solo" | "online" | "replay";
  /** Route id: "solo" or the room id. */
  abstract readonly id: string;
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
  abstract leave(): void;
}

export let session: GameSession | null = null;

export function endSession(): void {
  session?.setAutoMode("off");
  session?.leave();
  session = null;
}

// ------------------------------------------------------------------ solo

const SOLO_KEY = "bm.solo";
const SOLO_VERSION = 1;

interface SoloSave {
  v: number;
  match: string;
  weights: ScoreWeights;
  recorded: boolean;
  last: number;
  /** The recorder beside `match` (absent on saves from before records). */
  rec?: string;
  /** IndexedDB id of the `.bdrec` exported when the match ended. */
  replayId?: string;
}

/**
 * One solo match. The `SoloMatch` (wasm), its 50 ms fixed-step tick loop and
 * the engine's standard bots run in a module worker (`soloWorker.ts`); this
 * class is the page-side half: it caches the frames the worker pushes, drives
 * the 进阶 seats through the bot pool, and keeps a `localStorage` save fresh.
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
    // Set before the first sync push, which would otherwise export the record
    // again on resume of an already-finished match.
    this.replayId = replayId;
    engine.onPush = (p) => this.onPush(p);
    this.ready = open();
    // Mark the rejection handled here too -- the setup path never awaits
    // `ready`; Play's effect is what backs out of a failed open.
    void this.ready.catch((e) => {
      console.warn("solo engine could not open the match:", e);
      if (this.closed) return;
      localStorage.removeItem(SOLO_KEY);
    });
    this.ponderTimer = window.setInterval(() => {
      if (!this.closed) this.driveAdvancedBots();
    }, 350);
    window.addEventListener("pagehide", this.onHide);
    document.addEventListener("visibilitychange", this.onHide);
  }

  static start(player: string, playerCharacter: string, bots: SoloBot[], weights: ScoreWeights): SoloSession {
    // The solo setup screen's picks, its `""` (random) seats resolved here
    // among the free characters -- everything not explicitly picked. The
    // engine sees a character on every seat and starts past ban / pick.
    const chars = resolveSoloCharacters([playerCharacter, ...bots.map((b) => b.character)]);
    const seat = (id: number, name: string, character: string, bot: boolean, mentality: BotMentality): RoomMember => {
      const c = D.character(character);
      return {
        id,
        player: name,
        character,
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
    const engine = new SoloEngine();
    return new SoloSession(engine, weights, async () => {
      await engine.boot();
      await engine.start({ members, mode: 0, weights, you: 1 });
      engine.setVisibility(document.hidden);
    });
  }

  /** The match saved before a refresh, if any. The save's engine strings go to
   *  the worker; a restore failure (a corrupt save) rejects `ready` and Play
   *  drops the session. */
  static resume(): SoloSession | null {
    const raw = localStorage.getItem(SOLO_KEY);
    if (!raw) return null;
    try {
      const s: SoloSave = JSON.parse(raw);
      if (s.v !== SOLO_VERSION) throw new Error("old save");
      const engine = new SoloEngine();
      const out = new SoloSession(
        engine,
        s.weights,
        async () => {
          await engine.boot();
          await engine.restore({ save: s.match, rec: s.rec, last: s.last, you: 1 });
          engine.setVisibility(document.hidden);
        },
        s.replayId ?? null,
      );
      out.recorded = s.recorded;
      return out;
    } catch (e) {
      console.warn("solo save dropped:", e);
      localStorage.removeItem(SOLO_KEY);
      return null;
    }
  }

  static hasSave(): boolean {
    return !!localStorage.getItem(SOLO_KEY);
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
    else this.driveAdvancedBots();
  }

  /**
   * Keep every Advanced bot seat moving (`docs/BOT.md` B6). The engine holds
   * their `ai` off (`Match::new`), exactly like the server does for online
   * Advanced seats -- so this driver is the one applying answers through the
   * normal `act` path. Setup (ban / pick / deck) stays engine-side
   * (`MatchPlayer::auto_setup`). Fallback to the existing bot policy on any
   * search failure so a match never stalls (§1).
   *
   * The seats' frames come from the worker's pushes (they change together with
   * the state); the driver runs on every sync and on a light idle timer, which
   * is what keeps the speculative `ponder` (`docs/BOT.md` §3.5) firing while
   * the match is quiet.
   */
  private driveAdvancedBots(): void {
    const v0 = this.view;
    if (!v0 || v0.state.phase !== "play") return;
    // One turn key per (round, turn) for the budget spread.
    const turnKey = `${v0.state.round}:${v0.state.turn}`;
    if (turnKey !== this.botTurnKey) {
      this.botTurnKey = turnKey;
      this.botPlayed.clear();
    }
    for (const member of advancedSeats(v0)) {
      if (this.botInFlight.has(member)) continue;
      const v = this.botViews.get(member);
      if (!v) continue;
      if (decisionAt(v.state, v.playerId) == null) {
        // Nothing to answer right now. Speculate on the seat's **next own
        // decision** when it is near (BOT-RESEARCH #5, `docs/BOT.md` §3.5) --
        // rate-limited like the server's idle probe, one in flight per seat.
        // An idle view has no searchable surface; `ponderUpcoming` gates on
        // `nextTurnNear` and the worker predicts the turn-start 运营 view.
        const now = performance.now();
        if (
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
          onThinking: (m, on) => this.setBotThinking(m, on),
        },
      ).finally(() => {
        this.botInFlight.delete(member);
        // A search can span several ticks; catch up on the next decision.
        if (!this.closed) this.driveAdvancedBots();
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
      characters: D.characters,
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

  /** Write the cached engine snapshot to `localStorage` (the envelope fields
   *  -- weights / recorded / replayId -- are the page's). Called on the
   *  worker's save pushes (at most one a second while dirty) and on the way
   *  out (`pagehide`); see `soloProtocol.ts` for why the snapshot is cached. */
  private persist(): void {
    if (this.closed || !this.snap.match) return;
    const save: SoloSave = {
      v: SOLO_VERSION,
      match: this.snap.match,
      weights: this.weights,
      recorded: this.recorded,
      // The cursor that belongs with these engine strings -- not a later one,
      // or a resume would skip events the snapshot does not have.
      last: this.snap.last,
      rec: this.snap.rec,
      replayId: this.replayId ?? undefined,
    };
    try {
      localStorage.setItem(SOLO_KEY, JSON.stringify(save));
    } catch (e) {
      console.warn("could not save the solo match:", e);
    }
  }

  /** Export the `.bdrec` once, when the match ends: zstd-frame it into
   *  IndexedDB (keep 10) and hold the bytes for the Results buttons. Guarded
   *  by `replayId` in the save, so a refresh on the results screen does not
   *  export twice. The compression runs in the worker (`record_zst`). */
  private exportReplay(): void {
    if (this.exportStarted || this.replayId) return;
    this.exportStarted = true;
    const created = new Date().toISOString();
    void (async () => {
      try {
        const bytes = await this.engine.export(created);
        const header = JSON.parse(rules.record_header_bytes(bytes)) as RecordHeader;
        const id = await putReplay(header, bytes);
        this.replayBytes = bytes;
        this.replayId = id;
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
    this.closed = true;
    clearInterval(this.ponderTimer);
    window.removeEventListener("pagehide", this.onHide);
    document.removeEventListener("visibilitychange", this.onHide);
    localStorage.removeItem(SOLO_KEY);
    this.engine.close();
  }
}

/** One bot seat in a solo match: its name, character and decision policy. */
export interface SoloBot {
  name: string;
  mentality: BotMentality;
  /** Explicit character name, or `""` for a random free one at start. */
  character: string;
}

/**
 * Resolve `""` (random) seats to distinct characters from the free pool
 * (everything not explicitly picked). Explicit picks are kept as they are,
 * so "random" never steals a character somebody named.
 */
export function resolveSoloCharacters(want: readonly string[]): string[] {
  const taken = new Set(want.filter(Boolean));
  const free = D.characters.map((c) => c.name).filter((n) => !taken.has(n));
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
  localStorage.removeItem(SOLO_KEY);
}

/** The running solo match, or the one saved before a refresh. */
export function resumeSolo(): SoloSession | null {
  if (session instanceof SoloSession) return session;
  const s = SoloSession.resume();
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
  private close: () => void;
  dissolved: Msg | null = null;

  constructor(room: RoomInfo, you: number, view: MatchView | null = null) {
    super();
    this.id = room.id;
    this.room = room;
    this.you = you;
    this.view = view;
    this.close = openStream(room.id, {
      room: (r) => {
        this.room = r;
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
      },
    });
  }

  async act(cmd: Command): Promise<Msg | null> {
    const r = await api.act(this.id, cmd);
    return r.ok ? null : (r.error ?? { k: "err.unknown_act" });
  }

  leave(): void {
    this.close();
    if (!this.dissolved) void api.leave(this.id);
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
export async function resumeOnline(id: string, player: string, character: string, cnId: string): Promise<OnlineSession | Msg> {
  if (session instanceof OnlineSession && session.id === id && !session.dissolved) return session;
  if (!(await ensureSession(player, character, cnId))) return { k: "err.offline" };
  const r = await api.roomState(id);
  if (!r.ok || !r.data) return r.error ?? { k: "err.room.gone" };
  return startOnline(r.data.room, r.data.you, r.data.game);
}

/** Which part of the match screen the phase belongs in. */
export function matchScene(v: MatchView | null): "select" | "board" | null {
  if (!v || !v.state.phase) return null;
  return ["order", "ban", "pick", "deck"].includes(v.state.phase) ? "select" : "board";
}
