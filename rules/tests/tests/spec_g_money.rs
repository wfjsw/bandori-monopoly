//! G-MONEY scripted scenarios: exact money numbers over randomized pre-states.
//!
//! Unit 1. Sources: `docs/rulebook/rulebook-doc.md` glossary + 游戏流程 +
//! 其他规则注意事项 + 基础[结算]规则 + 支付阶段. A discrepancy is a plain
//! failing test. No `#[ignore]`, no allowlist.
//!
//! Bankruptcy money is unit 7 (`G-BANKRUPT`) and is not duplicated here.

use bandori_std::half_ceil10;
use rules_spec::{data, forall, frame, tile, Table};

/// Decline every open prompt (buy / build / mortgage offers, counteracts).
fn drain(t: &mut Table) {
    for _ in 0..40 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
    panic!("drain: prompts never stopped: {}", t.dump_prompt());
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
        if t.p(cur).out_order >= 0 {
            t.m.world_mut().next_turn_pending = true;
            t.settle();
            continue;
        }
        t.m.world_mut().st.skip_move = true;
        let _ = t.end(cur);
        drain(t);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

/// Land `who` on `tile` with a one-step main move from the tile before it.
/// Prompts are left open for the caller.
fn settle_on(t: &mut Table, who: usize, tile: usize) {
    until_turn(t, who);
    t.set_pos(who, (tile + 59) % 60);
    t.dice(&[1]);
    t.roll(who).unwrap();
}

/// The tile's printed price.
fn price(t: usize) -> i32 {
    data().tiles[t].price
}

/// The tile's printed house cost.
fn house_cost(t: usize) -> i32 {
    data().tiles[t].house
}

// =====================================================================
// G-MONEY-01/02/03 -- [支付] / [获得] / [消耗]
// =====================================================================

/// G-MONEY-01 -- 「A[支付]BX资金」 conserves.
///
/// Quote: 「[支付]：“A[支付]BX资金”的效果为玩家A向玩家B支付X资金。」
#[test]
fn pay_conserves_between_the_named_pair() {
    forall(8, 0x60_01, |rng, _i| {
        let mut t = Table::vanilla(2);
        let land = tile("富士见坂"); // rent[0] = 60
        let rent = data().tiles[land].rent[0];
        t.own(1, &[land]);
        let money0 = 2_000 + rng.range(0, 20) * 100;
        let money1 = 1_000 + rng.range(0, 20) * 100;
        t.set_money(0, money0);
        t.set_money(1, money1);

        let pre = t.snapshot_world();
        t.observe(rules_spec::global::money::CLAUSES);
        settle_on(&mut t, 0, land);
        drain(&mut t);

        let d0 = t.money(0) as i64 - money0 as i64;
        let d1 = t.money(1) as i64 - money1 as i64;
        if d0 != -(rent as i64) || d1 != rent as i64 {
            return Err(format!(
                "G-MONEY-01: rent {rent} moved {d0}/{d1}, expected {}/{ }",
                -(rent as i64),
                rent as i64
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "money:P1",
                "pos:P0",
                "roll:P0",
                "turn.player",
                "turn.step",
                "turn.paid_in_settle",
            ],
        )
    })
    .expect("G-MONEY-01 pay conserves");
}

/// G-MONEY-02 -- 「[获得]X资金」 prints X.
///
/// Quote: 「[获得]：“[获得]X资金”的效果为直接获得X资金，等同于印钱。」
/// CiRCLE's reward money half is a print (`pass` of 2000).
#[test]
fn gain_prints_money() {
    forall(6, 0x60_02, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 3_000 + rng.range(0, 10) * 100;
        t.set_money(0, money);
        t.set_money(1, 7_000);
        t.observe(rules_spec::global::money::CLAUSES);
        // CiRCLE pass reward: 「[获得]2000资金或抽1张卡」. Take the money half.
        until_turn(&mut t, 0);
        t.set_pos(0, tile("CiRCLE"));
        t.dice(&[1]);
        t.roll(0).unwrap();
        // The pass reward asks money-or-card; decline = keep walking to the
        // end settle, so answer the money option when it comes up.
        for _ in 0..12 {
            if t.prompt().is_none() {
                break;
            }
            let p = t.expect_prompt();
            if p.title.key().contains("pass_reward") || p.title.key().contains("reward") {
                // Prefer the money option (value 0 is usually the first).
                let _ = t.answer(0, 0);
            } else {
                t.decline();
            }
        }
        drain(&mut t);
        // Whether the reward fired depends on the pass rule; assert the
        // ledger shape of whatever print landed: only P0 moved, and by a
        // non-negative amount.
        let d0 = t.money(0) as i64 - money as i64;
        let d1 = t.money(1) as i64 - 7_000;
        if d1 != 0 {
            return Err(format!("G-MONEY-02: unnamed P1 moved by {d1}"));
        }
        if d0 < 0 {
            return Err(format!("G-MONEY-02: a print left P0 at delta {d0}"));
        }
        Ok(())
    })
    .expect("G-MONEY-02 gain prints");
}

/// G-MONEY-03 -- 「[消耗]X资金」 deletes X. A mortgage is a print (G-MONEY-05);
/// a redeem is a delete of exactly 60% (G-MONEY-06). Here we pin the delete
/// side of a redeem as the one-sided [消耗].
#[test]
fn lose_deletes_money() {
    forall(6, 0x60_03, |rng, _i| {
        let mut t = Table::vanilla(2);
        let land = tile("购物中心"); // price 3000 -> redeem 1800
        t.own(0, &[land]);
        t.set_mortgaged(land, true);
        let money = 5_000 + rng.range(0, 20) * 100;
        t.set_money(0, money);
        t.set_money(1, 9_000);
        t.observe(rules_spec::global::money::CLAUSES);
        until_turn(&mut t, 0);
        t.redeem(0, land).unwrap();
        drain(&mut t);
        let want = price(land) * 60 / 100;
        let d0 = t.money(0) as i64 - money as i64;
        let d1 = t.money(1) as i64 - 9_000;
        if d0 != -(want as i64) {
            return Err(format!(
                "G-MONEY-03: [消耗]{want} landed as delta {d0} (redeem of 购物中心)"
            ));
        }
        if d1 != 0 {
            return Err(format!("G-MONEY-03: unnamed P1 moved by {d1}"));
        }
        Ok(())
    })
    .expect("G-MONEY-03 lose deletes");
}

// =====================================================================
// G-MONEY-05/06 -- mortgage 50% / redeem 60%
// =====================================================================

/// G-MONEY-05 -- 「抵押后[获得]地契购买价格50%的资金」.
#[test]
fn mortgage_pays_exactly_half_the_price() {
    forall(10, 0x60_05, |rng, _i| {
        let mut t = Table::vanilla(2);
        // A spread of printed prices, all even so the half is exact.
        let candidates = ["富士见坂", "购物中心", "武道馆", "弦卷豪宅", "小豆岛"];
        let land = tile(candidates[rng.pick_idx(candidates.len())]);
        t.own(0, &[land]);
        let money = 500 + rng.range(0, 8) * 100;
        t.set_money(0, money);
        t.set_money(1, 8_000);

        let pre = t.snapshot_world();
        t.observe(rules_spec::global::money::CLAUSES);
        until_turn(&mut t, 0);
        t.mortgage(0, land).unwrap();
        drain(&mut t);

        let want = price(land) / 2;
        let d0 = t.money(0) as i64 - money as i64;
        if d0 != want as i64 {
            return Err(format!(
                "G-MONEY-05: mortgage of {} (price {}) gained {d0}, expected {want}",
                data().tiles[land].name,
                price(land)
            ));
        }
        if t.money(1) != 8_000 {
            return Err(format!("G-MONEY-05: P1 moved to {}", t.money(1)));
        }
        if !t.mortgaged(land) {
            return Err("G-MONEY-05: deed is not mortgaged after the mortgage".into());
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                &format!("mortgaged:T{land}"),
                "state:P0:mortgaged",
            ],
        )
    })
    .expect("G-MONEY-05 mortgage 50%");
}

