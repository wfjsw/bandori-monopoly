// Which i18n key carries a wire state key's display phrase. Kept free of the
// i18n runtime so it can be unit-tested with a bare i18next instance (the
// markers list on a player resolves through here).
//
// A wire state key is not its own display phrase. Resolution order:
//
//   1. `board.state.<key>` in `ui` -- a client-side override
//   2. the shared log phrase (`log.state.*`, the same words
//      `game_rules::trigger_why::state_key_name` puts in the log)
//   3. the key itself as a `game` key -- `skill.*` scratch names and kin
//      already carry their phrase there (tools/i18n/check.py pins those)
//
// A key that resolves to nothing falls through to `fallbackLabel`: a bare wire
// key is dropped (the markers list never prints it), anything else (a mark
// kind's own display name) stands as itself.

/** Standard engine state keys share the log's display phrase. Scratch keys the
 *  log leaves silent (`outcome_state` returns early on them) still sit in
 *  `MatchPlayer::state` and need a phrase here. */
export const STATE_LOG: Record<string, string> = {
  handLimit: "log.state.hand_limit",
  charge: "log.state.charge",
  stay: "log.state.stay",
  stun: "log.state.stun",
  stunStart: "log.state.stun",
  exile: "log.state.exile",
  exileTo: "log.state.exile",
  exileMain: "log.state.exile",
  exileHold: "log.state.exile",
  noHand: "log.state.no_hand",
  unstoppable: "log.state.unstoppable",
  startHand: "log.state.start_hand",
  skillState: "log.state.skill_state",
  // Not rulebook state (`state_key_name` leaves these to `log.state.other`),
  // but the markers list still shows them.
  lastWalk: "log.state.last_walk",
  "noRoad.who": "log.state.no_road_who",
};

/** The `game`-namespace key a wire state key's display phrase lives under. */
export function stateKeyOf(key: string): string {
  return STATE_LOG[key] ?? key;
}

/** A bare wire key (`charge`, `sports_talent_next`, `fate_hit`, `layer.keep`):
 *  snake/camel segments joined by dots. One of these with no phrase is
 *  bookkeeping the player must never see spelled out. A mark kind or kin is
 *  not one -- 「水母标记」 carries its display name in the spelling itself. */
const WIRE_KEY = /^[A-Za-z][A-Za-z0-9_]*([.][A-Za-z0-9_]+)*$/;

/** What the markers list shows for a key that resolved to no phrase: `null`
 *  drops the row for a bare wire key, anything else is already its display
 *  name and stands as-is. */
export function fallbackLabel(key: string): string | null {
  return WIRE_KEY.test(key) ? null : key;
}
