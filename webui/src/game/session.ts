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
class Autopilot {
  private timer = 0;
  private inFlight = false;
  private seq = -1;
  private tried = new Set<string>();
  private played = 0;
  private turnKey = "";
  private off: (() => void) | null = null;

  constructor(private sess: GameSession) {}

  start(): void {
    if (this.off) return;
    this.off = this.sess.subscribe((v) => this.onView(v));
    if (this.sess.view) this.onView(this.sess.view);
  }

  stop(): void {
    clearTimeout(this.timer);
    this.timer = 0;
    this.inFlight = false;
    this.seq = -1;
    this.tried.clear();
    this.off?.();
    this.off = null;
  }

  private delay(): number {
    return 400 + Math.random() * 600;
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
    if (v.state.seq === this.seq && this.tried.size > 0) return;
    this.schedule();
  }

  private ctx(): AutopilotCtx {
    return {
      tiles: D.tiles,
      characters: D.characters,
      playedThisTurn: this.played,
      deckPreset: (c) => JSON.parse(rules.deck_preset(c)),
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

  private step(): void {
    const s = this.sess;
    const mode = s.autoMode;
    if (!isAuto(mode) || this.inFlight) return;
    const v = s.view;
    if (!v) return;
    if (v.state.seq !== this.seq) {
      this.seq = v.state.seq;
      this.tried.clear();
    }
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
        if (isAuto(s.autoMode) && now && now.state.seq !== this.seq) this.schedule();
      });
  }
}

export abstract class GameSession {
  abstract readonly kind: "solo" | "online";
  /** Route id: "solo" or the room id. */
  abstract readonly id: string;
  view: MatchView | null = null;
  room: RoomInfo | null = null;
  you = 0;
  connected = true;
  /** Set when the match result has been recorded on the profile. */
  recorded = false;
  /**
   * Auto-play mode for this seat: `off` (you), `bot` (托管), `chaos` (混沌).
   * Per-session, not persisted. Any non-`off` value locks the user's inputs.
   */
  autoMode: AutoMode = "off";
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

  /** Set 托管 / 混沌 / off. Takes effect immediately (mid-command included). */
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

  /** Re-render when the auto mode flips. Returns unsubscribe. */
  subscribeAutoMode(cb: () => void): () => void {
    this.autoCbs.add(cb);
    return () => this.autoCbs.delete(cb);
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
}

export class SoloSession extends GameSession {
  readonly kind = "solo";
  readonly id = "solo";
  readonly weights: ScoreWeights;
  private m: InstanceType<typeof rules.SoloMatch>;
  private timer: number;
  private last = 0;
  private lastTick = performance.now();
  private dirty = true;
  private savedAt = 0;
  private closed = false;
  private onHide = () => this.persist(true);

  private constructor(m: InstanceType<typeof rules.SoloMatch>, weights: ScoreWeights, last = 0) {
    super();
    this.you = 1;
    this.m = m;
    this.weights = weights;
    this.last = last;
    this.timer = window.setInterval(() => this.tick(), 50);
    window.addEventListener("pagehide", this.onHide);
    document.addEventListener("visibilitychange", this.onHide);
    this.pump();
  }

  static start(player: string, bots: SoloBot[], weights: ScoreWeights): SoloSession {
    const members: RoomMember[] = [
      { id: 1, player, character: "", cnId: "", ready: true, host: true, bot: false, away: false, mentality: "standard" },
      ...bots.map((b, i) => ({
        id: i + 2,
        player: b.name,
        character: "",
        cnId: "",
        ready: true,
        host: false,
        bot: true,
        away: false,
        mentality: b.mentality,
      })),
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
      const out = new SoloSession(rules.SoloMatch.restore(s.match), s.weights, s.last);
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

  private tick(): void {
    const t = performance.now();
    const dt = Math.min(0.5, (t - this.lastTick) / 1000);
    this.lastTick = t;
    this.m.tick(dt);
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
  }

  /** Save at most once a second (and always when the page is hidden). */
  private persist(force: boolean): void {
    if (this.closed || !this.dirty) return;
    const now = performance.now();
    if (!force && now - this.savedAt < 1000) return;
    this.savedAt = now;
    this.dirty = false;
    const save: SoloSave = { v: SOLO_VERSION, match: this.m.save(), weights: this.weights, recorded: this.recorded, last: this.last };
    try {
      localStorage.setItem(SOLO_KEY, JSON.stringify(save));
    } catch (e) {
      console.warn("could not save the solo match:", e);
    }
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

/** One bot seat in a solo match: its name and decision policy. */
export interface SoloBot {
  name: string;
  mentality: BotMentality;
}

export function startSolo(player: string, bots: SoloBot[], weights: ScoreWeights): SoloSession {
  endSession();
  const s = SoloSession.start(player, bots, weights);
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