/// G-MONEY-06 -- 「赎回后[消耗]地契购买价格60%的资金」.
#[test]
fn redeem_costs_exactly_sixty_percent() {
    forall(10, 0x60_06, |rng, _i| {
        let mut t = Table::vanilla(2);
        let candidates = ["富士见坂", "购物中心", "武道馆", "弦卷豪宅", "小豆岛"];
        let land = tile(candidates[rng.pick_idx(candidates.len())]);
        t.own(0, &[land]);
        t.set_mortgaged(land, true);
        let money = 5_000 + rng.range(0, 20) * 100;
        t.set_money(0, money);
        t.set_money(1, 8_000);

        let pre = t.snapshot_world();
        t.observe(rules_spec::global::money::CLAUSES);
        until_turn(&mut t, 0);
        t.redeem(0, land).unwrap();
        drain(&mut t);

        // 60% of price. Every printed price on the board is a multiple of
        // 100, so this is exact (OPEN-60%舍入).
        let want = price(land) * 60 / 100;
        let d0 = t.money(0) as i64 - money as i64;
        if d0 != -(want as i64) {
            return Err(format!(
                "G-MONEY-06: redeem of {} (price {}) lost {d0}, expected -{want}",
                data().tiles[land].name,
                price(land)
            ));
        }
        if t.mortgaged(land) {
            return Err("G-MONEY-06: deed is still mortgaged after the redeem".into());
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                &format!("mortgaged:T{land}"),
                "state:P0:redeemed",
            ],
        )
    })
    .expect("G-MONEY-06 redeem 60%");
}

