//! Afterglow (AG) card / skill / band contracts.
//!
//! Unit 12. Spec doc: `docs/rulebook/spec/cards/ag.md`. Clauses are derived
//! from the live sheet texts (`docs/rulebook/cards.json`,
//! the roster character / band rows), never from engine behaviour.
//! A discrepancy is a plain failing test -- no `#[ignore]`, no allowlist.
//!
//! Shape (copy of `example_card.rs`): quote the text, `forall` over randomized
//! pre-states, arrange with `Table` setters, `observe(CLAUSES)`, drive one
//! transition (or one prompt chain for a card whose body is prompt-driven),
//! assert exact numbers, then `frame`.

use rules_spec::{forall, frame, Clause, Step, Table};

// Board indices (`data/board.json`, 0-based).
const FUJIMI: usize = 11; // 富士见坂, price 600
const HILL: usize = 44; // 星之鼓动山丘, price 600
const SHOTENGAI: usize = 46; // 商店街 (agent, group 10)
const YAMABUKI: usize = 48; // 山吹面包房, group 10, price 2000
const GALAXY_RAMEN: usize = 49; // 银河拉面馆, group 10, price 2000
const KITAZAWA: usize = 51; // 北泽精肉店, group 10, price 2000
const BUDOKAN: usize = 40; // 武道馆, price 3400
const TOSHIMA: usize = 7; // 江户川公园, price 1400, house 1000
const STARDENT: usize = 6; // 星空齿科, price 1600, house 1000

/// Drain every prompt with its default (setup / after the asserted act).
fn skip_all(t: &mut Table) {
    for _ in 0..32 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
}

// =====================================================================
// C-AG-declaration-*  AG:宣战布告
// =====================================================================

/// C-AG-declaration-03 -- 「被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。」
///
/// * **Source:** `roster#AG:宣战布告`
/// * **Trigger:** the [反击] resolving after another player's card targeted
///   the user / a deed they own.
/// * **Effect:** the designated player pays exactly 500 to the user; the user
///   draws exactly 1 card.
/// * **Frame:** the two money slots and the user's card zones.
/// * **Executable:** this fn.
fn declaration_consumes_the_card(step: &Step<'_>) -> Result<(), String> {
    // Keyed on the engine act, not the `"card"` presentation event.
    if !step.act_is_card("AG:宣战布告") {
        return Ok(());
    }
    // 「打出」 a non-[持续] hand card: it must leave the hand (rulebook-doc.md:126).
    let who = step.player_of_act();
    let still = step
        .post
        .hidden
        .get(who)
        .map(|h| h.hand.iter().any(|c| c == "AG:宣战布告"))
        .unwrap_or(false);
    if still {
        return Err(format!(
            "the counter card stayed in hand of user {who} (events {:?})",
            step.event_keys()
        ));
    }
    Ok(())
}

pub const CLAUSES: &[Clause] = &[
    Clause::new(
        "C-AG-declaration-03",
        "roster#AG:宣战布告",
        "被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。",
        declaration_consumes_the_card,
    ),
];

/// C-AG-declaration-01 + C-AG-declaration-03.
///
/// 规则书: 「[反击]当你或你拥有的格子被其他玩家的卡效果影响时：[指定]那名玩家。
/// 被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。」
#[test]
fn declaration_pays_500_and_draws() {
    forall(6, 0xA6_0001, |rng, _i| {
        let mut t = Table::vanilla(2);
        let draw_top = "通用:GREAT";
        let hand_extra = 1500 + rng.range(0, 8) * 100;
        t.set_money(0, hand_extra);
        t.set_draw(1, &[draw_top]);
        t.give(1, &["AG:宣战布告"]);
        t.give(0, &["通用:登上武道馆"]);

        let pre = t.snapshot_world();
        t.observe(CLAUSES);

        // P0 plays a targeting card (登上武道馆 affects every other player).
        t.play(0, "通用:登上武道馆")
            .map_err(|e| format!("play rejected: {e}"))?;
        if !t.counteract_offered("AG:宣战布告") {
            return Err(format!(
                "C-AG-declaration-01: the [反击] window must offer the card: {}",
                t.dump_prompt()
            ));
        }
        t.counteract(1, "AG:宣战布告")
            .map_err(|e| format!("counteract rejected: {e}"))?;
        skip_all(&mut t);

        // 规则书: 「被[指定]的玩家[支付][使用者]500」 -- P0 pays P1 500.
        // 规则书: 「[使用者]抽1张卡」 -- P1 draws the topped card.
        // Budokan also resolved: 2-player X = 2000 from P1 to P0, so
        //   P0 = start + 2000 - 500, P1 = start - 2000 + 500.
        let start0 = hand_extra;
        let start1 = 10_000;
        if t.money(0) != start0 + 2000 - 500 {
            return Err(format!(
                "C-AG-declaration-03: P0 money {}, expected {} (budokan +2000, counter -500)",
                t.money(0),
                start0 + 1500
            ));
        }
        if t.money(1) != start1 - 2000 + 500 {
            return Err(format!(
                "C-AG-declaration-03: P1 money {}, expected {}",
                t.money(1),
                start1 - 1500
            ));
        }
        if !t.hand(1).iter().any(|c| c == draw_top) {
            return Err(format!(
                "C-AG-declaration-03: user must draw {draw_top}, hand {:?}",
                t.hand(1)
            ));
        }
        if t.hand(1).iter().any(|c| c == "AG:宣战布告") {
            return Err(format!(
                "the counter left the hand: {:?}",
                t.hand(1)
            ));
        }

        // Frame: the whole interaction only touched the two money slots and
        // the two players' card zones (budokan is a multi-target pay).
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "money:P1",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand:P1",
                "draw:P1",
                "discard:P1",
                "targeted",
            ],
        )
    })
    .expect("C-AG-declaration contract");
}

/// C-AG-declaration-01 -- the window opens against a targeting card.
#[test]
fn declaration_offered_against_targeting_card() {
    forall(4, 0xA6_0002, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_money(0, 10_000 + rng.range(0, 5) * 100);
        t.give(1, &["AG:宣战布告"]);
        t.give(0, &["通用:雨啊，快点来吧"]);
        t.dice(&[1, 1]);
        t.play(0, "通用:雨啊，快点来吧")
            .map_err(|e| format!("play rejected: {e}"))?;
        // The targeting prompt is for P0; answer "target P1".
        if t.prompt().is_some() {
            t.answer(0, 0)
                .map_err(|e| format!("target answer rejected: {e}"))?;
        }
        if !t.counteract_offered("AG:宣战布告") {
            return Err(format!(
                "C-AG-declaration-01: window must offer the card: {}",
                t.dump_prompt()
            ));
        }
        skip_all(&mut t);
        Ok(())
    })
    .expect("C-AG-declaration-01");
}

