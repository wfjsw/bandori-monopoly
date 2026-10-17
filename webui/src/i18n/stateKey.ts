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
// An unknown key falls back to its wire spelling, never to a lookup path.

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