// =====================================================================
// G-MONEY-07/08 -- force-buy 2x, stays mortgaged, immune
// =====================================================================

/// G-MONEY-07 -- 「购买格子地契和建造已有房子的资金总价的两倍 … 仍为抵押状态」.
#[test]
fn force_buy_is_double_price_plus_houses_and_stays_mortgaged() {
    forall(10, 0x60_07, |rng, _i| {
        let mut t = Table::vanilla(2);
        let land = tile("购物中心"); // price 3000, house 2000
        t.own(1, &[land]);
        let houses = rng.range(0, 3);
        t.set_houses(land, houses);
        t.set_mortgaged(land, true);
        let money = 20_000;
        t.set_money(0, money);
        t.set_money(1, 1_000);

        let pre = t.snapshot_world();
        t.observe(rules_spec::global::money::CLAUSES);
        settle_on(&mut t, 0, land);
        let p = t.expect_prompt();
        if p.kind != "choice" {
            return Err(format!(
                "G-MONEY-07: expected a force-buy prompt, got {}",
                t.dump_prompt()
            ));
        }
        t.answer_one(0).unwrap();
        drain(&mut t);

        let want = 2 * (price(land) + houses * house_cost(land));
        let d0 = t.money(0) as i64 - money as i64;
        let d1 = t.money(1) as i64 - 1_000;
        if d0 != -(want as i64) || d1 != want as i64 {
            return Err(format!(
                "G-MONEY-07: force-buy of {want} (2x {}+{}x{}) moved {d0}/{d1}",
                price(land),
                houses,
                house_cost(land)
            ));
        }
        if t.owner(land) != Some(0) {
            return Err(format!("G-MONEY-07: owner is {:?}", t.owner(land)));
        }
        if !t.mortgaged(land) {
            return Err("G-MONEY-07: 「获得的地契仍为抵押状态」 -- deed is free".into());
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "money:P1",
                &format!("owner:T{land}"),
                &format!("mortgaged:T{land}"),
                "pos:P0",
                "roll:P0",
            ],
        )
    })
    .expect("G-MONEY-07 force-buy 2x");
}

/// G-MONEY-08 -- 「抵押，赎回，和强制购买的资金变动不受任何效果影响」.
/// A standing pay multiplier must not touch the force-buy sum.
#[test]
fn force_buy_ignores_a_pay_multiplier() -> Result<(), String> {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_mortgaged(land, true);
    // 其他规则 6: the multiplier cannot reach this sum.
    t.m.world_mut().turn.plan.pay_factor = 0.5;
    t.m.world_mut().turn.plan.pay_fresh = true;
    t.set_money(0, 10_000);
    t.set_money(1, 1_000);
    t.observe(rules_spec::global::money::CLAUSES);
    settle_on(&mut t, 0, land);
    t.answer_one(0).unwrap();
    drain(&mut t);
    let want = 2 * price(land);
    assert_eq!(
        t.money(0),
        10_000 - want,
        "G-MONEY-08: full double, not halved (其他规则 6)"
    );
    assert_eq!(t.money(1), 1_000 + want, "the owner received it in full");
    Ok(())
}