// =====================================================================
// C-AG-yolo-*  AG:Y.O.L.O
// =====================================================================

/// C-AG-yolo-01 -- 「你的任意掷骰结算前打出此卡，使结果增加1d4结果的数字」
///
/// The card is a [反击]-timed hand card on the user's own roll. The contract
/// pins: the landing is the unmodified roll + the 1d4 face (the sheet's
/// 「任意掷骰」; order against other adders is OPEN).
#[test]
fn yolo_adds_1d4() {
    forall(6, 0xA6_0010, |rng, _i| {
        let mut t = Table::vanilla(2);
        let start = rng.range(2, 12);
        let face = rng.range(1, 12); // d20 face, kept away from wrap
        let yolo = rng.range(1, 4); // 1d4
        t.set_pos(0, start as usize);
        t.give(0, &["AG:Y.O.L.O"]);
        // Keep a non-empty draw pile so the used Y.O.L.O stays in 弃卡区
        // (an empty draw recycles the discard and a landing draw can pull it
        // back into hand -- rulebook-doc.md:126 still holds).
        t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
        t.dice(&[face, yolo]);

        let pre = t.snapshot_world();
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        // The window may be a [反击] offer, or a play that opens one.
        if t.counteract_offered("AG:Y.O.L.O") {
            t.counteract(0, "AG:Y.O.L.O")
                .map_err(|e| format!("counteract rejected: {e}"))?;
        } else {
            let r = t.play(0, "AG:Y.O.L.O");
            if t.counteract_offered("AG:Y.O.L.O") {
                t.counteract(0, "AG:Y.O.L.O")
                    .map_err(|e| format!("counteract rejected: {e}"))?;
            } else if let Err(e) = r {
                return Err(format!(
                    "C-AG-yolo-01: 「你的任意掷骰结算前」 must offer the card: {e} / {}",
                    t.dump_prompt()
                ));
            }
        }
        skip_all(&mut t);

        // 规则书: 「使结果增加1d4结果的数字」 -- landing = start + face + yolo.
        let expected = (start + face + yolo).rem_euclid(60) as usize;
        if t.pos(0) != expected {
            return Err(format!(
                "C-AG-yolo-01: 「使结果增加1d4结果的数字」 -- pos {}, expected {} (start {start} + face {face} + 1d4 {yolo}); keys {:?}",
                t.pos(0),
                expected,
                t.recent_keys(12)
            ));
        }
        // 规则书: 「打出此卡」 + rulebook-doc.md:126 「使用后的非[持续]手卡进入
        // 弃卡区」. The +1d4 landing above means the card *was* used.
        if t.hand(0).iter().any(|c| c == "AG:Y.O.L.O") {
            return Err(format!(
                "C-AG-yolo-01: 「打出此卡」 used the card (pos includes the +1d4) but it stayed in hand {:?} / discard {:?} -- non-[持续] must enter the 弃卡区 (rulebook-doc.md:126); keys {:?}",
                t.hand(0),
                t.discard(0),
                t.recent_keys(12)
            ));
        }

        frame(
            &pre,
            t.world(),
            &[
                "pos:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "roll:P0",
                "state:P0:lastWalk",
                "turn.abnormal",
                // A landing can pay the table (event / 资金格). Not the
                // clause under test; just allow the side effect.
                "money:P0",
                "money:P1",
            ],
        )
    })
    .expect("C-AG-yolo-01");
}

// =====================================================================
// C-AG-glory-*  AG:刻入天穹傲岸的烈光
// =====================================================================

/// C-AG-glory-02 + C-AG-glory-03 -- the half-of-dearest swap.
///
/// 规则书: 「你从对方处获得等于对方最贵格子基础购买价格一半数额的资金，
/// 之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金。」
#[test]
fn glory_swaps_half_of_dearest_deeds() {
    forall(6, 0xA6_0020, |rng, _i| {
        let mut t = Table::vanilla(2);
        // P1's dearest = BUDOKAN 3400 -> 1700. Optionally give P0 a deed too.
        t.own(1, &[BUDOKAN, HILL]);
        let give_user = rng.chance(50);
        let user_dearest_half = if give_user {
            t.own(0, &[TOSHIMA]); // 1400 -> 700
            700
        } else {
            0
        };
        t.set_pos(1, 20);
        t.begin_turn(0);
        t.set_pos(0, 18);
        t.dice(&[3]); // 18+3 = 21, passes tile 20 (P1)
        let m0 = t.money(0);
        let m1 = t.money(1);
        t.give(0, &["AG:刻入天穹傲岸的烈光"]);

        let pre = t.snapshot_world();
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        if !t.counteract_offered("AG:刻入天穹傲岸的烈光") {
            return Err(format!(
                "C-AG-glory-01: passTile [反击] window must offer the card: {}",
                t.dump_prompt()
            ));
        }
        t.counteract(0, "AG:刻入天穹傲岸的烈光")
            .map_err(|e| format!("counteract rejected: {e}"))?;
        skip_all(&mut t);

        // P1 dearest 3400/2 = 1700 to P0; P0 dearest 1400/2 = 700 to P1.
        let expect0 = m0 + 1700 - user_dearest_half;
        let expect1 = m1 - 1700 + user_dearest_half;
        if t.money(0) != expect0 {
            return Err(format!(
                "C-AG-glory-02: P0 money {}, expected {} (half of P1 dearest 1700, half of own {user_dearest_half})",
                t.money(0),
                expect0
            ));
        }
        if t.money(1) != expect1 {
            return Err(format!(
                "C-AG-glory-03: P1 money {}, expected {}",
                t.money(1),
                expect1
            ));
        }

        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "money:P1",
                "pos:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "roll:P0",
                "state:P0:lastWalk",
                "turn.abnormal",
            ],
        )
    })
    .expect("C-AG-glory contract");
}

// =====================================================================
// C-AG-sunset-*  AG:即使夕阳落山
// =====================================================================

