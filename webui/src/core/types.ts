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
  /** Bot decision policy (`"standard"` / `"chaos"`). Ignored on a human seat. */
  mentality: BotMentality;
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
export interface FieldCard {
  uid: number; card: string; owner: number; user: number; tile: number;
  /** Miracle crystals on this instance. A band skill's count is the player's 「乐队卡 / 团卡」 pool. */
  crystals: number;
  /** On-card [CP点] -- 「自己[场上]N个[CP点]」, the CP points attached to this
   *  card (user ruling 2026-10-07). The other [CP点] kind is the tile mark
   *  (`TileMark.category === "cp"`). Shown as the field card's counter badge. */
  cp: number;
  faceDown: boolean;
  /** This instance is a band skill (`skill:<band>:<skill>`); its `crystals` are the band-card pool. */
  bandSkill?: boolean;
  note: Msg;
}
export interface SkillAction { id: string; source: string; title: Msg; text: Msg; enabled: boolean; reason: Msg }
export interface ActiveEvent { id: string; playerId: number; counter: number; counter2: number; note: Msg; faceDown: boolean }
/** A tile marker. `category` sorts it: `""` is a player/generic mark (coloured
 *  by `owner`'s seat), `"cp"` is a neutral [CP点] -- `owner` is then -1 and the
 *  provenance is `src` (the placing card instance) / `card` (its id, 「来自」). */
export interface TileMark {
  uid: number; tile: number; kind: string; category: string; owner: number;
  count: number; card: string; src: number; note: Msg;
}
export interface MatchPrompt {
  id: number; kind: string; title: Msg; text: Msg; card: string; options: Msg[];
  fallback: number; players: number[]; answers: number[]; timeLeft: number; tile: number;
  bid: number; bidder: number; items: string[]; count: number;
  /** The quoted price on a buy / force_buy prompt; -1 when not a purchase. */
  price: number;
  /** Per-option prices on an agent prompt, parallel to `options`. */
  prices: number[];
}
export interface MatchVote { id: number; by: number; players: number[]; answers: number[]; timeLeft: number }
export interface MatchEvent {
  id: number; type: string; playerId: number; other: number; value: number;
  from: number; to: number; dice: number; card: string; msg: Msg;
  /** A `card` activation's trigger kind: play | skill | event | counter | hook.
   *  Empty on every other event type (serde-defaulted on the wire). */
  kind?: string;
  /** A `card` activation a counteraction negated: the card still flashes,
   *  marked 无效, but its body did not run. */
  negated?: boolean;
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
  /** Would `act {act:"buy"}` be accepted now (`why_not_act`'s buy branch). */
  canBuyHere: boolean;
  /** Would `act {act:"build"}` be accepted now (`why_not_act`'s build branch). */
  canBuildHere: boolean;
  /** Would `act {act:"roll"}` be accepted now (`why_not_act`'s roll branch). */
  canRollHere: boolean;
  /** Would `act {act:"end"}` be accepted now (`why_not_act`'s end branch). */
  canEndHere: boolean;
  /** The movement this turn is taking. */
  plan: MovePlan;
  timeLeft: number; shield: number; bank: number;
  bought: boolean; built: boolean; players: MatchPlayer[]; bans: string[]; owners: number[]; houses: number[];
  mortgaged: boolean[]; embers: number[]; marks: TileMark[];
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
  /**
   * The engine's own AI answer for *this* player's live prompt, only while they
   * are still waiting on it (`Match::view_extra`). Never another seat's entry.
   * Absent online when the frame predates the field; the autopilot then falls
   * back to the prompt's `fallback`.
   */
  aiAnswer?: { answer: number; picked: string[]; worth: number } | null;
  /** Parallel to `hand`: would the engine allow each card right now? */
  playable?: boolean[];
  /**
   * Parallel to `hand`: the card's bot-only **estimated execution cost**
   * (`prop::EST_COST`, user ruling 2026-10-07). Only bots / autopilot read it,
   * as a reserve check -- never legality. `0` = unknown / assume free.
   */
  estCost?: number[];
}

export interface ScoreWeights { money: number; property: number; houses: number }
/**
 * Bot decision policy: `standard` is the ported C# bot, `chaos` is legal but
 * maximally disruptive, `advanced` is the server-side search bot
 * (`docs/BOT.md` B5 -- online rooms only; solo stays standard / chaos until
 * the browser bundle lands). Serde-defaults to `standard` on the wire, so a
 * missing field is standard. Only meaningful on a `bot` member. With no
 * `bot-service` attached the server rewrites `advanced` to `standard` when
 * the match starts.
 */
export type BotMentality = "standard" | "chaos" | "advanced";
export interface RoomMember {
  id: number; player: string; character: string; cnId: string;
  ready: boolean; host: boolean; bot: boolean; away: boolean;
  mentality: BotMentality;
}
export interface RoomInfo {
  id: string; name: string; ranked: boolean; maxPlayers: number; locked: boolean;
  playing: boolean; theme: string; weights: ScoreWeights; members: RoomMember[];
  /** Commit-reveal fairness (`docs/FAIRNESS.md`): the commitment is public
   *  from the moment the room starts; the openings never travel here. */
  fair?: FairPublic;
}

/** What a client may see of the commit-reveal state during the match. */
export interface FairPublic {
  /** SHA-256 commitment (hex), copyable in the match UI. */
  commit: string;
  /** Canonical room settings string the commitment hashed. */
  settings: string;
  /** True while the player-nonce window is still open. */
  collecting: boolean;
}

/** A match command (NetMessage); only the fields a command uses matter. */
export interface Command {
  act: string; character?: string; card?: string; cards?: string[];
  value?: number; prompt?: number; target?: number;
  /** Replayable solo console operation. Rejected by the engine online. */
  debug?: string;
}

export interface MatchRecord {
  time: string; mode: number; ranked: boolean; rank: number; players: number; character: string;
  exp: number; fireUsed: number; coins: number; stars: number; levelAfter: number;
}
/** `slot` is the stable deck id within the character; `name` is UI-only
 * metadata ("" = auto, displayed as 「卡组 n」/"Deck n"). Never sent in a match. */
export interface SavedDeck { character: string; slot: number; name: string; cards: string[] }
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
export interface SoundSettings { bgm: number; voice: number; se: number; skipLine: boolean; greet: boolean; idleTalk: boolean; skillTextSimple: boolean; keepAwake: boolean }
