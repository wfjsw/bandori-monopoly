// Deed-popup mortgage / force-buy / redeem figures -- one source, matching
// the tile face caption and the tile tooltip (`purchase::tile_quotes`). Run with:
//   node --test webui/src/scenes/board/deedCost.test.ts

import { strict as assert } from "node:assert";
import { test } from "node:test";

import {
  deedStandingCost,
  forceBuyCost,
  mortgageValue,
  redeemCost,
} from "./deedCost.ts";
import type { TileQuote } from "../../core/types.ts";

test("mortgage value is half the land price, floored", () => {
  assert.equal(mortgageValue(1000), 500);
  assert.equal(mortgageValue(1001), 500, "floor, not round -- purchase::mortgage_value");
});

test("redeem cost is 60% of the land price", () => {
  assert.equal(redeemCost(1000), 600);
  assert.equal(redeemCost(999), 599);
  assert.equal(redeemCost(1001), 601);
  // purchase::redeem_cost's (price * 3 + 2) / 5 twin.
  for (const p of [1, 2, 3, 5, 7, 8, 999, 1000, 1001, 2500]) {
    assert.equal(redeemCost(p), Math.trunc((p * 3 + 2) / 5), `redeem(${p})`);
  }
});

test("force-buy is twice the land + standing houses", () => {
  assert.equal(forceBuyCost(1000, 500, 0), 2000);
  assert.equal(forceBuyCost(1000, 500, 2), 4000, "houses count at their build cost");
});

test("other's mortgaged deed quotes the force-buy price", () => {
  assert.deepEqual(
    deedStandingCost(null, { owner: 2, playerId: 1, price: 1000, house: 500, houses: 2 }),
    { kind: "forceBuy", value: 4000 },
  );
});

test("my mortgaged deed quotes the redeem price (force-buy never applies)", () => {
  assert.deepEqual(
    deedStandingCost(null, { owner: 1, playerId: 1, price: 1000, house: 500, houses: 2 }),
    { kind: "redeem", value: 600 },
  );
});

test("a spectator's seat-independent reading is the force-buy list price", () => {
  assert.deepEqual(
    deedStandingCost(null, { owner: 0, playerId: -1, price: 1000, house: 500, houses: 0 }),
    { kind: "forceBuy", value: 2000 },
  );
});

test("a live tileQuotes entry wins, so the popup cannot drift from the board", () => {
  const q: TileQuote = { kind: "forceBuy", flow: "mayPay", value: 4000 };
  assert.deepEqual(
    deedStandingCost(q, { owner: 2, playerId: 1, price: 1000, house: 500, houses: 0 }),
    { kind: "forceBuy", value: 4000 },
    "the quote's figure is printed even when the fallback formula would differ",
  );
  const r: TileQuote = { kind: "redeem", flow: "mayPay", value: 600 };
  assert.deepEqual(
    deedStandingCost(r, { owner: 1, playerId: 1, price: 1000, house: 500, houses: 0 }),
    { kind: "redeem", value: 600 },
  );
});

test("an unrelated quote kind falls back to the rulebook formulas", () => {
  const rent: TileQuote = { kind: "rent", flow: "mustPay", value: 200 };
  assert.deepEqual(
    deedStandingCost(rent, { owner: 2, playerId: 1, price: 1000, house: 500, houses: 1 }),
    { kind: "forceBuy", value: 3000 },
  );
});