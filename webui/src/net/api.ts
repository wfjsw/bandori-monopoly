// REST client and SSE stream for the Rust server (docs/SERVER.md).

import type { Msg } from "../i18n/msg";
import type { Command, MatchEvent, MatchView, RoomInfo, ScoreWeights } from "../core/types";

const TOKEN_KEY = "bm.token";
let token = sessionStorage.getItem(TOKEN_KEY) ?? "";

export interface Result<T> {
  ok: boolean;
  data?: T;
  /** Server message (localizable), or a transport-level one from `msg`. */
  error?: Msg;
  reason?: string;
}

/** Client-side transport errors, as messages (game namespace). */
const msg = (k: string, a?: Msg["a"]): Msg => (a ? { k, a } : { k });

async function call<T>(method: "GET" | "POST", path: string, body?: unknown): Promise<Result<T>> {
  try {
    const r = await fetch(path, {
      method,
      credentials: "same-origin",
      headers: { ...(token ? { Authorization: `Bearer ${token}` } : {}), ...(body !== undefined ? { "Content-Type": "application/json" } : {}) },
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
    const data = await r.json().catch(() => null);
    if (!r.ok) return { ok: false, error: data?.error ?? msg("err.http", { status: { i: r.status } }), reason: data?.reason ?? undefined };
    return { ok: true, data: data as T };
  } catch {
    return { ok: false, error: msg("err.offline") };
  }
}

/** Make sure this tab has a server session for `player`. */
export async function ensureSession(player: string, character = "", cnId = ""): Promise<boolean> {
  if (token) {
    const me = await call<{ player: string }>("GET", "/api/session");
    if (me.ok && me.data?.player === player) return true;
  }
  const r = await call<{ token: string }>("POST", "/api/session", { player, character, cnId });
  if (!r.ok || !r.data) return false;
  token = r.data.token;
  sessionStorage.setItem(TOKEN_KEY, token);
  return true;
}

export const api = {
  health: () => call<{ version: string; rooms: number }>("GET", "/api/health"),
  session: () => call<{ room: string | null; member: number }>("GET", "/api/session"),
  rooms: () => call<RoomInfo[]>("GET", "/api/rooms"),
  createRoom: (o: { name: string; ranked: boolean; maxPlayers: number; password: string; weights?: ScoreWeights }) => call<{ room: RoomInfo; you: number }>("POST", "/api/rooms", o),
  join: (id: string, password = "") => call<{ room: RoomInfo; you: number }>("POST", `/api/rooms/${id}/join`, { password, version: "9" }),
  ready: (id: string, on: boolean) => call<RoomInfo>("POST", `/api/rooms/${id}/ready`, { on }),
  bot: (id: string, op: "add" | "remove", member = 0) => call<RoomInfo>("POST", `/api/rooms/${id}/bots`, { op, member }),
  weights: (id: string, w: ScoreWeights) => call<RoomInfo>("POST", `/api/rooms/${id}/weights`, w),
  start: (id: string, force: boolean) => call<RoomInfo>("POST", `/api/rooms/${id}/start`, { force }),
  leave: (id: string) => call<unknown>("POST", `/api/rooms/${id}/leave`, {}),
  roomState: (id: string) => call<{ room: RoomInfo; you: number; game: MatchView | null }>("GET", `/api/rooms/${id}/state`),
  act: (id: string, cmd: Command) => call<unknown>("POST", `/api/rooms/${id}/act`, cmd),
};

export interface StreamHandlers {
  room(r: RoomInfo): void;
  match(v: MatchView): void;
  event(e: MatchEvent): void;
  dissolve(reason: Msg): void;
  status(connected: boolean): void;
}

/** The room's SSE stream. The browser reconnects by itself and resends `Last-Event-ID`. */
export function openStream(roomId: string, h: StreamHandlers): () => void {
  const es = new EventSource(`/api/rooms/${roomId}/stream?token=${encodeURIComponent(token)}`);
  const on = <T>(name: string, f: (v: T) => void) =>
    es.addEventListener(name, (e) => {
      try {
        f(JSON.parse((e as MessageEvent).data));
      } catch (err) {
        console.error(name, err);
      }
    });
  on<RoomInfo>("room", h.room);
  on<MatchView>("match", h.match);
  on<MatchEvent>("event", h.event);
  on<{ reason: Msg }>("dissolve", (d) => {
    h.dissolve(d.reason);
    es.close();
  });
  es.onopen = () => h.status(true);
  es.onerror = () => h.status(false);
  return () => es.close();
}