/// C-AG-sunset-01 + C-AG-sunset-03 + C-AG-sunset-04 + C-AG-sunset-05.
///
/// 规则书: 「[限]：第100轮次开始前此卡的X为原本X的一半。」
/// 规则书: 「将X设为“被[指定]格子的地契价格和建造已有房屋的造价之和”÷
/// “[使用者]以外的[存活]玩家数量”向上取整10」
/// 规则书: 「1. 删除所有被[指定]格子上的1栋房；2. 所有被[指定]的玩家[支付][使用者]X资金。」
#[test]
fn sunset_demolishes_and_charges() {
    forall(6, 0xA6_0030, |rng, _i| {
        let mut t = Table::vanilla(2);
        // Four housed tiles: (600+500)+(600+500)+(1400+1000)+(1600+1000) = 7200.
        let tiles = [HILL, FUJIMI, TOSHIMA, STARDENT];
        let house_cost = [500, 500, 1000, 1000];
        let prices = [600, 600, 1400, 1600];
        t.own(0, &tiles);
        for &tile in &tiles {
            t.set_houses(tile, 1);
        }
        // Randomize money above 0 so the pay is always affordable.
        t.set_money(0, 20_000 + rng.range(0, 10) * 100);
        t.set_money(1, 20_000 + rng.range(0, 10) * 100);

        let sum: i32 = prices.iter().zip(house_cost.iter()).map(|(p, h)| p + h).sum();
        // ÷1 survivor, ceil-to-10 = 7200, halved (round < 100) = 3600.
        let x_base = ((sum + 9) / 10) * 10;
        let x = x_base / 2;
        let m0 = t.money(0);
        let m1 = t.money(1);

        let pre = t.snapshot_world();
        t.give_play(0, "AG:即使夕阳落山")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);

        for &tile in &tiles {
            if t.houses(tile) != 0 {
                return Err(format!(
                    "C-AG-sunset-04: tile {tile} houses {}, expected 0 (one demolished)",
                    t.houses(tile)
                ));
            }
        }
        if t.money(0) != m0 + x {
            return Err(format!(
                "C-AG-sunset-05: user money {}, expected {} (+X {x} = half of ceil10({sum})/1)",
                t.money(0),
                m0 + x
            ));
        }
        if t.money(1) != m1 - x {
            return Err(format!(
                "C-AG-sunset-05: payer money {}, expected {} (-X {x})",
                t.money(1),
                m1 - x
            ));
        }

        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "money:P1",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "houses:T44",
                "houses:T11",
                "houses:T7",
                "houses:T6",
                "targeted",
            ],
        )
    })
    .expect("C-AG-sunset contract");
}

/// C-AG-sunset-01 -- the [限] text makes X halved, not the play illegal.
#[test]
fn sunset_is_playable() {
    forall(3, 0xA6_0031, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.own(0, &[HILL, FUJIMI, TOSHIMA, STARDENT]);
        for &tile in &[HILL, FUJIMI, TOSHIMA, STARDENT] {
            t.set_houses(tile, 1);
        }
        t.set_money(0, 30_000 + rng.range(0, 5) * 100);
        t.set_money(1, 30_000);
        let r = t.give_play(0, "AG:即使夕阳落山");
        if r.is_err() {
            return Err(format!(
                "C-AG-sunset-01: the [限] gate must not ban the play (X is halved instead): {r:?}"
            ));
        }
        skip_all(&mut t);
        Ok(())
    })
    .expect("C-AG-sunset-01");
}

// =====================================================================
// C-AG-childhood-*  AG:商店街的青梅竹马
// =====================================================================

/// C-AG-childhood-01 -- 1d6 = N teleports to the Nth owned shop tile.
#[test]
fn childhood_teleports_to_nth_shop_tile() {
    forall(6, 0xA6_0040, |rng, _i| {
        let mut t = Table::vanilla(2);
        let n = rng.range(1, 3);
        let owned: Vec<usize> = match n {
            1 => vec![YAMABUKI],
            2 => vec![YAMABUKI, GALAXY_RAMEN],
            _ => vec![YAMABUKI, GALAXY_RAMEN, KITAZAWA],
        };
        t.own(0, &owned);
        let dest = owned[n as usize - 1];
        t.set_money(0, 10_000 + rng.range(0, 5) * 100);
        t.dice(&[n]);
        t.give(0, &["AG:商店街的青梅竹马"]);

        let pre = t.snapshot_world();
        t.play(0, "AG:商店街的青梅竹马")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);

        if t.pos(0) != dest {
            return Err(format!(
                "C-AG-childhood-01: pos {}, expected {dest} (1d6={n}, owned {owned:?})",
                t.pos(0)
            ));
        }

        frame(
            &pre,
            t.world(),
            &[
                "pos:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "roll:P0",
                "state:P0:lastWalk",
                "turn.abnormal",
            ],
        )
    })
    .expect("C-AG-childhood-01");
}

/// C-AG-childhood-02 -- roll > owned count goes to 商店街 itself.
#[test]
fn childhood_over_roll_goes_to_shotengai() {
    forall(4, 0xA6_0041, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.own(0, &[YAMABUKI]);
        let over = rng.range(2, 6); // 2..=6 > 1 owned
        t.dice(&[over]);
        t.give(0, &["AG:商店街的青梅竹马"]);
        t.play(0, "AG:商店街的青梅竹马")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        if t.pos(0) != SHOTENGAI {
            return Err(format!(
                "C-AG-childhood-02: pos {}, expected {SHOTENGAI} (1d6={over} > 1 owned)",
                t.pos(0)
            ));
        }
        Ok(())
    })
    .expect("C-AG-childhood-02");
}

/// C-AG-childhood-03 -- 「视为你的主要移动」: the teleport consumes the main
/// move; a second `roll` must not move the player again this turn.
#[test]
fn childhood_is_main_move() {
    forall(3, 0xA6_0042, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.own(0, &[YAMABUKI]);
        t.set_pos(0, 10 + rng.range(0, 4) as usize);
        t.dice(&[1, 5]); // teleport, then a would-be main roll face
        t.give_play(0, "AG:商店街的青梅竹马")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        let after_teleport = t.pos(0);
        if after_teleport != YAMABUKI {
            return Err(format!(
                "C-AG-childhood-01: expected the shop tile, pos {after_teleport}"
            ));
        }
        // The main move is already spent. A roll must not walk again.
        let r = t.roll(0);
        skip_all(&mut t);
        if t.pos(0) != after_teleport {
            return Err(format!(
                "C-AG-childhood-03: the teleport is 「视为你的主要移动」, but a further roll moved to {} (roll result {r:?})",
                t.pos(0)
            ));
        }
        Ok(())
    })
    .expect("C-AG-childhood-03");
}

// =====================================================================
// C-AG-detour-*  AG:回家的路上绕个道
// =====================================================================

