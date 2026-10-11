// The hand limit, as the engine counts it (`MatchPlayer::hand_limit`).

import type { MatchPlayer } from "./types";

/** The `handLimit` value meaning 「手牌数没有上限」 (no limit). Mirrors
 *  `card_sdk::abi::state_key::NO_HAND_LIMIT` / `game_core::state::key::NO_HAND_LIMIT`. */
export const NO_HAND_LIMIT = 2147483647;

/** The player's hand limit: the `handLimit` state (base 5) plus every placed
 *  card's `handLimitDelta` prop (「手卡上限数量减1」). `null` is no limit. */
export function handLimit(me: MatchPlayer): number | null {
  const base = me.state?.handLimit?.value ?? 5;
  if (base === NO_HAND_LIMIT) return null;
  return (me.field ?? []).reduce((n, f) => n + (f.props?.handLimitDelta ?? 0), base);
}

/** Holding `held` cards puts the player over the hand limit. */
export function overHand(me: MatchPlayer, held: number): boolean {
  const limit = handLimit(me);
  return limit !== null && held > limit;
}
