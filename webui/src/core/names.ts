// Resolve message arguments to display names against the match state / game data.

import { D, cardTitle } from "./data";
import type { Names } from "../i18n/msg";
import type { MatchPlayer, MatchState } from "./types";
import { t as tr } from "../i18n/t";

/** Value of a keyed state item ({@link StateVar}); 0 when absent. */
export const stateOf = (x: MatchPlayer, key: string): number => x.state?.[key]?.value ?? 0;

/** The mandated cap on a keyed state item; 0 when no skill has declared one. */
export const stateMax = (x: MatchPlayer, key: string): number => x.state?.[key]?.max ?? 0;

/** The `"charge"` row of {@link MatchPlayer.tokens} (fire pots, 「火罐」);
 *  absent on saves/replays that still keep them in `state["charge"]`. */
const chargeTok = (x: MatchPlayer) => x.tokens?.find((t) => t.name === "charge");

/** Fire pots held: the `"charge"` token after the instance re-home, falling
 *  back to `state["charge"]` for saves that predate the move. */
export const fireOf = (x: MatchPlayer): number =>
  chargeTok(x)?.value ?? stateOf(x, "charge");

/** Fire-pot ceiling (「上限」); 0 = uncapped. Prefers the token row's `max`. */
export const fireMaxOf = (x: MatchPlayer): number => {
  const tok = chargeTok(x);
  return tok ? tok.max ?? 0 : stateMax(x, "charge");
};

/** Use the seat's persona once chosen; pre-selection seats still have users. */
export function personaName(player?: Pick<MatchPlayer, "persona" | "player">): string {
  return player?.persona ? D.persona(player.persona)?.display ?? player.persona : player?.player ?? "";
}

/** In-match messages identify seats by persona, tiles/cards from the data. */
export function namesOf(st?: MatchState | null): Names {
  return {
    playerId: (n) => personaName(st?.players[n]) || `#${n + 1}`,
    tile: (n) => {
      const t = D.tiles[n];
      return t ? t.name.replace(/\n/g, "") : `#${n}`;
    },
    card: (id) => cardTitle(id),
    persona: (name) => D.persona(name)?.display ?? name,
    event: (id) => D.event(id)?.name ?? id,
    group: (b) => D.group(b)?.name ?? b,
  };
}

/** Only the match log's turn heading also identifies the controlling user. */
export function turnNamesOf(st?: MatchState | null): Names {
  const names = namesOf(st);
  return { ...names, playerId: (n) => {
    const player = st?.players[n];
    const who = names.playerId(n);
    return player?.persona && player.player ? who + tr("common.paren", { x: player.player }) : who;
  } };
}
