// Read-only helpers over a match view, shared by the board's components and
// popups.

import { D } from "../../core/data";
import type { CharacterData, Command, MatchSeat, MatchState, MatchView } from "../../core/types";
import type { GameSession } from "../../game/session";
import { toast } from "../../ui/Toast";
import { fmtMsg } from "../../i18n/msg";
import { namesOf } from "../../core/names";
import { t as tr } from "../../i18n/t";

const FALLBACK_COLORS = ["#ED4E76", "#6BBAF1", "#F6B21E", "#4CC38A", "#C97DFC", "#EE5E4A", "#3EC1C9", "#E05CCF", "#8BBF2A", "#7C8796"];
export const modeName = (mode: number) => [tr("mode.solo"), tr("mode.casual"), tr("mode.ranked")][mode] ?? "";
export const RING_MULTIPLIER = 10;

export interface Model {
  v: MatchView;
  S: MatchState;
  seat: number;
  me: MatchSeat;
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
  const me = S.seats[v.seat];
  const charOf = (i: number) => D.character(S.seats[i]?.character ?? "");
  return {
    v,
    S,
    seat: v.seat,
    me,
    myTurn: S.phase === "play" && S.turn === v.seat,
    asking: S.prompt.id > 0,
    out: me.bankrupt || me.left,
    charOf,
    colorOf: (i) => charOf(i)?.color ?? FALLBACK_COLORS[i % FALLBACK_COLORS.length],
    nameOf: (i) => (i === v.seat ? tr("common.you") : S.seats[i]?.player ?? ""),
    overHand: v.hand.length > (me.handLimit || 5),
  };
}

/** Send a command; toast the error. Resolves to success. */
export async function act(sess: GameSession, cmd: Command): Promise<boolean> {
  const err = await sess.act(cmd);
  if (err) toast(fmtMsg(err, namesOf(sess.view?.state)), "error");
  return !err;
}

export function buyable(m: Model, i: number): boolean {
  const t = D.tiles[i];
  const S = m.S;
  return m.myTurn && S.step === 3 && !S.busy && !S.bought && (t.kind === "property" || t.kind === "ring") && S.landed === i && m.me.pos === i && S.owners[i] < 0;
}

export function canBuildOn(m: Model, i: number): boolean {
  const S = m.S;
  const t = D.tiles[i];
  return S.owners[i] === m.seat && t.kind === "property" && t.rent.length > 1 && !S.mortgaged[i] && S.houses[i] < t.rent.length - 1
    && m.myTurn && S.step === 3 && S.landed === i && !S.bought && !S.built;
}

export const mortgageValue = (i: number) => Math.floor(D.tiles[i].price / 2);
export const redeemCost = (i: number) => Math.round(D.tiles[i].price * 0.6);
