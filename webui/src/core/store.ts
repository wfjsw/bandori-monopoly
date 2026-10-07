// Profile and sound settings, persisted in localStorage. All game logic (fire refill,
// EXP, rewards, deck slots) runs in the shared Rust code via wasm.

import { rules } from "./data";
import type { MatchReward, PlayerProfile, SoundSettings } from "./types";

const PROFILE_KEY = "bm.profile";
const SETTINGS_KEY = "bm.settings";

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
  localStorage.setItem(PROFILE_KEY, json);
  profile = JSON.parse(json);
  listeners.forEach((cb) => cb());
  return profile!;
}

/** The saved profile (repaired/migrated, fire refreshed for today), or null. */
export function loadProfile(): PlayerProfile | null {
  const raw = localStorage.getItem(PROFILE_KEY);
  if (!raw) return null;
  try {
    const p = rules.profile_refresh_daily(rules.profile_load(raw), today());
    return save(p);
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
  return save(rules.profile_refresh_daily(rules.profile_create(name, id, now(), today()), today()));
}

export function updateProfile(json: string): PlayerProfile {
  return save(json);
}

export function patchProfile(patch: Partial<PlayerProfile>): PlayerProfile {
  return save(JSON.stringify({ ...getProfile(), ...patch }));
}

export function deleteProfile(): void {
  localStorage.removeItem(PROFILE_KEY);
  profile = null;
  listeners.forEach((cb) => cb());
}

/** Record a finished match; returns what it earned. */
export function applyMatch(mode: number, rank: number, players: number, character: string): MatchReward {
  const r = JSON.parse(rules.profile_apply_match(profileJson(), mode, rank, players, character, now()));
  save(JSON.stringify(r.profile));
  return r.reward;
}

export function hasNew(what: "gallery" | "deck" | "rules" | "history" | "any"): boolean {
  return profile ? rules.profile_has_new(profileJson(), what) : false;
}

export function markSeen(what: "gallery" | "deck" | "rules" | "history"): void {
  if (profile) save(rules.profile_mark_seen(profileJson(), what));
}

// ------------------------------------------------------------------ settings

const DEFAULT_SETTINGS: SoundSettings = { bgm: 5, voice: 10, se: 7, skipLine: true, greet: true, idleTalk: true, skillTextSimple: true, keepAwake: true };

export function settings(): SoundSettings {
  try {
    return { ...DEFAULT_SETTINGS, ...JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "{}") };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export function saveSettings(s: SoundSettings): void {
  localStorage.setItem(SETTINGS_KEY, JSON.stringify(s));
  listeners.forEach((cb) => cb());
}
