// How a `Msg` key is looked up in i18next. Kept free of the i18n runtime so it
// can be unit-tested with a bare i18next instance.

/**
 * Engine keys have no prefix and live in `game`; card keys name their own
 * namespace (`cards:crate.key`). A colon is otherwise part of the key -- a
 * tile-mark kind is the key itself (`mark:cp`), and i18next would otherwise
 * split it as namespace `mark` and never find the `game` entry. Only a real
 * namespace prefix opts out of the `game` default.
 */
export function msgLookup(key: string): { ns: string | undefined; nsSeparator: ":" | false } {
  return key.startsWith("cards:")
    ? { ns: undefined, nsSeparator: ":" }
    : { ns: "game", nsSeparator: false };
}