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
        t.counteract_offered("Mor:迷茫之蝶们的三全音"),
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
fn tritone_gains_amount_and_places_with_crystals() {
    // 规则书: 「立刻获得此次失去的资金金额，此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）」
    let mut t = Table::vanilla(2);
    t.own(1, &[7]);
    t.set_pos(0, 6);
    t.give(0, &["Mor:迷茫之蝶们的三全音"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    t.counteract(0, "Mor:迷茫之蝶们的三全音").unwrap();
    // the 140 rent is the 「此次失去的资金金额」: gained back on the spot
    assert_eq!(t.money(0), 10_000, "gains the 140 it just lost");
    assert!(t.on_field(0, "Mor:迷茫之蝶们的三全音"), "{:?}", t.field_ids(0));
    assert_eq!(t.crystals(0, "Mor:迷茫之蝶们的三全音"), Some(3));
}

#[test]
fn tritone_countdown_discards_and_pays_back() {
    // 规则书: 「三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金」
    // 「三回合后」 is three of the **owner's** turns (C# `DecayCard.DecayOn` =
    // the player the card sits at): an opponent's turn end does not tick the
    // countdown. The harness's `end_quiet` re-rolls after the counteract (the
    // player already rolled), so drive the turns by hand with
    // `begin_turn` / `roll` / `end`.
    let mut t = Table::vanilla(2);
    t.own(1, &[7]);
    t.set_pos(0, 6);
    t.give(0, &["Mor:迷茫之蝶们的三全音"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    t.counteract(0, "Mor:迷茫之蝶们的三全音").unwrap();
    assert_eq!(t.crystals(0, "Mor:迷茫之蝶们的三全音"), Some(3));
    // P0 already rolled this turn (the counteract answered its rent); end the
    // turn without re-rolling. Owner turn-end #1: 3 -> 2.
    t.end(0).unwrap();
    decline_all(&mut t);
    assert_eq!(t.crystals(0, "Mor:迷茫之蝶们的三全音"), Some(2));
    // P1's turn comes around; its end does not tick the owner's countdown.
    t.begin_turn(1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    decline_all(&mut t);
    t.end(1).unwrap();
    decline_all(&mut t);
    assert_eq!(
        t.crystals(0, "Mor:迷茫之蝶们的三全音"),
        Some(2),
        "an opponent's turn end does not tick 「三回合后」"
    );
    // Owner turn-end #2: 2 -> 1, still on the field.
    t.begin_turn(0);
    t.dice(&[1]);
    t.roll(0).unwrap();
    decline_all(&mut t);
    t.end(0).unwrap();
    decline_all(&mut t);
    assert_eq!(t.crystals(0, "Mor:迷茫之蝶们的三全音"), Some(1));
    // Owner turn-end #3: 1 -> 0 -> 「弃置此卡并支付由此卡获得的资金」.
    t.begin_turn(0);
    t.dice(&[1]);
    t.roll(0).unwrap();
    decline_all(&mut t);
    t.end(0).unwrap();
    decline_all(&mut t);
    assert!(!t.on_field(0, "Mor:迷茫之蝶们的三全音"), "{:?}", t.field_ids(0));
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
    // The card's own RollAfter offer (「可以放弃第一次的结果重骰一次」) pauses
    // the walk before it starts; decline it so the walk runs on the original 3.
    t.decline();
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
        t.draw_pile(0).contains(&"Mor:蝴蝶飞舞的星月夜".to_string()),
        "discard={:?}",
        t.draw_pile(0)
    );
}

#[test]
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
//
// Ruling 2026-10-06 (corrected): the owner pays
//   land price + (total house cost / 2)
// -- only the house cost is halved; the land price counts in full.
// Sheet: 「该地块的购买价格+地块已有房子的建造价格总额的一半」.

#[test]
fn courage_owner_pays_land_plus_half_houses() {
    // 3d20 sum = tile number #N (engine index N-1). sum 14 -> 月之森 (#14).
    // 月之森: price 3200, house 2000. One house → 3200 + 2000/2 = 4200.
    // (The old reading (3200+2000)/2 = 2600 is wrong.)
    let mut t = Table::vanilla(2);
    let tsukinomori = tile("月之森女子学院");
    t.own(1, &[tsukinomori]);
    t.set_houses(tsukinomori, 1);
    t.dice(&[5, 5, 4]); // sum 14
    t.give_play(0, "Mor:勇气展翅高飞之时").unwrap();
    assert_eq!(t.money(0), 10_000 + 4200, "3200 + 2000/2 = 4200");
    assert_eq!(t.money(1), 10_000 - 4200);
}

#[test]
fn courage_land_price_is_not_halved() {
    // The distinguishing case: land 2000 with houses totalling 1000.
    // Ruled: 2000 + 1000/2 = 2500, NOT (2000+1000)/2 = 1500.
    // 山吹面包房 (#49, index 48): price 2000, house 1000. One house → 2500.
    let mut t = Table::vanilla(2);
    let yamabuki = tile("山吹面包房");
    assert_eq!(data().tiles[yamabuki].price, 2000);
    assert_eq!(data().tiles[yamabuki].house, 1000);
    t.own(1, &[yamabuki]);
    t.set_houses(yamabuki, 1);
    t.dice(&[20, 20, 9]); // sum 49 -> #49 = 山吹面包房
    t.give_play(0, "Mor:勇气展翅高飞之时").unwrap();
    assert_eq!(t.money(0), 10_000 + 2500, "2000 + 1000/2 = 2500, not 1500");
    assert_eq!(t.money(1), 10_000 - 2500);
}

#[test]
fn courage_two_houses_halve_their_total() {
    // Two houses on 山吹面包房: total house cost 2000, half = 1000.
    // Payment = 2000 + 1000 = 3000.
    let mut t = Table::vanilla(2);
    let yamabuki = tile("山吹面包房");
    t.own(1, &[yamabuki]);
    t.set_houses(yamabuki, 2);
    t.dice(&[20, 20, 9]); // sum 49
    t.give_play(0, "Mor:勇气展翅高飞之时").unwrap();
    assert_eq!(t.money(0), 10_000 + 3000, "2000 + (2*1000)/2 = 3000");
    assert_eq!(t.money(1), 10_000 - 3000);
}

#[test]
fn courage_sums_3d20_for_the_tile_number() {
    // 规则书: 「掷骰3d20，结果对应序号格子」 — sum 3 -> #3 = 天文馆 (price 2600).
    // No houses: the land price alone is paid, in full (2600), not halved.
    let mut t = Table::vanilla(2);
    t.own(1, &[tile("天文馆")]);
    t.dice(&[1, 1, 1]); // sum 3
    t.give_play(0, "Mor:勇气展翅高飞之时").unwrap();
    assert_eq!(t.money(0), 10_000 + 2600, "land price counts in full");
    assert_eq!(t.money(1), 10_000 - 2600);
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
#[ignore = "DISCREPANCY: card bug in Mor:（NNM）稍微努力了一下 -- its CardDef declares TWO On::Play entries (the (3) crystal-press `can_use_skill`/`use_skill`, then the hand body `nanami_effort`) and the host dispatches only the first, so a hand play is refused with nanami_effort_not_placed (is_placed() is false). Merge them into one Play entry that branches on is_placed(). The 角色标记 path itself is wired: 广町七深（2） now offers on skillUsed (the skill id rides t.cards) and names marks 角色标记:<skill id>, which NNM's TOKEN_PREFIX sees"]
fn nnm_discard_marks_draw_x() {
    // 规则书 (sheet 2026-10-06 新卡组卡 G7): 「弃置手中x枚角色标记…（1）抽x张卡（可超过上限），
    // 回合结束后将手牌弃置到五张」
    // The 角色标记 come from 广町七深（2）「得到一个该角色的标记」; arrange one
    // directly (namespace `角色标记:<skill id>`, what the skill now writes).
    let mut t = Table::new(&["广町七深", "户山香澄", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    add_token(&mut t, 0, "角色标记:skill:户山香澄:鼓动", 1);
    t.set_draw(0, &["R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压"]);
    t.give(0, &["Mor:（NNM）稍微努力了一下"]);
    t.play(0, "Mor:（NNM）稍微努力了一下").unwrap();
    t.answer_one(0).unwrap();
    decline_all(&mut t);
    assert_eq!(t.hand(0).len(), 1);
    end_quiet(&mut t, 0);
    assert!(t.hand(0).len() <= 5);
}

#[test]
#[ignore = "DISCREPANCY: card bug in Mor:（NNM）稍微努力了一下 -- its CardDef declares TWO On::Play entries (the (3) crystal-press `can_use_skill`/`use_skill`, then the hand body `nanami_effort`) and the host dispatches only the first, so a hand play is refused with nanami_effort_not_placed (is_placed() is false). Merge them into one Play entry that branches on is_placed(). Sheet 2026-10-06 新卡组卡 G7 (2): 「获得x次经过CiRCLE时的资金奖励」 (was 「获得x*2000资金」)"]
fn nnm_option2_gains_x_times_circle_money_reward() {
    // Sheet 2026-10-06 新卡组卡 G7 (2): 「获得x次经过CiRCLE时的资金奖励」
    // -- x times the CiRCLE pass *money* reward (专有名词 11: 「[CiRCLE奖励]：
    // [获得]2000资金或抽1张卡」), not a flat x*2000. With the default reward
    // that is 2000 per time; the point of the reword is that a modified
    // CiRCLE money reward (e.g. Morfonica's 1000/1500/2000 cycle) scales it.
    let mut t = Table::new(&["广町七深", "户山香澄", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    add_token(&mut t, 0, "角色标记:skill:户山香澄:鼓动", 1);
    t.give(0, &["Mor:（NNM）稍微努力了一下"]);
    t.play(0, "Mor:（NNM）稍微努力了一下").unwrap();
    // Options are (1) draw x / (2) money / (3) place. Pick the money one.
    let p = t.expect_prompt();
    let k = t.option("2000")
        .or(t.option("资金"))
        .or(t.option("CiRCLE"))
        .unwrap_or(1);
    t.answer_one(k).unwrap();
    decline_all(&mut t);
    // x = 1 → one CiRCLE money reward = 2000 (default).
    assert_eq!(
        t.money(0),
        10_000 + 2_000,
        "x=1 → one CiRCLE money reward"
    );
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
    assert!(t.counteract_offered("Mor:（小白）"), "{}", t.dump_prompt());
    t.counteract(0, "Mor:（小白）").unwrap();
    // payer loses 140 (not paid over); the target loses 140/2 = 70
    assert_eq!(t.money(0), 10_000 - 140);
    assert_eq!(t.money(1), 10_000 - 70);
}

#[test]
fn xiaobai_offered_on_card_driven_payments_too() {
    // 规则书: 「当你将要向其他玩家支付时打出此卡」
    let mut t = Table::vanilla(3);
    t.set_character_raw(1, "仓田真白");
    t.give(1, &["Mor:（小白）"]);
    t.begin_turn(0);
    t.give(0, &["通用:登上武道馆"]);
    t.play(0, "通用:登上武道馆").unwrap();
    assert!(t.counteract_offered("Mor:（小白）"), "{}", t.dump_prompt());
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
        !t.counteract_offered("Mor:离心力，不为所动"),
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
        t.counteract_offered("Mor:离心力，不为所动"),
        "{}",
        t.dump_prompt()
    );
    t.counteract(0, "Mor:离心力，不为所动").unwrap();
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
    assert!(t.counteract_offered("Mor:再次牵起手来"), "{}", t.dump_prompt());
    assert_eq!(t.asked(), vec![1]);
    t.counteract(1, "Mor:再次牵起手来").unwrap();
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
    t.counteract(1, "Mor:再次牵起手来").unwrap();
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
fn again_spent_card_goes_to_discard() {
    // 规则书: 「取消此次资金变动并将此卡放置到弃卡区」
    let mut t = Table::vanilla(3);
    t.own(2, &[7]);
    t.set_pos(0, 6);
    t.give(1, &["Mor:再次牵起手来"]);
    t.begin_turn(0);
    t.dice(&[1]);
    t.roll(0).unwrap();
    t.counteract(1, "Mor:再次牵起手来").unwrap();
    t.begin_turn(1);
    t.set_pos(1, 6);
    t.dice(&[1]);
    t.roll(1).unwrap();
    decline_all(&mut t);
    assert!(
        t.draw_pile(1).contains(&"Mor:再次牵起手来".to_string()),
        "discard={:?}",
        t.draw_pile(1)
    );
}

// --------------------------------------------------- 纯真振翅

#[test]
fn wing_teleports_20_without_settling() {
    // 规则书: 「传送到移动方向20格后（不触发结算），立刻进行移动掷骰」
    let mut t = Table::vanilla(2);
    t.own(1, &[25]); // the jump destination must NOT be settled
    t.set_pos(0, 5);
    t.dice(&[3]);
    t.give_play(0, "Mor:纯真振翅").unwrap();
    // The jump lands on 25 and settles nothing (no rent there); 「立刻进行
    // 移动掷骰」 then rolls 3 and walks on from 25 to 28.
    assert_eq!(t.pos(0), 5 + 20 + 3, "keys={:?}", t.recent_keys(16));
    // no settlement at the teleport destination: no rent was paid
    assert_eq!(t.money(0), 10_000);
    assert_eq!(t.money(1), 10_000);
}

#[test]
fn wing_then_rolls_the_movement_dice() {
    // 规则书: 「…（不触发结算），立刻进行移动掷骰」
    let mut t = Table::vanilla(2);
    t.set_pos(0, 5);
    t.dice(&[3]);
    t.give_play(0, "Mor:纯真振翅").unwrap();
    assert_eq!(t.pos(0), 5 + 20 + 3);
}

// --------------------------------------------------- 秘密与青春的虹彩
//
// Ruling 2026-10-06: grades come from BanG Dream character facts, shipped as
// data accompanying the rule. Grades advance for everyone at the same time,
// so the relative order (same / higher / lower) is static; the tests below
// depend only on that relation, not on an absolute year.
//
// ============ NORMALIZED GRADE ORDINALS ================================
// Everyone is placed in ONE reference school year -- the latest era the
// wiki covers, the MyGO / Ave Mujica era (the wiki's `Year_S3` column) --
// and converted to a single absolute ordinal:
//   jr-hi 1=7, jr-hi 2=8, jr-hi 3=9, hi 1=10, hi 2=11, hi 3=12,
//   university / adult = 13+.
// Comparing ordinals gives 学妹 (<), 同级生 (=), 学姐 (>) directly.
//
// The wiki's `School_S1/S2/S3` columns are ONE timeline of three
// consecutive school years, not per-band snapshots. Mixing the columns
// (e.g. PPP's S1 "1st yr" against Morfonica's S2 "1st yr") wrongly makes
// cross-band peers "the same grade"; normalized, Morfonica is a year
// junior to PPP and MyGO/Ave Mujica are two years junior.
//
// ---- Band debut offsets (in PPP's high-school year) --------------------
// Evidence: the `School_S*` columns on each wiki page -- Morfonica and RAS
// entries have S2+S3 but no S1; MyGO and Ave Mujica have only S3. PPP runs
// S1 1st-yr → S2 2nd-yr → S3 3rd-yr. Morfonica's band page also describes
// the band as "second-year students" (= S3), confirming the S2=debuts-as-
// 1st-years reading.
//   S1 = PPP hi-1: PPP, Afterglow, PP, Roselia, HHW exist.
//   S2 = PPP hi-2: Morfonica and RAS debut.
//        Morfonica 1st-yr hi  → 1 year junior to PPP.
//        RAS: Rei/Masuki/Chiyu 2nd-yr (same as PPP), Rokka 1st-yr (1 jr),
//        Reona 2nd-yr *junior high* (3 years younger than PPP's hi-2).
//   S3 = PPP hi-3: MyGO!!!!! and Ave Mujica debut.
//        Both 1st-yr hi  → 2 years junior to PPP.
// So: Morfonica's first-years are juniors to PPP (who are 2nd-years when
// Morfonica debuts); MyGO/Mujica first-years are two years junior to PPP.
//
// ---- Normalized ordinal table (reference = wiki Year_S3) ---------------
//   band       | character         | wiki page          | S3 school / year          | ord
//   -----------+-------------------+--------------------+---------------------------+-----
//   PPP        | 户山香澄 Kasumi    | Toyama_Kasumi      | Hanasakigawa Hi 3rd       | 12
//   PPP        | 花园多惠 Tae       | Hanazono_Tae       | Hanasakigawa Hi 3rd       | 12
//   PPP        | 牛込里美 Rimi      | Ushigome_Rimi      | Hanasakigawa Hi 3rd       | 12
//   PPP        | 山吹沙绫 Saaya     | Yamabuki_Saaya     | Hanasakigawa Hi 3rd       | 12
//   PPP        | 市谷有咲 Arisa     | Ichigaya_Arisa     | Hanasakigawa Hi 3rd       | 12
//   Afterglow  | 美竹兰 Ran         | Mitake_Ran         | Haneoka Hi 3rd            | 12
//   Afterglow  | 青叶摩卡 Moca      | Aoba_Moca          | Haneoka Hi 3rd            | 12
//   Afterglow  | 上原绯玛丽 Himari   | Uehara_Himari      | Haneoka Hi 3rd            | 12
//   Afterglow  | 宇田川巴 Tomoe     | Udagawa_Tomoe      | Haneoka Hi 3rd            | 12
//   Afterglow  | 羽泽鸫 Tsugumi     | Hazawa_Tsugumi     | Haneoka Hi 3rd            | 12
//   PP         | 丸山彩 Aya         | Maruyama_Aya       | Yotsuba Univ 1st          | 13
//   PP         | 冰川日菜 Hina      | Hikawa_Hina        | Keiho Univ 1st            | 13
//   PP         | 白鹭千圣 Chisato   | Shirasagi_Chisato  | Yotsuba Univ 1st          | 13
//   PP         | 大和麻弥 Maya      | Yamato_Maya        | Keiho Univ 1st            | 13
//   PP         | 若宫伊芙 Eve       | Wakamiya_Eve       | Hanasakigawa Hi 3rd       | 12
//   Roselia    | 凑友希那 Yukina    | Minato_Yukina      | Yotsuba Univ 1st          | 13
//   Roselia    | 冰川纱夜 Sayo      | Hikawa_Sayo        | Keiho Univ 1st            | 13
//   Roselia    | 今井莉莎 Lisa      | Imai_Lisa          | Yotsuba Univ 1st          | 13
//   Roselia    | 白金燐子 Rinko      | Shirokane_Rinko    | Yotsuba Univ 1st          | 13
//   Roselia    | 宇田川亚子 Ako     | Udagawa_Ako        | Haneoka Hi 2nd            | 11
//   HHW        | 弦卷心 Kokoro      | Tsurumaki_Kokoro   | Hanasakigawa Hi 3rd       | 12
//   HHW        | 濑田薰 Kaoru       | Seta_Kaoru         | Yotsuba Univ 1st          | 13
//   HHW        | 北泽育美 Hagumi    | Kitazawa_Hagumi    | Hanasakigawa Hi 3rd       | 12
//   HHW        | 松原花音 Kanon     | Matsubara_Kanon    | Keiho Univ 1st            | 13
//   HHW        | 奥泽美咲 Misaki    | Okusawa_Misaki     | Hanasakigawa Hi 3rd       | 12
//   Morfonica  | 仓田真白 Mashiro   | Kurata_Mashiro     | Tsukinomori Hi 2nd        | 11
//   Morfonica  | 桐谷透子 Touko     | Kirigaya_Touko     | Tsukinomori Hi 2nd        | 11
//   Morfonica  | 广町七深 Nanami    | Hiromachi_Nanami   | Tsukinomori Hi 2nd        | 11
//   Morfonica  | 二叶筑紫 Tsukushi  | Futaba_Tsukushi    | Tsukinomori Hi 2nd        | 11
//   Morfonica  | 八潮瑠唯 Rui       | Yashio_Rui         | Tsukinomori Hi 2nd        | 11
//   RAS        | 和奏瑞依 Rei       | Wakana_Rei         | Geijutsu Academy 3rd      | 12
//   RAS        | 朝日六花 Rokka     | Asahi_Rokka        | Haneoka Hi 2nd            | 11
//   RAS        | 佐藤益木 Masuki    | Satou_Masuki       | Shirayuki Private Hi 3rd  | 12
//   RAS        | 鳰原令王那 Reona   | Nyubara_Reona      | Kamogawa Jr-Hi 3rd        |  9
//   RAS        | 珠手知由 CHU²      | Tamade_Chiyu       | Celosia Intl 3rd/12th gr  | 12
//   MyGO       | 高松灯 Tomori      | Takamatsu_Tomori   | Haneoka Hi 1st            | 10
//   MyGO       | 千早爱音 Anon      | Chihaya_Anon       | Haneoka Hi 1st            | 10
//   MyGO       | 要乐奈 Raana       | Kaname_Raana       | Hanasakigawa Jr-Hi 3rd    |  9
//   MyGO       | 长崎素世 Soyo      | Nagasaki_Soyo      | Tsukinomori Hi 1st        | 10
//   MyGO       | 椎名立希 Taki      | Shiina_Taki        | Hanasakigawa Hi 1st       | 10
//   Ave Mujica | 三角初华 Uika      | Misumi_Uika        | Hanasakigawa Hi 1st       | 10
//   Ave Mujica | 若叶睦 Mutsumi     | Wakaba_Mutsumi     | Tsukinomori Hi 1st        | 10
//   Ave Mujica | 丰川祥子 Sakiko    | Togawa_Sakiko      | Haneoka Hi 1st            | 10
//   Ave Mujica | 八幡海铃 Umiri     | Yahata_Umiri       | Hanasakigawa Hi 1st       | 10
//   Ave Mujica | 祐天寺若麦 Nyamu   | Yuutenji_Nyamu     | Geijutsu Academy 1st      | 10
//   Sumimi     | 纯田真奈 Mana      | Sumita_Mana        | (no school year on wiki)  | UNCERTAIN (adult 13+)
//   CiRCLE     | 月岛麻里奈 Marina  | Tsukishima_Marina  | (CiRCLE staff, adult)     | UNCERTAIN (adult 13+)
//   CiRCLE     | 都筑诗船 Shifune   | Tsuzuki_Shifune    | (SPACE owner, grandmother)| UNCERTAIN (adult 13+)
//
// ---- Entries corrected against the wiki pages (2026-10-06) -------------
// The previous table mixed debut seasons (PPP "1st yr (S1)" vs Morfonica
// "1st yr (S2)" vs MyGO "1st yr (S3)"), which wrongly made cross-band
// peers look like the same grade. Corrections vs the wiki prose:
//   * 鳰原令王那 Nyubara_Reona -- "a third-year student at Kamogawa Chuuou
//     Middle School" (page prose). JUNIOR HIGH, not high school. S3 3rd-yr
//     jr-hi → ord 9 (was wrongly listed as a high-school 2nd-yr).
//   * 珠手知由 Tamade_Chiyu -- "a third-year returnee student at Celosia
//     International School", `Year_S3 = Third Year (12th Grade)`. 12th
//     grade → ord 12 (was listed as S2 2nd-yr / 11th grade). Page notes she
//     skipped grades and Celosia uses the Western school-year system; the
//     card compares GRADE, so 12th-grade stands.
//   * 和奏瑞依 Wakana_Rei -- "a third-year student at Geijutsu Academy's
//     Musical Department" → ord 12 (was S2 2nd-yr).
//   * 佐藤益木 Satou_Masuki -- "a third-year student at Shirayuki Private
//     Academy" → ord 12 (was S2 2nd-yr).
//   * 长崎素世 Nagasaki_Soyo -- school is Tsukinomori Girls' Academy (not
//     Hanasakigawa); "a first-year student at Tsukinomori" → ord 10.
//   * 祐天寺若麦 Yuutenji_Nyamu -- school is Geijutsu Academy (Theater
//     Dept), "a first-year" → ord 10 (not Hanasakigawa).
//   * 要乐奈 Kaname_Raana -- "a third-year student at Hanasakigawa Girls'
//     Junior High School" → ord 9 (jr-hi, confirmed).
//   * 若叶睦 Wakaba_Mutsumi -- "a first-year student at Tsukinomori Girls'
//     Academy" → ord 10.
//   * 丰川祥子 Togawa_Sakiko / 高松灯 Takamatsu_Tomori / 千早爱音
//     Chihaya_Anon -- "a first-year student at Haneoka Girls' High School"
//     → ord 10 (Haneoka, not Hanasakigawa).
//
// ---- Sources -----------------------------------------------------------
// https://bandori.fandom.com/wiki/<wiki page>, the `Infobox character`
// `School_S1/S2/S3` / `Year_S1/S2/S3` fields plus the lead prose, fetched
// 2026-10-06 via the MediaWiki API (`action=parse&prop=wikitext`). Band
// debut offsets from the `School_S*` column presence per band page and
// https://bandori.fandom.com/wiki/Morfonica ("second-year students").
//
// The card keys on the relation only:
//   学妹 (junior)  = strictly lower ordinal
//   同级生 (same)  = equal ordinal
//   学姐 (senior)  = strictly higher ordinal
//
// Card:
//   (1) 「当你向学妹或同级生支付时，打出此卡，此次支付金额减半」
//   (2) 「当学姐或同级生向你支付的时候，打出此卡，使此次支付资金变成1.5倍」
// So (1) needs the payee to be junior-or-same; (2) needs the payer to be
// senior-or-same. Paying a senior, or being paid by a junior, is unmodified.

#[test]
fn rainbow_halves_a_payment_to_a_same_grade() {
    // 规则书: 「（1）当你向学妹或同级生支付时，打出此卡，此次支付金额减半」
    // Normalized: 户山香澄 = 12, 花园多惠 = 12 (same grade, both PPP hi-3).
    // The payer holds the card → (1) applies (payee is 同级生) → halved.
    let mut t = Table::vanilla(2);
    t.own(1, &[7]); // rent 140
    t.set_pos(0, 6);
    t.give(0, &["Mor:秘密与青春的虹彩"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered("Mor:秘密与青春的虹彩"), "{}", t.dump_prompt());
    t.counteract(0, "Mor:秘密与青春的虹彩").unwrap();
    assert_eq!(t.money(0), 10_000 - 70);
    assert_eq!(t.money(1), 10_000 + 70);
}

// ---- Cross-band pair whose relation DEPENDS on the normalization --------
// 户山香澄 (PPP) = 12 vs 广町七深 (Morfonica) = 11. Normalized, PPP is a
// year SENIOR to Morfonica (Morfonica debuts as 1st-years when PPP is in
// 2nd year). A naive "debut year" table (both "1st yr") would call them the
// same grade and halve in BOTH directions; the ruled relation only halves
// when the PPP member is the payer.

#[test]
fn rainbow_cross_band_ppp_senior_pays_morfonica_junior() {
    // (1) covers 学妹: 户山香澄 = 12 pays 广町七深 = 11. The senior (PPP)
    // holds the card and pays a junior → halved.
    let mut t = Table::new(&["户山香澄", "广町七深"]);
    t.clean();
    t.begin_turn(0);
    decline_all(&mut t);
    t.own(1, &[7]); // rent 140
    t.set_pos(0, 6);
    t.give(0, &["Mor:秘密与青春的虹彩"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered("Mor:秘密与青春的虹彩"), "{}", t.dump_prompt());
    t.counteract(0, "Mor:秘密与青春的虹彩").unwrap();
    assert_eq!(t.money(0), 10_000 - 70, "PPP senior pays a Morfonica junior: halved");
    assert_eq!(t.money(1), 10_000 + 70);
}

#[test]
fn rainbow_cross_band_morfonica_junior_pays_ppp_senior_unmodified() {
    // Neither (1) nor (2) covers junior→senior. 广町七深 = 11 pays
    // 户山香澄 = 12. The junior (Morfonica) holds the card: (1) wants
    // 学妹/同级生 as the payee (here the payee is a 学姐), (2) wants the
    // payer to be 学姐/同级生 (here the payer is a 学妹). Unmodified.
    // This is the pair that flips under the normalization.
    let mut t = Table::new(&["广町七深", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    decline_all(&mut t);
    t.own(1, &[7]); // rent 140
    t.set_pos(0, 6);
    t.give(0, &["Mor:秘密与青春的虹彩"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    if t.counteract_offered("Mor:秘密与青春的虹彩") {
        t.counteract(0, "Mor:秘密与青春的虹彩").ok();
    }
    decline_all(&mut t);
    assert_eq!(
        t.money(0),
        10_000 - 140,
        "Morfonica junior pays a PPP senior: unmodified (NOT halved)"
    );
    assert_eq!(t.money(1), 10_000 + 140);
}

// Morfonica (11) vs MyGO (10): Morfonica is a year senior. Same flip.
#[test]
fn rainbow_cross_band_morfonica_senior_pays_mygo_junior() {
    // (1) covers 学妹: 广町七深 = 11 pays 椎名立希 = 10. The Morfonica
    // senior holds the card and pays a MyGO junior → halved.
    let mut t = Table::new(&["广町七深", "椎名立希"]);
    t.clean();
    t.begin_turn(0);
    decline_all(&mut t);
    t.own(1, &[7]); // rent 140
    t.set_pos(0, 6);
    t.give(0, &["Mor:秘密与青春的虹彩"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered("Mor:秘密与青春的虹彩"), "{}", t.dump_prompt());
    t.counteract(0, "Mor:秘密与青春的虹彩").unwrap();
    assert_eq!(t.money(0), 10_000 - 70, "Morfonica senior pays a MyGO junior: halved");
    assert_eq!(t.money(1), 10_000 + 70);
}

#[test]
fn rainbow_halves_a_payment_from_a_senior_to_a_junior() {
    // (1) covers 学妹: 丸山彩 = 13 (univ) pays 若宫伊芙 = 12 (hi-3).
    // The senior holds the card and pays a junior → halved.
    let mut t = Table::new(&["丸山彩", "若宫伊芙"]);
    t.clean();
    t.begin_turn(0);
    decline_all(&mut t);
    t.own(1, &[7]); // rent 140
    t.set_pos(0, 6);
    t.give(0, &["Mor:秘密与青春的虹彩"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered("Mor:秘密与青春的虹彩"), "{}", t.dump_prompt());
    t.counteract(0, "Mor:秘密与青春的虹彩").unwrap();
    assert_eq!(t.money(0), 10_000 - 70, "senior pays a junior: halved");
    assert_eq!(t.money(1), 10_000 + 70);
}

#[test]
fn rainbow_no_effect_when_a_junior_pays_a_senior() {
    // Neither (1) nor (2) covers junior→senior. 若宫伊芙 = 12 pays
    // 丸山彩 = 13. The junior holds the card: (1) wants 学妹/同级生 as the
    // payee (here the payee is a 学姐), (2) wants the payer to be
    // 学姐/同级生 (here the payer is a 学妹). Unmodified.
    let mut t = Table::new(&["若宫伊芙", "丸山彩"]);
    t.clean();
    t.begin_turn(0);
    decline_all(&mut t);
    t.own(1, &[7]); // rent 140
    t.set_pos(0, 6);
    t.give(0, &["Mor:秘密与青春的虹彩"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    // The card may still be offered (it is a legal [反击]), but playing it must
    // not change the payment. Try it if offered and assert the money is bare.
    if t.counteract_offered("Mor:秘密与青春的虹彩") {
        t.counteract(0, "Mor:秘密与青春的虹彩").ok();
    }
    decline_all(&mut t);
    assert_eq!(t.money(0), 10_000 - 140, "junior pays a senior: unmodified");
    assert_eq!(t.money(1), 10_000 + 140);
}

#[test]
fn rainbow_senior_pays_you_at_1_5x() {
    // 规则书: 「（2）当学姐或同级生向你支付的时候，打出此卡，使此次支付资金变成1.5倍」
    // Normalized: 丸山彩 = 13 is a 学姐 of 若宫伊芙 = 12. The junior holds
    // the card → (2) applies (payer is 学姐) → 1.5x.
    let mut t = Table::new(&["若宫伊芙", "丸山彩"]);
    t.clean();
    t.begin_turn(1);
    decline_all(&mut t);
    t.own(0, &[7]);
    t.set_pos(1, 6);
    t.give(0, &["Mor:秘密与青春的虹彩"]); // the payee (junior) holds
    t.dice(&[1]);
    t.roll(1).unwrap();
    assert!(t.counteract_offered("Mor:秘密与青春的虹彩"), "{}", t.dump_prompt());
    t.counteract(0, "Mor:秘密与青春的虹彩").unwrap();
    assert_eq!(t.money(0), 10_000 + 210);
    assert_eq!(t.money(1), 10_000 - 210);
}

#[test]
fn rainbow_same_grade_pays_you_at_1_5x() {
    // (2) also covers 同级生. Normalized: 广町七深 = 11, 二叶筑紫 = 11
    // (both Morfonica hi-2 -- same grade, NOT 1y vs 3y). The payee holds.
    let mut t = Table::new(&["广町七深", "二叶筑紫"]);
    t.clean();
    t.begin_turn(1);
    decline_all(&mut t);
    t.own(0, &[7]);
    t.set_pos(1, 6);
    t.give(0, &["Mor:秘密与青春的虹彩"]); // the payee holds
    t.dice(&[1]);
    t.roll(1).unwrap();
    assert!(t.counteract_offered("Mor:秘密与青春的虹彩"), "{}", t.dump_prompt());
    t.counteract(0, "Mor:秘密与青春的虹彩").unwrap();
    assert_eq!(t.money(0), 10_000 + 210, "same-grade payer → 1.5x");
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
fn rui_card_sets_x_to_5_when_the_skill_fires() {
    // 规则书 (sheet 2026-10-06 新卡组卡 G15): 「在此卡结算过程中，每当你的技能被触发后，
    // 立即将x设置为5」 (was 「若你的技能被触发，将x设置为5」 -- now every fire, immediately).
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
    // 交给班长吧（1）: 「当你获得资金…可以改为指定场上除你以外的一个角色」 fires
    // on the card's own 「获得100资金」; decline the redirect so the 100 stays.
    t.decline();
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
    // 交给班长吧（1） fires on the card's own 「获得100资金」; decline the
    // redirect so the 100 stays here.
    t.decline();
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
fn tsukushi_skill1_redirects_a_gain() {
    // 规则书: 「（1）当你获得资金或者抽卡时，可以改为指定场上除你以外的一个角色进行一次该动作，发送给对方一个X（占位）标记并记录获得因此效果获得资金的数量」
    let mut t = Table::new(&["二叶筑紫", "户山香澄", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.give_play(0, "R:[衍生] 压").unwrap();
    assert!(t.prompt().is_some(), "{}", t.dump_prompt());
    t.answer_one(0).unwrap(); // yes, redirect (「可以改为…」)
    // the pick defaults to the first other player (p1)
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
    assert!(!t.counteract_offered("Mor:离心力，不为所动"));
    decline_all(&mut t);
    t.play(1, "通用:登上武道馆").unwrap();
    assert!(t.counteract_offered("Mor:离心力，不为所动"), "{}", t.dump_prompt());
    t.counteract(0, "Mor:离心力，不为所动").unwrap();
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
    assert!(t.counteract_offered("通用:网络链接异常"), "{}", t.dump_prompt());
    t.counteract(1, "通用:网络链接异常").unwrap();
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
    assert!(t.counteract_offered("AG:宣战布告"), "1st: {}", t.dump_prompt());
    t.decline();
    // The 1000 collection runs through the money pipeline, which opens its own
    // [反击] window on each payment (宣战布告: 「当你或你拥有的格子被其他玩家的
    // 卡效果影响时」 matches the payer). Skip those before the next play.
    decline_all(&mut t);
    t.play(2, "通用:登上武道馆").unwrap();
    assert!(t.counteract_offered("Mor:离心力，不为所动"), "2nd: {}", t.dump_prompt());
    t.counteract(0, "Mor:离心力，不为所动").unwrap();
    assert!(t.on_field(0, "Mor:离心力，不为所动"));
}