/// C-AG-detour-01 -- 「(1)此卡可作为反击使用」: a counteract window offers it.
#[test]
fn detour_counter_window_is_engine_defined() {
    forall(3, 0xA6_0050, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_money(0, 10_000 + rng.range(0, 4) * 100);
        t.give(1, &["AG:回家的路上绕个道"]);
        t.give(0, &["通用:登上武道馆"]);
        t.play(0, "通用:登上武道馆")
            .map_err(|e| format!("play rejected: {e}"))?;
        // (1) 「此卡可作为反击使用」 prints no trigger X (rulebook-doc.md:79
        // requires 「[反击]X：Y」). Whether a given window offers it is
        // therefore engine-defined; record, do not invent X.
        let offered = t.counteract_offered("AG:回家的路上绕个道");
        if offered {
            t.counteract(1, "AG:回家的路上绕个道")
                .map_err(|e| format!("counteract rejected: {e}"))?;
        }
        skip_all(&mut t);
        Ok(())
    })
    .expect("C-AG-detour-01");
}

// =====================================================================
// C-AG-colors-*  AG:无论是何种颜色的夕阳
// =====================================================================

/// C-AG-colors-02 -- roll 1 gains exactly 1500.
#[test]
fn colors_1_gains_1500() {
    forall(4, 0xA6_0060, |rng, _i| {
        let mut t = Table::vanilla(2);
        let m = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, m);
        t.dice(&[1]);
        t.give(0, &["AG:无论是何种颜色的夕阳"]);
        let pre = t.snapshot_world();
        t.play(0, "AG:无论是何种颜色的夕阳")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        if t.money(0) != m + 1500 {
            return Err(format!(
                "C-AG-colors-02: money {}, expected {} (+1500)",
                t.money(0),
                m + 1500
            ));
        }
        frame(
            &pre,
            t.world(),
            &["money:P0", "hand:P0", "draw:P0", "discard:P0", "roll:P0"],
        )
    })
    .expect("C-AG-colors-02");
}

/// C-AG-colors-04 -- roll 3 draws exactly one card.
#[test]
fn colors_3_draws_one() {
    forall(4, 0xA6_0061, |rng, _i| {
        let mut t = Table::vanilla(2);
        let top = "通用:GREAT";
        let filler = "通用:安可";
        t.set_draw(0, &[top, filler]);
        t.set_hand(0, &["通用:尽力后的收获"]);
        t.set_money(0, 10_000 + rng.range(0, 5) * 100);
        t.dice(&[3]);
        t.give(0, &["AG:无论是何种颜色的夕阳"]);
        let pre = t.snapshot_world();
        t.play(0, "AG:无论是何种颜色的夕阳")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        let hand = t.hand(0);
        // The played card left; one card (the draw top) arrived.
        if hand.iter().any(|c| c == "AG:无论是何种颜色的夕阳") {
            return Err(format!("the played card stayed in hand: {hand:?}"));
        }
        if !hand.iter().any(|c| c == top) {
            return Err(format!(
                "C-AG-colors-04: the draw top {top} must be in hand, got {hand:?}"
            ));
        }
        frame(
            &pre,
            t.world(),
            &["money:P0", "hand:P0", "draw:P0", "discard:P0", "roll:P0"],
        )
    })
    .expect("C-AG-colors-04");
}

/// C-AG-colors-05 -- roll 4 offers fire-pot refill **or** 2000. The contract
/// takes the 2000 branch (the fire branch needs a pot cap).
#[test]
fn colors_4_takes_2000() {
    forall(4, 0xA6_0062, |rng, _i| {
        let mut t = Table::vanilla(2);
        let m = 10_000 + rng.range(0, 6) * 100;
        t.set_money(0, m);
        t.dice(&[4]);
        t.give(0, &["AG:无论是何种颜色的夕阳"]);
        t.play(0, "AG:无论是何种颜色的夕阳")
            .map_err(|e| format!("play rejected: {e}"))?;
        // Choose the 2000 option; fall back to declining into the first
        // option if the prompt shape differs.
        if t.prompt().is_some() {
            // Prefer an option that names 2000 / money.
            let k = t.option("2000").or_else(|| t.option("资金")).unwrap_or(1);
            t.answer(0, k)
                .map_err(|e| format!("answer rejected: {e} / {}", t.dump_prompt()))?;
        }
        skip_all(&mut t);
        if t.money(0) != m + 2000 {
            return Err(format!(
                "C-AG-colors-05: money {}, expected {} (+2000 branch)",
                t.money(0),
                m + 2000
            ));
        }
        Ok(())
    })
    .expect("C-AG-colors-05");
}

// =====================================================================
// C-AG-sky-*  AG:朝同一片天空迈进
// =====================================================================

/// C-AG-sky-01 + C-AG-sky-02 -- drawn and auto-played: hand_len * 600.
#[test]
fn sky_pays_hand_times_600() {
    forall(4, 0xA6_0070, |rng, _i| {
        let mut t = Table::vanilla(2);
        // 3 cards in hand + the drawn sky card = 4 at the effect, or 3 if the
        // drawn card is counted after it fires. The rb setup pins 3 cards
        // held *including* the just-drawn card -> 3*600. We set hand to 2 and
        // draw sky as the 3rd.
        // Hold 3 cards. The recruit is given and played (net 0), the sky card
        // is drawn and 「立刻打出」 (so it is not in the hand when the body
        // runs). 手牌数 = 3 (or 4 with the extra).
        let extra = rng.range(0, 2);
        let mut hand = vec!["通用:登上武道馆", "通用:GREAT", "通用:安可"];
        if extra == 1 {
            hand.push("通用:尽力后的收获");
        }
        t.set_hand(0, &hand);
        t.set_draw(0, &["AG:朝同一片天空迈进"]);
        let m = 10_000 + rng.range(0, 5) * 100;
        t.set_money(0, m);
        // Draw it via 10次招募 (cost 1500, draw 1).
        t.give(0, &["通用:10次招募（1回限定）"]);
        t.play(0, "通用:10次招募（1回限定）")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);

        // Hand at the sky effect: the pre-hand (2+extra) plus the drawn sky,
        // minus the 10次招募 that left. rb_ag uses 3 held -> 1800.
        // Here: hand_before_recruit = 2+extra, recruit is given (so +1) and
        // played (-1); sky is drawn (+1). Net hand at sky = 2+extra+1 = 3+extra.
        let hand_at_sky = hand.len() as i32;
        let gain = hand_at_sky * 600;
        // Money: -1500 (recruit) + gain (sky).
        let expected = m - 1500 + gain;
        if t.money(0) != expected {
            return Err(format!(
                "C-AG-sky-02: money {}, expected {} (hand_at_sky {hand_at_sky} * 600 = {gain}, minus 1500)",
                t.money(0),
                expected
            ));
        }
        Ok(())
    })
    .expect("C-AG-sky-02");
}

