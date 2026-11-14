//! Payment-pipeline class-B closing tests: ABI-11 (NOW SUMIMI's 「即将超过」
//! running-loss window), ABI-12 (tri-state `FORCE_STAYS_MORTGAGED`),
//! FORCE-WRITE-01 (the 强制购买 price stages) and NFO-CANCEL-01 (the
//! `paySplitAt`-ran bit). Each test is written to fail on the pre-fix engine.
mod common;

use common::*;
use game_core::state::prop;

const SUMIMI: &str = "Sumimi:现在她是Sumimi的小初啦";

/// Decline every open prompt (auctions are *passed*, their 0 fallback is below
/// the minimum bid).
fn drain(t: &mut Table) {
    for _ in 0..100 {
        let Some(p) = t.prompt() else { return };
        if p.kind == "auction" {
            for who in t.asked() {
                let _ = t.answer(who, -1);
            }
        } else {
            t.decline();
        }
    }
    panic!("drain: prompts never stopped: {}", t.dump_prompt());
}

/// End `who`'s turn without a main move.
fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

/// Advance to `who`'s 运营 stage, skipping other seats' main moves.
fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..20 {
        if t.turn() == who && t.step() == 2 {
            return;
        }
        let cur = t.turn();
        if t.turn() == who {
            t.end(who).unwrap();
            drain(t);
            continue;
        }
        if t.p(cur).out() {
            t.m.world_mut().next_turn_pending = true;
            t.settle();
            continue;
        }
        pass(t, cur);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

/// Advance to `who`'s 运营 stage and land them on `tile` with a one-step main
/// move from the tile before it. Prompts are left open for the caller.
fn settle_on(t: &mut Table, who: usize, tile: usize) {
    until_turn(t, who);
    t.set_pos(who, (tile + 59) % 60);
    t.dice(&[1]);
    t.roll(who).unwrap();
}

// =====================================================================
// ABI-11 -- NOW SUMIMI fires in the 「即将超过」 window only
// =====================================================================

/// Play a `TEST:payAmount` pay of `n` and answer every prompt, declaring
/// SUMIMI whenever it is offered. Returns whether it was offered on this
/// payment.
fn pay_watching_sumimi(t: &mut Table, n: i32) -> bool {
    t.set_state(0, "TEST.pay.amount", n);
    t.play(0, "TEST:payAmount").unwrap();
    let mut offered = false;
    let mut declared = false;
    while let Some(p) = t.prompt() {
        if t.counteract_offered(SUMIMI) {
            offered = true;
            if !declared {
                declared = true;
                t.counteract(0, SUMIMI).unwrap();
                continue;
            }
        }
        if p.kind == "auction" {
            for who in t.asked() {
                let _ = t.answer(who, -1);
            }
        } else {
            t.decline();
        }
    }
    offered
}

/// 规则书 (Sumimi): 「当你在你的本回合开始后到下回合开始前之间失去资金的总额
/// 即将超过你所在格子的[收费标价]时打出此卡」 -- 「即将超过」 is the crossing
/// window: the running total before this payment is at or under the tag while
/// the payment pushes it over. On 水族馆 (0 houses, tag 320) two 200 payments
/// cross at the second; the third, already past the tag, does not fire.
#[test]
fn now_sumimi_fires_only_at_the_crossing_payment() {
    let mut t = Table::vanilla(2);
    let aq = tile("水族馆");
    t.set_pos(0, aq);
    // A declared counter is consumed, so a second copy keeps the third
    // payment's assertion meaningful. The hand stays under 6 (CRYCHIC's group
    // PlayGate locks a 6-card hand).
    t.give(
        0,
        &[
            SUMIMI,
            SUMIMI,
            "TEST:payAmount",
            "TEST:payAmount",
            "TEST:payAmount",
        ],
    );

    assert!(
        !pay_watching_sumimi(&mut t, 200),
        "200 alone is under the 320 tag: {}",
        t.dump_prompt()
    );
    assert_eq!(t.money(0), 9_800);
    assert!(
        pay_watching_sumimi(&mut t, 200),
        "200 + 200 crosses the 320 tag: {}",
        t.dump_prompt()
    );
    assert_eq!(t.money(0), 9_800 - 200 + 320, "crossing paid out the tag");
    assert!(
        !pay_watching_sumimi(&mut t, 200),
        "600 is already past the tag -- no second firing: {}",
        t.dump_prompt()
    );
    assert_eq!(t.money(0), 9_920 - 200, "the third 200 is not refunded");

    // The total restarts at the seat's own turn start (「本回合开始后」), so
    // the next crossing fires again.
    t.give(0, &[SUMIMI, "TEST:payAmount", "TEST:payAmount"]);
    pass(&mut t, 0);
    until_turn(&mut t, 0);
    assert!(
        !pay_watching_sumimi(&mut t, 200),
        "a fresh own turn starts the total over: {}",
        t.dump_prompt()
    );
    assert!(
        pay_watching_sumimi(&mut t, 200),
        "the new turn's own crossing fires: {}",
        t.dump_prompt()
    );
}

// =====================================================================
// ABI-12 -- `FORCE_STAYS_MORTGAGED` is tri-state
// =====================================================================

/// 规则书 基础[结算] 6.2: 「获得的地契仍为抵押状态」 -- the default. A tile rule
/// that writes the prop overrides it: `0` clears, non-zero keeps.
#[test]
fn force_buy_keeps_the_mortgage_by_default() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_mortgaged(land, true);
    t.set_money(0, 10_000);
    settle_on(&mut t, 0, land);
    assert_eq!(t.expect_prompt().kind, "choice", "{}", t.dump_prompt());
    t.answer_one(0).unwrap();
    drain(&mut t);
    assert_eq!(t.owner(land), Some(0), "deed changed hands");
    assert!(t.mortgaged(land), "「获得的地契仍为抵押状态」");
}

