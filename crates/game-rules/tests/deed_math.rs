//! Pin the engine-side deed / half-rent figures to the rules-side ones.
//!
//! `game-core` cannot depend on `bandori-std` (the dep edge is
//! `card-sdk` ← `bandori-std` ← the rules crates; `game-core` sits beside
//! that chain), so the two integer forms are written twice: once in
//! `game_core::engine::purchase` (the `StubRules` default and the sim) and
//! once in `bandori_std` (the rules-side authority that `rules/tiles` aliases
//! and `rules/spec` calls). This test is the pin that keeps them equal.
//!
//! Also asserts the redeem rounding: `redeem_cost` is the integer form of
//! `round_ties_even(price * 0.6)`. The product of an integer price and 0.6
//! never lands on `.5`, so `(price * 3 + 2) / 5` is that form.

use game_core::data::{GameData, TileData};
use game_core::engine::purchase::{half_ceil10, mortgage_value, redeem_cost};

/// Land prices that include non-multiples of 5 and 10, so a drifted formula
/// (e.g. `price * 60 / 100` vs `(price * 3 + 2) / 5`) cannot hide behind the
/// shipped board's multiples of 100.
const PRICES: [i32; 10] = [0, 1, 3, 7, 11, 50, 99, 100, 111, 1000];

fn board_with_prices() -> GameData {
    let mut data = GameData::default();
    data.tiles = PRICES
        .iter()
        .enumerate()
        .map(|(i, &price)| TileData {
            index: i as i32,
            price,
            ..TileData::default()
        })
        .collect();
    data
}

/// Independent `round_ties_even(price * 0.6)`: the product of an integer
/// price and 0.6 is the rational `3p/5`, which never lands on `.5` (that
/// would need `6p ≡ 5 (mod 10)`, impossible for even `6p`). So the result is
/// plain nearest: `3p = 5q + r` with `r ∈ 0..=4`, and `r/5 ∈ {0,.2,.4,.6,.8}`
/// rounds to `q` for `r ≤ 2` and `q + 1` for `r ≥ 3`.
fn redeem_round_ties_even(price: i32) -> i32 {
    assert!(price >= 0, "board prices are non-negative");
    let num = price as i64 * 3;
    let q = num / 5;
    let r = num % 5;
    if r >= 3 {
        (q + 1) as i32
    } else {
        q as i32
    }
}

#[test]
fn deed_math_engine_matches_bandori_std() {
    let data = board_with_prices();
    for (t, &price) in PRICES.iter().enumerate() {
        assert_eq!(
            mortgage_value(&data, t),
            bandori_std::mortgage_value(price),
            "mortgage_value(price={price})"
        );
        assert_eq!(
            redeem_cost(&data, t),
            bandori_std::redeem_cost(price),
            "redeem_cost(price={price})"
        );
    }
}

#[test]
fn redeem_cost_is_round_ties_even_of_price_times_0_6() {
    let data = board_with_prices();
    for (t, &price) in PRICES.iter().enumerate() {
        // The product never lands on .5: that would need 6p ≡ 5 (mod 10),
        // impossible for even 6p. So ties-even is plain nearest.
        assert_ne!(
            (price * 6) % 10,
            5,
            "price={price}: product lands on .5; the .5-free claim is false"
        );
        assert_eq!(
            redeem_cost(&data, t),
            redeem_round_ties_even(price),
            "redeem_cost(price={price}) must be round_ties_even(price * 0.6)"
        );
        // The drifted `price * 60 / 100` (floor of 3p/5) disagrees with the
        // nearest form exactly when `3p % 5 >= 3`, i.e. `p % 5 ∈ {1, 3}`
        // (e.g. price 11 → 7 vs 6). Pin that this table really catches it.
        if matches!(price % 5, 1 | 3) {
            assert_ne!(
                redeem_cost(&data, t),
                price * 60 / 100,
                "price={price}: the drifted 60/100 form must not match"
            );
        }
    }
}

#[test]
fn half_ceil10_engine_matches_bandori_std() {
    for full in 0..400 {
        assert_eq!(
            half_ceil10(full),
            bandori_std::half_ceil10(full),
            "half_ceil10(full={full})"
        );
    }
    // The rounding table the rulebook suite already pins.
    assert_eq!(half_ceil10(301), 160, "301/2 = 150.5 -> 160");
    assert_eq!(half_ceil10(300), 150, "300/2 = 150 is already a multiple");
    assert_eq!(half_ceil10(1), 10, "1/2 = 0.5 -> 10");
}