// Read-only helpers over a match view, shared by the board's components and
// popups.

import { D } from "../../core/data";
import type { CharacterData, Command, MatchPlayer, MatchState, MatchView } from "../../core/types";
import type { GameSession } from "../../game/session";
import { toast } from "../../ui/Toast";
import { fmtMsg } from "../../i18n/msg";
import { namesOf, stateOf } from "../../core/names";
import { t as tr } from "../../i18n/t";

const FALLBACK_COLORS = ["#ED4E76", "#6BBAF1", "#F6B21E", "#4CC38A", "#C97DFC", "#EE5E4A", "#3EC1C9", "#E05CCF", "#8BBF2A", "#7C8796"];
export const modeName = (mode: number) => [tr("mode.solo"), tr("mode.casual"), tr("mode.ranked")][mode] ?? "";
export const RING_MULTIPLIER = 10;

export interface Model {
  v: MatchView;
  S: MatchState;
  playerId: number;
  me: MatchPlayer;
  myTurn: boolean;
  asking: boolean;
  out: boolean;
  charOf(i: number): CharacterData | undefined;
  colorOf(i: number): string;
  /** tr("common.you") for yourself. */
  nameOf(i: number): string;
  overHand: boolean;
}

export function model(v: MatchView): Model {
  const S = v.state;
  // A replay's spectator perspective has no seat (`playerId` -1); give the
  // read-only chrome an empty player rather than crash on `me.*`.
  const me = S.players[v.playerId] ?? blankPlayer();
  const charOf = (i: number) => D.character(S.players[i]?.character ?? "");
  return {
    v,
    S,
    playerId: v.playerId,
    me,
    myTurn: v.playerId >= 0 && S.phase === "play" && S.turn === v.playerId,
    asking: S.prompt.id > 0,
    out: v.playerId < 0 || me.bankrupt || me.left,
    charOf,
    colorOf: (i) => charOf(i)?.color ?? FALLBACK_COLORS[i % FALLBACK_COLORS.length],
    nameOf: (i) => (i === v.playerId && i >= 0 ? tr("common.you") : namesOf(S).playerId(i)),
    overHand: v.hand.length > (stateOf(me, "handLimit") || 5),
  };
}

/** A zeroed seat, for the spectator perspective (no `me` in the state). */
function blankPlayer(): MatchPlayer {
  return {
    member: 0,
    player: "",
    bot: false,
    ai: false,
    roll: 0,
    banDone: false,
    ban: "",
    character: "",
    deckReady: false,
    money: 0,
    pos: 0,
    hand: 0,
    draw: 0,
    discard: [],
    mulligan: false,
    bankrupt: false,
    left: false,
    outOrder: 0,
    assets: 0,
    score: 0,
    rank: 0,
    state: {},
    skillCharacter: "",
    bands: "",
    tokens: [],
    skillNote: { k: "" },
    field: [],
    actions: [],
    mentality: "standard",
  };
}

/**
 * Send a command; toast the error. Resolves to success.
 * While 托管 / 混沌 is on the seat belongs to the autopilot, so a user-driven
 * command is dropped here (the buttons that would send one are disabled too).
 * The autopilot itself calls `sess.act` directly and bypasses this guard.
 */
export async function act(sess: GameSession, cmd: Command): Promise<boolean> {
  if (sess.readOnly || sess.autoMode !== "off") return false;
  const err = await sess.act(cmd);
  if (err) toast(fmtMsg(err, namesOf(sess.view?.state)), "error");
  return !err;
}

export function buyable(m: Model, i: number): boolean {
  const t = D.tiles[i];
  const S = m.S;
  return m.myTurn && S.step === 4 && !S.busy && !S.bought && (t.kind === "property" || t.kind === "ring") && S.landed === i && m.me.pos === i && S.owners[i] < 0;
}

export function canBuildOn(m: Model, i: number): boolean {
  const S = m.S;
  const t = D.tiles[i];
  return S.owners[i] === m.playerId && t.kind === "property" && t.rent.length > 1 && !S.mortgaged[i] && S.houses[i] < t.rent.length - 1
    && m.myTurn && S.step === 4 && S.landed === i && !S.bought && !S.built;
}

export const mortgageValue = (i: number) => Math.floor(D.tiles[i].price / 2);
export const redeemCost = (i: number) => Math.round(D.tiles[i].price * 0.6);