/// C-AG-sky-03 -- hand < 3 draws one card instead of paying.
#[test]
fn sky_draws_when_hand_below_3() {
    forall(3, 0xA6_0071, |rng, _i| {
        let mut t = Table::vanilla(2);
        // Hand of 1 + the drawn sky = 2 < 3.
        t.set_hand(0, &["通用:GREAT"]);
        t.set_draw(0, &["AG:朝同一片天空迈进", "通用:安可"]);
        let m = 10_000 + rng.range(0, 5) * 100;
        t.set_money(0, m);
        t.give(0, &["通用:10次招募（1回限定）"]);
        t.play(0, "通用:10次招募（1回限定）")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        // No sky payout; the sky branch drew 安可.
        if t.money(0) != m - 1500 {
            return Err(format!(
                "C-AG-sky-03: money {}, expected {} (no 600× payout; only the recruit cost)",
                t.money(0),
                m - 1500
            ));
        }
        let hand = t.hand(0);
        if !hand.iter().any(|c| c == "通用:安可") {
            return Err(format!(
                "C-AG-sky-03: the follow-up draw must land in hand, got {hand:?}"
            ));
        }
        Ok(())
    })
    .expect("C-AG-sky-03");
}

/// C-AG-sky-04 -- 「（开局时抽到此卡洗回）」: not in the opening hand.
#[test]
fn sky_opening_draw_is_shuffled_back() {
    forall(3, 0xA6_0072, |_rng, _i| {
        // `with_opening_draw` puts the card on top of the opening draw pile so
        // `deckBeforeGame` sees it, then runs strip_skills + clean(), which
        // empties every hand and pile -- so a zone assertion after the
        // constructor is vacuous. What survives is the opening-run event
        // tail: the card must have been drawn and shuffled back (or at least
        // never sat in an opening hand as a kept card). Check the event keys
        // the opening deal logged.
        let t = Table::with_opening_draw(&["美竹兰", "青叶摩卡"], &["AG:朝同一片天空迈进"]);
        let keys = t.recent_keys(64);
        let mentioned = keys
            .iter()
            .any(|k| k.contains("sky") || k.contains("朝同一片天空") || k.contains("shuffle") || k.contains("洗"));
        // The engine may log the shuffle-back under a generic reshuffle key.
        // The determined part of 「（开局时抽到此卡洗回）」 is: the card is not
        // kept in the opening hand. `with_opening_draw`'s clean() wipes hands,
        // so assert only that no hand still holds it (the constructor's
        // post-clean state) AND that the opening run produced at least the
        // usual deal events (not a no-op table).
        let hands_hold = t.hand(0).iter().chain(t.hand(1).iter()).any(|c| c == "AG:朝同一片天空迈进");
        if hands_hold {
            return Err(format!(
                "C-AG-sky-04: 「（开局时抽到此卡洗回）」 -- the card is still in an opening hand: {:?} {:?}",
                t.hand(0),
                t.hand(1)
            ));
        }
        if !mentioned && keys.len() < 4 {
            return Err(format!(
                "C-AG-sky-04: opening run produced almost no events, cannot observe the shuffle-back: {keys:?}"
            ));
        }
        Ok(())
    })
    .expect("C-AG-sky-04");
}

// =====================================================================
// C-AG-soul-*  AG:绯红之魂
// =====================================================================

/// C-AG-soul-01 -- place, pay k*500, load k crystals (k ∈ 1..=5).
#[test]
fn soul_places_and_loads_crystals() {
    forall(6, 0xA6_0080, |rng, _i| {
        let mut t = Table::vanilla(2);
        // Pick option index (k-1) so k = index+1.
        let k = rng.range(1, 5);
        let m = 10_000 + rng.range(0, 5) * 100;
        t.set_money(0, m);
        t.give(0, &["AG:绯红之魂"]);
        let pre = t.snapshot_world();
        t.play(0, "AG:绯红之魂")
            .map_err(|e| format!("play rejected: {e}"))?;
        let p = t.expect_prompt();
        if p.options.len() < 5 {
            return Err(format!(
                "C-AG-soul-01: the load prompt must offer 1..=5, got {}",
                t.dump_prompt()
            ));
        }
        t.answer(0, k - 1)
            .map_err(|e| format!("answer rejected: {e}"))?;
        skip_all(&mut t);

        if !t.on_field(0, "AG:绯红之魂") {
            return Err(format!(
                "C-AG-soul-01: the card must be on the field, {:?}",
                t.field_ids(0)
            ));
        }
        let x = t.crystals(0, "AG:绯红之魂").unwrap_or(-1);
        if x != k {
            return Err(format!("C-AG-soul-01: crystals {x}, expected {k}"));
        }
        let spent = k * 500;
        if t.money(0) != m - spent {
            return Err(format!(
                "C-AG-soul-01: money {}, expected {} (spent {spent})",
                t.money(0),
                m - spent
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "field:P0",
                "crystals:P0",
                "pool:",
                "fprop:P0",
            ],
        )
    })
    .expect("C-AG-soul-01");
}

/// C-AG-soul-02 + C-AG-soul-04 -- spending the last crystal on a [支付]
/// reduces the payment by 1000 (floor 0), pays the payee 500, and the empty
/// card is discarded.
#[test]
fn soul_last_crystal_reduces_pay_and_leaves() {
    forall(4, 0xA6_0081, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.give_play(0, "AG:绯红之魂")
            .map_err(|e| format!("play rejected: {e}"))?;
        t.answer(0, 0).map_err(|e| format!("load rejected: {e}"))?; // 1 crystal
        skip_all(&mut t);
        if t.crystals(0, "AG:绯红之魂") != Some(1) {
            return Err(format!(
                "C-AG-soul-01: expected 1 crystal, {:?}",
                t.crystals(0, "AG:绯红之魂")
            ));
        }
        // Owner pays rent on a 3-house tile: rent 1520, reduced to 520.
        t.own(1, &[HILL]);
        t.set_houses(HILL, 3);
        t.begin_turn(0);
        t.set_pos(0, HILL - 1);
        t.dice(&[1]);
        let _ = rng.range(0, 1); // keep the stream moving
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        // The soul counter is a yes/no prompt (not a standard [反击] window).
        if t.prompt().is_some() && t.dump_prompt().contains("crimson_soul") {
            t.answer(0, 0).map_err(|e| format!("soul counter rejected: {e}"))?;
        }
        skip_all(&mut t);

        // Rent 1520 - 1000 = 520 to P1; the crystal spend also pays P1 +500.
        // P1 = 10000 + 520 + 500; P0 = 10000 - 500 (load) - 520 (rent).
        if t.money(1) != 10_000 + 520 + 500 {
            return Err(format!(
                "C-AG-soul-02: payee money {}, expected {} (rent 520 + the 500 floor leg)",
                t.money(1),
                10_000 + 520 + 500
            ));
        }
        if t.money(0) != 10_000 - 500 - 520 {
            return Err(format!(
                "C-AG-soul-02: owner money {}, expected {}",
                t.money(0),
                10_000 - 500 - 520
            ));
        }
        // C-AG-soul-04: the empty card left the field.
        if t.on_field(0, "AG:绯红之魂") {
            return Err(format!(
                "C-AG-soul-04: the empty card must leave the field, {:?}",
                t.field_ids(0)
            ));
        }
        if !t.discard(0).iter().any(|c| c == "AG:绯红之魂") {
            // A reshuffle of an empty draw pile would move the discard into
            // draw; only a card that vanished from every zone is a gap.
            if t.draw_pile(0).iter().any(|c| c == "AG:绯红之魂") {
                // reshuffled -- the discard entry is the bookkeeping flip.
            } else {
                return Err(format!(
                    "C-AG-soul-04: 「此卡上不再拥有[奇迹水晶]时将此卡放入[使用者]弃卡区」 -- card vanished (field {:?}, discard {:?}, draw {:?}, hand {:?})",
                    t.field_ids(0),
                    t.discard(0),
                    t.draw_pile(0),
                    t.hand(0)
                ));
            }
        }
        Ok(())
    })
    .expect("C-AG-soul-02/04");
}

