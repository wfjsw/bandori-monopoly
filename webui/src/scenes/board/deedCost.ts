// One source for the deed-popup's mortgage / force-buy / redeem figures, so
// the popup prints the **same number** the tile face caption and the tile
// tooltip quote (`MatchView.tileQuotes`, `purchase::tile_quotes`). Run the
// unit test with:
//   node --test webui/src/scenes/board/deedCost.test.ts

import type { TileQuote } from "../../core/types.ts";

/** Mortgage payout: 「地契购买价格50%」. Floor of half, matching
 *  `purchase::mortgage_value` / `bandori_std::mortgage_value`. */
export const mortgageValue = (price: number) => Math.floor(price / 2);

/** Redeem charge: 「地契购买价格60%」. The integer form of
 *  `round_ties_even(price * 0.6)` never lands on `.5`, so `Math.round` is
 *  safe and matches `purchase::redeem_cost`'s `(price * 3 + 2) / 5`. */
export const redeemCost = (price: number) => Math.round(price * 0.6);

/** Force-buy of a mortgaged deed: 「购买格子地契和建造已有房子的资金总价的
 *  两倍」 (`docs/PURCHASE.md`, G-MONEY-07). The engine quote is
 *  `purchase::force_price_native`; this is the no-`tileQuotes` fallback. */
export const forceBuyCost = (price: number, house: number, houses: number) =>
  2 * (price + houses * house);

/**
 * The one standing cost the deed popup must print for a **mortgaged** deed --
 * the same figure the tile face caption and the tile tooltip quote.
 *
 * - other's (or a spectator's seat-independent reading) → force-buy
 *   (「可选择[支付]…两倍…强行购买」; force-buy never applies to one's own
 *   deed);
 * - mine → redeem (the natural cost to clear the mortgage).
 *
 * Prefers the engine's `tileQuotes` entry so the popup cannot drift from the
 * board (hook-aware, quote == charge). Falls back to the rulebook formulas
 * when the frame predates `tileQuotes`.
 */
export function deedStandingCost(
  quote: TileQuote | null | undefined,
  o: { owner: number; playerId: number; price: number; house: number; houses: number },
): { kind: "forceBuy" | "redeem"; value: number } {
  if (quote && (quote.kind === "forceBuy" || quote.kind === "redeem")) {
    return { kind: quote.kind, value: quote.value };
  }
  const mine = o.owner >= 0 && o.owner === o.playerId;
  return mine
    ? { kind: "redeem", value: redeemCost(o.price) }
    : { kind: "forceBuy", value: forceBuyCost(o.price, o.house, o.houses) };
}