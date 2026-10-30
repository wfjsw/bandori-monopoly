// Which game server this page talks to, and which rule build it loads.
//
// The profile server and the game servers are separate services. A game server
// is addressed by an absolute base URL; `""` means this page's own origin (the
// classic single-server setup, and the only one that serves `/assets/rules`
// without a `/builds/...` route). The selected build is a game-server concept:
// it rides `/builds/{id}/rules` and `/builds/{id}/data`, and `""` asks the
// server for its default build.
//
// Everything else (REST, SSE, the solo worker, the boot-time data load) reads
// the base through here so a server switch cannot leave one path behind.

import { useSyncExternalStore } from "react";
import { kv } from "../core/kv.ts";

const prefs = kv("prefs");
/** Absolute game-server base URL; absent = this page's origin. */
const SERVER_KEY = "gameServer";
/** Selected build id; absent = the server's default build. */
const BUILD_KEY = "build";
/** Human name of the selected build (the pill's label; the id is the truth). */
const BUILD_NAME_KEY = "buildName";

function read(key: string): string {
  return prefs.get(key);
}

function write(key: string, value: string): void {
  if (value) prefs.set(key, value);
  else prefs.remove(key);
}

/**
 * Normalize a user-supplied server URL: trimmed, http(s) only, no trailing
 * slash. `""` (or an invalid input) means "not a server address"; callers
 * decide whether that is the local fallback or a validation error.
 */
export function normalizeServer(raw: string | null | undefined): string {
  const trimmed = (raw ?? "").trim();
  if (!trimmed) return "";
  try {
    const u = new URL(trimmed);
    if (u.protocol !== "http:" && u.protocol !== "https:") return "";
    return (u.origin + u.pathname).replace(/\/+$/, "");
  } catch {
    return "";
  }
}

const initialBase = normalizeServer(read(SERVER_KEY));

let base = initialBase;
let build = read(BUILD_KEY);
let buildName = read(BUILD_NAME_KEY);
let version = 0;
const listeners = new Set<() => void>();

function emit(): void {
  version++;
  listeners.forEach((cb) => cb());
}

/** Adopt the stored server / build once the store has hydrated (boot). */
export function applyStoredServer(): void {
  base = normalizeServer(read(SERVER_KEY));
  build = read(BUILD_KEY);
  buildName = read(BUILD_NAME_KEY);
}

/** The game-server base URL (`""` = this page's origin). */
export function serverBase(): string {
  return base;
}

/**
 * Point the page at another game server (or back at this origin with `null`).
 * A build id only means something on the server it was chosen on, so it is
 * cleared with the server. Callers then reload the page's data with
 * `switchSource` (`core/data`) -- the live rules / tables are keyed to this
 * choice.
 */
export function setServer(url: string | null): void {
  const next = normalizeServer(url);
  if (next === base) return;
  base = next;
  build = "";
  buildName = "";
  write(SERVER_KEY, next);
  write(BUILD_KEY, "");
  write(BUILD_NAME_KEY, "");
  emit();
}

/** The selected build id (`""` = the server's default build). */
export function buildId(): string {
  return build;
}

/** The selected build's human name, when the caller that chose it knew one. */
export function buildNameOf(): string {
  return buildName;
}

/** Select a rule build (`null`/`""` = the server's default). The data is
 *  loaded by `switchSource` (`core/data`), not here. */
export function setBuild(id: string | null, name = ""): void {
  const next = (id ?? "").trim();
  if (next === build && name === buildName) return;
  build = next;
  buildName = name;
  write(BUILD_KEY, next);
  write(BUILD_NAME_KEY, name);
  emit();
}

/** True when an explicit game server (not this page's origin) is selected. */
export function isRemote(): boolean {
  return base !== "";
}

/** True when the selection is this page's own origin (or no server at all).
 *  That server hosts this SPA, so its bundled rules / tables are exactly what
 *  it plays; probing `/builds/...` there would only hit the SPA fallback. */
function isLocalBase(): boolean {
  if (!base) return true;
  try {
    return base === location.origin;
  } catch {
    return false;
  }
}

/** Prefix a REST path with the selected server. */
export function gameUrl(path: string): string {
  return base ? base + path : path;
}

/**
 * Where the ruleset modules live. The local bundled path is the classic
 * single-server set; it applies with no explicit build, including when this
 * page's own origin is selected, because that origin served the SPA and its
 * bundle. A build (even on this origin) reads that build's served modules.
 */
export function rulesBase(): string {
  if (isLocalBase() && !build) return "/assets/rules/";
  return `${base}/builds/${encodeURIComponent(build || "default")}/rules/`;
}

/** Where the game tables live (`board.json`, `match_rules.json`, ...). */
export function dataBase(): string {
  if (isLocalBase() && !build) return "/data/";
  return `${base}/builds/${encodeURIComponent(build || "default")}/data/`;
}

/** Resolve a possibly root-relative URL against this page -- workers need an
 *  absolute URL. */
export function absolute(url: string): string {
  if (typeof location === "undefined") return url;
  try {
    return new URL(url, location.href).href;
  } catch {
    return url;
  }
}

// ---------------------------------------------------------------- react binding

export const subscribeServer = (cb: () => void): (() => void) => {
  listeners.add(cb);
  return () => listeners.delete(cb);
};
const serverVersion = (): number => version;

export interface ServerState {
  /** Absolute base URL; `""` = this page's origin. */
  base: string;
  /** Selected build id; `""` = default. */
  build: string;
  /** Human name of the selected build, when known. */
  buildName: string;
}

/** Re-render on a server / build change (mirrors `core/hooks.ts`). */
export function useServer(): ServerState {
  useSyncExternalStore(subscribeServer, serverVersion, serverVersion);
  return { base, build, buildName };
}

// ---------------------------------------------------------------- probing

export interface ServerHealth {
  ok: boolean;
  version?: string;
  rooms?: number;
  builds?: number;
}

/** Ping a candidate game server before connecting to it. Never throws. */
export async function probeGameServer(url: string): Promise<ServerHealth> {
  const raw = (url ?? "").trim();
  const target = normalizeServer(raw);
  // A non-empty input that does not normalize is not addressable.
  if (raw && !target) return { ok: false };
  try {
    const r = await fetch(`${target}/api/health`, { credentials: "same-origin" });
    if (!r.ok) return { ok: false };
    const d = (await r.json().catch(() => null)) as { version?: string; rooms?: number; builds?: number } | null;
    return { ok: true, version: d?.version, rooms: d?.rooms, builds: d?.builds };
  } catch {
    return { ok: false };
  }
}