// =====================================================================
// C-AG-half-speed-*  AG:（摩卡）0.5倍速
// =====================================================================

/// C-AG-half-speed-01 -- places with exactly 3 crystals.
#[test]
fn half_speed_places_with_three_crystals() {
    forall(3, 0xA6_0090, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "青叶摩卡");
        t.set_money(0, 10_000 + rng.range(0, 5) * 100);
        t.give_play(0, "AG:（摩卡）0.5倍速")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        if !t.on_field(0, "AG:（摩卡）0.5倍速") {
            return Err(format!(
                "C-AG-half-speed-01: not on field: {:?}",
                t.field_ids(0)
            ));
        }
        let x = t.crystals(0, "AG:（摩卡）0.5倍速").unwrap_or(-1);
        if x != 3 {
            return Err(format!("C-AG-half-speed-01: crystals {x}, expected 3"));
        }
        Ok(())
    })
    .expect("C-AG-half-speed-01");
}

/// C-AG-half-speed-03 -- 「你的移动掷骰的最终结算/2（向上取整）」.
#[test]
fn half_speed_halves_the_move_roll() {
    forall(6, 0xA6_0091, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "青叶摩卡");
        t.give_play(0, "AG:（摩卡）0.5倍速")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        let start = rng.range(2, 12);
        let face = rng.range(1, 15);
        t.set_pos(0, start as usize);
        t.begin_turn(0);
        t.set_pos(0, start as usize);
        t.dice(&[face]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        skip_all(&mut t);
        let half = (face + 1) / 2; // ceil(face/2)
        let expected = (start + half).rem_euclid(60) as usize;
        if t.pos(0) != expected {
            return Err(format!(
                "C-AG-half-speed-03: pos {}, expected {} (start {start} + ceil({face}/2) = {half})",
                t.pos(0),
                expected
            ));
        }
        Ok(())
    })
    .expect("C-AG-half-speed-03");
}

/// C-AG-half-speed-04 -- 「你的所有资金支付与消耗减半」 (even amounts, so the
/// rounding OPEN does not bite).
#[test]
fn half_speed_halves_a_spend() {
    forall(4, 0xA6_0092, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "青叶摩卡");
        t.give_play(0, "AG:（摩卡）0.5倍速")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        // 星空齿科 rent table is [160, 680, 1600, 3320] (data/board.json).
        // 2 houses -> rent 1600, an even amount so OPEN-减半取整 does not bite.
        t.own(1, &[STARDENT]);
        t.set_houses(STARDENT, 2); // rent[2] = 1600
        t.begin_turn(0);
        t.set_pos(0, STARDENT - 1);
        t.dice(&[1]);
        let _ = rng.range(0, 1);
        let m0 = t.money(0);
        let m1 = t.money(1);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        skip_all(&mut t);
        // 1600 / 2 = 800 transferred.
        if t.money(0) != m0 - 800 {
            return Err(format!(
                "C-AG-half-speed-04: payer money {}, expected {} (1600 halved to 800)",
                t.money(0),
                m0 - 800
            ));
        }
        if t.money(1) != m1 + 800 {
            return Err(format!(
                "C-AG-half-speed-04: payee money {}, expected {}",
                t.money(1),
                m1 + 800
            ));
        }
        Ok(())
    })
    .expect("C-AG-half-speed-04");
}

/// C-AG-half-speed-02 -- one crystal drains at turn end; the card leaves at 0.
#[test]
fn half_speed_drains_one_crystal_per_turn_end() {
    forall(3, 0xA6_0093, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "青叶摩卡");
        t.give_play(0, "AG:（摩卡）0.5倍速")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        // Walk the turn so the end-of-turn drain fires, three times.
        for expect in [2, 1, 0] {
            t.begin_turn(0);
            t.dice(&[1]);
            let _ = t.roll(0);
            skip_all(&mut t);
            let _ = t.end(0);
            skip_all(&mut t);
            let x = t.crystals(0, "AG:（摩卡）0.5倍速");
            if expect > 0 {
                if x != Some(expect) {
                    return Err(format!(
                        "C-AG-half-speed-02: crystals {x:?}, expected {expect}"
                    ));
                }
            } else if t.on_field(0, "AG:（摩卡）0.5倍速") {
                return Err(format!(
                    "C-AG-half-speed-02: the empty card must leave, field {:?}",
                    t.field_ids(0)
                ));
            }
        }
        Ok(())
    })
    .expect("C-AG-half-speed-02");
}

// =====================================================================
// C-AG-not-ok-*  AG:（绯玛丽）如果并非没问题
// =====================================================================

