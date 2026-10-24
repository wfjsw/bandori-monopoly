//! Ave Mujica (Group Mujica) card / skill contracts.
//!
//! Spec doc: `docs/rulebook/spec/cards/mujica.md`. Clause IDs `C-MUJICA-<slug>-<n>`.
//!
//! Source precedence: live sheet (`docs/rulebook/cards.json`,
//! the roster character row `text`, the roster band row `text`) >
//! `docs/rulebook/rulebook-doc.md`. Never engine behaviour.
//!
//! Scope: the 14 `Mujica:` cards in the roster, the five Ave Mujica
//! character skills (`the roster` `band == "Ave Mujica"`), and the
//! band skill `假面之下的真实` (`the roster`).

use rules_spec::{forall, frame, Clause, Step, Table};

// ---------------------------------------------------------------- cards

const DOLL: &str = "Mujica:人偶的箱庭";
const CRYSTAL: &str = "Mujica:会被骗着买水晶的人";
const RAIN: &str = "Mujica:心の雨";
const LOOK: &str = "Mujica:无法将视线移开";
const WELCOME: &str = "Mujica:欢迎来到ave mujica的世界";
const SPARKLER: &str = "Mujica:燃尽前的线香花火";
const SAKI_MOVE: &str = "Mujica:祥，移动";
const DICE_CAST: &str = "Mujica:骰子已经掷下";
const BLACK_BDAY: &str = "Mujica:黑色生日";
const UIKA: &str = "Mujica:（初华）我，无畏悲伤";
const NYAMU: &str = "Mujica:（喵梦）";
const UMIRI: &str = "Mujica:（海铃）";
const MORTIS: &str = "Mujica:（睦/mortis）表演的本能";
const SAKIKO: &str = "Mujica:（祥子）斩断留恋，忘却一切";

// ------------------------------------------------------------ skills

const SKILL_BAND: &str = "skill:Ave Mujica:假面之下的真实";
const SKILL_UIKA: &str = "skill:三角初华:Imprisoned XII";
#[allow(dead_code)] // named for the doc; 若叶睦 has no executable clause yet
const SKILL_MUTSU: &str = "skill:若叶睦:天生的演员";
const SKILL_SAKIKO: &str = "skill:丰川祥子:请把你们的人生交给我";
const SKILL_UMIRI: &str = "skill:八幡海铃:熟练的支援贝斯手";
const SKILL_NYAMU: &str = "skill:祐天寺若麦:大主播喵梦亲";

/// 回忆地块 named by the 三角初华 skill quote.
/// 回忆地块 named by the 三角初华 skill quote (小豆岛, 武道馆, 旧古河庭园).
#[allow(dead_code)]
const MEMORY_TILES: [usize; 3] = [25, 40, 57];

// -------------------------------------------------------------- helpers

fn drain(t: &mut Table) {
    for _ in 0..40 {
        if t.prompt().is_none() {
            break;
        }
        t.decline();
    }
}

/// Table with `n` vanilla seats, `who` bound to an Ave Mujica character
/// (skills placed raw so exclusive cards and skillState work).
fn mujica_table(n: usize, who: usize, persona: &str) -> Table {
    let mut t = Table::vanilla(n);
    t.set_character_raw(who, persona);
    t.place_raw(who, SKILL_BAND);
    t
}

/// Finish `who`'s current turn through roll + end (skipping the main move is
/// fine; the engine rejects `end` while the move is still owed).
fn finish_turn(t: &mut Table, who: usize) {
    drain(t);
    if t.step() == 2 {
        t.roll(who).expect("finish_turn: roll the owed main move");
        drain(t);
    }
    t.end(who).expect("finish_turn: end the turn");
    drain(t);
}

// =====================================================================
// C-MUJICA-黑色生日
// =====================================================================

