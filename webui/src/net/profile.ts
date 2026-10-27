// Client for the profile server: its game-server directory, game-data uploads
// and an optional account session.
//
// The profile server is a *different service* from the game server. Its token
// is used here only (uploads), and is never forwarded to a game server; the
// game server has its own per-server session (`net/api.ts`). Its address is
// separate too (`bm.profileServer`, `""` = this page's origin). Everything here
// tolerates the service being absent: reads answer offline-shaped results and
// writes answer `null`, never throw.

/** Profile-server base URL; absent = this page's origin. */
const PROFILE_BASE_KEY = "bm.profileServer";
const PROFILE_TOKEN_PREFIX = "bm.profileToken:";

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
    /* private mode / quota */
  }
}

/** Same address rules as a game server: http(s), no trailing slash. */
function normalizeProfileBase(raw: string | null | undefined): string {
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

let base = normalizeProfileBase(read(PROFILE_BASE_KEY));

/** The profile-server base URL (`""` = this page's origin). */
export function profileBase(): string {
  return base;
}

/** Point at a profile server (or back at this origin with `null`/""). */
export function setProfileBase(url: string | null): void {
  base = normalizeProfileBase(url);
  write(PROFILE_BASE_KEY, base);
}

/** A directory entry (`GET /api/servers`). */
export interface ProfileGameServer {
  id: string;
  name: string;
  url: string;
  note: string;
}

/** One uploaded blob (`POST /api/uploads`). */
export interface UploadView {
  id: string;
  name: string;
  kind: string;
  size: number;
  sha256: string;
  owner: string;
  created: number;
  /** Path relative to the profile server (`/uploads/<id>/<name>`). */
  url: string;
  download: string;
}

function profileUrl(path: string): string {
  return base ? base + path : path;
}

/** Tokens are cached per profile-server base, in this tab only. */
const tokens = new Map<string, string>();

function tokenKey(): string {
  return PROFILE_TOKEN_PREFIX + (base || "local");
}

/**
 * Sign in to the profile server by display name and cache the token for this
 * tab. The token is per profile base. Offline / absent service = `null`.
 */
export async function profileSession(player: string): Promise<string | null> {
  const cached = tokens.get(base) ?? sessionStorage.getItem(tokenKey()) ?? "";
  if (cached) return cached;
  try {
    const r = await fetch(profileUrl("/api/session"), {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ player }),
    });
    if (!r.ok) return null;
    const d = (await r.json().catch(() => null)) as { token?: unknown } | null;
    const token = typeof d?.token === "string" ? d.token : "";
    if (!token) return null;
    tokens.set(base, token);
    sessionStorage.setItem(tokenKey(), token);
    return token;
  } catch {
    return null;
  }
}

/** The game servers the profile server advertises; `[]` when offline. */
export async function listServers(): Promise<ProfileGameServer[]> {
  try {
    const r = await fetch(profileUrl("/api/servers"), { credentials: "same-origin" });
    if (!r.ok) return [];
    const d = (await r.json().catch(() => null)) as unknown;
    if (!Array.isArray(d)) return [];
    return d.filter((x): x is ProfileGameServer => !!x && typeof x === "object" && typeof (x as ProfileGameServer).url === "string");
  } catch {
    return [];
  }
}

export interface ProfileHealth {
  ok: boolean;
  version?: string;
  service?: string;
}

/** `GET /api/health` on the profile server; never throws. */
export async function health(): Promise<ProfileHealth> {
  try {
    const r = await fetch(profileUrl("/api/health"), { credentials: "same-origin" });
    if (!r.ok) return { ok: false };
    const d = (await r.json().catch(() => null)) as { version?: string; service?: string } | null;
    return { ok: true, version: d?.version, service: d?.service };
  } catch {
    return { ok: false };
  }
}

/**
 * Upload one game-data blob (raw body) to the profile server.
 *
 * The upload is a convenience (sharing a ruleset), never a prerequisite: any
 * failure -- no profile server, no account, too large, transport -- answers
 * `null` and the caller keeps its local copy.
 */
export async function uploadGameData(file: Blob, kind: string): Promise<UploadView | null> {
  let player = "";
  try {
    // Imported lazily so the upload path does not pull the wasm/data boot into
    // modules that only render the server list.
    const store = await import("../core/store");
    player = store.getProfile().playerName;
  } catch {
    return null;
  }
  if (!player) return null;
  const token = await profileSession(player);
  if (!token) return null;
  const name = typeof (file as File).name === "string" && (file as File).name ? (file as File).name : `upload-${Date.now()}`;
  try {
    const r = await fetch(
      profileUrl(`/api/uploads?name=${encodeURIComponent(name)}&kind=${encodeURIComponent(kind)}`),
      {
        method: "POST",
        credentials: "same-origin",
        headers: {
          Authorization: `Bearer ${token}`,
          "Content-Type": "application/octet-stream",
        },
        body: file,
      },
    );
    if (!r.ok) return null;
    return (await r.json().catch(() => null)) as UploadView | null;
  } catch {
    return null;
  }
}