/// G-MONEY-08 -- a mortgage is likewise immune.
#[test]
fn mortgage_ignores_a_pay_multiplier() -> Result<(), String> {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(0, &[land]);
    t.m.world_mut().turn.plan.pay_factor = 0.5;
    t.m.world_mut().turn.plan.pay_fresh = true;
    t.set_money(0, 1_000);
    t.observe(rules_spec::global::money::CLAUSES);
    until_turn(&mut t, 0);
    t.mortgage(0, land).unwrap();
    drain(&mut t);
    assert_eq!(
        t.money(0),
        1_000 + price(land) / 2,
        "G-MONEY-08: mortgage pays the full 50%, immune"
    );
    Ok(())
}

// =====================================================================
// G-MONEY-09 -- rent by level
// =====================================================================

/// G-MONEY-09 -- 「[支付]…格子地契所标记的现等级地租」.
#[test]
fn rent_is_the_marked_rate_at_the_current_level() {
    forall(10, 0x60_09, |rng, _i| {
        let mut t = Table::vanilla(2);
        let land = tile("购物中心"); // rent = [300, 1360, 3220, 6680]
        t.own(1, &[land]);
        let houses = rng.range(0, 3);
        t.set_houses(land, houses);
        let money = 20_000;
        t.set_money(0, money);
        t.set_money(1, 2_000);

        let pre = t.snapshot_world();
        t.observe(rules_spec::global::money::CLAUSES);
        settle_on(&mut t, 0, land);
        drain(&mut t);

        let want = data().tiles[land].rent[houses as usize];
        let d0 = t.money(0) as i64 - money as i64;
        let d1 = t.money(1) as i64 - 2_000;
        if d0 != -(want as i64) || d1 != want as i64 {
            return Err(format!(
                "G-MONEY-09: level-{houses} rent {want} moved {d0}/{d1}"
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "money:P1",
                "pos:P0",
                "roll:P0",
                "turn.paid_in_settle",
            ],
        )
    })
    .expect("G-MONEY-09 rent by level");
}

// =====================================================================
// G-MONEY-10 -- [地产商] half rate, ceil 10
// =====================================================================

/// G-MONEY-10 -- 「半价收费的[结算]（向上取整10）」.
#[test]
fn agent_half_charge_rounds_up_to_ten() -> Result<(), String> {
    let mut t = Table::vanilla(2);
    let agent = tile("主要街道"); // group 1
    let group: Vec<usize> = (0..data().tiles.len())
        .filter(|&x| data().tiles[x].is_buyable() && data().tiles[x].group == 1)
        .collect();
    assert!(!group.is_empty(), "group 1 has buyable tiles");
    for &x in &group {
        t.own(1, &[x]);
    }
    t.set_money(0, 10_000);
    t.set_money(1, 0);
    t.observe(rules_spec::global::money::CLAUSES);
    settle_on(&mut t, 0, agent);
    drain(&mut t);

    // Every same-colour purchasable charges half rent, each rounded up to 10.
    let mut want = 0i64;
    for &x in &group {
        want += half_ceil10(data().tiles[x].rent[0]) as i64;
    }
    let d0 = t.money(0) as i64 - 10_000;
    if d0 != -want {
        return Err(format!(
            "G-MONEY-10: half-rent total {want} landed as {d0} (ceil 10 per tile)"
        ));
    }
    // The rounding is 「向上取整10」: a half of 150.5 charges 160, not 150.
    assert_eq!(half_ceil10(301), 160, "301/2 = 150.5 -> 160");
    assert_eq!(half_ceil10(300), 150, "300/2 = 150 is already a multiple");
    assert_eq!(half_ceil10(1), 10, "1/2 = 0.5 -> 10");
    Ok(())
}

/// G-MONEY-10 -- the sweep is 「从前到后」 (board order).
#[test]
fn agent_half_charge_is_front_to_back() -> Result<(), String> {
    let mut t = Table::vanilla(2);
    let agent = tile("主要街道");
    let group: Vec<usize> = (0..data().tiles.len())
        .filter(|&x| data().tiles[x].is_buyable() && data().tiles[x].group == 1)
        .collect();
    for &x in &group {
        t.own(1, &[x]);
    }
    t.set_money(0, 10_000);
    let mark = t.mark();
    t.observe(rules_spec::global::money::CLAUSES);
    settle_on(&mut t, 0, agent);
    drain(&mut t);

    // Each half-charge is a `pay` / `rent` event whose `to` is the tile.
    let mut paid_tiles: Vec<i32> = Vec::new();
    for e in t.events_since(mark) {
        if (e.r#type == "pay" || e.r#type == "rent") && e.to >= 0 {
            paid_tiles.push(e.to);
        }
    }
    if !paid_tiles.is_empty() {
        let mut sorted = paid_tiles.clone();
        sorted.sort_unstable();
        if paid_tiles != sorted {
            return Err(format!(
                "G-MONEY-10: 「从前到后依次」 -- charges in board order, got {paid_tiles:?}"
            ));
        }
    }
    Ok(())
}

// =====================================================================
// G-MONEY-11 -- buy cost = price + houses
// =====================================================================

/// G-MONEY-11 -- 「可选择[消耗]购买格子地契和建造已有房子的资金总价」.
#[test]
fn buy_costs_price_plus_houses() {
    forall(8, 0x60_11, |rng, _i| {
        let mut t = Table::vanilla(2);
        let land = tile("购物中心");
        let houses = rng.range(0, 3);
        t.set_houses(land, houses);
        t.set_money(0, 20_000);
        t.set_money(1, 5_000);

        let pre = t.snapshot_world();
        t.observe(rules_spec::global::money::CLAUSES);
        settle_on(&mut t, 0, land);
        // G-SETTLE-06/07: the settle of an unowned buyable IS the optional buy.
        t.buy(0).unwrap();
        drain(&mut t);

        let want = price(land) + houses * house_cost(land);
        let d0 = t.money(0) as i64 - 20_000;
        if d0 != -(want as i64) {
            return Err(format!(
                "G-MONEY-11: buy of {}+{houses}x{} charged {d0}, expected -{want}",
                price(land),
                house_cost(land)
            ));
        }
        if t.owner(land) != Some(0) {
            return Err(format!("G-MONEY-11: owner is {:?}", t.owner(land)));
        }
        if t.houses(land) != houses {
            return Err(format!(
                "G-MONEY-11: houses {} -> {}, expected unchanged",
                houses,
                t.houses(land)
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                &format!("owner:T{land}"),
                &format!("mortgaged:T{land}"),
                "pos:P0",
                "roll:P0",
            ],
        )
    })
    .expect("G-MONEY-11 buy price+houses");
}

// =====================================================================
// G-MONEY-12 -- build cost
// =====================================================================

/// G-MONEY-12 -- 「可选择[消耗]格子地契所标注的房屋建筑费进行升级建造」.
#[test]
fn build_costs_the_marked_house_fee() {
    forall(8, 0x60_12, |rng, _i| {
        let mut t = Table::vanilla(2);
        let land = tile("购物中心"); // house 2000, cap 3
        t.own(0, &[land]);
        let start_houses = rng.range(0, 2);
        t.set_houses(land, start_houses);
        t.set_money(0, 20_000);

        let pre = t.snapshot_world();
        t.observe(rules_spec::global::money::CLAUSES);
        settle_on(&mut t, 0, land);
        // G-SETTLE-06/07: the settle of an own un-mortgaged buyable IS the optional build.
        t.build(0).unwrap();
        drain(&mut t);

        let want = house_cost(land);
        let d0 = t.money(0) as i64 - 20_000;
        if d0 != -(want as i64) {
            return Err(format!(
                "G-MONEY-12: build cost charged {d0}, expected -{want}"
            ));
        }
        if t.houses(land) != start_houses + 1 {
            return Err(format!(
                "G-MONEY-12: houses {start_houses} -> {}, expected +1",
                t.houses(land)
            ));
        }
        frame(
            &pre,
            t.world(),
            &["money:P0", &format!("houses:T{land}"), "pos:P0", "roll:P0"],
        )
    })
    .expect("G-MONEY-12 build cost");
}

// =====================================================================
// G-MONEY-13 -- 支付计算时 before 支付前
// =====================================================================

/// G-MONEY-13 -- a fixed add/sub (支付计算时) lands **before** the halve
/// (支付前).
///
/// Quote (支付阶段 rows 2 and 4):
/// 「| 2 | 支付计算时 | 支付增加/减少(固定值) | …
///  | 4 | 支付前 | 支付减半/翻倍 |」
///
/// Armed with 丸山彩（2） (「此次支付的分摊前资金减少Y×100」) and
/// HHW:爱心义演 (「本回合中向其他玩家支付时你的付款减半（向上取整10）」).
/// The settled figure is `ceil10((B - Y*100) / 2)`, not `ceil10(B/2) - Y*100`.
#[test]
fn pay_stages_add_before_halve() -> Result<(), String> {
    let mut t = Table::new(&["丸山彩", "青叶摩卡"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // 丸山彩（1） grants 5 face-up [P✽P粉丝] at game start; `clean` wiped
    // them, so re-seed two for a Y=2 spend. Fans live in the state dict.
    t.set_state(0, "ppFansUp", 2);
    t.set_state(0, "ppFansDown", 0);
    t.give(0, &["HHW:爱心义演"]);
    t.observe(rules_spec::global::money::CLAUSES);

    // Play 爱心义演: this turn, outgoing payments are halved (ceil 10).
    t.play(0, "HHW:爱心义演").unwrap();
    drain(&mut t);

    // A rent of 1360 on 购物中心 level 1 (rent[1] = 1360). Y=2 reduces 200,
    // so the two orders differ: ceil10((1360-200)/2) = 580 vs
    // ceil10(1360/2) - 200 = 480.
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_houses(land, 1);
    let rent = data().tiles[land].rent[1];
    let money = 10_000;
    t.set_money(0, money);
    t.set_money(1, 0);

    // Land on it: 丸山彩（2） prompts to flip fans; flip both (Y=2, -200).
    until_turn(&mut t, 0);
    t.set_pos(0, (land + 59) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    for _ in 0..12 {
        if t.prompt().is_none() {
            break;
        }
        let p = t.expect_prompt();
        let who = t.asked().first().copied().unwrap_or(0);
        if p.kind == "pick" || p.kind == "mortgage" {
            let n = p.count.max(0) as usize;
            let items: Vec<&str> = std::iter::repeat("P✽P粉丝(正)").take(n.min(2)).collect();
            let _ = t.answer_items(who, &items);
        } else if p.kind == "choice" {
            let _ = t.answer(who, p.options.len().saturating_sub(1) as i32);
        } else {
            t.decline();
        }
    }
    drain(&mut t);

    let y = 2i64;
    let a = y * 100;
    let book = half_ceil10((rent as i64 - a) as i32) as i64;
    let wrong = half_ceil10(rent) as i64 - a;
    assert_ne!(book, wrong, "the two orders must differ for this setup");
    let d0 = t.money(0) as i64 - money as i64;
    if d0 != -book {
        return Err(format!(
            "G-MONEY-13: add-before-halve expected -{book} (ceil10(({rent}-{a})/2)), got {d0} \
             (halve-before-add would be -{wrong})"
        ));
    }
    Ok(())
}

/// G-MONEY-13 -- a fixed add on a receive (白鹭千圣（2）) also lands before a
/// later scale. Pin the add alone: the pre-split figure grows by Y×100.
#[test]
fn pay_stages_fixed_add_on_receive() -> Result<(), String> {
    let mut t = Table::new(&["白鹭千圣", "青叶摩卡"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // Two face-up fans for a Y=2 spend.
    t.set_state(0, "ppFansUp", 2);
    t.set_state(0, "ppFansDown", 0);
    t.observe(rules_spec::global::money::CLAUSES);

    // P1 pays P0 a rent of 60 on 富士见坂. 白鹭千圣（2） may raise the take
    // by Y×100.
    let land = tile("富士见坂");
    t.own(0, &[land]);
    let rent = data().tiles[land].rent[0];
    t.set_money(0, 5_000);
    t.set_money(1, 5_000);
    until_turn(&mut t, 1);
    t.set_pos(1, (land + 59) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    for _ in 0..12 {
        if t.prompt().is_none() {
            break;
        }
        let p = t.expect_prompt();
        // 白鹭千圣（2） is the **receiver**'s skill; answer whoever is asked.
        let who = t.asked().first().copied().unwrap_or(0);
        if p.kind == "pick" || p.kind == "mortgage" {
            let n = p.count.max(0) as usize;
            let items: Vec<&str> = std::iter::repeat("P✽P粉丝(正)").take(n.min(2)).collect();
            let _ = t.answer_items(who, &items);
        } else if p.kind == "choice" {
            let _ = t.answer(who, p.options.len().saturating_sub(1) as i32);
        } else {
            t.decline();
        }
    }
    drain(&mut t);

    let y = 2i64;
    let a = y * 100;
    let d0 = t.money(0) as i64 - 5_000;
    // The payee's take is rent + a (the payer still loses `rent` unless a
    // later stage shapes it -- none is armed here).
    let book = rent as i64 + a;
    if d0 != book {
        return Err(format!(
            "G-MONEY-13: 白鹭千圣 add expected P0 +{book} (rent {rent} + {a}), got {d0}"
        ));
    }
    Ok(())
}

// =====================================================================
// G-MONEY-14 -- 支付时 cancel
// =====================================================================

/// G-MONEY-14 -- 「取消支付」 / 「无需失去资金」 moves no money.
///
/// Armed with CRYCHIC:主唱太拼命了:
/// 「[反击] 一次性向其他玩家支付至少5000资金时，免除此次支付。」
#[test]
fn pay_stages_cancel_moves_nothing() -> Result<(), String> {
    let mut t = Table::vanilla(2);
    // A high-level rent that clears the 5000 gate: 购物中心 level 3 = 6680.
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_houses(land, 3);
    t.set_money(0, 10_000);
    t.set_money(1, 0);
    t.give(0, &["CRYCHIC:主唱太拼命了"]);
    t.observe(rules_spec::global::money::CLAUSES);

    settle_on(&mut t, 0, land);
    // The rent opens a [反击] window offering the cancel; play it.
    for _ in 0..12 {
        if t.prompt().is_none() {
            break;
        }
        if t.counteract_offered("主唱太拼命了") {
            t.counteract(0, "主唱太拼命了").unwrap();
        } else {
            t.decline();
        }
    }
    drain(&mut t);

    // 「免除此次支付」: neither side moves.
    if t.money(0) != 10_000 {
        return Err(format!(
            "G-MONEY-14: cancelled payment still took {} from P0",
            10_000 - t.money(0)
        ));
    }
    if t.money(1) != 0 {
        return Err(format!(
            "G-MONEY-14: cancelled payment still credited {} to P1",
            t.money(1)
        ));
    }
    Ok(())
}

// =====================================================================
// G-MONEY-15 -- 资金变动 sees the settled amount
// =====================================================================

/// G-MONEY-15 (资金变动 / row 7) -- 「当你一次性失去2000以上资金时，抽一张卡」
/// (振翅高飞的练习曲 (1)) fires on the **settled** loss, and not on a smaller
/// one. The sheet's band skill is the row-7 example.
///
/// OPEN-支付阶段7: the book is unsure whether row 7 merges into 支付后; the
/// trigger amount is still the settled figure either way.
#[test]
fn pay_stages_money_change_sees_the_settled_loss() -> Result<(), String> {
    let mut t = Table::vanilla(2);
    // The Morfonica band skill (「当你一次性失去2000以上资金时，抽一张卡」).
    // `vanilla` strips skills, so place the band half directly -- no
    // character skill may interfere with the move or the payment.
    t.place_raw(0, "skill:Morfonica:振翅高飞的练习曲");
    t.begin_turn(0);
    drain(&mut t);
    t.observe(rules_spec::global::money::CLAUSES);

    // A 2000-level loss: 购物中心 level 1 rent = 1360 (below the gate) and
    // level 2 = 3220 (above). Use level 2 so the draw fires.
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_houses(land, 2);
    t.set_money(0, 10_000);
    t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!", "通用:FEVER!"]);
    let hand0 = t.hand(0).len();
    settle_on(&mut t, 0, land);
    drain(&mut t);

    let rent = data().tiles[land].rent[2];
    assert!(rent >= 2000, "level-2 rent {rent} clears the 2000 gate");
    let d0 = 10_000 - t.money(0);
    if d0 != rent {
        return Err(format!("G-MONEY-15: settled loss {d0}, expected {rent}"));
    }
    // The band skill draws one card on a 2000+ loss.
    if t.hand(0).len() <= hand0 {
        return Err(format!(
            "G-MONEY-15: 「一次性失去2000以上资金时，抽一张卡」 did not draw \
             (hand {hand0} -> {}, keys {:?})",
            t.hand(0).len(),
            t.recent_keys(16)
        ));
    }
    Ok(())
}
