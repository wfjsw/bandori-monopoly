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
import { advancedSeats, driveSeat, decisionAt, type DriveHooks } from "./botDrive";
import { putReplay, recordFilename, type RecordHeader } from "./record";

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
/** One recorded tick quantum (`docs/REPLAY.md` §1). */
const TICK_STEP = 0.05;

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
 * Rebuild a solo match from its save plus the recorder beside it. Old saves
 * carry no `rec` and fall back to a partial record (`RecordedMatch` starts a
 * fresh log at the snapshot). `restore_with_record` is bound as an instance
 * method in the glue, so borrow a throwaway instance to call it -- the Rust
 * side does not read `self`.
 */
function restoreSoloMatch(save: string, rec: string | undefined): InstanceType<typeof rules.SoloMatch> {
  if (!rec) return rules.SoloMatch.restore(save);
  const tmp = rules.SoloMatch.restore(save);
  try {
    return tmp.restore_with_record(save, rec);
  } finally {
    tmp.free();
  }
}

export class SoloSession extends GameSession {
  readonly kind = "solo";
  readonly id = "solo";
  readonly weights: ScoreWeights;
  /** The `.bdrec` exported when the match ended (zstd-framed), kept for Results. */
  replayBytes: Uint8Array | null = null;
  replayId: string | null = null;
  replayName = "bdrec-replay.bdrec";
  private m: InstanceType<typeof rules.SoloMatch>;
  private timer: number;
  private last = 0;
  private lastTick = performance.now();
  /** Whole `TICK_STEP` quanta still owed to the engine. */
  private acc = 0;
  private dirty = true;
  private savedAt = 0;
  private closed = false;
  private exportStarted = false;
  /** Seats the 进阶 driver is currently searching for (one ask at a time). */
  private botInFlight = new Set<number>();
  /** Per-seat decisions already spent this turn (the budget spread). */
  private botTurnKey = "";
  private botPlayed = new Map<number, number>();
  private onHide = () => this.persist(true);

  private constructor(m: InstanceType<typeof rules.SoloMatch>, weights: ScoreWeights, last = 0, replayId: string | null = null) {
    super();
    this.you = 1;
    this.m = m;
    this.weights = weights;
    this.last = last;
    // Set before the first `pump`, which would otherwise export the record
    // again on resume of an already-finished match.
    this.replayId = replayId;
    this.timer = window.setInterval(() => this.tick(), 50);
    window.addEventListener("pagehide", this.onHide);
    document.addEventListener("visibilitychange", this.onHide);
    this.pump();
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
    const seed = Math.floor(Math.random() * 0xffffffff);
    return new SoloSession(new rules.SoloMatch(JSON.stringify(members), seed, 0, JSON.stringify(weights)), weights);
  }

  /** The match saved before a refresh, if any. */
  static resume(): SoloSession | null {
    const raw = localStorage.getItem(SOLO_KEY);
    if (!raw) return null;
    try {
      const s: SoloSave = JSON.parse(raw);
      if (s.v !== SOLO_VERSION) throw new Error("old save");
      const out = new SoloSession(restoreSoloMatch(s.match, s.rec), s.weights, s.last, s.replayId ?? null);
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

  /** Fixed-step: whole `TICK_STEP` quanta, `tick_steps(k)` with k in 1..=10,
   *  the remainder carried over -- so the record's tick runs match what ran. */
  private tick(): void {
    const t = performance.now();
    const dt = Math.min(0.5, (t - this.lastTick) / 1000);
    this.lastTick = t;
    this.acc += dt;
    let k = Math.floor(this.acc / TICK_STEP);
    if (k > 10) k = 10;
    if (k > 0) {
      this.acc -= k * TICK_STEP;
      this.m.tick_steps(k);
    }
    this.pump();
    this.persist(false);
  }

  private pump(): void {
    const evs: MatchEvent[] = JSON.parse(this.m.events_since(this.last));
    for (const e of evs) {
      this.last = e.id;
      this.emitEvent(e);
    }
    if (evs.length) this.dirty = true;
    if (this.m.take_changed() || !this.view) {
      this.dirty = true;
      this.emitView(JSON.parse(this.m.view(this.you)));
    }
    if (this.m.ended()) this.exportReplay();
    else this.driveAdvancedBots();
  }

  /**
   * Keep every Advanced bot seat moving (`docs/BOT.md` B6). The engine holds
   * their `ai` off (`Match::new`), exactly like the server does for online
   * Advanced seats -- so this driver is the one applying answers through the
   * normal `act` path. Setup (ban / pick / deck) stays engine-side
   * (`MatchPlayer::auto_setup`). Fallback to the existing bot policy on any
   * search failure so a match never stalls (§1).
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
      const v = JSON.parse(this.m.view(member)) as MatchView;
      if (decisionAt(v.state, v.playerId) == null) continue;
      this.botInFlight.add(member);
      const played = this.botPlayed.get(member) ?? 0;
      void driveSeat(
        async (m, cmd) => {
          const err = this.actAs(m, cmd);
          if (!err && cmd.act === "play") this.botPlayed.set(m, (this.botPlayed.get(m) ?? 0) + 1);
          return err;
        },
        (m) => (m === member ? v : (JSON.parse(this.m.view(m)) as MatchView)),
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
  private actAs(member: number, cmd: Command): Msg | null {
    const raw = this.m.act(member, JSON.stringify(cmd));
    this.pump();
    this.persist(true);
    return raw ? (JSON.parse(raw) as Msg) : null;
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

  /** Save at most once a second (and always when the page is hidden). The
   *  recorder rides along in the same write, so a refresh keeps the log. */
  private persist(force: boolean): void {
    if (this.closed || !this.dirty) return;
    const now = performance.now();
    if (!force && now - this.savedAt < 1000) return;
    this.savedAt = now;
    this.dirty = false;
    const save: SoloSave = {
      v: SOLO_VERSION,
      match: this.m.save(),
      weights: this.weights,
      recorded: this.recorded,
      last: this.last,
      rec: this.m.record_state(),
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
   *  export twice. */
  private exportReplay(): void {
    if (this.exportStarted || this.replayId) return;
    this.exportStarted = true;
    const created = new Date().toISOString();
    let bytes: Uint8Array;
    try {
      bytes = this.m.record_zst(created);
    } catch (e) {
      console.warn("could not export the replay:", e);
      return;
    }
    void (async () => {
      try {
        const header = JSON.parse(rules.record_header_bytes(bytes)) as RecordHeader;
        const id = await putReplay(header, bytes);
        this.replayBytes = bytes;
        this.replayId = id;
        this.replayName = recordFilename(created);
        this.dirty = true;
        this.persist(true);
      } catch (e) {
        console.warn("could not save the replay:", e);
      } finally {
        this.emitOther();
      }
    })();
  }

  markRecorded(): void {
    this.recorded = true;
    this.dirty = true;
    this.persist(true);
  }

  act(cmd: Command): Promise<Msg | null> {
    const raw = this.m.act(this.you, JSON.stringify(cmd));
    this.pump();
    this.persist(true);
    return Promise.resolve(raw ? (JSON.parse(raw) as Msg) : null);
  }

  /** Skip the character pick with a random one and preset decks. */
  quickStart(): void {
    this.m.quick_start();
    this.pump();
  }

  leave(): void {
    this.closed = true;
    clearInterval(this.timer);
    window.removeEventListener("pagehide", this.onHide);
    document.removeEventListener("visibilitychange", this.onHide);
    localStorage.removeItem(SOLO_KEY);
    this.m.free();
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
