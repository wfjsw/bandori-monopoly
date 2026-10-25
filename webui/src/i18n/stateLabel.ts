// Display name for a keyed state item -- the markers list on a player (the
// charge-icon peek and the player info sheet), and a held-token or on-card
// counter name. See `stateKey.ts` for how a wire key becomes an i18n key.

import { t as tr } from "./t";
import { fmtMsg } from "./msg";
import { fallbackLabel, stateKeyOf } from "./stateKey";

/** Display name for a keyed state item. Known keys have i18n; a mark kind or
 *  kin that carries its own display name (「水母标记」, 「PAREO标记」) is that
 *  name; a bare wire key with no phrase is `null` and the caller drops the row
 *  rather than printing the key. */
export function stateLabel(key: string): string | null {
  const o = tr(`board.state.${key}`);
  if (o !== `board.state.${key}`) return o;
  const k = stateKeyOf(key);
  const s = fmtMsg({ k });
  if (s !== k) return s;
  return fallbackLabel(key);
}

/** Display name for an on-card counter. Same resolution as {@link stateLabel},
 *  but through the shared log phrase `trigger_why::counter_name` already puts
 *  in the log (`log.counter.*`). */
export function counterLabel(name: string): string | null {
  const k = `log.counter.${name}`;
  const s = fmtMsg({ k });
  if (s !== k) return s;
  return fallbackLabel(name);
}
