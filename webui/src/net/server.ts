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

/** Absolute game-server base URL; absent = this page's origin. */
const SERVER_KEY = "bm.gameServer";
/** Selected build id; absent = the server's default build. */
const BUILD_KEY = "bm.build";
/** Human name of the selected build (the pill's label; the id is the truth). */
const BUILD_NAME_KEY = "bm.buildName";

function read(key: string): string {
  try {
    return localStorage.getItem(key) ?? "";
  } catch {
    return "";
  }
}

function write(key: string, value: string): void {
  try {
    if (value) localStorage.setItem(key, value);
    else localStorage.removeItem(key);
  } catch {
    /* private mode / quota: the in-memory value is still this page's */
  }
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

/** The game-server base URL (`""` = this page's origin). */
export function serverBase(): string {
  return base;
}

/**
 * Point the page at another game server (or back at this origin with `null`).
 * A build id only means something on the server it was chosen on, so it is
 * cleared with the server. Callers reload the page afterwards: boot-time data
 * is keyed to this choice.
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

/** Select a rule build (`null`/`""` = the server's default). */
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

/** Prefix a REST path with the selected server. */
export function gameUrl(path: string): string {
  return base ? base + path : path;
}

/**
 * Where the ruleset modules live. With neither a server nor a build selected
 * this is the classic bundled path, unchanged; any explicit choice (a build on
 * this same-origin server included) reads that build's served modules.
 */
export function rulesBase(): string {
  if (!base && !build) return "/assets/rules/";
  return `${base}/builds/${encodeURIComponent(build || "default")}/rules/`;
}

/** Where the game tables live (`board.json`, `match_rules.json`, ...). */
export function dataBase(): string {
  if (!base && !build) return "/data/";
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