/// The explicit `0` an absent key cannot be told from by `tile_prop` -- the
/// tile rule clears the mortgage.
#[test]
fn force_buy_with_an_explicit_zero_clears_the_mortgage() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_mortgaged(land, true);
    t.set_money(0, 10_000);
    let wrote = t
        .m
        .world_mut()
        .set_tile_prop(land as i32, prop::FORCE_STAYS_MORTGAGED, 0);
    assert_eq!(wrote, 0, "a tile rule instance governs the tile");
    settle_on(&mut t, 0, land);
    assert_eq!(t.expect_prompt().kind, "choice", "{}", t.dump_prompt());
    t.answer_one(0).unwrap();
    drain(&mut t);
    assert_eq!(t.owner(land), Some(0));
    assert!(!t.mortgaged(land), "an explicit 0 clears the mortgage");
}

/// A non-zero write keeps the default's behaviour.
#[test]
fn force_buy_with_a_nonzero_prop_keeps_the_mortgage() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_mortgaged(land, true);
    t.set_money(0, 10_000);
    t.m.world_mut()
        .set_tile_prop(land as i32, prop::FORCE_STAYS_MORTGAGED, 1);
    settle_on(&mut t, 0, land);
    assert_eq!(t.expect_prompt().kind, "choice", "{}", t.dump_prompt());
    t.answer_one(0).unwrap();
    drain(&mut t);
    assert_eq!(t.owner(land), Some(0));
    assert!(t.mortgaged(land));
}

// =====================================================================
// FORCE-WRITE-01 -- the 强制购买 price stages
// =====================================================================

/// 规则书 其他规则 6: 「抵押，赎回，和强制购买的资金变动不受任何效果影响，
/// 除非效果写明改动抵押，赎回，或强制购买」. A hook that writes
/// 「强制购买」 (`ForceBuyAdd` here, +1500) retunes the force quote:
/// 2 × 3000 + 1500 = 7500.
#[test]
fn a_force_buy_add_hook_retunes_the_force_price() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心"); // price 3000, house 2000
    t.place_raw(0, "TEST:forceBuyAdd");
    t.own(1, &[land]);
    t.set_mortgaged(land, true);
    t.set_money(0, 10_000);
    t.set_money(1, 0);
    settle_on(&mut t, 0, land);
    let p = t.expect_prompt();
    assert_eq!(p.kind, "choice", "{}", t.dump_prompt());
    assert_eq!(p.price, 7_500, "2 × 3000 + 1500");
    t.answer_one(0).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000 - 7_500, "charged the retuned figure");
    assert_eq!(t.money(1), 7_500, "the owner received it");
}