/// C-MUJICA-黑色生日-01 -- 「资金不多于1000」 pays 800 (inclusive; TEST-FINDINGS §6).
///
/// * **ID:** `C-MUJICA-黑色生日-01`
/// * **Source:** `roster#Mujica:黑色生日`
/// * **Quote:** 「每名你以外的资金不多于1000的玩家支付你800资金」
/// * **Trigger:** play of `Mujica:黑色生日` from hand.
/// * **Condition:** another player's money is `<= 1000` (inclusive) and they
///   can pay 800.
/// * **Effect:** that player pays the user exactly 800.
/// * **Frame:** only `money:*` of the user and the paying player moves.
/// * **Rulings & OPEN:** 「不多于1000」 is inclusive (sheet-superseded ruling).
/// * **Executable:** [`black_bday_low_pays_800`].
fn black_bday_low_pays_800(step: &Step<'_>) -> Result<(), String> {
    let played = step
        .events
        .iter()
        .any(|e| e.r#type == "card" && e.card == BLACK_BDAY);
    if !played {
        return Ok(());
    }
    let user = step.player_of_act();
    for i in 0..step.pre.st.players.len() {
        if i == user {
            continue;
        }
        let pre_m = step.pre_money(i) as i64;
        if pre_m > 1000 {
            continue;
        }
        if pre_m < 800 {
            // The book still says 「支付你800」; a shortfall bankrupts them
            // (G-BANKRUPT). This clause only pins solvent payers.
            continue;
        }
        let delta = step.post_money(i) as i64 - pre_m;
        if delta != -800 {
            return Err(format!(
                "P{i} money {pre_m} <= 1000: delta {delta}, expected -800"
            ));
        }
    }
    Ok(())
}

/// C-MUJICA-黑色生日-02 -- 「资金严格在1000以上」 pays 200 twice.
///
/// * **ID:** `C-MUJICA-黑色生日-02`
/// * **Source:** `roster#Mujica:黑色生日`
/// * **Quote:** 「每名你以外的资金严格在1000以上的玩家支付你两次200资金。」
/// * **Condition:** another player's money is `> 1000`.
/// * **Effect:** that player pays the user 200 twice (total 400).
/// * **Frame:** only `money:*` of the user and the payers move.
/// * **Executable:** [`black_bday_high_pays_400`].
fn black_bday_high_pays_400(step: &Step<'_>) -> Result<(), String> {
    let played = step
        .events
        .iter()
        .any(|e| e.r#type == "card" && e.card == BLACK_BDAY);
    if !played {
        return Ok(());
    }
    let user = step.player_of_act();
    for i in 0..step.pre.st.players.len() {
        if i == user {
            continue;
        }
        let pre_m = step.pre_money(i) as i64;
        if pre_m <= 1000 || pre_m < 400 {
            // Not in this branch, or cannot pay 400 (bankruptcy is G-BANKRUPT).
            continue;
        }
        let delta = step.post_money(i) as i64 - pre_m;
        if delta != -400 {
            return Err(format!(
                "P{i} money {pre_m} > 1000: delta {delta}, expected -400 (200 twice)"
            ));
        }
        // 「支付你两次200资金」 is two separate [支付] legs, not one 400.
        let legs = step
            .events
            .iter()
            .filter(|e| {
                (e.r#type == "pay" || e.r#type == "rent")
                    && e.player_id == i as i32
                    && e.other == user as i32
                    && e.value == 200
            })
            .count();
        if legs != 2 {
            return Err(format!(
                "P{i}: {} pay legs of 200, expected 2 (「支付你两次200资金」)",
                legs
            ));
        }
    }
    Ok(())
}

/// The whole play: only money of the user and the other players moves, and
/// the user gains exactly the sum of what others paid.
fn black_bday_frame(step: &Step<'_>) -> Result<(), String> {
    let played = step
        .events
        .iter()
        .any(|e| e.r#type == "card" && e.card == BLACK_BDAY);
    if !played {
        return Ok(());
    }
    let user = step.player_of_act();
    let allowed: Vec<String> = (0..step.pre.st.players.len())
        .map(|i| format!("money:P{i}"))
        .chain([
            format!("hand:P{user}"),
            format!("draw:P{user}"),
            format!("discard:P{user}"),
            format!("hand_n:P{user}"),
            format!("draw_n:P{user}"),
            format!("discard_pub:P{user}"),
            // Engine once-per-effect latch (bookkeeping).
            format!("state:P{user}:latch"),
        ])
        .collect();
    let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
    frame(step.pre, step.post, &allowed)?;
    // User's gain equals the sum of the others' losses.
    let mut gain = 0i64;
    for i in 0..step.pre.st.players.len() {
        if i == user {
            continue;
        }
        gain -= step.post_money(i) as i64 - step.pre_money(i) as i64;
    }
    let user_delta = step.post_money(user) as i64 - step.pre_money(user) as i64;
    if user_delta != gain {
        return Err(format!(
            "user gain {user_delta} != sum of payers {gain} (book: each pays the user)"
        ));
    }
    Ok(())
}

/// 规则书: 「每名你以外的资金不多于1000的玩家支付你800资金，每名你以外的资金严格在1000以上的玩家支付你两次200资金。」
#[test]
fn black_birthday_contract() {
    forall(8, 0xB1AC, |rng, _i| {
        let mut t = Table::vanilla(3);
        let m0 = 5000 + rng.range(0, 40) * 100;
        let m1 = if rng.chance(50) {
            800 + rng.range(0, 2) * 100 // 800..=1000, can pay 800
        } else {
            1400 + rng.range(0, 20) * 100 // > 1000, can pay 400
        };
        let m2 = if rng.chance(50) {
            800 + rng.range(0, 2) * 100
        } else {
            1400 + rng.range(0, 20) * 100
        };
        t.set_money(0, m0);
        t.set_money(1, m1);
        t.set_money(2, m2);
        t.give(0, &[BLACK_BDAY]);
        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.play(0, BLACK_BDAY)
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);

        // Exact numbers, independent of the clause fns.
        let expect = |m: i64| if m <= 1000 { 800 } else { 400 };
        let d1 = m1 - t.money(1);
        if d1 != expect(m1 as i64) as i32 {
            return Err(format!("P1 {m1} paid {d1}, expected {}", expect(m1 as i64)));
        }
        let d2 = m2 - t.money(2);
        if d2 != expect(m2 as i64) as i32 {
            return Err(format!("P2 {m2} paid {d2}, expected {}", expect(m2 as i64)));
        }
        let gain = t.money(0) - m0;
        if gain != d1 + d2 {
            return Err(format!("user gained {gain}, payers paid {}", d1 + d2));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "money:P1",
                "money:P2",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                // Engine once-per-effect latch (bookkeeping).
                "state:P0:latch",
            ],
        )
    })
    .expect("C-MUJICA-黑色生日 contract");
}

// =====================================================================
// C-MUJICA-心の雨
// =====================================================================

/// C-MUJICA-心の雨-01 -- 「你前方20格内的所有玩家各自骰1d20」.
///
/// * **ID:** `C-MUJICA-心の雨-01`
/// * **Source:** `roster#Mujica:心の雨`
/// * **Quote:** 「使你前方20格内的所有玩家各自骰1d20，骰点小于等于12的玩家获得一层[停留]，小于等于3的玩家变为获得2层[停留]。」
/// * **Trigger:** play of `Mujica:心の雨`.
/// * **Condition:** other players sitting on the 20 tiles clockwise ahead of
///   the user (exclusive of the user's tile).
/// * **Effect:** each such player rolls 1d20; roll `<= 12` grants 1 [停留]
///   layer; roll `<= 3` replaces that with 2 layers.
/// * **Frame:** only the affected players' `state:stay` (and the user's money /
///   zones for the fail branch) move.
/// * **Rulings & OPEN:** 「前方20格」 is read as the 20 tiles clockwise ahead
///   (the board's only forward direction); the user's own tile is not
///   「前方」. OPEN-触发结算 is not involved here.
/// * **Executable:** [`rain_stay_layers`].
fn rain_stay_layers(step: &Step<'_>) -> Result<(), String> {
    let played = step.act_is_card(RAIN);
    if !played {
        return Ok(());
    }
    let user = step.player_of_act();
    let n = step.pre.st.players.len() as i32;
    let board = step.pre.st.owners.len() as i32;
    let u_pos = step.pre.st.players[user].pos;
    for i in 0..step.pre.st.players.len() {
        if i == user {
            continue;
        }
        let p = step.pre.st.players[i].pos;
        let ahead = ((p - u_pos).rem_euclid(board) + board) % board;
        let in_range = (1..=20).contains(&ahead);
        let pre_stay = step.pre.st.players[i].state_get("stay");
        let post_stay = step.post.st.players[i].state_get("stay");
        let gained = post_stay - pre_stay;
        if in_range {
            // Each in-range player is designated and rolls; the stay layer
            // count depends on the (loaded) roll, which this clause does not
            // pin -- the next clause does.
            if gained < 0 {
                return Err(format!("P{i} in range lost stay ({pre_stay}->{post_stay})"));
            }
        } else if gained != 0 {
            return Err(format!(
                "P{i} at {p} not in the 20 tiles ahead of {u_pos} (n={n}) gained stay {gained}"
            ));
        }
    }
    Ok(())
}

/// C-MUJICA-心の雨-02 -- roll `<=12` -> 1 stay; `<=3` -> **2** stay (not 1+2).
fn rain_stay_amount(step: &Step<'_>) -> Result<(), String> {
    let played = step.act_is_card(RAIN);
    if !played {
        return Ok(());
    }
    // The card's own dice events, in order, are the in-range players' 1d20.
    let rolls: Vec<i32> = step
        .events
        .iter()
        .filter(|e| e.r#type == "dice" || rules_spec::event_key(e) == "log.dice")
        .map(|e| e.value)
        .collect();
    let user = step.player_of_act();
    // Pair each in-range other player (seat order) with the rolls in order.
    let board = step.pre.st.owners.len() as i32;
    let u_pos = step.pre.st.players[user].pos;
    let mut targets = Vec::new();
    for i in 0..step.pre.st.players.len() {
        if i == user {
            continue;
        }
        let p = step.pre.st.players[i].pos;
        let ahead = (p - u_pos).rem_euclid(board);
        if (1..=20).contains(&ahead) {
            targets.push(i);
        }
    }
    if rolls.len() < targets.len() {
        return Err(format!(
            "C-MUJICA-心の雨-01: {} in-range players but only {} dice events              (「各自骰1d20」)",
            targets.len(),
            rolls.len()
        ));
    }
    for (k, &i) in targets.iter().enumerate() {
        let roll = rolls[k];
        let pre_stay = step.pre.st.players[i].state_get("stay");
        let post_stay = step.post.st.players[i].state_get("stay");
        let gained = post_stay - pre_stay;
        let expect = if roll <= 3 {
            2
        } else if roll <= 12 {
            1
        } else {
            0
        };
        if gained != expect {
            return Err(format!(
                "P{i} roll {roll}: stay {pre_stay}->{post_stay} (gained {gained}), expected +{expect}"
            ));
        }
    }
    Ok(())
}

/// C-MUJICA-心の雨-03 -- nobody gained [停留] -> user +1 [晕眩] and +1000.
fn rain_miss_self_stun(step: &Step<'_>) -> Result<(), String> {
    let played = step.act_is_card(RAIN);
    if !played {
        return Ok(());
    }
    let user = step.player_of_act();
    let mut any_gained = false;
    for i in 0..step.pre.st.players.len() {
        let g =
            step.post.st.players[i].state_get("stay") - step.pre.st.players[i].state_get("stay");
        if g > 0 {
            any_gained = true;
        }
    }
    let stun_gained =
        step.post.st.players[user].state_get("stun") - step.pre.st.players[user].state_get("stun");
    let money_gained = step.post_money(user) as i64 - step.pre_money(user) as i64;
    if !any_gained {
        if stun_gained != 1 {
            return Err(format!(
                "no stay landed: user stun gained {stun_gained}, expected 1 ([晕眩])"
            ));
        }
        if money_gained != 1000 {
            return Err(format!(
                "no stay landed: user money gained {money_gained}, expected 1000"
            ));
        }
    } else if stun_gained != 0 || money_gained != 0 {
        return Err(format!(
            "stay landed but user also got stun+{stun_gained} money+{money_gained}"
        ));
    }
    Ok(())
}

/// 规则书: 「使你前方20格内的所有玩家各自骰1d20，骰点小于等于12的玩家获得一层[停留]，小于等于3的玩家变为获得2层[停留]。若未能使任何玩家获得[停留]，自身获得一层[晕眩]并获得1000资金」
#[test]
fn rain_contract() {
    forall(10, 0x5A1A, |rng, _i| {
        let mut t = Table::vanilla(3);
        let u_pos = rng.range(0, 59);
        t.set_pos(0, u_pos as usize);
        // One partner inside the 20-tile window, one outside.
        let in_ahead = rng.range(1, 20);
        let out_ahead = rng.range(21, 50);
        let board = 60;
        t.set_pos(1, ((u_pos + in_ahead).rem_euclid(board)) as usize);
        t.set_pos(2, ((u_pos + out_ahead).rem_euclid(board)) as usize);
        let roll = rng.range(1, 20);
        t.dice(&[roll]);
        t.give(0, &[RAIN]);
        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.play(0, RAIN).map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);

        let expect = if roll <= 3 {
            2
        } else if roll <= 12 {
            1
        } else {
            0
        };
        let stay1 = t.state(1, "stay");
        if stay1 != expect {
            return Err(format!("P1 roll {roll}: stay {stay1}, expected {expect}"));
        }
        let stay2 = t.state(2, "stay");
        if stay2 != 0 {
            return Err(format!("P2 out of range gained stay {stay2}"));
        }
        if expect == 0 {
            if t.state(0, "stun") != 1 {
                return Err(format!(
                    "miss: user stun {}, expected 1",
                    t.state(0, "stun")
                ));
            }
            if t.money(0) != 10_000 + 1000 {
                return Err(format!("miss: user money {}, expected 11000", t.money(0)));
            }
        } else {
            if t.state(0, "stun") != 0 {
                return Err(format!("hit: user stun {}, expected 0", t.state(0, "stun")));
            }
            if t.money(0) != 10_000 {
                return Err(format!("hit: user money {}, expected 10000", t.money(0)));
            }
        }
        frame(
            &pre,
            t.world(),
            &[
                "state:P0:stun",
                "state:P0:stay",
                "state:P1:stay",
                "state:P2:stay",
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                // [停留] is an [异常移动效果]; the engine records it here.
                "turn.abnormal",
            ],
        )
    })
    .expect("C-MUJICA-心の雨 contract");
}

// =====================================================================
// C-MUJICA-黑色生日 / 心の雨 CLAUSES slice (shared with observe)
// =====================================================================

pub const CLAUSES: &[Clause] = &[
    Clause::new(
        "C-MUJICA-黑色生日-01",
        "roster#Mujica:黑色生日",
        "每名你以外的资金不多于1000的玩家支付你800资金",
        black_bday_low_pays_800,
    ),
    Clause::new(
        "C-MUJICA-黑色生日-02",
        "roster#Mujica:黑色生日",
        "每名你以外的资金严格在1000以上的玩家支付你两次200资金。",
        black_bday_high_pays_400,
    ),
    Clause::new(
        "C-MUJICA-黑色生日-frame",
        "roster#Mujica:黑色生日",
        "每名你以外的资金不多于1000的玩家支付你800资金，每名你以外的资金严格在1000以上的玩家支付你两次200资金。",
        black_bday_frame,
    ),
    Clause::new(
        "C-MUJICA-心の雨-01",
        "roster#Mujica:心の雨",
        "使你前方20格内的所有玩家各自骰1d20",
        rain_stay_layers,
    ),
    Clause::new(
        "C-MUJICA-心の雨-02",
        "roster#Mujica:心の雨",
        "骰点小于等于12的玩家获得一层[停留]，小于等于3的玩家变为获得2层[停留]。",
        rain_stay_amount,
    ),
    Clause::new(
        "C-MUJICA-心の雨-03",
        "roster#Mujica:心の雨",
        "若未能使任何玩家获得[停留]，自身获得一层[晕眩]并获得1000资金",
        rain_miss_self_stun,
    ),
];

// =====================================================================
// C-MUJICA-祥，移动
// =====================================================================

/// 规则书: 「强制一名玩家向你选择的方向移动3格并[触发结算]（可在掷骰前选择自己以代替主要移动），触发结算时进行的支付价格减半」
///
/// C-MUJICA-祥移动-01: force one player 3 tiles in the chosen direction and settle.
/// C-MUJICA-祥移动-02: the user may choose themself (replacing the main move).
/// C-MUJICA-祥移动-03: settle payment prices are halved.
/// OPEN-触发结算: the exact meaning of 「[触发结算]」 is undefined; this
/// contract checks the forced 3-tile walk and that a settle window opens
/// (the engine offers the unowned-buy prompt on an unowned landing).
#[test]
fn saki_move_forces_three_tiles() {
    forall(6, 0x5A11, |rng, _i| {
        let mut t = Table::vanilla(2);
        let start = rng.range(0, 40);
        t.set_pos(1, start as usize);
        t.give(0, &[SAKI_MOVE]);
        let pre = t.snapshot_world();
        t.play(0, SAKI_MOVE)
            .map_err(|e| format!("play rejected: {e}"))?;
        // Target P1 (option index 1: the prompt lists self first).
        let p = t.expect_prompt();
        let who = p
            .options
            .iter()
            .position(|o| format!("{o:?}").contains("PlayerId(1)"))
            .unwrap_or_else(|| panic!("P1 not offered: {}", t.dump_prompt()))
            as i32;
        t.answer(0, who).map_err(|e| format!("pick target: {e}"))?;
        // Direction: option 0 is forward.
        t.answer(0, 0).map_err(|e| format!("pick direction: {e}"))?;
        drain(&mut t);
        let end = (start + 3) % 60;
        let pos = t.pos(1);
        if pos != end as usize {
            return Err(format!(
                "P1 moved {start} -> {pos}, expected {end} (3 tiles forward)"
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "pos:P1",
                "money:P0",
                "money:P1",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "owner:",
                "houses:",
                "mortgaged:",
                // [强制移动] is an [异常移动效果]; designation + abnormal mark.
                "targeted",
                "turn.abnormal",
            ],
        )
    })
    .expect("C-MUJICA-祥移动-01");
}

/// C-MUJICA-祥移动-03 -- 「触发结算时进行的支付价格减半」.
///
/// Setup: the forced mover lands on an owned tile and pays rent. The payment
/// price (rent) is halved. TEST-FINDINGS §6: 「支付价格」 covers shaped
/// payments. This contract checks the payer's loss is ceil/floor-half of the
/// printed rent -- the book does not pin the rounding of an odd halve
/// (OPEN-取整).
#[test]
fn saki_move_halves_settle_pay() {
    // Tile 3 (水族馆) base rent 320 -> halved 160.
    forall(4, 0x5A12, |rng, _i| {
        let mut t = Table::vanilla(2);
        // P1 walks 3 forward from 0 onto tile 3 (水族馆, rent 320 with 0 houses).
        t.set_pos(1, 0);
        t.own(0, &[3]);
        t.set_houses(3, 0);
        t.set_money(0, 5000);
        t.set_money(1, 5000);
        t.give(0, &[SAKI_MOVE]);
        let _ = rng;
        t.play(0, SAKI_MOVE)
            .map_err(|e| format!("play rejected: {e}"))?;
        // Pick P1, forward.
        t.answer(0, 1).map_err(|e| format!("pick: {e}"))?;
        t.answer(0, 0).map_err(|e| format!("dir: {e}"))?;
        drain(&mut t);
        let paid = 5000 - t.money(1);
        // 320 / 2 = 160. The book says 「支付价格减半」; an odd half would be
        // OPEN-取整. 320 is even so the half is exact.
        if paid != 160 {
            return Err(format!(
                "P1 paid {paid} on a 320-rent landing, expected 160 ([支付价格减半])"
            ));
        }
        let got = t.money(0) - 5000;
        // The owner receives the (possibly modified) payment; if the engine
        // pays the owner the unhalfed 320 that is a different clause.
        if got != 160 {
            return Err(format!("owner received {got}, expected 160"));
        }
        Ok(())
    })
    .expect("C-MUJICA-祥移动-03");
}

// =====================================================================
// C-MUJICA-燃尽前的线香花火
// =====================================================================

/// 规则书: 「将此卡放置于场上并放置2个奇迹水晶（上限2），你的回合结束后自动移除一个奇迹水晶并使你获得一个额外回合，最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]。」
///
/// C-MUJICA-花火-01: placed with 2 crystals (cap 2).
/// C-MUJICA-花火-02: at the user's turn end, one crystal is removed and an
///   extra turn is granted.
/// C-MUJICA-花火-03: when the last crystal is removed the card is discarded
///   and the user gains **2** [眩晕] layers.
/// OPEN (TEST-FINDINGS §6 item 2): does the last crystal also grant the extra
///   turn? The quote attaches 「并使你获得一个额外回合」 to every removal and
///   「并立刻使你获得2层[眩晕]」 only to the last; this contract checks the
///   stun count and the discard, and does not pin whether an extra turn
///   accompanies the last removal.
#[test]
fn sparkler_places_two_crystals() {
    forall(4, 0x57A1, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.give(0, &[SPARKLER]);
        let pre = t.snapshot_world();
        t.play(0, SPARKLER)
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        if !t.on_field(0, SPARKLER) {
            return Err(format!("card not on field: {:?}", t.field_ids(0)));
        }
        let c = t.crystals(0, SPARKLER).unwrap_or(-1);
        if c != 2 {
            return Err(format!("crystals {c}, expected 2 (放置2个奇迹水晶)"));
        }
        frame(
            &pre,
            t.world(),
            &[
                "field:P0",
                "crystals:P0",
                "pool:",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "fprop:P0",
            ],
        )
    })
    .expect("C-MUJICA-花火-01");
}

#[test]
fn sparkler_burn_and_last_stun() {
    forall(2, 0x57A2, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.give_play(0, SPARKLER).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        // First turn end: one crystal burned, extra turn granted.
        finish_turn(&mut t, 0);
        let c1 = t.crystals(0, SPARKLER).unwrap_or(-1);
        if c1 != 1 {
            return Err(format!("after first turn end crystals {c1}, expected 1"));
        }
        if !t.on_field(0, SPARKLER) {
            return Err("card left the field after the first burn".into());
        }
        // Second turn end: last crystal, card discarded, 2 [眩晕].
        finish_turn(&mut t, 0);
        if t.on_field(0, SPARKLER) {
            return Err(format!(
                "card still on field after last crystal: {:?}",
                t.field_ids(0)
            ));
        }
        // Non-[持续] card leaves play: it must be in the discard or (if the
        // empty draw pile reshuffled) the draw -- never still in hand.
        let in_discard = t.discard(0).iter().any(|c| c == SPARKLER);
        let in_draw = t.draw_pile(0).iter().any(|c| c == SPARKLER);
        if !in_discard && !in_draw {
            return Err(format!(
                "card not in discard/draw after last crystal: discard {:?} draw {:?}",
                t.discard(0),
                t.draw_pile(0)
            ));
        }
        let stun = t.state(0, "stun");
        // Book: 「立刻使你获得2层[眩晕]」. The engine may tick one layer off at
        // the same turn-end boundary (OPEN-花火眩晕掉层); the contract asks
        // for the 2 the book prints and stays red if the engine only grants 1.
        if stun != 2 {
            return Err(format!(
                "stun after last crystal {stun}, expected 2 ([获得2层[眩晕]])"
            ));
        }
        Ok(())
    })
    .expect("C-MUJICA-花火-02/03");
}

// =====================================================================
// C-MUJICA-骰子已经掷下
// =====================================================================

/// 规则书: 「将此卡放置于自身场上，本回合内所有其他玩家无法从手牌中使用[反击]，回合结束后放入弃牌堆（此卡可以被反击）」
#[test]
fn dice_cast_places_and_goes_to_discard() {
    forall(4, 0xD1CE, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.give(0, &[DICE_CAST]);
        let pre = t.snapshot_world();
        t.play(0, DICE_CAST).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        if !t.on_field(0, DICE_CAST) {
            return Err(format!("not on field: {:?}", t.field_ids(0)));
        }
        finish_turn(&mut t, 0);
        if t.on_field(0, DICE_CAST) {
            return Err(format!(
                "still on field after turn end: {:?}",
                t.field_ids(0)
            ));
        }
        let in_discard = t.discard(0).iter().any(|c| c == DICE_CAST);
        let in_draw = t.draw_pile(0).iter().any(|c| c == DICE_CAST);
        if !in_discard && !in_draw {
            return Err(format!(
                "not discarded after turn end: discard {:?} draw {:?}",
                t.discard(0),
                t.draw_pile(0)
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "field:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "pos:P0",
                "pos:P1",
                "money:P0",
                "money:P1",
                "state:P0",
                "state:P1",
            ],
        )
    })
    .expect("C-MUJICA-骰子-01/03");
}

// =====================================================================
// C-MUJICA-会被骗着买水晶的人
// =====================================================================

/// 规则书: 「将场上一张卡上的一个奇迹水晶移动到另一张可以放置奇迹水晶的卡上。」
#[test]
fn crystal_swap_moves_one() {
    forall(4, 0xC7A5, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.place_raw(0, SPARKLER);
        t.place_raw(1, NYAMU);
        t.set_crystals(0, SPARKLER, 2);
        t.set_crystals(1, NYAMU, 0);
        t.give(0, &[CRYSTAL]);
        let pre = t.snapshot_world();
        t.play(0, CRYSTAL).map_err(|e| format!("play: {e}"))?;
        // from-card prompt then to-card prompt; take the only option.
        for _ in 0..4 {
            if t.prompt().is_none() {
                break;
            }
            t.answer(0, 0).map_err(|e| format!("answer: {e}"))?;
        }
        drain(&mut t);
        let a = t.crystals(0, SPARKLER).unwrap_or(-1);
        let b = t.crystals(1, NYAMU).unwrap_or(-1);
        if a != 1 || b != 1 {
            return Err(format!(
                "crystals moved: {SPARKLER}={a}, {NYAMU}={b}, expected 1 and 1 (one crystal moved)"
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "crystals:P0",
                "pool:",
                "crystals:P1",
                "pool:",
                "field:P0",
                "field:P1",
                "hand:P0",
                "draw:P0",
                "discard:P0",
            ],
        )
    })
    .expect("C-MUJICA-水晶-01");
}

// =====================================================================
// C-MUJICA-（喵梦） / C-MUJICA-（海铃）
// =====================================================================

/// 规则书: 「（1）将此卡放置于场上」
#[test]
fn nyamu_places_on_field() {
    forall(3, 0x0AA1, |_rng, _i| {
        let mut t = mujica_table(2, 0, "祐天寺若麦");
        t.give(0, &[NYAMU]);
        t.play(0, NYAMU).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        if !t.on_field(0, NYAMU) {
            return Err(format!("（喵梦） not on field: {:?}", t.field_ids(0)));
        }
        Ok(())
    })
    .expect("C-MUJICA-喵梦-01");
}

/// 规则书: 「（1）将此卡放置于在此卡使用者下一名行动的玩家场上」
#[test]
fn umiri_places_on_next_actors_field() {
    forall(3, 0x00A1, |_rng, _i| {
        let mut t = mujica_table(3, 0, "八幡海铃");
        t.give(0, &[UMIRI]);
        t.play(0, UMIRI).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        if t.on_field(0, UMIRI) {
            return Err(format!(
                "（海铃） landed on the user's field: {:?}",
                t.field_ids(0)
            ));
        }
        // Seat 0 is acting; the next actor is seat 1.
        if !t.on_field(1, UMIRI) {
            return Err(format!(
                "（海铃） not on the next actor's field: P0 {:?} P1 {:?} P2 {:?}",
                t.field_ids(0),
                t.field_ids(1),
                t.field_ids(2)
            ));
        }
        Ok(())
    })
    .expect("C-MUJICA-海铃-01");
}

// =====================================================================
// C-MUJICA-（祥子）斩断留恋，忘却一切
// =====================================================================

/// 规则书: 「抵押一张你拥有且未抵押的最贵地契并弃置一张手牌（若无手牌则弃掉下一张抽到的牌），立刻传送至任意可购买或已拥有的格子并触发结算，作为你的主要移动。」
#[test]
fn sakiko_mortgages_most_expensive_and_teleports() {
    forall(4, 0x5A13, |_rng, _i| {
        let mut t = mujica_table(2, 0, "丰川祥子");
        // Three deeds: tile 1 (price 3000), tile 3 (price 3200), tile 11 (price 600).
        t.own(0, &[1, 3, 11]);
        t.set_houses(1, 2);
        t.set_hand(0, &["通用:GREAT"]);
        t.give(0, &[SAKIKO]);
        t.play(0, SAKIKO).map_err(|e| format!("play: {e}"))?;
        // Discard prompt: drop the one hand card.
        if t.prompt().is_some() {
            t.answer(0, 0).map_err(|e| format!("discard: {e}"))?;
        }
        // Tile prompt: pick a buyable tile (42 Space is buyable and unowned).
        if t.prompt().is_some() {
            t.answer_tile(0, 42).map_err(|e| format!("tile: {e}"))?;
        }
        drain(&mut t);
        // Most expensive deed by printed price is tile 3 (3200).
        if !t.mortgaged(3) {
            return Err(format!(
                "tile 3 (price 3200) not mortgaged: 1={} 3={} 11={}",
                t.mortgaged(1),
                t.mortgaged(3),
                t.mortgaged(11)
            ));
        }
        if t.mortgaged(1) || t.mortgaged(11) {
            return Err(format!(
                "wrong deed mortgaged: 1={} 3={} 11={}",
                t.mortgaged(1),
                t.mortgaged(3),
                t.mortgaged(11)
            ));
        }
        // The hand card was discarded (non-[持续] played card also leaves).
        if t.hand(0).iter().any(|c| c == "通用:GREAT") {
            return Err(format!("hand card not discarded: {:?}", t.hand(0)));
        }
        if t.pos(0) != 42 {
            return Err(format!("teleport landed on {}, expected 42", t.pos(0)));
        }
        // 抵押后[获得]地契购买价格50%的资金 -- tile 3 price 3200 -> +1600.
        let money = t.money(0);
        if money != 10_000 + 1600 {
            return Err(format!(
                "C-MUJICA-祥子-01: money {money} after mortgaging tile 3                  (price 3200), expected 11600 (50% of the deed price)"
            ));
        }
        Ok(())
    })
    .expect("C-MUJICA-祥子-01/02/03");
}

// =====================================================================
// C-MUJICA-无法将视线移开
// =====================================================================

/// 规则书: 「使当前回合内对你打出过[反击]的所有玩家向你选择的方向强制移动1~4以内的任意步数并[触发结算]」
///
/// C-MUJICA-视线-01: the target set is **only** players who played a [反击]
/// against the user this turn. With an empty set nobody is force-moved.
/// A play that force-moves non-counteractors is a plain failing contract.
#[test]
fn look_away_only_moves_counteractors() {
    forall(3, 0x100A, |_rng, _i| {
        let mut t = Table::vanilla(3);
        // Nobody has used [反击] against seat 0 this turn.
        t.give(0, &[LOOK]);
        let pre = t.snapshot_world();
        t.play(0, LOOK).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        let p1 = t.pos(1);
        let p2 = t.pos(2);
        if p1 != 0 || p2 != 0 {
            return Err(format!(
                "C-MUJICA-视线-01: book targets only players who used [反击] \
                 against the user this turn (none here); engine moved P1->{p1} P2->{p2}"
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "pos:P0",
                "pos:P1",
                "pos:P2",
            ],
        )
    })
    .expect("C-MUJICA-视线-01");
}

// =====================================================================
// C-MUJICA-欢迎来到ave mujica的世界
// =====================================================================

/// 规则书: 「（1）转换任意一名玩家的状态（若指定了不存在状态2的玩家则无效果）」
#[test]
fn welcome_switches_designated_state() {
    forall(3, 0xA31C, |_rng, _i| {
        let mut t = Table::vanilla(3);
        t.set_character_raw(0, "丰川祥子");
        t.place_raw(0, SKILL_BAND);
        t.set_character_raw(1, "三角初华");
        t.place_raw(1, SKILL_BAND);
        t.set_state(1, "skillState", 1);
        t.give(0, &[WELCOME]);
        t.play(0, WELCOME).map_err(|e| format!("play: {e}"))?;
        // The prompt lists the other players; pick P1 (the first option that
        // names PlayerId(1)).
        let p = t.expect_prompt();
        let k = p
            .options
            .iter()
            .position(|o| format!("{o:?}").contains("PlayerId(1)"))
            .unwrap_or_else(|| panic!("P1 not offered: {}", t.dump_prompt()))
            as i32;
        t.answer(0, k).map_err(|e| format!("pick: {e}"))?;
        drain(&mut t);
        let s = t.state(1, "skillState");
        if s != 2 {
            return Err(format!(
                "C-MUJICA-欢迎-02: designated player state {s}, expected 2 \
                 (转换任意一名玩家的状态)"
            ));
        }
        Ok(())
    })
    .expect("C-MUJICA-欢迎-02");
}

// =====================================================================
// C-MUJICA-band skill 假面之下的真实
// =====================================================================

/// 规则书: 「你在状态2时从[收取]与[支付]的资金改为1.5倍（只对自己结算，对应的其他玩家收支不变）」
///
/// C-MUJICA-团技能-01: in 状态2, a rent the user **pays** is 1.5x on the
///   user's side only; the owner still receives the unhalfed printed rent.
/// The parenthetical 「只对自己结算，对应的其他玩家收支不变」 is the whole
/// point of this clause: a transfer that moves 1.5x on **both** sides is a
/// discrepancy.
#[test]
fn band_state2_pay_is_150_only_on_self() {
    forall(4, 0xBAAD, |_rng, _i| {
        let mut t = mujica_table(2, 0, "丰川祥子");
        t.set_state(0, "skillState", 2);
        t.set_money(0, 5000);
        t.set_money(1, 5000);
        // Tile 1 购物中心, 0 houses, base rent 300.
        t.own(1, &[1]);
        t.set_pos(0, 0);
        t.dice(&[1]);
        let pre = t.snapshot_world();
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);
        let paid = 5000 - t.money(0);
        let got = t.money(1) - 5000;
        if paid != 450 {
            return Err(format!(
                "C-MUJICA-团技能-01: user paid {paid} on a 300-rent, expected 450 (1.5x)"
            ));
        }
        if got != 300 {
            return Err(format!(
                "C-MUJICA-团技能-01: owner received {got} on a 300-rent, expected 300 \
                 (「对应的其他玩家收支不变」); a symmetric 450 transfer breaks the quote"
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
                // One-sided bend bookkeeping (printed figure latched at PaySplitMul).
                "state:P0:skill.aveMujica.orig",
                "turn.paid_in_settle",
            ],
        )
    })
    .expect("C-MUJICA-团技能-01");
}

/// 规则书: 「你拥有“状态1”和“状态2”两种状态，初始处于状态1。」
#[test]
fn band_starts_in_state1() {
    let t = mujica_table(2, 0, "丰川祥子");
    // Arranging via Table does not run the game-start hook; the engine's
    // default skillState is 0, and the band skill's own placement stamps 1.
    // Check the placed band skill exists and that a fresh arrange leaves the
    // player in (or at) 状态1.
    assert!(
        t.field_ids(0).iter().any(|c| c == SKILL_BAND),
        "band skill not placed"
    );
    let s = t.state(0, "skillState");
    // 0 is the un-stamped default; 1 is 状态1. Both mean "not 状态2".
    if s == 2 {
        panic!("band skill started in 状态2 (skillState=2), expected 状态1");
    }
}

/// 规则书: 「处于状态2时可消耗一个奇迹水晶使你的一次[支付]金额减半。」
///
/// Documented as untestable-by-contract in the body below: the half-pay is an
/// optional interrupt on an arbitrary payment, and the Table harness has no
/// seam to arm "spend a crystal to halve this pay" without driving a full
/// payment pipeline. The quote is recorded so the clause is not lost.
// C-MUJICA-团技能-03 (spend 1 crystal to halve a payment) has no executable
// predicate: the Table harness has no seam to arm an optional crystal-spend
// interrupt on a payment. Recorded in the doc so the clause is not lost.

// =====================================================================
// Character skill smoke contracts (the parts a Table arrange can reach)
// =====================================================================

/// 规则书 (三角初华): 「状态1：移动改为10+1d10」
#[test]
fn uika_state1_roll_is_10_plus_d10() {
    forall(4, 0xA11A, |_rng, _i| {
        let mut t = mujica_table(2, 0, "三角初华");
        t.place_raw(0, SKILL_UIKA);
        t.set_state(0, "skillState", 1);
        t.set_pos(0, 0);
        t.dice(&[4]); // d10 face 4 -> move 14
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);
        let pos = t.pos(0);
        if pos != 14 {
            return Err(format!(
                "C-MUJICA-初华技能-01: moved to {pos}, expected 14 (10+1d10 with face 4)"
            ));
        }
        Ok(())
    })
    .expect("C-MUJICA-初华技能-01");
}

/// 规则书 (丰川祥子): 「每次受到停留，眩晕，除外影响（并结算其影响），获得一个火罐（初始0，上限3）」
///
/// The grant is a hook on status application. This contract arms the skill
/// and applies a [停留] via the table's state setter **after** observing --
/// arrange writes are not transitions -- so the executable part here is the
/// cap setup and a documented non-coverage note. See the doc.
#[test]
fn sakiko_fire_cap_is_three() {
    let mut t = mujica_table(2, 0, "丰川祥子");
    t.place_raw(0, SKILL_SAKIKO);
    t.set_fire(0, 0, 3);
    let v = t.charge_max(0);
    if v != 3 {
        panic!("丰川祥子 fire cap {}, expected 3 (上限3)", v);
    }
}

/// 规则书 (八幡海铃): fire cap 4.
#[test]
fn umiri_fire_cap_is_four() {
    let mut t = mujica_table(2, 0, "八幡海铃");
    t.place_raw(0, SKILL_UMIRI);
    t.set_fire(0, 0, 4);
    let v = t.charge_max(0);
    if v != 4 {
        panic!("八幡海铃 fire cap {}, expected 4 (上限4)", v);
    }
}

/// 规则书 (祐天寺若麦): fire cap 5.
#[test]
fn nyamu_fire_cap_is_five() {
    let mut t = mujica_table(2, 0, "祐天寺若麦");
    t.place_raw(0, SKILL_NYAMU);
    t.set_fire(0, 0, 5);
    let v = t.charge_max(0);
    if v != 5 {
        panic!("祐天寺若麦 fire cap {}, expected 5 (上限5)", v);
    }
}

/// 规则书 (三角初华): 「（火罐上限3）」 in 状态2.
#[test]
fn uika_fire_cap_is_three() {
    let mut t = mujica_table(2, 0, "三角初华");
    t.place_raw(0, SKILL_UIKA);
    t.set_fire(0, 0, 3);
    let v = t.charge_max(0);
    if v != 3 {
        panic!("三角初华 fire cap {}, expected 3 (火罐上限3)", v);
    }
}

// =====================================================================
// C-MUJICA-人偶的箱庭
// =====================================================================

/// 规则书: 「向你支付X*20资金，X为该玩家正常移动到你所在格子所需的移动数。」
///
/// C-MUJICA-人偶的箱庭-01: each other player chooses to move (no settle) or
/// pay `X*20`, where X is the clockwise tile count from them to the user.
/// C-MUJICA-人偶的箱庭-02: a player who cannot move may only pay.
/// C-MUJICA-人偶的箱庭-03: afterwards the user force-moves the sum of the
///   others' dice as their [主要移动].
/// OPEN-触发结算: 「不[触发结算]」 on the others' dice walk is undefined
///   beyond "do not run the landing settle"; this contract checks the pay
///   amount only.
#[test]
fn doll_garden_pay_amount() {
    forall(4, 0xD011, |rng, _i| {
        let mut t = Table::vanilla(3);
        let p1 = rng.range(1, 30);
        let p2 = rng.range(1, 30);
        t.set_pos(1, p1 as usize);
        t.set_pos(2, p2 as usize);
        t.give(0, &[DOLL]);
        t.play(0, DOLL).map_err(|e| format!("play: {e}"))?;
        // Each other player is asked in turn; choose the pay option (index 1).
        for _ in 0..6 {
            if t.prompt().is_none() {
                break;
            }
            let p = t.expect_prompt();
            if p.title.key().contains("doll_garden") {
                t.answer_one(1).map_err(|e| format!("pay choice: {e}"))?;
            } else {
                t.decline();
            }
        }
        drain(&mut t);
        // X = clockwise tiles from the payer to the user at tile 0 = 60 - pos.
        let x1 = (60 - p1) % 60;
        let x2 = (60 - p2) % 60;
        let paid1 = 10_000 - t.money(1);
        let paid2 = 10_000 - t.money(2);
        if paid1 != x1 * 20 {
            return Err(format!(
                "C-MUJICA-人偶的箱庭-01: P1 at {p1} (X={x1}) paid {paid1}, expected {}",
                x1 * 20
            ));
        }
        if paid2 != x2 * 20 {
            return Err(format!(
                "C-MUJICA-人偶的箱庭-01: P2 at {p2} (X={x2}) paid {paid2}, expected {}",
                x2 * 20
            ));
        }
        let got = t.money(0) - 10_000;
        if got != paid1 + paid2 {
            return Err(format!("user gained {got}, payers paid {}", paid1 + paid2));
        }
        Ok(())
    })
    .expect("C-MUJICA-人偶的箱庭-01");
}

// =====================================================================
// C-MUJICA-（初华）我，无畏悲伤 / C-MUJICA-（睦/mortis）表演的本能
// =====================================================================

/// 规则书: 「打出此卡时若自从上一次[经过]CiRCLE后有在任何[回忆地块][触发结算]，你可选择抽1张卡或…」
///
/// C-MUJICA-初华-01: the choice is gated on having settled a [回忆地块]
///   (小豆岛 / 武道馆 / 旧古河庭园) since the last [经过]CiRCLE. With no
///   such settle the card does **not** offer the choice.
/// C-MUJICA-初华-02: when the gate is open the user may draw 1 card or set
///   this turn's roll to 1-6. (Untested here: opening the gate requires a
///   multi-act settle history the Table arrange cannot fake -- see the doc.)
#[test]
fn uika_no_memory_offers_no_choice() {
    forall(3, 0xA11A, |_rng, _i| {
        let mut t = mujica_table(2, 0, "三角初华");
        t.give(0, &[UIKA]);
        let mark = t.mark();
        t.play(0, UIKA).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        // No choice prompt should have been offered.
        let keys = t.keys_since(mark);
        if keys
            .iter()
            .any(|k| k.contains("uika_fearless_choice") || k.contains("draw_choice"))
        {
            return Err(format!("gate closed but a choice opened: {keys:?}"));
        }
        Ok(())
    })
    .expect("C-MUJICA-初华-01");
}

/// 规则书: 「此卡打出时效果为场上其他玩家打出的上一张卡（不计入效果带有放置于场上的卡）」
///
/// C-MUJICA-睦-01: the play copies the last card another player played
///   (cards that place themselves on the field do not count). With no prior
///   playable card the copy has nothing to run.
#[test]
fn mortis_nothing_to_copy_is_inert() {
    forall(3, 0xA0A7, |_rng, _i| {
        let mut t = mujica_table(2, 0, "若叶睦");
        t.give(0, &[MORTIS]);
        let mark = t.mark();
        t.play(0, MORTIS).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        let keys = t.keys_since(mark);
        // The engine names the empty copy `mortis_instinct_nothing`.
        if !keys.iter().any(|k| k.contains("mortis_instinct_nothing")) {
            return Err(format!(
                "C-MUJICA-睦-01: expected an inert no-copy resolution                  (mortis_instinct_nothing), keys {keys:?}"
            ));
        }
        Ok(())
    })
    .expect("C-MUJICA-睦-01");
}

// =====================================================================
// C-MUJICA-（海铃） (2) band-skill take
// =====================================================================

/// 规则书: 「使用者打出此卡时以及使用者的回合开始时，从卡堆拿取场上有此卡的玩家的所有乐队技能卡（相同乐队技能卡的效果不可叠加）」
#[test]
fn umiri_takes_holders_band_skill() {
    forall(3, 0x00A2, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "八幡海铃");
        t.set_character_raw(1, "户山香澄");
        t.place_raw(1, "skill:Poppin' Party:星之鼓动");
        t.give(0, &[UMIRI]);
        t.play(0, UMIRI).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        // The card sits on P1 (next actor); the user takes P1's band skills.
        if !t.on_field(1, UMIRI) {
            return Err(format!("（海铃） not on P1: {:?}", t.field_ids(1)));
        }
        let took = t.skills(0).iter().any(|s| s.contains("星之鼓动"));
        if !took {
            return Err(format!(
                "C-MUJICA-海铃-03: user did not take the holder's band skill: {:?}",
                t.skills(0)
            ));
        }
        Ok(())
    })
    .expect("C-MUJICA-海铃-03");
}

