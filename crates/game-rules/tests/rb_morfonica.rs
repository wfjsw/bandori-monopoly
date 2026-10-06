//! Morfonica rulebook integration tests (black-box).
//!
//! Spec: `target/scratch/rb/morfonica.md` (live Google Sheet text). Every
//! assertion is on observable game state; each block quotes the clause it
//! checks. Disagreements with the text are marked
//! `#[ignore = "DISCREPANCY: ..."]` after ruling out setup errors.

mod common;
use common::*;
use game_core::state::{stage, Counter};

// ---------------------------------------------------------------- helpers

/// [`Table::settle`], but also accepts a resting MOVE stage (a card that moves
/// a non-turn player can leave the engine in MOVE at rest).
fn settle_move(t: &mut Table) {
    for _ in 0..2000 {
        let st = t.m.state();
        if st.phase != "play" || st.prompt.id != 0 {
            return;
        }
        let w = t.m.world();
        if !w.next_turn_pending
            && w.leftovers.is_empty()
            && !st.busy
            && (st.step == stage::OPS || st.step == stage::END || st.step == stage::MOVE)
        {
            return;
        }
        t.m.tick(0.25);
    }
    panic!("settle_move: stuck at {:?}", t.m.state().step);
}

fn decline_all(t: &mut Table) {
    for _ in 0..10 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
}

/// Roll a main move and absorb any incidental prompts.
fn roll_quiet(t: &mut Table, who: usize, faces: &[i32]) {
    t.dice(faces);
    t.roll(who).unwrap();
    decline_all(t);
}

/// End a turn (requires a roll first in 运营) and absorb prompts.
fn end_quiet(t: &mut Table, who: usize) {
    roll_quiet(t, who, &[1]);
    t.end(who).unwrap();
    decline_all(t);
}

fn add_token(t: &mut Table, who: usize, name: &str, value: i32) {
    t.m.world_mut().st.players[who]
        .tokens
        .push(Counter { name: name.into(), value });
}

// ============================================================ cards

// --------------------------------------------------- 迷茫之蝶们的三全音