/// The control on the generic side: a plain `BuyMul` hook halves an ordinary
/// land buy.
#[test]
fn a_generic_buy_mul_hook_retunes_an_ordinary_buy() {
    let mut t = Table::vanilla(2);
    let land = tile("富士见坂"); // price 600
    t.place_raw(0, "TEST:buyMulHalf");
    t.set_money(0, 10_000);
    settle_on(&mut t, 0, land);
    t.buy(0).unwrap();
    drain(&mut t);
    assert_eq!(t.owner(land), Some(0));
    assert_eq!(t.money(0), 10_000 - 300, "600 halved");
}

/// 其他规则 6: a generic `BuyMul` does **not** retune a 强制购买 quote -- the
/// entry did not 「写明」 force-buy. The charge stays 2 × 3000.
#[test]
fn a_generic_buy_mul_hook_does_not_retune_a_force_buy() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.place_raw(0, "TEST:buyMulHalf");
    t.own(1, &[land]);
    t.set_mortgaged(land, true);
    t.set_money(0, 10_000);
    t.set_money(1, 0);
    settle_on(&mut t, 0, land);
    let p = t.expect_prompt();
    assert_eq!(p.kind, "choice", "{}", t.dump_prompt());
    assert_eq!(p.price, 6_000, "2 × 3000, not halved");
    t.answer_one(0).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000 - 6_000);
    assert_eq!(t.money(1), 6_000);
    assert!(t.mortgaged(land), "still the force default");
}

// =====================================================================
// NFO-CANCEL-01 -- only a payment that reached the split stage uses NFO up
// =====================================================================

const NFO: &str = "R:NFO";

/// Place NFO through its own result 3.
fn place_nfo(t: &mut Table) {
    t.dice(&[3]);
    t.give_play(0, NFO).unwrap();
    drain(t);
    assert!(t.on_field(0, NFO), "{:?}", t.field_ids(0));
}

/// 规则书 (NFO face 3): 「你下次付款时自动减免1000资金的消耗并将此卡置入弃牌堆」.
/// A payment cancelled before the `paySplitAt` shave never reaches the
/// discount, so the card has to stay placed for the next payment.
#[test]
fn nfo_stays_placed_when_the_payment_is_cancelled_before_the_split() {
    let mut t = Table::vanilla(2);
    place_nfo(&mut t);
    t.give(0, &["TEST:payNegate", "TEST:payAmount"]);
    let mark = t.mark();
    t.set_state(0, "TEST.pay.amount", 1_000);
    t.play(0, "TEST:payAmount").unwrap();
    // The negation is declared in the `effect` window, before paySplitAt.
    assert_eq!(
        t.expect_prompt().title.key(),
        "ask.counteract.title",
        "{}",
        t.dump_prompt()
    );
    t.counteract(0, "TEST:payNegate").unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000, "nothing was paid");
    assert!(t.on_field(0, NFO), "the shave never ran, card stays");
    assert!(
        !t.keys_since(mark).iter().any(|k| k.ends_with(".nfo_used")),
        "the shave logged nothing: {:?}",
        t.keys_since(mark)
    );
}

/// The other half: a payment the shave itself brings to 0 did reach the stage
/// and uses the card up (「并将此卡置入弃牌堆」).
#[test]
fn nfo_is_unplaced_when_the_shave_brings_the_payment_to_zero() {
    let mut t = Table::vanilla(2);
    place_nfo(&mut t);
    t.give(0, &["TEST:payAmount"]);
    let mark = t.mark();
    t.set_state(0, "TEST.pay.amount", 1_000);
    t.play(0, "TEST:payAmount").unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000, "the whole 1000 was shaved");
    assert!(!t.on_field(0, NFO), "the card was used up");
    assert!(
        t.keys_since(mark).iter().any(|k| k.ends_with(".nfo_used")),
        "the shave ran: {:?}",
        t.keys_since(mark)
    );
}
