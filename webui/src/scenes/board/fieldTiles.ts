// Which tiles a card in the field row is bound to (「指定」 / designate marks).
// Pure -- the row's hover-or-hold peek and its tests read the same table, so
// the hold effect can be attached only to the cards that actually have tiles.

import type { TileMark } from "../../core/types";

/**
 * Tiles the matching marks name. Empty when the card binds none -- the caller
 * then attaches no hold-to-highlight at all (`useCardTileFocus` stays off).
 */
export function designatedTiles(marks: readonly TileMark[] | undefined, hit: (m: TileMark) => boolean): number[] {
  const out: number[] = [];
  for (const x of marks ?? []) {
    if (x.category === "designate" && hit(x) && !out.includes(x.tile)) out.push(x.tile);
  }
  return out;
}

/** A field card's marks: bound to its instance (`src` / `instance` = `uid`). */
export function fieldCardTiles(uid: number): (m: TileMark) => boolean {
  return (m) => m.src === uid || m.instance === uid;
}

/**
 * The tile a `tile:*` attachment stands for -- a deed parked on its host card
 * by a rule that carried a hand copy of the tile rule. The rule card itself
 * has no face (every property would read as the same tile), so the instance
 * records the real tile in the `one_of_us_tile` prop (`tile + 1`, the usual
 * "0 is unset" guard). `-1` for a non-deed attachment and for one that lost
 * its record.
 */
export function attachedDeedTile(card: string, props: Record<string, number> | undefined): number {
  if (!card.startsWith("tile:")) return -1;
  return (props?.["one_of_us_tile"] ?? 0) - 1;
}

/**
 * An active event's marks. The event rule instance sits on the board field
 * under its rule id (`event:<id>`); `ActiveEvent.id` is the bare roster id, and
 * a mark's `card` provenance is the instance's card id. Accept either spelling
 * so a draw that carries one or the other still lights its tiles.
 */
export function eventTiles(eventId: string): (m: TileMark) => boolean {
  const bare = eventId.startsWith("event:") ? eventId.slice(6) : eventId;
  const rid = `event:${bare}`;
  return (m) => m.card === bare || m.card === rid;
}