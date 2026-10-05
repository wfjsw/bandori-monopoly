// Resolve message arguments to display names against the match state / game data.

import { D, cardTitle } from "./data";
import type { Names } from "../i18n/msg";
import type { MatchState } from "./types";

/** Names for a match state: seats are player names, tiles/cards from the data. */
export function namesOf(st?: MatchState | null): Names {
  return {
    seat: (n) => st?.seats[n]?.player ?? `#${n + 1}`,
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