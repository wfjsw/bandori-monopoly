// How a card is still titled when its trimmed `name` is empty.
//
// Pure on purpose (`deckNames.ts` convention): the "unnamed" label is injected,
// so this runs under `node --test` without the i18n runtime. `cardTitle` in
// `data.ts` owns the skill / event / tile lookups that sit in front of this.

/** One card row's display fields. */
export interface CardFace {
  /** Trimmed display name (the sheet's `name` column). */
  name?: string;
  /** Untrimmed sheet name (`rawName`) -- may be just the character prefix. */
  rawName?: string;
}

/** The title a log line / card face shows for `card`.
 *
 *  A card with no `name` is still known by its sheet's untrimmed `rawName`
 *  (「（美咲）」 / 「（喵梦）」 -- the prefix *is* the title). A row whose sheet
 *  name line was blank is not nameless either: its `name` carries the
 *  sheet-cell handle (`#J11`) directly. Only a row with neither falls back to
 *  the caller's `unnamed` label.
 */
export function cardFaceTitle(card: CardFace | undefined, unnamed: string): string {
  if (card?.name) return card.name;
  if (card?.rawName) return card.rawName;
  return unnamed;
}