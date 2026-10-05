// One interface for a match whether it runs on the server (online) or in the
// browser's wasm engine (solo). Scenes only talk to `session`.
//
// Refresh recovery: a solo match is saved to localStorage (engine snapshot) and
// resumed on load; an online player is re-attached through the server session
// (the token lives in sessionStorage) and `GET /api/rooms/{id}/state`.

import { rules } from "../core/data";
import type { Command, MatchEvent, MatchView, RoomInfo, RoomMember, ScoreWeights } from "../core/types";
import { api, ensureSession, openStream } from "../net/api";
import type { Msg } from "../i18n/msg";

type ViewCb = (v: MatchView) => void;
type EventCb = (e: MatchEvent) => void;

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

  static start(player: string, bots: string[], weights: ScoreWeights): SoloSession {
    const members: RoomMember[] = [
      { id: 1, player, character: "", cnId: "", ready: true, host: true, bot: false, away: false },
      ...bots.map((name, i) => ({ id: i + 2, player: name, character: "", cnId: "", ready: true, host: false, bot: true, away: false })),
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

export function startSolo(player: string, bots: string[], weights: ScoreWeights): SoloSession {
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
