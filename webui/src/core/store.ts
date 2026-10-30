// Profile and sound settings, persisted in the `account` store. All game logic
// (fire refill, EXP, rewards, deck slots) runs in the shared Rust code via wasm.

import { DEFAULT_HOME_PERSONA, RULES_VERSION, rules } from "./data";
import { kv } from "./kv.ts";
import type { MatchReward, PlayerProfile, SoundSettings } from "./types";

const account = kv("account");
const PROFILE_KEY = "profile";
const SETTINGS_KEY = "settings";

const pad = (n: number) => String(n).padStart(2, "0");
export function today(): string {
  const d = new Date();
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}
export function now(): string {
  const d = new Date();
  return `${today()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

let profile: PlayerProfile | null = null;
const listeners = new Set<() => void>();

export function onProfile(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

function save(json: string): PlayerProfile {
  account.set(PROFILE_KEY, json);
  profile = JSON.parse(json);
  listeners.forEach((cb) => cb());
  return profile!;
}

/** The saved profile (repaired/migrated, fire refreshed for today), or null. */
export function loadProfile(): PlayerProfile | null {
  const raw = account.get(PROFILE_KEY);
  if (!raw) return null;
  try {
    let json = rules.profile_refresh_daily(rules.profile_load(raw), today());
    // Slice-2 `profile_load` falls back to an empty home ("the client
    // substitutes its own default"); land it on the UI constant here.
    const p = JSON.parse(json) as PlayerProfile;
    if (!p.homePersona) json = JSON.stringify({ ...p, homePersona: DEFAULT_HOME_PERSONA });
    return save(json);
  } catch {
    return null;
  }
}

export function getProfile(): PlayerProfile {
  if (!profile) throw new Error("no profile");
  return profile;
}

export function profileJson(): string {
  return JSON.stringify(getProfile());
}

export function hasProfile(): boolean {
  return profile !== null;
}

export function createProfile(name: string): PlayerProfile {
  const id = String(100_000_000 + Math.floor(Math.random() * 899_999_999));
  const p = JSON.parse(rules.profile_refresh_daily(rules.profile_create(name, id, now(), today()), today())) as PlayerProfile;
  // Slice-2 `profile_create` leaves `homePersona` empty ("the client sets it");
  // a new profile also starts with the rules already acknowledged, so stamp
  // the current `RULES_VERSION` (Rust only stores a boolean flag).
  p.homePersona = DEFAULT_HOME_PERSONA;
  p.seenRulesVersion = RULES_VERSION;
  return save(JSON.stringify(p));
}

export function updateProfile(json: string): PlayerProfile {
  return save(json);
}

export function patchProfile(patch: Partial<PlayerProfile>): PlayerProfile {
  return save(JSON.stringify({ ...getProfile(), ...patch }));
}

export function deleteProfile(): void {
  account.remove(PROFILE_KEY);
  profile = null;
  listeners.forEach((cb) => cb());
}

/** Record a finished match; returns what it earned. `replayId` links the new
 *  history row to the local replay store (`game/record.ts`), when the client
 *  kept this match's `.bdrec`. */
export function applyMatch(mode: number, rank: number, players: number, character: string, replayId = ""): MatchReward {
  const r = JSON.parse(rules.profile_apply_match(profileJson(), mode, rank, players, character, now(), replayId));
  save(JSON.stringify(r.profile));
  return r.reward;
}

export function hasNew(what: "gallery" | "deck" | "rules" | "history" | "any"): boolean {
  if (!profile) return false;
  // The rules NEW badge is client-driven (slice 2: "The webui decides when the
  // content itself is new"). `RULES_VERSION` is the webui's constant; bump it
  // when the rules book changes and the badge reappears. Rust's `Seen::Rules`
  // is only a boolean acknowledge flag (`seenRulesVersion == 0`).
  const rulesNew = (profile.seenRulesVersion ?? 0) < RULES_VERSION;
  if (what === "rules") return rulesNew;
  if (what === "any") return rulesNew || rules.profile_has_new(profileJson(), "any");
  return rules.profile_has_new(profileJson(), what);
}

export function markSeen(what: "gallery" | "deck" | "rules" | "history"): void {
  if (!profile) return;
  if (what === "rules") {
    // Stamp the version we just acknowledged (Rust's mark_seen would only set
    // the boolean flag to 1 and lose the version).
    save(JSON.stringify({ ...getProfile(), seenRulesVersion: RULES_VERSION }));
    return;
  }
  save(rules.profile_mark_seen(profileJson(), what));
}

// ------------------------------------------------------------------ settings

const DEFAULT_SETTINGS: SoundSettings = { bgm: 1, voice: 2, se: 1.4, skipLine: true, greet: true, idleTalk: true, keepAwake: true };

export function settings(): SoundSettings {
  try {
    return { ...DEFAULT_SETTINGS, ...JSON.parse(account.get(SETTINGS_KEY) || "{}") };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export function saveSettings(s: SoundSettings): void {
  account.set(SETTINGS_KEY, JSON.stringify(s));
  listeners.forEach((cb) => cb());
}
