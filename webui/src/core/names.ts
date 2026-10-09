// Resolve message arguments to display names against the match state / game data.

import { D, cardTitle } from "./data";
import type { Names } from "../i18n/msg";
import type { MatchPlayer, MatchState } from "./types";
import { t as tr } from "../i18n/t";

/** Value of a keyed state item ({@link StateVar}); 0 when absent. */
export const stateOf = (x: MatchPlayer, key: string): number => x.state?.[key]?.value ?? 0;

/** The mandated cap on a keyed state item; 0 when no skill has declared one. */
export const stateMax = (x: MatchPlayer, key: string): number => x.state?.[key]?.max ?? 0;

/** Use the seat's character once chosen; pre-selection seats still have users. */
export function characterName(player?: Pick<MatchPlayer, "character" | "player">): string {
  return player?.character ? D.character(player.character)?.display ?? player.character : player?.player ?? "";
}

/** In-match messages identify seats by character, tiles/cards from the data. */
export function namesOf(st?: MatchState | null): Names {
  return {
    playerId: (n) => characterName(st?.players[n]) || `#${n + 1}`,
    tile: (n) => {
      const t = D.tiles[n];
      return t ? t.name.replace(/\n/g, "") : `#${n}`;
    },
    card: (id) => cardTitle(id),
    chara: (name) => D.character(name)?.display ?? name,
    event: (id) => D.event(id)?.name ?? id,
    band: (b) => D.band(b)?.name ?? b,
  };
}

/** Only the match log's turn heading also identifies the controlling user. */
export function turnNamesOf(st?: MatchState | null): Names {
  const names = namesOf(st);
  return { ...names, playerId: (n) => {
    const player = st?.players[n];
    const who = names.playerId(n);
    return player?.character && player.player ? who + tr("common.paren", { x: player.player }) : who;
  } };
}