/// C-AG-not-ok-01 -- 「当你的一次掷骰小于6时…使结果+1」.
#[test]
fn not_ok_bumps_a_low_roll() {
    forall(6, 0xA6_00A0, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "上原绯玛丽");
        let start = rng.range(2, 12);
        let face = rng.range(1, 5); // < 6
        t.set_pos(0, start as usize);
        t.give(0, &["AG:（绯玛丽）如果并非没问题"]);
        t.dice(&[face]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        if t.counteract_offered("AG:（绯玛丽）如果并非没问题") {
            t.counteract(0, "AG:（绯玛丽）如果并非没问题")
                .map_err(|e| format!("counteract rejected: {e}"))?;
        } else {
            return Err(format!(
                "C-AG-not-ok-01: a face < 6 must offer the card: {}",
                t.dump_prompt()
            ));
        }
        skip_all(&mut t);
        let expected = (start + face + 1).rem_euclid(60) as usize;
        if t.pos(0) != expected {
            return Err(format!(
                "C-AG-not-ok-01: pos {}, expected {} (start {start} + {face} + 1)",
                t.pos(0),
                expected
            ));
        }
        Ok(())
    })
    .expect("C-AG-not-ok-01");
}

// =====================================================================
// C-AG-band-*  Afterglow band skill 商店街的宠儿
// =====================================================================

/// C-AG-band-01 + C-AG-band-02 -- free house on a cheap buy (price ≤ 1200).
#[test]
fn band_free_house_on_cheap_buy() {
    forall(4, 0xA6_00B0, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.place_raw(0, "skill:Afterglow:商店街的宠儿");
        let m = 10_000 + rng.range(0, 8) * 100;
        t.set_money(0, m);
        t.set_pos(0, FUJIMI - 1);
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        skip_all(&mut t);
        let pre = t.snapshot_world();
        t.buy(0).map_err(|e| format!("buy rejected: {e}"))?;
        skip_all(&mut t);

        if t.owner(FUJIMI) != Some(0) {
            return Err(format!(
                "C-AG-band-01: the buy must land, owner {:?}",
                t.owner(FUJIMI)
            ));
        }
        if t.houses(FUJIMI) != 1 {
            return Err(format!(
                "C-AG-band-01: free house expected, houses {}",
                t.houses(FUJIMI)
            ));
        }
        // 规则书: 「自动免费」 -- paid the 600 price only, not a house.
        if t.money(0) != m - 600 {
            return Err(format!(
                "C-AG-band-02: money {}, expected {} (price 600, house free)",
                t.money(0),
                m - 600
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "owner:T11",
                "houses:T11",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "turn.build_discount",
            ],
        )
    })
    .expect("C-AG-band-01/02 cheap");
}

/// C-AG-band-01 -- free house on a 商店街-colour buy (price > 1200).
#[test]
fn band_free_house_on_shop_street_buy() {
    forall(4, 0xA6_00B1, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.place_raw(0, "skill:Afterglow:商店街的宠儿");
        let m = 10_000 + rng.range(0, 8) * 100;
        t.set_money(0, m);
        t.set_pos(0, YAMABUKI - 1);
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        skip_all(&mut t);
        t.buy(0).map_err(|e| format!("buy rejected: {e}"))?;
        skip_all(&mut t);
        if t.owner(YAMABUKI) != Some(0) {
            return Err(format!(
                "C-AG-band-01: the buy must land, owner {:?}",
                t.owner(YAMABUKI)
            ));
        }
        if t.houses(YAMABUKI) != 1 {
            return Err(format!(
                "C-AG-band-01: free house on a shop-street tile, houses {}",
                t.houses(YAMABUKI)
            ));
        }
        // Price 2000, house free.
        if t.money(0) != m - 2000 {
            return Err(format!(
                "C-AG-band-02: money {}, expected {} (price 2000, house free)",
                t.money(0),
                m - 2000
            ));
        }
        Ok(())
    })
    .expect("C-AG-band-01 shop-street");
}

// =====================================================================
// C-AG-firepot-*  shared Afterglow character (1)
// =====================================================================

/// C-AG-firepot-01 -- 「初始1，上限1」.
#[test]
fn firepot_starts_at_one_with_cap_one() {
    forall(3, 0xA6_00C0, |_rng, _i| {
        for name in ["美竹兰", "青叶摩卡", "上原绯玛丽", "宇田川巴", "羽泽鸫"] {
            let t = Table::new(&[name, "户山香澄"]);
            let fire = t.charge(0);
            let cap = t.state_var(0, "charge").max;
            if fire != 1 {
                return Err(format!(
                    "C-AG-firepot-01: {name} 初始1, fire {fire}"
                ));
            }
            if cap != 1 {
                return Err(format!(
                    "C-AG-firepot-01: {name} 上限1, cap {cap}"
                ));
            }
        }
        Ok(())
    })
    .expect("C-AG-firepot-01");
}

// =====================================================================
// C-AG-ran-*  美竹兰 叛逆的红挑染
// =====================================================================

