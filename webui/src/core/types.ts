// Mirrors of the Rust serde types (crates/game-core). Field names are the JSON
// names. All display text is a localizable `Msg` (see i18n/msg.ts), not a string.

import type { Msg } from "../i18n/msg";

export interface TileData {
  index: number; name: string; shortName: string; kind: string; group: number;
  color: string; tier: number; area: string; price: number; house: number; rent: number[]; note: string;
}
export interface CardData {
  id: string; band: string; name: string; rawName: string; owner: string;
  derived: boolean; tags: string[]; text: string; cell: string;
}
export interface CharacterData {
  name: string; display: string; band: string; color: string; skill: string; text: string;
  /** Simplified skill body, from `skill_simple.json` (see {@link skillText}). */
  simple?: string;
  cnId: string; art: string; costume: string; exclusiveCards: string[]; preset: string[];
}
export interface BandData {
  name: string; shortName: string; color: string; skill: string; text: string;
  /** Simplified skill body, from `skill_simple.json` (see {@link skillText}). */
  simple?: string;
  logo: string;
}
export interface EventData { id: string; name: string; derived: boolean; text: string; cell: string }
export interface VoiceLine { text: string; voice: string; motion: string; from: string }

export interface MatchPlayer {
  member: number; player: string; bot: boolean; ai: boolean; roll: number; banDone: boolean; ban: string;
  character: string; deckReady: boolean; money: number; pos: number; hand: number; draw: number;
  discard: string[]; mulligan: boolean; bankrupt: boolean; left: boolean; outOrder: number;
  assets: number; score: number; rank: number;
  /** Keyed state: the counters this player carries. See {@link StateVar}. */
  state: Record<string, StateVar>;
  skillCharacter: string; bands: string; tokens: { name: string; value: number }[];
  skillNote: Msg; field: FieldCard[]; actions: SkillAction[];
}
/**
 * One keyed state item: `value` plus the bounds a consumer may enforce, and
 * when it wears off. The engine holds these and enforces nothing -- a skill
 * that mandates a fire-pot cap writes `max`, and whoever moves the value is
 * free to honour it.
 */
export interface StateVar {
  value: number; min: number; max: number;
  expires?: "turnStart" | "turnEnd" | null;
}
export interface FieldCard { uid: number; card: string; owner: number; user: number; tile: number; crystals: number; faceDown: boolean; note: Msg }
export interface SkillAction { id: string; source: string; title: Msg; text: Msg; enabled: boolean; reason: Msg }
export interface ActiveEvent { id: string; playerId: number; counter: number; counter2: number; note: Msg; faceDown: boolean }
export interface TileMark { uid: number; tile: number; kind: string; owner: number; count: number; card: string; note: Msg }
export interface MatchPrompt {
  id: number; kind: string; title: Msg; text: Msg; card: string; options: Msg[];
  fallback: number; players: number[]; answers: number[]; timeLeft: number; tile: number;
  bid: number; bidder: number; items: string[]; count: number;
}
export interface MatchVote { id: number; by: number; players: number[]; answers: number[]; timeLeft: number }
export interface MatchEvent {
  id: number; type: string; playerId: number; other: number; value: number;
  from: number; to: number; dice: number; card: string; msg: Msg;
}
/** C# `MovePlan` -- the movement the current turn is taking. `reach[k]` is the
 * tile after k+1 steps; `steps` is how far along it the seat has got. */
export interface MovePlan {
  playerId: number; from: number; steps: number; started: boolean; reach: number[];
  /** Whether this move can build where it lands. */
  canBuild: boolean;
}
export interface MatchState {
  phase: string; matchId: number; seq: number; mode: number; turn: number; round: number; step: number;
  debugOpen: boolean;
  roller: number; busy: boolean; skipMove: boolean; landed: number;
  /** ThinkTime room setting: 0 Standard / 1 Relaxed / 2 VeryRelaxed / 3 Unlimited. */
  thinkTime: number;
  /** Preview of the price to buy where the turn stands; -1 = nothing buyable. */
  buyPrice: number;
  /** The same preview for building; -1 = cannot build. */
  buildCost: number;
  /** The movement this turn is taking. */
  plan: MovePlan;
  timeLeft: number; shield: number; bank: number;
  bought: boolean; built: boolean; players: MatchPlayer[]; bans: string[]; owners: number[]; houses: number[];
  mortgaged: boolean[]; embers: number[]; marks: TileMark[]; tileColors: number[];
  eventDeck: number; eventTop: string[]; eventDiscard: string[]; eventActive: ActiveEvent[]; prompt: MatchPrompt; vote: MatchVote; events: MatchEvent[];
  endReason: string; winner: number; scoreMoney: number; scoreProperty: number; scoreHouses: number;
}
/** What one player sees: the shared state plus their own hand and deck. */
export interface MatchView {
  state: MatchState;
  hand: string[];
  handNotes: Msg[];
  /** Remaining draw pile, sorted by card id -- the draw order never leaves the engine. */
  draw: string[];
  you: number;
  playerId: number;
}

export interface ScoreWeights { money: number; property: number; houses: number }
export interface RoomMember {
  id: number; player: string; character: string; cnId: string;
  ready: boolean; host: boolean; bot: boolean; away: boolean;
}
export interface RoomInfo {
  id: string; name: string; ranked: boolean; maxPlayers: number; locked: boolean;
  playing: boolean; theme: string; weights: ScoreWeights; members: RoomMember[];
}

/** A match command (NetMessage); only the fields a command uses matter. */
export interface Command {
  act: string; character?: string; card?: string; cards?: string[];
  value?: number; prompt?: number; target?: number;
}

export interface MatchRecord {
  time: string; mode: number; ranked: boolean; rank: number; players: number; character: string;
  exp: number; fireUsed: number; coins: number; stars: number; levelAfter: number;
}
export interface SavedDeck { character: string; slot: number; cards: string[] }
export interface PlayerProfile {
  saveVersion: number; playerName: string; playerId: string; createdAt: string;
  level: number; exp: number; totalExp: number; fire: number; fireDate: string; firePerGame: number;
  coins: number; stars: number; homeCharacter: string; games: number; soloGames: number;
  casualGames: number; rankedGames: number; rankedWins: number;
  history: MatchRecord[]; unreadResult: boolean; decks: SavedDeck[];
  [k: string]: unknown;
}
export interface MatchReward {
  mode: number; rank: number; players: number; baseExp: number; fireUsed: number; multiplier: number;
  exp: number; levelBefore: number; levelAfter: number; progressBefore: number; progressAfter: number;
  coins: number; stars: number;
}
export interface SoundSettings { bgm: number; voice: number; se: number; skipLine: boolean; greet: boolean; idleTalk: boolean; skillTextSimple: boolean }
