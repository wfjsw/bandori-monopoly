// Named decks: the `rules.deck_list` entry shape and display-name resolution.
//
// Deck names are profile-only UI metadata -- a match (and its record/replay)
// carries just the card list, so nothing here crosses into match state.
// Pure on purpose (`msg.test.ts` convention): the localized auto name is
// injected, so this runs under `node --test` without the i18n runtime.

/** One entry of `rules.deck_list(profile, character)`. */
export interface DeckEntry {
  /** Stable deck id within the character (0 is the preset, never listed). */
  id: number;
  /** User-editable label; "" means auto (localized "Deck {id}" / 「卡组 {id}」). */
  name: string;
  cards: string[];
}

/** Max characters in a deck name (mirrors game-core `DECK_NAME_MAX`). */
export const DECK_NAME_MAX = 24;

export function parseDeckList(json: string): DeckEntry[] {
  const list = JSON.parse(json) as unknown;
  return Array.isArray(list) ? (list as DeckEntry[]) : [];
}

/** Trim and cap what the rename field sends; "" restores the auto name. */
export function sanitizeDeckName(name: string): string {
  const t = name.trim();
  return t === "" ? "" : [...t].slice(0, DECK_NAME_MAX).join("");
}

/** Display name: the user's name, or the auto name from the deck id. */
export function deckLabelWith(d: { id: number; name: string }, autoName: (n: number) => string): string {
  const n = (d.name ?? "").trim();
  return n === "" ? autoName(d.id) : n;
}