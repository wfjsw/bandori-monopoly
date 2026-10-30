// REST client and SSE stream for the Rust server (docs/SERVER.md).

import type { Msg } from "../i18n/msg";
import type { BotMentality, ClockSettings, Command, MatchEvent, MatchView, RoomInfo, ScoreWeights } from "../core/types";
import { gameUrl, serverBase, subscribeServer } from "./server";
import { kv } from "../core/kv.ts";

/** Last tab-independent session per server (recovered when a tab closes). */
const sessions = kv("session");

/** One game server's advertised rule build (`GET /api/builds`). */
export interface BuildView {
  id: string;
  name: string;
  description: string;
  rulesetSha256: string | null;
  cards: number;
  default: boolean;
}

/**
 * Game-server sessions are per server: the same browser may hold a different
 * token (and a different seat) on each one. The classic origin keeps the bare
 * `bm.token` key; every other server gets its own suffixed key.
 */
function tokenKey(): string {
  const b = serverBase();
  return b ? `bm.token:${b}` : "bm.token";
}

let token = sessionStorage.getItem(tokenKey()) ?? "";
let tokenServer = serverBase();

/** Re-read the tab's token after the stored server has hydrated (boot). */
export function initToken(): void {
  tokenServer = serverBase();
  token = sessionStorage.getItem(tokenKey()) ?? "";
}
subscribeServer(() => {
  if (serverBase() === tokenServer) return;
  // A switch must not carry the old server's token anywhere -- not in memory,
  // not as a request header. `ensureSession` mints (or recovers) one on the
  // new server.
  tokenServer = serverBase();
  token = "";
});

/** Catch a server switch that happened without the subscribe callback (tests,
 *  or a storage write from elsewhere) before a call reads `token`. */
function syncToken(): void {
  if (serverBase() !== tokenServer) {
    tokenServer = serverBase();
    token = "";
  }
}

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
  syncToken();
  try {
    const r = await fetch(gameUrl(path), {
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
  syncToken();
  if (!token) {
    // A fresh tab (browser was closed): take the last token over only if it is still seated in a
    // room, so the player can rejoin their match. Otherwise tabs keep separate sessions. The key
    // is per server, so a token from another game server is never presented here.
    const key = tokenKey();
    const last = sessions.get(key);
    if (last) {
      token = last;
      const me = await call<{ player: string; room: string | null }>("GET", "/api/session");
      if (me.ok && me.data?.player === player && me.data.room) sessionStorage.setItem(key, token);
      else token = "";
    }
  }
  if (token) {
    const me = await call<{ player: string }>("GET", "/api/session");
    if (me.ok && me.data?.player === player) return true;
  }
  // The server field is `persona`; `character` is only this function's name.
  const r = await call<{ token: string }>("POST", "/api/session", { player, persona: character, cnId });
  if (!r.ok || !r.data) return false;
  token = r.data.token;
  const key = tokenKey();
  sessionStorage.setItem(key, token);
  sessions.set(key, token);
  return true;
}

export const api = {
  health: () => call<{ version: string; rooms: number; builds?: number }>("GET", "/api/health"),
  session: () => call<{ room: string | null; member: number }>("GET", "/api/session"),
  /** The rule builds this server offers. Older servers 404 the route. */
  builds: () => call<BuildView[]>("GET", "/api/builds"),
  rooms: () => call<RoomInfo[]>("GET", "/api/rooms"),
  createRoom: (o: { name: string; ranked: boolean; maxPlayers: number; password: string; weights?: ScoreWeights; clock?: ClockSettings; build?: string }) => call<{ room: RoomInfo; you: number }>("POST", "/api/rooms", o),
  /**
   * Join (or rejoin) a room. `spectate` takes a watcher's place instead of a
   * seat: allowed while the room is playing or full, still gated by the
   * password. The reply's `you` is the room member id (a spectator id is never
   * a seat id).
   */
  join: (id: string, password = "", spectate = false) =>
    call<{ room: RoomInfo; you: number }>("POST", `/api/rooms/${id}/join`, { password, version: "9", spectate }),
  ready: (id: string, on: boolean) => call<RoomInfo>("POST", `/api/rooms/${id}/ready`, { on }),
  bot: (id: string, op: "add" | "remove", member = 0, mentality: BotMentality = "standard") =>
    call<RoomInfo>("POST", `/api/rooms/${id}/bots`, { op, member, mentality }),
  weights: (id: string, w: ScoreWeights) => call<RoomInfo>("POST", `/api/rooms/${id}/weights`, w),
  setClock: (id: string, c: ClockSettings) => call<RoomInfo>("POST", `/api/rooms/${id}/clock`, c),
  start: (id: string, force: boolean) => call<RoomInfo>("POST", `/api/rooms/${id}/start`, { force }),
  /** This seat's commit-reveal nonce (32 random bytes, hex) -- `docs/FAIRNESS.md`. */
  nonce: (id: string, nonce: string) => call<RoomInfo>("POST", `/api/rooms/${id}/nonce`, { nonce }),
  leave: (id: string) => call<unknown>("POST", `/api/rooms/${id}/leave`, {}),
  roomState: (id: string) => call<{ room: RoomInfo; you: number; game: MatchView | null }>("GET", `/api/rooms/${id}/state`),
  act: (id: string, cmd: Command) => call<unknown>("POST", `/api/rooms/${id}/act`, cmd),
  /**
   * The acting seat's animation queue drained for state version `seq`.
   * Releases the turn-clock hold (a 10s cap starts the clock anyway).
   * Ignored by the server when stale; never sent in replay / solo.
   */
  animDone: (id: string, seq: number) => call<{ ok: true; released: boolean }>("POST", `/api/rooms/${id}/anim-done`, { seq }),
  /**
   * This seat has the rolled turn order on screen. The order phase ends once
   * every seat with a client has acked; a 4s cap covers the rest. Ignored
   * when stale; never sent in replay / solo.
   */
  orderReady: (id: string) => call<{ ok: true; ready: boolean }>("POST", `/api/rooms/${id}/order-ready`, {}),
  /**
   * The last finished match's `.bdrec` (docs/SERVER.md). The full record
   * reveals every hand, so the server answers 409 `err.record.live` while the
   * room is playing, 403 `err.record.forbidden` to anyone who was not in it,
   * and 404 `err.record.none` when there is nothing sealed yet. The body is a
   * zstd-framed `.bdrec` (`Content-Type: application/zstd`); older stores may
   * answer plain JSON. The engine's `from_record_bytes` sniffs the magic.
   */
  record: async (id: string): Promise<{ ok: true; bytes: Uint8Array; filename: string } | { ok: false; error: Msg; status: number }> => {
    syncToken();
    try {
      const r = await fetch(gameUrl(`/api/rooms/${id}/record`), {
        credentials: "same-origin",
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      });
      if (!r.ok) {
        const data = await r.json().catch(() => null);
        return { ok: false, error: data?.error ?? msg("err.http", { status: { i: r.status } }), status: r.status };
      }
      const disp = r.headers.get("content-disposition") ?? "";
      const named = /filename="([^"]+)"/.exec(disp)?.[1];
      return { ok: true, bytes: new Uint8Array(await r.arrayBuffer()), filename: named || `bdrec-${id}.bdrec` };
    } catch {
      return { ok: false, error: msg("err.offline"), status: 0 };
    }
  },
  /**
   * One NTP-style clock sample (`docs/SERVER.md` §Turn clock). `t1` is the
   * caller's `performance.now()` at send; the server stamps `t2` at receive
   * and `t3` just before replying, on the same monotonic clock the turn
   * deadlines use.
   */
  clock: (t1: number) => call<{ t1: number; t2: number; t3: number }>("POST", "/api/clock", { t1 }),
};