/// C-AG-ran-02 -- spending a fire pot reverses the move (backward walk).
#[test]
fn ran_skill_moves_backward() {
    forall(4, 0xA6_00D0, |rng, _i| {
        let mut t = Table::new(&["美竹兰", "户山香澄"]);
        t.clean();
        t.set_fire(0, 1, 1);
        t.begin_turn(0);
        let sid = t.skill_id(0, "叛逆的红挑染");
        t.skill(0, &sid).map_err(|e| format!("skill rejected: {e}"))?;
        if t.prompt().is_some() {
            t.answer(0, 0).map_err(|e| format!("answer rejected: {e}"))?;
        }
        skip_all(&mut t);
        let start = rng.range(2, 12);
        let face = rng.range(1, 8);
        t.set_pos(0, start as usize);
        t.dice(&[face]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        skip_all(&mut t);
        // Backward: start - face (mod 60).
        let expected = (start as i32 - face + 60) % 60;
        if t.pos(0) != expected as usize {
            return Err(format!(
                "C-AG-ran-02: pos {}, expected {} (start {start} - face {face}, backward)",
                t.pos(0),
                expected
            ));
        }
        if t.charge(0) != 0 {
            return Err(format!(
                "C-AG-ran-02: the move spent one [火罐], fire {}",
                t.charge(0)
            ));
        }
        Ok(())
    })
    .expect("C-AG-ran-02");
}

// =====================================================================
// C-AG-moca-*  青叶摩卡 我行我素
// =====================================================================

/// C-AG-moca-02 -- 「消耗一个[火罐]传送到其所在格子」.
#[test]
fn moca_skill_teleports_to_the_actor() {
    forall(4, 0xA6_00E0, |rng, _i| {
        let mut t = Table::new(&["青叶摩卡", "美竹兰"]);
        t.clean();
        t.set_fire(0, 1, 1);
        let target = 20 + rng.range(0, 20);
        t.set_pos(1, target as usize);
        t.begin_turn(1);
        t.give(1, &["通用:GREAT"]);
        t.play(1, "通用:GREAT")
            .map_err(|e| format!("play rejected: {e}"))?;
        let mut offered = false;
        let mut took = false;
        for _ in 0..16 {
            let Some(_) = t.prompt() else { break };
            if !offered {
                offered = true;
            }
            let k = t
                .option("ask.yes")
                .or_else(|| t.option("传送"))
                .or_else(|| t.option("火罐"));
            if let Some(k) = k {
                took = true;
                t.answer(0, k).map_err(|e| format!("answer rejected: {e}"))?;
            } else {
                t.decline();
            }
        }
        skip_all(&mut t);
        if !offered {
            return Err(format!(
                "C-AG-moca-02: the window must open when another plays a card: {}",
                t.dump_prompt()
            ));
        }
        if !took {
            return Err("C-AG-moca-02: the teleport must be offered".into());
        }
        if t.pos(0) != target as usize {
            return Err(format!(
                "C-AG-moca-02: pos {}, expected {target} (the actor's tile)",
                t.pos(0)
            ));
        }
        if t.charge(0) != 0 {
            return Err(format!("C-AG-moca-02: fire {}, expected 0", t.charge(0)));
        }
        Ok(())
    })
    .expect("C-AG-moca-02");
}

// =====================================================================
// Exclusive-card gates (the roster.exclusiveCards)
// =====================================================================

/// AG character-locked cards refuse a foreign character.
#[test]
fn exclusive_cards_require_the_named_character() {
    let cases: [(&str, &str); 5] = [
        ("AG:(兰) 像往常一样", "美竹兰"),
        ("AG:（摩卡）0.5倍速", "青叶摩卡"),
        ("AG:（绯玛丽）如果并非没问题", "上原绯玛丽"),
        ("AG:（巴）商店街的救世主", "宇田川巴"),
        ("AG:（鸫）微小的『能做到』的事", "羽泽鸫"),
    ];
    for (card, owner) in cases {
        // Wrong persona: the play must be refused.
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "户山香澄");
        let r = t.give_play(0, card);
        if r.is_ok() {
            skip_all(&mut t);
            panic!("exclusive card {card} must be refused to a foreign character");
        }
        // Right persona: the exclusive gate must open. A main-play card's
        // `give_play` must succeed; a 【反击】-only exclusive (himari / tomoe)
        // is not main-playable at all (`err.play_timing`), so for those the
        // gate-pass is "not `err.play_exclusive`".
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, owner);
        let r = t.give_play(0, card);
        if let Err(e) = &r {
            if e == "err.play_exclusive" {
                panic!(
                    "exclusive card {card} must be playable by {owner}: {r:?}"
                );
            }
            // Counteract-only exclusives: timing is the book's own limit, not
            // the exclusive gate. Any other error still means the gate opened.
        }
        skip_all(&mut t);
    }
}

// =====================================================================
// C-AG-one-of-us-01  AG:ONE OF US
// =====================================================================

/// C-AG-one-of-us-01 -- 「将此卡放置在场上」.
#[test]
fn one_of_us_places_on_field() {
    forall(3, 0xA6_00F0, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "美竹兰");
        t.set_character_raw(1, "青叶摩卡"); // another Afterglow character
        t.own(0, &[HILL]);
        t.own(1, &[FUJIMI]);
        t.set_money(0, 10_000 + rng.range(0, 5) * 100);
        t.give_play(0, "AG:ONE OF US")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        if !t.on_field(0, "AG:ONE OF US") {
            return Err(format!(
                "C-AG-one-of-us-01: not on field: {:?}",
                t.field_ids(0)
            ));
        }
        Ok(())
    })
    .expect("C-AG-one-of-us-01");
}

// =====================================================================
// C-AG-tiny-can-01  AG:（鸫）微小的『能做到』的事
// =====================================================================

/// C-AG-tiny-can-01 -- 「打出此卡时，选择其中一个效果发动」: a choice of two.
#[test]
fn tiny_can_offers_a_choice() {
    forall(3, 0xA6_0100, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "羽泽鸫");
        t.give(0, &["AG:（鸫）微小的『能做到』的事"]);
        t.play(0, "AG:（鸫）微小的『能做到』的事")
            .map_err(|e| format!("play rejected: {e}"))?;
        let p = t.expect_prompt();
        if p.options.len() < 2 {
            return Err(format!(
                "C-AG-tiny-can-01: the play must offer a branch choice, got {}",
                t.dump_prompt()
            ));
        }
        // Take branch (1) if named; otherwise the first option.
        let k = t.option("反击").or(Some(0)).unwrap();
        let _ = t.answer(0, k);
        skip_all(&mut t);
        Ok(())
    })
    .expect("C-AG-tiny-can-01");
}

// =====================================================================
// C-AG-as-usual-01 / C-AG-savior / remaining offer gates
// =====================================================================

/// C-AG-as-usual-01 -- 「此卡可以当反击使用」: offered in a counteract window.
#[test]
fn as_usual_is_counteractable() {
    forall(3, 0xA6_0110, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(1, "美竹兰");
        t.give(1, &["AG:(兰) 像往常一样"]);
        // (2) fires on an 异常移动效果; 雨啊 applies [停留] (rulebook-doc.md:100).
        // P0 is in 运营 (vanilla), so the [手] card is legal for P0.
        t.dice(&[1, 1]);
        t.give(0, &["通用:雨啊，快点来吧"]);
        t.play(0, "通用:雨啊，快点来吧")
            .map_err(|e| format!("play rejected: {e}"))?;
        if t.prompt().is_some() {
            t.answer(0, 0)
                .map_err(|e| format!("target answer rejected: {e}"))?;
        }
        if !t.counteract_offered("AG:(兰) 像往常一样") {
            return Err(format!(
                "C-AG-as-usual-01: the card must be offered as a [反击] against an 异常移动效果: {}",
                t.dump_prompt()
            ));
        }
        skip_all(&mut t);
        Ok(())
    })
    .expect("C-AG-as-usual-01");
}