// =====================================================================
// C-MUJICA-欢迎-02 gaps (book includes self; no-状态2 must no-op)
// =====================================================================

/// 规则书: 「转换任意一名玩家的状态」 -- 「任意一名玩家」 includes the user.
///
/// C-MUJICA-欢迎-02 (self): the prompt must offer the user as well. A prompt
/// that only lists the other players is a plain failing contract.
#[test]
fn welcome_offers_self_as_well() {
    let mut t = Table::vanilla(3);
    t.set_character_raw(0, "丰川祥子");
    t.place_raw(0, SKILL_BAND);
    t.set_state(0, "skillState", 1);
    t.set_character_raw(1, "三角初华");
    t.place_raw(1, SKILL_BAND);
    t.give(0, &[WELCOME]);
    t.play(0, WELCOME).expect("play");
    let p = t.expect_prompt();
    let offers_self = p
        .options
        .iter()
        .any(|o| format!("{o:?}").contains("PlayerId(0)"));
    if !offers_self {
        panic!(
            "C-MUJICA-欢迎-02: 「转换任意一名玩家的状态」 includes the user; \
             prompt offers only others: {}",
            t.dump_prompt()
        );
    }
}

/// 规则书: 「（若指定了不存在状态2的玩家则无效果）」.
///
/// Designating a player with no 状态2 must be a no-op. This contract drives
/// the designate of a non-Ave-Mujica seat and checks `skillState` is unchanged.
#[test]
fn welcome_no_state2_is_noop() {
    let mut t = Table::vanilla(3);
    t.set_character_raw(0, "丰川祥子");
    t.place_raw(0, SKILL_BAND);
    // P2 is a vanilla non-Mujica seat (no band skill, no 状态2).
    t.set_state(2, "skillState", 0);
    t.give(0, &[WELCOME]);
    t.play(0, WELCOME).expect("play");
    let p = t.expect_prompt();
    let k = p
        .options
        .iter()
        .position(|o| format!("{o:?}").contains("PlayerId(2)"))
        .unwrap_or_else(|| panic!("P2 not offered: {}", t.dump_prompt())) as i32;
    t.answer(0, k).expect("designate P2");
    drain(&mut t);
    let s = t.state(2, "skillState");
    if s != 0 {
        panic!(
            "C-MUJICA-欢迎-02: designated a player with no 状态2; \
             skillState became {s}, expected 0 (「则无效果」)"
        );
    }
}