#[test]
fn tritone_offered_when_about_to_pay() {
    // 规则书: 「[反击] 任意时刻当你将要失去或支付资金时打出此卡」
    let mut t = Table::vanilla(2);
    t.own(1, &[7]); // 江户川公园, rent[0]=140
    t.set_pos(0, 6);
    t.give(0, &["Mor:迷茫之蝶们的三全音"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(
        t.react_offered("Mor:迷茫之蝶们的三全音"),
        "{}",
        t.dump_prompt()
    );
    assert_eq!(t.asked(), vec![0]);
    // skip: the payment settles normally
    t.decline();
    assert_eq!(t.money(0), 10_000 - 140);
    assert_eq!(t.money(1), 10_000 + 140);
}

#[test]
#[ignore = "DISCREPANCY: book gains the lost amount and places the card with 3 crystals; engine pays normally and discards the card (money 9860 not 10140/10000, field empty, no crystals)"]
fn tritone_gains_amount_and_places_with_crystals() {
    // 规则书: 「立刻获得此次失去的资金金额，此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）」
    let mut t = Table::vanilla(2);
    t.own(1, &[7]);
    t.set_pos(0, 6);
    t.give(0, &["Mor:迷茫之蝶们的三全音"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    t.react(0, "Mor:迷茫之蝶们的三全音").unwrap();
    // the 140 rent is the 「此次失去的资金金额」: gained back on the spot
    assert_eq!(t.money(0), 10_000, "gains the 140 it just lost");
    assert!(t.on_field(0, "Mor:迷茫之蝶们的三全音"), "{:?}", t.field_ids(0));
    assert_eq!(t.crystals(0, "Mor:迷茫之蝶们的三全音"), Some(3));
}

#[test]
#[ignore = "DISCREPANCY: depends on the field placement above; engine never puts the card on the field"]
fn tritone_countdown_discards_and_pays_back() {
    // 规则书: 「三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金」
    let mut t = Table::vanilla(2);
    t.own(1, &[7]);
    t.set_pos(0, 6);
    t.give(0, &["Mor:迷茫之蝶们的三全音"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    t.react(0, "Mor:迷茫之蝶们的三全音").unwrap();
    assert_eq!(t.crystals(0, "Mor:迷茫之蝶们的三全音"), Some(3));
    // two turn-ends: 3 -> 2 -> 1, still on field
    end_quiet(&mut t, 0);
    assert_eq!(t.crystals(0, "Mor:迷茫之蝶们的三全音"), Some(2));
    end_quiet(&mut t, 1);
    assert_eq!(t.crystals(0, "Mor:迷茫之蝶们的三全音"), Some(1));
    // third turn-end: discard and pay back the 140
    end_quiet(&mut t, 0);
    assert!(!t.on_field(0, "Mor:迷茫之蝶们的三全音"));
    assert_eq!(t.money(0), 10_000 - 140, "pays back the 140 it gained");
}

// --------------------------------------------------- 夏日合宿

#[test]
fn summer_plays_to_field_until_next_turn_then_draws() {
    // 规则书: 「打出此卡，直到下个自己的回合开始前…当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌」
    let mut t = Table::vanilla(2);
    t.set_draw(0, &["R:[衍生] 压", "R:[衍生] 压"]);
    t.give_play(0, "Mor:夏日合宿").unwrap();
    assert!(t.on_field(0, "Mor:夏日合宿"), "{:?}", t.field_ids(0));
    assert!(t.hand(0).is_empty());
    // still up through the opponent's turn
    end_quiet(&mut t, 0);
    assert!(t.on_field(0, "Mor:夏日合宿"));
    end_quiet(&mut t, 1);
    // p0's next turn starts: effect ends, nothing was negated -> draw 1
    assert!(!t.on_field(0, "Mor:夏日合宿"), "{:?}", t.field_ids(0));
    assert_eq!(t.hand(0), vec!["R:[衍生] 压".to_string()], "drew one card");
}

// --------------------------------------------------- 你的光芒将照亮前路

#[test]
fn light_gate_within_20_of_tsukinomori() {
    // 规则书: 「当你在“月之森女子学院”格子前后20格之内，可以从手牌中打出此卡」
    let tsukinomori = tile("月之森女子学院"); // 13
    let mut t = Table::vanilla(2);
    // +20 and -20 (wrap) are the boundary and are in range
    t.set_pos(0, (tsukinomori + 20) % 60);
    t.give(0, &["Mor:你的光芒将照亮前路"]);
    assert!(t.play(0, "Mor:你的光芒将照亮前路").is_ok(), "at +20");
    t.set_pos(0, (tsukinomori + 60 - 20) % 60);
    t.give(0, &["Mor:你的光芒将照亮前路"]);
    assert!(t.play(0, "Mor:你的光芒将照亮前路").is_ok(), "at -20 (wrap)");
    // +21 out of range
    t.set_pos(0, (tsukinomori + 21) % 60);
    t.give(0, &["Mor:你的光芒将照亮前路"]);
    assert!(t.play(0, "Mor:你的光芒将照亮前路").is_err(), "at +21");
    assert!(t.hand(0).contains(&"Mor:你的光芒将照亮前路".to_string()));
    // a far tile is out of range
    t.set_pos(0, 45);
    t.give(0, &["Mor:你的光芒将照亮前路"]);
    assert!(t.play(0, "Mor:你的光芒将照亮前路").is_err(), "at 45");
}

#[test]
#[ignore = "DISCREPANCY: book moves the piece from 月之森 (13+4=17); engine walks from the real position (18+4=22)"]
fn light_move_starts_from_tsukinomori() {
    // 规则书: 「此次移动以“月之森女子学院”为起点（不触发起点地块效果）」
    let mut t = Table::vanilla(2);
    let tsukinomori = tile("月之森女子学院");
    t.set_pos(0, tsukinomori + 5);
    t.give_play(0, "Mor:你的光芒将照亮前路").unwrap();
    t.dice(&[4]);
    t.roll(0).unwrap();
    decline_all(&mut t);
    assert_eq!(t.pos(0), tsukinomori + 4, "start = 月之森, +4 steps");
}

// --------------------------------------------------- 蝴蝶飞舞的星月夜

#[test]
fn starry_pays_x_times_1000_and_places_x_crystals() {
    // 规则书: 「支付X次1000的的资金，将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]」
    let mut t = Table::vanilla(2);
    t.give_play(0, "Mor:蝴蝶飞舞的星月夜").unwrap();
    // X is asked as 1..=5
    let p = t.expect_prompt();
    assert_eq!(p.options.len(), 5, "X offers 1..5: {}", t.dump_prompt());
    t.answer_one(2).unwrap(); // X=3
    assert_eq!(t.money(0), 10_000 - 3 * 1000);
    assert!(t.on_field(0, "Mor:蝴蝶飞舞的星月夜"), "{:?}", t.field_ids(0));
    assert_eq!(t.crystals(0, "Mor:蝴蝶飞舞的星月夜"), Some(3));
}

#[test]
fn starry_x_is_at_least_one() {
    // 规则书: 「支付X次1000的的资金」 (X chosen by the player; engine offers 1..5)
    let mut t = Table::vanilla(2);
    t.give_play(0, "Mor:蝴蝶飞舞的星月夜").unwrap();
    t.answer_one(0).unwrap(); // X=1
    assert_eq!(t.money(0), 10_000 - 1000);
    assert_eq!(t.crystals(0, "Mor:蝴蝶飞舞的星月夜"), Some(1));
}

#[test]
fn starry_circle_pass_removes_crystal_and_pays_1000() {
    // 规则书: 「此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]」「此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金」
    let mut t = Table::vanilla(2);
    t.give_play(0, "Mor:蝴蝶飞舞的星月夜").unwrap();
    t.answer_one(2).unwrap(); // X=3, money 7000
    t.set_pos(0, 58);
    t.dice(&[3]); // 58 -> 59 -> 0(CiRCLE) -> 1
    t.roll(0).unwrap();
    assert_eq!(t.crystals(0, "Mor:蝴蝶飞舞的星月夜"), Some(2), "one removed");
    assert_eq!(t.money(0), 7000 + 1000, "each removed crystal pays 1000");
    decline_all(&mut t); // the CiRCLE-bonus choice
}

#[test]
fn starry_no_crystals_removes_the_card() {
    // 规则书: 「此卡没有[奇迹水晶]时加入弃牌堆」 (the card stops being in play)
    let mut t = Table::vanilla(2);
    t.give_play(0, "Mor:蝴蝶飞舞的星月夜").unwrap();
    t.answer_one(0).unwrap(); // X=1
    t.set_pos(0, 58);
    t.dice(&[3]); // one CiRCLE pass: the last crystal goes
    t.roll(0).unwrap();
    decline_all(&mut t);
    assert!(!t.on_field(0, "Mor:蝴蝶飞舞的星月夜"), "{:?}", t.field_ids(0));
    assert_eq!(t.crystals(0, "Mor:蝴蝶飞舞的星月夜"), None);
}

#[test]
#[ignore = "DISCREPANCY: book adds the empty card to the discard pile; engine removes it from the game (discard empty)"]
fn starry_empty_goes_to_discard() {
    // 规则书: 「此卡没有[奇迹水晶]时加入弃牌堆」
    let mut t = Table::vanilla(2);
    t.give_play(0, "Mor:蝴蝶飞舞的星月夜").unwrap();
    t.answer_one(0).unwrap();
    t.set_pos(0, 58);
    t.dice(&[3]);
    t.roll(0).unwrap();
    decline_all(&mut t);
    assert!(
        t.discard(0).contains(&"Mor:蝴蝶飞舞的星月夜".to_string()),
        "discard={:?}",
        t.discard(0)
    );
}

#[test]
#[ignore = "DISCREPANCY: book offers to abandon the first dice result and reroll once; engine never prompts (pos stays 50+3)"]
fn starry_owner_may_reroll_the_move_roll() {
    // 规则书: 「此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次」
    let mut t = Table::vanilla(2);
    t.give_play(0, "Mor:蝴蝶飞舞的星月夜").unwrap();
    t.answer_one(1).unwrap();
    t.set_pos(0, 50);
    t.dice(&[3, 7]);
    t.roll(0).unwrap();
    assert!(t.prompt().is_some(), "a reroll choice: {}", t.dump_prompt());
    t.answer_one(0).unwrap(); // accept the reroll
    decline_all(&mut t);
    assert_eq!(t.pos(0), 50 + 7);
}

#[test]
#[ignore = "DISCREPANCY: book offers to pay 1000 and remove a crystal when another player's move roll is even; engine never prompts"]
fn starry_even_roll_of_another_player_offers_crystal_removal() {
    // 规则书: 「当其他角色的移动掷骰结果是偶数时可消耗1000资金并移除此卡的一个[奇迹水晶]」
    let mut t = Table::vanilla(2);
    t.give_play(0, "Mor:蝴蝶飞舞的星月夜").unwrap();
    t.answer_one(1).unwrap(); // X=2, money 8000
    t.begin_turn(1);
    t.set_pos(1, 40);
    t.dice(&[2]); // even
    t.roll(1).unwrap();
    assert!(t.prompt().is_some(), "{}", t.dump_prompt());
    t.answer_one(0).unwrap(); // pay 1000, remove a crystal
    decline_all(&mut t);
    assert_eq!(t.money(0), 8000 - 1000 + 1000, "pay 1000, crystal payout 1000");
    assert_eq!(t.crystals(0, "Mor:蝴蝶飞舞的星月夜"), Some(1));
}

// --------------------------------------------------- 勇气展翅高飞之时

#[test]
fn courage_owner_pays_half_of_price_plus_houses() {
    // 规则书: 「掷骰3d20，结果对应序号格子的所有者向你支付该地块的购买价格+地块已有房子的建造价格总额的一半」
    // 3d20 sum = tile number #N (engine index N-1). sum 14 -> 月之森 (#14).
    let mut t = Table::vanilla(2);
    let tsukinomori = tile("月之森女子学院"); // price 3200, house 2000
    t.own(1, &[tsukinomori]);
    t.set_houses(tsukinomori, 1);
    t.dice(&[5, 5, 4]); // sum 14
    t.give_play(0, "Mor:勇气展翅高飞之时").unwrap();
    // (3200 + 1*2000) / 2 = 2600
    assert_eq!(t.money(0), 10_000 + 2600);
    assert_eq!(t.money(1), 10_000 - 2600);
}

#[test]
fn courage_sums_3d20_for_the_tile_number() {
    // 规则书: 「掷骰3d20，结果对应序号格子」 — sum 3 -> #3 = 天文馆 (price 2600)
    let mut t = Table::vanilla(2);
    t.own(1, &[tile("天文馆")]);
    t.dice(&[1, 1, 1]); // sum 3
    t.give_play(0, "Mor:勇气展翅高飞之时").unwrap();
    assert_eq!(t.money(0), 10_000 + 2600 / 2);
    assert_eq!(t.money(1), 10_000 - 1300);
}

#[test]
fn courage_agent_tile_gives_1000() {
    // 规则书: 「若为地产商地块，获得1000资金」
    let mut t = Table::vanilla(2);
    assert_eq!(data().tiles[tile("主要街道")].kind, "agent");
    t.dice(&[2, 2, 1]); // sum 5 -> #5 = 主要街道 (agent)
    t.give_play(0, "Mor:勇气展翅高飞之时").unwrap();
    assert_eq!(t.money(0), 10_000 + 1000);
}

#[test]
fn courage_unowned_tile_pays_nothing() {
    // 规则书: 「结果对应序号格子的所有者向你支付…」 — no owner, no payment
    let mut t = Table::vanilla(2);
    t.dice(&[5, 5, 4]); // sum 14 -> unowned 月之森
    t.give_play(0, "Mor:勇气展翅高飞之时").unwrap();
    assert_eq!(t.money(0), 10_000);
    assert_eq!(t.money(1), 10_000);
}

// --------------------------------------------------- （NNM）稍微努力了一下

#[test]
fn nnm_refused_without_character_marks() {
    // 规则书: 「弃置手中x枚角色标记，发动以下效果中的一个」 — a play without marks is refused
    let mut t = Table::new(&["广町七深", "户山香澄", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.give(0, &["Mor:（NNM）稍微努力了一下"]);
    assert!(t.play(0, "Mor:（NNM）稍微努力了一下").is_err());
    assert!(t.hand(0).contains(&"Mor:（NNM）稍微努力了一下".to_string()));
}

#[test]
#[ignore = "DISCREPANCY/harness gap: engine refuses the play even with a character token on the player (nanami_effort_not_placed); no way observed to obtain the hand-side 角色标记 that skill (2) is supposed to grant"]
fn nnm_discard_marks_draw_x_or_gain_x_times_2000() {
    // 规则书: 「弃置手中x枚角色标记…（1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张（2）获得x*2000资金」
    let mut t = Table::new(&["广町七深", "户山香澄", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    add_token(&mut t, 0, "户山香澄", 1);
    t.set_draw(0, &["R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压"]);
    t.give(0, &["Mor:（NNM）稍微努力了一下"]);
    t.play(0, "Mor:（NNM）稍微努力了一下").unwrap();
    t.answer_one(0).unwrap();
    decline_all(&mut t);
    assert_eq!(t.hand(0).len(), 1);
    end_quiet(&mut t, 0);
    assert!(t.hand(0).len() <= 5);
}

// --------------------------------------------------- （toko）

#[test]
fn toko_swaps_same_color_mortgaged_deeds_and_pays_difference() {
    // 规则书: 「选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换，地契+房屋价格更低的玩家向更高的玩家支付差价，互换后将你得到的地契免费赎回」
    let mut t = Table::new(&["桐谷透子", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    let mine = tile("月之森女子学院"); // 3200
    let theirs = tile("羽丘女子学院"); // 2200, same group
    t.own(0, &[mine]);
    t.own(1, &[theirs]);
    t.set_mortgaged(mine, true);
    t.set_mortgaged(theirs, true);
    t.give(0, &["Mor:（toko）"]);
    t.play(0, "Mor:（toko）").unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.options.len(), 1, "exactly one swap pair: {}", t.dump_prompt());
    t.answer_one(0).unwrap();
    // swapped
    assert_eq!(t.owner(mine), Some(1));
    assert_eq!(t.owner(theirs), Some(0));
    // lower (2200) pays higher (3200) the 1000 difference
    assert_eq!(t.money(0), 10_000 + 1000);
    assert_eq!(t.money(1), 10_000 - 1000);
    // the deed received is redeemed free; the deed given away stays mortgaged
    assert!(!t.mortgaged(theirs), "received deed redeemed free");
    assert!(t.mortgaged(mine), "given deed still mortgaged");
}

// --------------------------------------------------- （小白）

#[test]
fn xiaobai_turns_pay_into_lose_and_target_loses_half() {
    // 规则书: 「[反击]当你将要向其他玩家支付时打出此卡，此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金」
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "仓田真白");
    t.own(1, &[7]); // rent 140
    t.set_pos(0, 6);
    t.give(0, &["Mor:（小白）"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.react_offered("Mor:（小白）"), "{}", t.dump_prompt());
    t.react(0, "Mor:（小白）").unwrap();
    // payer loses 140 (not paid over); the target loses 140/2 = 70
    assert_eq!(t.money(0), 10_000 - 140);
    assert_eq!(t.money(1), 10_000 - 70);
}

#[test]
#[ignore = "DISCREPANCY: book counters any payment to another player; engine never opens the [反击] window on 通用:登上武道馆's 1000 collection (money moves 12000/9000/9000 with no prompt)"]
fn xiaobai_offered_on_card_driven_payments_too() {
    // 规则书: 「当你将要向其他玩家支付时打出此卡」
    let mut t = Table::vanilla(3);
    t.set_character_raw(1, "仓田真白");
    t.give(1, &["Mor:（小白）"]);
    t.begin_turn(0);
    t.give(0, &["通用:登上武道馆"]);
    t.play(0, "通用:登上武道馆").unwrap();
    assert!(t.react_offered("Mor:（小白）"), "{}", t.dump_prompt());
}

// --------------------------------------------------- 离心力，不为所动

#[test]
fn centrifuge_not_offered_on_the_first_targeting() {
    // 规则书: 「在两个你的回合之间…第二次成为其他角色技能或卡牌的目标时，你可以打出此卡」
    let mut t = Table::vanilla(3);
    t.give(0, &["Mor:离心力，不为所动"]);
    t.give(1, &["通用:登上武道馆"]);
    t.begin_turn(1);
    t.play(1, "通用:登上武道馆").unwrap();
    assert!(
        !t.react_offered("Mor:离心力，不为所动"),
        "1st targeting: {}",
        t.dump_prompt()
    );
    decline_all(&mut t);
}

#[test]
fn centrifuge_offers_on_the_second_targeting() {
    // 规则书: 「第二次成为其他角色技能或卡牌的目标时，你可以打出此卡，直到下个你的回合开始时，无效化你受到的所有效果」
    let mut t = Table::vanilla(3);
    t.give(0, &["Mor:离心力，不为所动"]);
    t.give(1, &["通用:登上武道馆", "通用:登上武道馆"]);
    t.begin_turn(1);
    t.play(1, "通用:登上武道馆").unwrap();
    decline_all(&mut t);
    t.play(1, "通用:登上武道馆").unwrap();
    assert!(
        t.react_offered("Mor:离心力，不为所动"),
        "{}",
        t.dump_prompt()
    );
    t.react(0, "Mor:离心力，不为所动").unwrap();
    // the shield is up (card on the field) until p0's next turn start
    assert!(t.on_field(0, "Mor:离心力，不为所动"), "{:?}", t.field_ids(0));
}

// --------------------------------------------------- 高贵的微蓝

#[test]
fn noble_gate_requires_deed_value_2200() {
    // 规则书: 「当你位于一块地契价值大于等于2200的地块时，可以打出此卡」
    let mut t = Table::vanilla(2);
    // CiRCLE (price 0) is below the gate
    t.give(0, &["Mor:高贵的微蓝"]);
    assert!(t.play(0, "Mor:高贵的微蓝").is_err());
    // 月之森 (3200) passes the gate
    t.set_pos(0, tile("月之森女子学院"));
    t.give(0, &["Mor:高贵的微蓝"]);
    assert!(t.play(0, "Mor:高贵的微蓝").is_ok());
}

#[test]
fn noble_on_own_tile_places_a_no_target_mark() {
    // 规则书: 「如果此地块属于你：…2.在该地块上放置一个标记，有标记时此地块不能被指定」
    let mut t = Table::vanilla(2);
    let tsukinomori = tile("月之森女子学院");
    t.own(0, &[tsukinomori]);
    t.set_pos(0, tsukinomori);
    t.give(0, &["Mor:高贵的微蓝"]);
    t.play(0, "Mor:高贵的微蓝").unwrap();
    let ms = t.marks_on(tsukinomori);
    assert_eq!(ms.len(), 1, "{ms:?}");
    assert_eq!(ms[0].owner, 0);
    assert_eq!(ms[0].kind, "noTarget");
}

#[test]
fn noble_on_foreign_tile_places_no_mark() {
    // 规则书: 「如果此地块属于你：1…2…」 — the mark is only for your own tile
    let mut t = Table::vanilla(2);
    let tsukinomori = tile("月之森女子学院");
    t.own(1, &[tsukinomori]);
    t.set_pos(0, tsukinomori);
    t.give(0, &["Mor:高贵的微蓝"]);
    t.play(0, "Mor:高贵的微蓝").unwrap();
    assert!(t.marks_on(tsukinomori).is_empty());
}

// --------------------------------------------------- 再次牵起手来

#[test]
fn again_hand_counter_places_on_field_and_loses_equal() {
    // 规则书: 「[反击]当[使用者]的行动序列前一名玩家[消耗]或[支付]大于0资金后将此卡放置在[使用者]的[场地]并[消耗]等量资金」
    let mut t = Table::vanilla(3);
    t.own(2, &[7]); // rent 140
    t.set_pos(0, 6);
    t.give(1, &["Mor:再次牵起手来"]); // user p1; p0 is the player before them
    t.begin_turn(0);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.react_offered("Mor:再次牵起手来"), "{}", t.dump_prompt());
    assert_eq!(t.asked(), vec![1]);
    t.react(1, "Mor:再次牵起手来").unwrap();
    // p0's 140 payment went through; p1 lost the same 140
    assert_eq!(t.money(0), 10_000 - 140);
    assert_eq!(t.money(1), 10_000 - 140);
    assert!(t.on_field(1, "Mor:再次牵起手来"), "{:?}", t.field_ids(1));
}

#[test]
fn again_persist_cancels_the_owners_next_payment() {
    // 规则书: 「[持续]：[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区」
    let mut t = Table::vanilla(3);
    t.own(2, &[7]);
    t.set_pos(0, 6);
    t.give(1, &["Mor:再次牵起手来"]);
    t.begin_turn(0);
    t.dice(&[1]);
    t.roll(0).unwrap();
    t.react(1, "Mor:再次牵起手来").unwrap();
    // p1 now lands on p2's tile: the rent is cancelled
    t.begin_turn(1);
    t.set_pos(1, 6);
    t.dice(&[1]);
    t.roll(1).unwrap();
    decline_all(&mut t);
    assert_eq!(t.money(1), 10_000 - 140, "the 140 rent was cancelled");
    assert_eq!(t.money(2), 10_000 + 140, "only the first rent arrived");
    assert!(!t.on_field(1, "Mor:再次牵起手来"), "{:?}", t.field_ids(1));
}

#[test]
#[ignore = "DISCREPANCY: book puts the spent [持续] card into the discard pile; engine removes it from the game (discard empty)"]
fn again_spent_card_goes_to_discard() {
    // 规则书: 「取消此次资金变动并将此卡放置到弃卡区」
    let mut t = Table::vanilla(3);
    t.own(2, &[7]);
    t.set_pos(0, 6);
    t.give(1, &["Mor:再次牵起手来"]);
    t.begin_turn(0);
    t.dice(&[1]);
    t.roll(0).unwrap();
    t.react(1, "Mor:再次牵起手来").unwrap();
    t.begin_turn(1);
    t.set_pos(1, 6);
    t.dice(&[1]);
    t.roll(1).unwrap();
    decline_all(&mut t);
    assert!(
        t.discard(1).contains(&"Mor:再次牵起手来".to_string()),
        "discard={:?}",
        t.discard(1)
    );
}

// --------------------------------------------------- 纯真振翅

#[test]
fn wing_teleports_20_without_settling() {
    // 规则书: 「传送到移动方向20格后（不触发结算），立刻进行移动掷骰」
    let mut t = Table::vanilla(2);
    t.own(1, &[25]); // a tile 20 ahead must NOT be settled
    t.set_pos(0, 5);
    t.dice(&[3]);
    t.give_play(0, "Mor:纯真振翅").unwrap();
    assert_eq!(t.pos(0), 25);
    // no settlement at the teleport destination: no rent was paid
    assert_eq!(t.money(0), 10_000);
    assert_eq!(t.money(1), 10_000);
}

#[test]
#[ignore = "DISCREPANCY: book then immediately performs the movement dice roll (5+20 then +3 = 28); engine consumes the main move on the teleport and never rolls (stays 25, dice queue untouched, roll refused)"]
fn wing_then_rolls_the_movement_dice() {
    // 规则书: 「…（不触发结算），立刻进行移动掷骰」
    let mut t = Table::vanilla(2);
    t.set_pos(0, 5);
    t.dice(&[3]);
    t.give_play(0, "Mor:纯真振翅").unwrap();
    assert_eq!(t.pos(0), 5 + 20 + 3);
}

// --------------------------------------------------- 秘密与青春的虹彩

#[test]
fn rainbow_halves_a_payment_to_a_same_grade() {
    // 规则书: 「（1）当你向学妹或同级生支付时，打出此卡，此次支付金额减半」
    // 户山香澄 / 花园多惠 are the same grade; the payer holds the card.
    let mut t = Table::vanilla(2);
    t.own(1, &[7]); // rent 140
    t.set_pos(0, 6);
    t.give(0, &["Mor:秘密与青春的虹彩"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.react_offered("Mor:秘密与青春的虹彩"), "{}", t.dump_prompt());
    t.react(0, "Mor:秘密与青春的虹彩").unwrap();
    assert_eq!(t.money(0), 10_000 - 70);
    assert_eq!(t.money(1), 10_000 + 70);
}

#[test]
#[ignore = "DISCREPANCY: book makes a senior's payment to you 1.5x (140 -> 210); engine settles it at 0 (both players stay at 10000) when 广町七深(1y) is paid by 二叶筑紫(3y)"]
fn rainbow_senior_pays_you_at_1_5x() {
    // 规则书: 「（2）当学姐或同级生向你支付的时候，打出此卡，使此次支付资金变成1.5倍」
    let mut t = Table::new(&["广町七深", "二叶筑紫"]);
    t.clean();
    t.begin_turn(1);
    t.own(0, &[7]);
    t.set_pos(1, 6);
    t.give(0, &["Mor:秘密与青春的虹彩"]); // the payee holds
    t.dice(&[1]);
    t.roll(1).unwrap();
    assert!(t.react_offered("Mor:秘密与青春的虹彩"), "{}", t.dump_prompt());
    t.react(0, "Mor:秘密与青春的虹彩").unwrap();
    assert_eq!(t.money(0), 10_000 + 210);
    assert_eq!(t.money(1), 10_000 - 210);
}

// --------------------------------------------------- （Rui）正论恶魔

#[test]
fn rui_pays_100_to_all_then_receives_300_from_all() {
    // 规则书: 「向全场玩家支付100资金（该数值不可被任何效果影响），之后全场玩家向你支付300资金（可被影响）」
    let mut t = Table::new(&["八潮瑠唯", "户山香澄", "花园多惠", "牛込里美"]);
    t.clean();
    t.begin_turn(0);
    t.give(0, &["Mor:（Rui）正论恶魔"]);
    t.play(0, "Mor:（Rui）正论恶魔").unwrap();
    // -300 + 900
    assert_eq!(t.money(0), 10_000 + 600);
    for who in 1..4 {
        assert_eq!(t.money(who), 10_000 - 200, "p{who}");
    }
}

#[test]
#[ignore = "DISCREPANCY: book sets X to 5 when the skill fires during the card's resolution; engine leaves X at 3 (three misses of +1 each)"]
fn rui_card_sets_x_to_5_when_the_skill_fires() {
    // 规则书: 「在此卡结算过程中，若你的技能被触发，将x设置为5」
    let mut t = Table::new(&["八潮瑠唯", "户山香澄", "花园多惠", "牛込里美"]);
    t.clean();
    t.begin_turn(0);
    t.give(0, &["Mor:（Rui）正论恶魔"]);
    t.play(0, "Mor:（Rui）正论恶魔").unwrap();
    assert_eq!(t.state(0, "skill.yuriCrit.x"), 5);
}

// --------------------------------------------------- （筑紫）迷茫的庭园

#[test]
fn tsukushi_gains_100_and_teleports_to_the_tile_before_a_player() {
    // 规则书: 「获得100资金。投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动」
    let mut t = Table::new(&["二叶筑紫", "户山香澄", "花园多惠", "牛込里美", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    t.set_pos(1, 20);
    t.give(0, &["Mor:（筑紫）迷茫的庭园"]);
    t.dice(&[2]); // 1d6 = 2 -> 2nd player in action order (p1) -> one ahead of them
    t.play(0, "Mor:（筑紫）迷茫的庭园").unwrap();
    let p = t.expect_prompt();
    // the settle is optional; the teleport target is already chosen
    assert_eq!(p.options.len(), 2, "yes/no settle: {}", t.dump_prompt());
    t.decline(); // 「可以选择是否触发结算」 — decline
    assert_eq!(t.money(0), 10_000 + 100);
    assert_eq!(t.pos(0), 21, "one ahead of p1 (20)");
}

#[test]
fn tsukushi_self_choice_advances_one_tile() {
    // 规则书: 「选中自己则前进一格」
    let mut t = Table::new(&["二叶筑紫", "户山香澄", "花园多惠", "牛込里美", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    t.set_pos(0, 10);
    t.give(0, &["Mor:（筑紫）迷茫的庭园"]);
    t.dice(&[1]); // 1d6 = 1 -> self -> advance one
    t.play(0, "Mor:（筑紫）迷茫的庭园").unwrap();
    decline_all(&mut t);
    assert_eq!(t.pos(0), 11);
}

#[test]
fn tsukushi_settle_is_optional() {
    // 规则书: 「可以选择是否触发结算」
    let mut t = Table::new(&["二叶筑紫", "户山香澄", "花园多惠", "牛込里美", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    t.own(1, &[21]); // the destination is owned: settling would pay rent
    t.set_pos(1, 20);
    t.give(0, &["Mor:（筑紫）迷茫的庭园"]);
    t.dice(&[2]);
    t.play(0, "Mor:（筑紫）迷茫的庭园").unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.options.len(), 2);
    t.answer_one(1).unwrap(); // no settle
    assert_eq!(t.money(0), 10_000 + 100, "no rent paid");
    assert_eq!(t.money(1), 10_000);
}

#[test]
fn tsukushi_teleport_counts_as_the_main_move() {
    // 规则书: 「…并视为主要移动」
    let mut t = Table::new(&["二叶筑紫", "户山香澄", "花园多惠", "牛込里美", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    t.give(0, &["Mor:（筑紫）迷茫的庭园"]);
    t.dice(&[1]);
    t.play(0, "Mor:（筑紫）迷茫的庭园").unwrap();
    decline_all(&mut t);
    // the main move is spent: no further roll this turn
    assert!(t.roll(0).is_err(), "main move already used");
}

// ============================================================ character skills

#[test]
fn mashiro_main_move_is_2d20_backwards() {
    // 规则书: 「（1）[主动移动]时移动掷骰变为2d20（2）反方向移动」
    let mut t = Table::new(&["仓田真白", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.set_pos(0, 30);
    t.dice(&[5, 6]); // 2d20 = 11, backwards
    t.roll(0).unwrap();
    decline_all(&mut t);
    assert_eq!(t.pos(0), 30 - 11);
}

#[test]
fn toko_fire_pot_cap_is_one() {
    // 规则书: 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」
    let mut t = Table::new(&["桐谷透子", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.set_fire(0, 1, 1);
    t.set_pos(0, 58);
    t.dice(&[3]); // passes CiRCLE
    t.roll(0).unwrap();
    decline_all(&mut t);
    assert_eq!(t.fire(0), 1, "capped at 1");
}

#[test]
fn toko_skill2_splits_half_the_rent_over_the_neighbours() {
    // 规则书: 「（2）一回合一次，在你的经营阶段，你可以消耗一个火罐并指定你的一个地块，指定前后各一格范围内（不包括该格子本身）的所有其他玩家支付你X，X为此地的地租的一半/指定玩家数（向上取整百）」
    let mut t = Table::new(&["桐谷透子", "户山香澄", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.set_fire(0, 1, 1);
    let tsukinomori = tile("月之森女子学院"); // rent[0]=320
    t.own(0, &[tsukinomori]);
    t.set_pos(1, tsukinomori - 1);
    t.set_pos(2, tsukinomori + 1);
    let sid = t.skill_id(0, "桐谷透子");
    t.skill(0, &sid).unwrap();
    t.answer_one(0).unwrap(); // pick the only offered tile
    decline_all(&mut t);
    // X = ceil100( (320/2) / 2 ) = ceil100(80) = 100 each
    assert_eq!(t.money(0), 10_000 + 200);
    assert_eq!(t.money(1), 10_000 - 100);
    assert_eq!(t.money(2), 10_000 - 100);
    assert_eq!(t.fire(0), 0, "the fire pot was spent");
}

#[test]
fn toko_skill2_is_once_per_turn() {
    // 规则书: 「一回合一次」
    let mut t = Table::new(&["桐谷透子", "户山香澄", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.set_fire(0, 1, 1);
    t.own(0, &[tile("月之森女子学院")]);
    t.set_pos(1, tile("月之森女子学院") - 1);
    let sid = t.skill_id(0, "桐谷透子");
    t.skill(0, &sid).unwrap();
    t.answer_one(0).unwrap();
    decline_all(&mut t);
    t.set_fire(0, 1, 1);
    assert!(t.skill(0, &sid).is_err(), "second press in the same turn");
}

#[test]
fn nana_fire_pot_cap_is_two() {
    // 规则书: 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限2）」
    let mut t = Table::new(&["广町七深", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.set_fire(0, 1, 2); // one below the cap
    t.set_pos(0, 58);
    t.dice(&[3]); // passes CiRCLE
    t.roll(0).unwrap();
    decline_all(&mut t);
    assert_eq!(t.fire(0), 2, "gains one, capped at 2");
}

#[test]
fn rui_crit_x_starts_at_zero_and_increments_on_a_miss() {
    // 规则书: 「（2）…roll1d20，若结果大于X，X+1。X初始为0」
    let mut t = Table::new(&["八潮瑠唯", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    assert_eq!(t.state(0, "skill.yuriCrit.x"), 0, "X initial 0");
    t.own(1, &[7]);
    t.set_pos(0, 6);
    t.dice(&[1]); // the move; the skill's own d20 is rolled from the RNG after
    t.roll(0).unwrap();
    decline_all(&mut t);
    // a loss of 140 fires the skill; with X=0 every d20 misses -> X+1
    assert_eq!(t.money(0), 10_000 - 140, "the loss still happens on a miss");
    assert_eq!(t.state(0, "skill.yuriCrit.x"), 1);
}

#[test]
#[ignore = "DISCREPANCY: book triggers the skill on gains too (「获得或失去资金」); engine never rolled the d20 nor moved X on a [获得] of 1000 (money 11000, X stayed 0)"]
fn rui_crit_fires_on_a_gain_as_well() {
    // 规则书: 「当你即将获得或失去资金（…结果不等于0）时，roll1d20…若结果大于X，X+1」
    let mut t = Table::new(&["八潮瑠唯", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.give_play(0, "R:[衍生] 压").unwrap(); // gain 1000
    assert_eq!(t.money(0), 11_000);
    assert_eq!(t.state(0, "skill.yuriCrit.x"), 1);
}

#[test]
#[ignore = "DISCREPANCY: book lets 二叶筑紫 redirect her gains/draws to another player (with an X mark and a record); engine never prompts on a gain of 1000 (money stays 11000, no marks)"]
fn tsukushi_skill1_redirects_a_gain() {
    // 规则书: 「（1）当你获得资金或者抽卡时，可以改为指定场上除你以外的一个角色进行一次该动作，发送给对方一个X（占位）标记并记录获得因此效果获得资金的数量」
    let mut t = Table::new(&["二叶筑紫", "户山香澄", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.give_play(0, "R:[衍生] 压").unwrap();
    assert!(t.prompt().is_some(), "{}", t.dump_prompt());
    t.answer_one(0).unwrap(); // redirect to p1
    decline_all(&mut t);
    assert_eq!(t.money(0), 10_000, "the gain was redirected");
    assert_eq!(t.money(1), 11_000);
    assert_eq!(t.token(1, "X"), 1);
}

// ============================================================ band skill

#[test]
fn morfonica_band_draws_on_a_2000_plus_loss() {
    // 规则书: 「（1）当你一次性失去2000以上资金时，抽一张卡」
    let mut t = Table::new(&["广町七深", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.set_draw(0, &["R:[衍生] 压", "R:[衍生] 压"]);
    let hall = tile("弦卷豪宅"); // rent[2]=3400
    t.own(1, &[hall]);
    t.set_houses(hall, 2);
    t.set_pos(0, hall - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    decline_all(&mut t);
    assert_eq!(t.money(0), 10_000 - 3400);
    assert_eq!(t.hand(0), vec!["R:[衍生] 压".to_string()], "drew one card");
}

#[test]
fn morfonica_band_circle_money_cycles_1000_1500_2000() {
    // 规则书: 「（2）[CiRCLE奖励]选择[获得]资金时根据选择[获得]资金次数设资金量为1000，1500，2000的循环」
    let mut t = Table::new(&["广町七深", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    let mut gains = Vec::new();
    for i in 0..4 {
        t.set_pos(0, 58);
        t.dice(&[3]); // passes CiRCLE
        t.roll(0).unwrap();
        let before = t.money(0);
        t.answer_one(0).unwrap(); // take the money option
        decline_all(&mut t);
        gains.push(t.money(0) - before);
        if i < 3 {
            t.begin_turn(0);
        }
    }
    assert_eq!(gains, vec![1000, 1500, 2000, 1000], "1000/1500/2000 cycle");
}

// ============================================================ interactions

#[test]
fn interaction_centrifuge_vs_toubudoukan() {
    // 通用:登上武道馆 (other group) is a targeting card: the 2nd targeting
    // between p0's turns offers Mor:离心力，不为所动.
    // 规则书: 「第二次成为其他角色技能或卡牌的目标时，你可以打出此卡」
    let mut t = Table::vanilla(3);
    t.give(0, &["Mor:离心力，不为所动"]);
    t.give(1, &["通用:登上武道馆", "通用:登上武道馆"]);
    t.begin_turn(1);
    t.play(1, "通用:登上武道馆").unwrap();
    assert!(!t.react_offered("Mor:离心力，不为所动"));
    decline_all(&mut t);
    t.play(1, "通用:登上武道馆").unwrap();
    assert!(t.react_offered("Mor:离心力，不为所动"), "{}", t.dump_prompt());
    t.react(0, "Mor:离心力，不为所动").unwrap();
    assert!(t.on_field(0, "Mor:离心力，不为所动"));
}

#[test]
fn interaction_net_error_negates_summer() {
    // 通用:网络链接异常 (other group) counters a [手] play: Mor:夏日合宿.
    // 规则书: 「打出此卡，直到下个自己的回合开始前…」 — the negated play
    // must not establish the shield.
    let mut t = Table::vanilla(2);
    t.give(0, &["Mor:夏日合宿"]);
    t.give(1, &["通用:网络链接异常"]);
    t.play(0, "Mor:夏日合宿").unwrap();
    assert!(t.react_offered("通用:网络链接异常"), "{}", t.dump_prompt());
    t.react(1, "通用:网络链接异常").unwrap();
    assert!(!t.on_field(0, "Mor:夏日合宿"), "{:?}", t.field_ids(0));
}

#[test]
fn interaction_again_cancels_toubudoukan_payment() {
    // 通用:登上武道馆 makes everyone else pay 1000; the [持续] on
    // Mor:再次牵起手来 cancels its owner's payment.
    // 规则书: 「[持续]：[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区」
    let mut t = Table::vanilla(3);
    t.place_raw(1, "Mor:再次牵起手来");
    t.begin_turn(0);
    t.give_play(0, "通用:登上武道馆").unwrap();
    decline_all(&mut t);
    // p1 (shielded) paid nothing; p2 paid 1000 to p0
    assert_eq!(t.money(1), 10_000, "the 1000 was cancelled");
    assert_eq!(t.money(2), 10_000 - 1000);
    assert_eq!(t.money(0), 10_000 + 1000);
    assert!(!t.on_field(1, "Mor:再次牵起手来"));
}

#[test]
fn interaction_summer_shield_vs_foreign_targeting() {
    // 夏日合宿's shield stops other players' effects from designating you;
    // 通用:登上武道馆's payment is not collected from the shielded player.
    // 规则书: 「直到下个自己的回合开始前，你只会被自己发动的效果指定」
    let mut t = Table::vanilla(3);
    t.give_play(0, "Mor:夏日合宿").unwrap();
    t.begin_turn(1);
    t.give(1, &["通用:登上武道馆"]);
    t.play(1, "通用:登上武道馆").unwrap();
    decline_all(&mut t);
    assert_eq!(t.money(0), 10_000, "shielded from the foreign effect");
    assert_eq!(t.money(2), 10_000 - 1000, "the unshielded player still pays");
}

#[test]
fn interaction_xuanzhan_and_centrifuge_answer_the_same_targeting() {
    // AG:宣战布告 (other group) answers the 1st targeting of 通用:登上武道馆;
    // Mor:离心力，不为所动 answers the 2nd -- two counters, one kind of timing.
    // 规则书: 「多个效果可[反击]同一个时点」 (rules.txt); 离心力: 「第二次…目标时」
    let mut t = Table::vanilla(3);
    t.give(0, &["Mor:离心力，不为所动"]);
    t.give(1, &["AG:宣战布告"]);
    t.give(2, &["通用:登上武道馆", "通用:登上武道馆"]);
    t.begin_turn(2);
    t.play(2, "通用:登上武道馆").unwrap();
    assert!(t.react_offered("AG:宣战布告"), "1st: {}", t.dump_prompt());
    t.decline();
    t.play(2, "通用:登上武道馆").unwrap();
    assert!(t.react_offered("Mor:离心力，不为所动"), "2nd: {}", t.dump_prompt());
    t.react(0, "Mor:离心力，不为所动").unwrap();
    assert!(t.on_field(0, "Mor:离心力，不为所动"));
}