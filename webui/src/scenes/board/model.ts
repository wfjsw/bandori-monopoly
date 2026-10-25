// Read-only helpers over a match view, shared by the board's components and
// popups.

import { D } from "../../core/data";
import { overHand } from "../../core/hand";
import type { PersonaData, Command, MatchPlayer, MatchState, MatchView } from "../../core/types";
import type { GameSession } from "../../game/session";
import { toast } from "../../ui/Toast";
import { fmtMsg } from "../../i18n/msg";
import { namesOf } from "../../core/names";
import { t as tr } from "../../i18n/t";
import { canSend } from "./inputLock";
import {
  forceBuyCost as deedForceBuyCost,
  mortgageValue as deedMortgageValue,
  redeemCost as deedRedeemCost,
} from "./deedCost";

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
  charOf(i: number): PersonaData | undefined;
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
  const charOf = (i: number) => D.persona(S.players[i]?.persona ?? "");
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
    overHand: overHand(me, v.hand.length),
  };
}

/** A zeroed seat, for the spectator perspective (no `me` in the state). */
export function blankPlayer(): MatchPlayer {
  return {
    member: 0,
    player: "",
    bot: false,
    ai: false,
    roll: 0,
    banDone: false,
    ban: "",
    persona: "",
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
    skillOwner: "",
    affiliations: "",
    tokens: [],
    skillNote: { k: "" },
    field: [],
    mentality: "standard",
  };
}

/**
 * Send a command; toast the error. Resolves to success.
 * Dropped when `canSend` is false (see `inputLock`); the autopilot itself
 * calls `sess.act` directly and bypasses this guard.
 */
export async function act(sess: GameSession, cmd: Command, overAuto = false): Promise<boolean> {
  if (!canSend(sess, overAuto)) return false;
  const err = await sess.act(cmd);
  if (err) toast(fmtMsg(err, namesOf(sess.view?.state)), "error");
  return !err;
}

/** Deed economics -- see `deedCost.ts`, the single source the tile face /
 *  tooltip / deed popup all quote. Index-based wrappers over the tile data. */
export const mortgageValue = (i: number) => deedMortgageValue(D.tiles[i].price);
export const redeemCost = (i: number) => deedRedeemCost(D.tiles[i].price);
export const forceBuyCost = (i: number, houses: number) =>
  deedForceBuyCost(D.tiles[i].price, D.tiles[i].house, houses);