export interface StreamHandlers {
  room(r: RoomInfo): void;
  match(v: MatchView): void;
  event(e: MatchEvent): void;
  dissolve(reason: Msg): void;
  status(connected: boolean): void;
}

/**
 * The room's SSE stream. The browser reconnects by itself after a dropped
 * connection, but gives up for good when the reconnect gets an HTTP error --
 * which is exactly what a server restart looks like behind a proxy (502 while
 * it boots). So once the source is `CLOSED` we reopen it ourselves, with
 * backoff, resuming from the last event id seen.
 */
export function openStream(roomId: string, h: StreamHandlers): () => void {
  syncToken();
  let es: EventSource | null = null;
  let stopped = false;
  let lastId = "";
  let retry = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const connect = () => {
    const q = `token=${encodeURIComponent(token)}` + (lastId ? `&lastEventId=${encodeURIComponent(lastId)}` : "");
    const src = new EventSource(gameUrl(`/api/rooms/${roomId}/stream?${q}`));
    es = src;
    const on = <T>(name: string, f: (v: T) => void) =>
      src.addEventListener(name, (e) => {
        const m = e as MessageEvent;
        if (m.lastEventId) lastId = m.lastEventId;
        try {
          f(JSON.parse(m.data));
        } catch (err) {
          console.error(name, err);
        }
      });
    on<RoomInfo>("room", h.room);
    on<MatchView>("match", h.match);
    on<MatchEvent>("event", h.event);
    on<{ reason: Msg }>("dissolve", (d) => {
      stopped = true;
      h.dissolve(d.reason);
      src.close();
    });
    src.onopen = () => {
      retry = 0;
      h.status(true);
    };
    src.onerror = () => {
      h.status(false);
      if (stopped || src.readyState !== EventSource.CLOSED) return;
      timer = setTimeout(connect, Math.min(5000, 500 * 2 ** retry++));
    };
  };
  connect();
  return () => {
    stopped = true;
    clearTimeout(timer);
    es?.close();
  };
}
