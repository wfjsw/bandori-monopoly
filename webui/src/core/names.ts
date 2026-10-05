// Resolve message arguments to display names against the match state / game data.

import { D, cardTitle } from "./data";
import type { Names } from "../i18n/msg";
import type { MatchPlayer, MatchState } from "./types";

/** Value of a keyed state item ({@link StateVar}); 0 when absent. */
export const stateOf = (x: MatchPlayer, key: string): number => x.state?.[key]?.value ?? 0;

/** The mandated cap on a keyed state item; 0 when no skill has declared one. */
export const stateMax = (x: MatchPlayer, key: string): number => x.state?.[key]?.max ?? 0;

/** Names for a match state: players are player names, tiles/cards from the data. */
export function namesOf(st?: MatchState | null): Names {
  return {
    playerId: (n) => st?.players[n]?.player ?? `#${n + 1}`,
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