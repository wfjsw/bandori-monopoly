// Display name for a keyed state item -- the markers list on a player (the
// charge-icon peek and the player info sheet). See `stateKey.ts` for how a
// wire key becomes an i18n key.

import { t as tr } from "./t";
import { fmtMsg } from "./msg";
import { stateKeyOf } from "./stateKey";

/** Display name for a keyed state item. Known keys have i18n; others show the
 *  wire key (same fallback `fmtMsg` uses for an unknown message key). */
export function stateLabel(key: string): string {
  const o = tr(`board.state.${key}`);
  if (o !== `board.state.${key}`) return o;
  const k = stateKeyOf(key);
  const s = fmtMsg({ k });
  return s === k ? key : s;
}