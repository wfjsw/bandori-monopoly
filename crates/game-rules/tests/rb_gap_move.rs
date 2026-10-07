//! Coverage-gap cases G08–G17 (roll / path / settlement),
//! `docs/rulebook/CROSS-TESTS.md` §6 G2–G4. Black-box: expectations from the
//! sheet extracts (`target/scratch/rb/*.md`) and `data/rules.txt`.

mod common;

use common::*;
use game_core::engine::CardRules;
use game_core::msg::Msg;
use game_core::net::NetMessage;

/// Use `who`'s `skill` with an explicit `value` (e.g. X for a fire-count skill).
fn skill_with(t: &mut Table, who: usize, skill: &str, x: i32) -> Result<(), String> {
    let r = t
        .m
        .act(
            who as i32 + 1,
            &NetMessage {
                card: skill.into(),
                value: x,
                ..NetMessage::act("skill")
            },
        )
        .map_err(|e| e.key().to_string());
    t.settle();
    r
}

/// Place `card` on `who`'s field at board `tile` (the arrangement for cards the
/// sheet drops on a square).
fn place_on_tile(t: &mut Table, who: usize, card: &str, tile: usize) {
    let d = data();
    let props = rules().card_props(card);
    t.m.world_mut().place_card_on(
        &d,
        who as i32,
        tile as i32,
        card,
        Msg::default(),
        props,
    );
}

/// Inert filler for draw piles / hands (never auto-plays).
const FILL: &str = "R:[衍生] 觉悟";

fn set_draw_n(t: &mut Table, who: usize, n: usize) {
    let cards: Vec<&str> = vec![FILL; n];
    t.set_draw(who, &cards);
}

fn give_n(t: &mut Table, who: usize, card: &str, n: usize) {
    let cards: Vec<&str> = vec![card; n];
    t.give(who, &cards);
}

/// Decline every open prompt.
fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// End `who`'s turn without a main move.
fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

/// Advance until it is `who`'s turn.
fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..20 {
        if t.turn() == who {
            return;
        }
        let cur = t.turn();
        pass(t, cur);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

/// Answer a `player`-choice prompt with the option naming `target`.
fn answer_player(t: &mut Table, who: usize, target: i32) {
    use game_core::msg::Arg;
    let p = t.expect_prompt();
    let k = p
        .options
        .iter()
        .position(|o| match o.a.get("who") {
            Some(Arg::PlayerId(v)) => *v == target,
            Some(Arg::I(v)) | Some(Arg::N(v)) => *v == target as i64,
            _ => false,
        })
        .unwrap_or_else(|| panic!("player {target} not offered: {}", t.dump_prompt()));
    t.answer(who, k as i32).unwrap();
}

/// Drain, declaring `card` as `who` whenever it is offered.
fn drain_playing(t: &mut Table, who: usize, card: &str) {
    for _ in 0..30 {
        if t.prompt().is_none() {
            return;
        }
        if t.counteract_offered(card) {
            t.counteract(who, card).unwrap();
        } else {
            t.decline();
        }
    }
}

// =====================================================================
// G08. dice-set x dice-reroll: 星月夜's re-roll on a 3d20 set
// =====================================================================

// 规则书 (CRUSH ON THE DRUM): 「本回合主要移动掷骰额外添加Xd20，X为你弃牌堆的
// 卡数」.
// 规则书 (星月夜): 「此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次」.
// RULING: is 「放弃第一次的结果重骰一次」 a re-roll of the full set, or of
// only the base die? Assert the full set.
#[ignore = "RULING: does 星月夜's re-roll cover the whole dice set or only the base die? assert full set"]
#[test]
fn g08_starlit_reroll_of_a_whole_3d20_set() {
    let mut t = Table::new(&["佐藤益木", "仓田真白"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // 星月夜 on the field (with crystals so it stays).
    t.place_raw(0, "Mor:蝴蝶飞舞的星月夜");
    t.set_crystals(0, "Mor:蝴蝶飞舞的星月夜", 3);
    // CRUSH adds Xd20, X = discard size = 2.
    t.set_discard(0, &[FILL, FILL]);
    t.give(0, &["RAS:（MASKING）CRUSH ON THE DRUM!!!"]);
    t.play(0, "RAS:（MASKING）CRUSH ON THE DRUM!!!").unwrap();
    drain(&mut t);
    // Main roll: 3d20 + 2d20 = 5 dice. First sum = 10+1+1+1+1 = 14. The re-roll
    // (if offered) replaces the whole set with 2+2+2+2+2 = 10.
    t.dice(&[10, 1, 1, 1, 1, 2, 2, 2, 2, 2]);
    t.roll(0).unwrap();
    drain_playing(&mut t, 0, "Mor:蝴蝶飞舞的星月夜");
    assert_eq!(t.dice_left(), 0, "every loaded face was rolled: {}", t.dice_left());
    // The walk is the second sum (10) from CiRCLE (index 0) -> index 10.
    assert_eq!(
        t.pos(0),
        10,
        "the re-roll replaced the whole set: pos = {}, events {:?}",
        t.pos(0),
        t.recent_keys(8)
    );
}

// =====================================================================
// G09. dice-reroll x dice-fix: 白金燐子's fixed X*6 plus a re-roll
// =====================================================================

// 规则书 (白金燐子 (2)): 「使这回合移动掷骰的结果固定为X*6。本次移动不受异常
// 移动效果影响」.
// 规则书 (星月夜): 「可以放弃第一次的结果重骰一次」.
// RULING: which wins when both are offered? Assert no re-roll. The engine
// already offers none over a fixed result, so this is a live pass.
#[test]
fn g09_fixed_roll_plus_starlit_reroll() {
    let mut t = Table::new(&["白金燐子", "仓田真白"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, "Mor:蝴蝶飞舞的星月夜");
    t.set_crystals(0, "Mor:蝴蝶飞舞的星月夜", 3);
    t.set_fire(0, 3, 3);
    // Fix the move at X*6 = 3*6 = 18.
    let skill = t.skill_id(0, "即使1cm");
    skill_with(&mut t, 0, &skill, 3).unwrap();
    // The skill may ask for X; answer 3 (the highest option).
    for _ in 0..6 {
        let Some(p) = t.prompt() else { break };
        if p.kind == "choice" {
            let k = (p.options.len() as i32 - 1).max(0);
            let _ = t.answer(0, k);
        } else {
            break;
        }
    }
    drain(&mut t);
    // Whatever the roll prompt asks, the result is fixed: queue plenty of faces.
    t.dice(&[1, 1, 1, 1, 1]);
    t.roll(0).unwrap();
    // No re-roll prompt: the fix leaves nothing to re-roll.
    assert!(
        !t.counteract_offered("Mor:蝴蝶飞舞的星月夜"),
        "no re-roll over a fixed result: {}",
        t.dump_prompt()
    );
    drain(&mut t);
    assert_eq!(t.fire(0), 0, "3 fire spent");
    assert_eq!(t.pos(0), 18, "the move is the fixed 18");
}

// =====================================================================
// G10. dice-set x dice-result: Eve's +Yd4 on a 2d20 move
// =====================================================================

// 规则书 (仓田真白 (1)): 「[主动移动]时移动掷骰变为2d20」.
// 规则书 (若宫伊芙 (1)): 「所有非Pastel✽Palettes玩家获得若宫伊芙的（2）技能」.
// 规则书 (若宫伊芙 (2)): 「将自己Y个正面[P✽P粉丝]变反并为此次投掷结果增加Yd4」.
// RULING: does a dice-set rewrite (2d20) happen before or after the additive
// modifier? The engine already yields 2d20 + 2d4 = 12 backwards, so this is a
// live pass.
#[test]
fn g10_eve_plus_yd4_on_a_2d20_move() {
    let mut t = Table::new(&["仓田真白", "若宫伊芙"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // Eve's (1) hands every non-PP player Eve's (2). P0 (Mashiro, non-PP)
    // should therefore hold it.
    let eve2 = t.skills(0).into_iter().find(|s| s.contains("天下统一"));
    assert!(
        eve2.is_some(),
        "P0 holds Eve's (2) via Eve's (1): {:?}",
        t.skills(0)
    );
    // Flip 2 fans for +2d4, then the 2d20 move goes backwards. Fans use the
    // `P✽P粉丝(正)` counter (clean() wipes them).
    t.m.world_mut().st.players[0]
        .tokens
        .push(game_core::state::Counter {
            name: "P✽P粉丝(正)".into(),
            value: 2,
        });
    t.dice(&[5, 5, 1, 1]);
    t.roll(0).unwrap();
    // Eve's (2) is a pre-roll choice; the roll may raise the window.
    for _ in 0..8 {
        if t.prompt().is_none() {
            break;
        }
        if t.prompt().unwrap().kind == "choice" {
            let _ = t.answer(0, 2);
        } else {
            t.decline();
        }
    }
    drain(&mut t);
    // 2d20 (5+5) + 2d4 (1+1) = 12, backwards from CiRCLE -> index 48.
    assert_eq!(
        t.pos(0),
        48,
        "2d20 + 2d4 backwards: pos = {}, keys {:?}",
        t.pos(0),
        t.recent_keys(8)
    );
}

// =====================================================================
// G11. forced-stop x end-rewrite: a forced stop on a path that LOCK rewrites
// =====================================================================

// 规则书 (LOCK (2)): 「主要移动[经过]了“Bandori车站”则在触发结算前将行动终点
// 改为“旭汤澡堂”」.
// 规则书 (乐奈）有趣的女人): 「当奇迹水晶总数为5或以上时使下一个经过的你以外的
// 玩家选择…强制停下并[触发结算]」.
// 规则书 (86): pass hooks fire in tile order; the first forced stop on the path
// wins and the path is recomputed only when an effect changes the 终点.
// RULING: if LOCK's rewrite (SettleBefore) is processed first, does the stop at
// 33 still count? Assert stop-at-33.
#[ignore = "RULING: does the per-tile forced stop at 33 beat LOCK's SettleBefore end-rewrite? assert stop-at-33"]
#[test]
fn g11_forced_stop_beats_lock_end_rewrite() {
    let mut t = Table::new(&["朝日六花", "要乐奈", "仓田真白"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // P0's LOCK rides along on its field (`clean()` drops non-skill field
    // cards, so re-place it).
    t.place_raw(0, "RAS:（LOCK）追逐梦想的步伐");
    assert!(
        t.field_ids(0).iter().any(|c| c.contains("LOCK")),
        "LOCK is on P0's field: {:?}",
        t.field_ids(0)
    );
    // 乐奈's card sits on the tile just after the start, with 5 crystals.
    let stop_at = 33;
    place_on_tile(&mut t, 1, "MyGO:（乐奈）有趣的女人", stop_at);
    t.set_crystals(1, "MyGO:（乐奈）有趣的女人", 5);
    // Walk 30 -> 40, passing 33 (forced stop) and 35 (LOCK's Bandori车站).
    t.set_pos(0, 30);
    t.dice(&[10]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(0),
        stop_at,
        "the first forced stop on the path wins: pos = {}",
        t.pos(0)
    );
    // LOCK never rewrote the end: the card is still on the field.
    assert!(
        t.field_ids(0).iter().any(|c| c.contains("LOCK")),
        "LOCK is untouched (35 was never reached): {:?}",
        t.field_ids(0)
    );
}

// =====================================================================
// G12. forced-stop x reverse: a stop while the move is reversed
// =====================================================================

// 规则书 (美竹兰 (2)): 「投掷移动步数前可使用一个[火罐]选择本次移动向后」.
// 规则书 (（香澄）大家我都喜欢哦): 「其他玩家[经过]且[移动终点]不为此卡所在
// 格子时那名玩家在此卡所在格子[强制停下]」.
// 规则书 (42): the [路径] runs from start to end in the direction of travel.
// RULING: is a backward walk's [路径] defined the same way? The engine
// already walks the backward direction's tiles, so this is a live pass.
#[test]
fn g12_forced_stop_on_a_backward_walk() {
    let mut t = Table::new(&["美竹兰", "户山香澄", "仓田真白"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // 香澄's card on 星之鼓动山丘 (index 44).
    let hill = tile("星之鼓动山丘");
    assert_eq!(hill, 44);
    place_on_tile(&mut t, 1, "PPP:（香澄）大家我都喜欢哦", hill);
    t.set_fire(0, 1, 1);
    // P0 walks backwards from 50 past 44 toward 40.
    t.set_pos(0, 50);
    let skill = t.skill_id(0, "叛逆的红挑染");
    t.skill(0, &skill).unwrap();
    drain(&mut t);
    t.dice(&[6]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(0),
        hill,
        "the hill force-stop fires on the backward path: pos = {}",
        t.pos(0)
    );
}

// =====================================================================
// G13. end-rewrite x reverse: 普通与理所当然 after a reversed move
// =====================================================================

// 规则书 (普通与理所当然): 「使你下一次主要移动的格数变为移动你最近一次非传送
// 的主要移动的移动格数」.
// 规则书 (羽泽鸫 (2)): 「你可以失去一个[火罐]使本次移动反向」.
// RULING: does 「移动格数」 include the sign? Assert unsigned (pos += 8).
#[ignore = "RULING: does 普通与理所当然 copy a signed or unsigned 移动格数? assert unsigned"]
#[test]
fn g13_ordinary_copies_a_reversed_move_distance() {
    let mut t = Table::new(&["羽泽鸫", "仓田真白"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 1, 1);
    // P0's main move is reversed: 8 backward from 20 -> 12.
    t.set_pos(0, 20);
    let skill = t.skill_id(0, "伟大的平凡");
    if let Err(e) = t.skill(0, &skill) {
        eprintln!("g13: skill rejected: {e}");
    }
    // Accept the "reverse?" choice (first option = yes).
    for _ in 0..6 {
        if t.prompt().is_none() {
            break;
        }
        eprintln!("g13 prompt: {}", t.dump_prompt());
        let _ = t.answer(0, 0);
    }
    drain(&mut t);
    t.dice(&[8]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 12, "8 backward from 20: pos = {}", t.pos(0));
    // P1 force-moves P0 (an abnormal move), P0 answers with 普通与理所当然.
    t.give(0, &["MyGO:普通与理所当然"]);
    t.give(1, &["Mujica:祥，移动"]);
    until_turn(&mut t, 1);
    t.play(1, "Mujica:祥，移动").unwrap();
    if t.prompt().is_some() {
        // Designate P0.
        if t.asked().contains(&1) {
            let _ = answer_player;
            answer_player(&mut t, 1, 0);
        } else {
            drain(&mut t);
        }
    }
    drain_playing(&mut t, 0, "MyGO:普通与理所当然");
    drain(&mut t);
    // P0's next main move is the copied distance 8, forward.
    until_turn(&mut t, 0);
    t.dice(&[15]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(0),
        12 + 8,
        "the copied distance 8 is unsigned: pos = {}",
        t.pos(0)
    );
}

// =====================================================================
// G14. reverse x teleport: 仓田真白's 2d20 backwards into a teleport start
// =====================================================================

// 规则书 (商店街的青梅竹马): 「投掷1d6并[传送]到商店街自己拥有的对应的格子…
// 视为你的主要移动」.
// RULING: can a 「视为主要移动」 teleport replace an already-started walk?
// Assert replacement. The engine already replaces it, so this is a live pass.
#[test]
fn g14_amekomi_replaces_a_started_backward_walk() {
    let mut t = Table::new(&["仓田真白", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // P0 owns a 商店街-colour tile so the 1d6 has a target. The colour group
    // of 商店街 (agent, engine index 46) is group 10: 羽泽咖啡厅 etc.
    let cafe = tile("羽泽咖啡厅");
    t.set_owner(cafe, Some(0));
    t.give(0, &["AG:商店街的青梅竹马"]);
    t.set_pos(0, 30);
    // The card rolls 1d6 (loaded 1 -> the first 商店街 tile P0 owns). The
    // backward 2d20 is discarded: only the 1d6 is rolled.
    t.dice(&[1]);
    t.play(0, "AG:商店街的青梅竹马").unwrap();
    drain(&mut t);
    // The teleport replaces the walk.
    assert_eq!(t.pos(0), cafe, "the teleport is the main move: pos = {}", t.pos(0));
    assert_eq!(t.dice_left(), 0, "every loaded face was rolled");
}

// =====================================================================
// G15. replace-settle x extra-settle: Parking Space's stay-settle + 轮符雨
// =====================================================================

// 规则书 (Parking Space (1)): 「此卡所在格子的[结算]改为回合结束后获得一层[停留]」.
// 规则书 (轮符雨): 「使自己获得一层[停留]并在回合结束时额外进行一次[触发结算]」.
// RULING: does 「额外进行一次[触发结算]」 re-run a *replaced* settle? Assert
// 2 stays. The engine already grants both stays, so this is a live pass.
#[test]
fn g15_parking_space_settle_replaced_plus_rain_extra_settle() {
    let mut t = Table::new(&["都筑诗船", "仓田真白"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    let space = tile("Space");
    assert_eq!(space, 42);
    // Parking Space already on Space.
    place_on_tile(&mut t, 0, "通用:[都筑诗船]Parking Space", space);
    t.give(0, &["MyGO:轮符雨"]);
    t.play(0, "MyGO:轮符雨").unwrap();
    drain(&mut t);
    assert_eq!(t.state(0, "stay"), 1, "轮符雨 grants a stay");
    // End the main move on Space: the settle is replaced by turn-end +1 stay.
    t.set_pos(0, space - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), space);
    // End the turn: the replaced settle's stay + the extra settle.
    pass(&mut t, 0);
    assert_eq!(
        t.state(0, "stay"),
        2,
        "one stay from the replaced settle, one from 轮符雨: {}",
        t.state(0, "stay")
    );
}

// =====================================================================
// G16. skip-settle x remote-settle: 人偶的箱庭 into 练习室里的风暴
// =====================================================================

// 规则书 (人偶的箱庭): 「进行一次移动掷骰并移动对应步数（不[触发结算]）」.
// 规则书 (练习室里的风暴 (2)): 「…在距此卡所在格子X个格子处[结算]时…」.
// RULING: does a forced remote settle count as a settle for storm's own trigger?
// Assert no (a no-settle move does not trip it). The engine already leaves the
// storm in place, so this is a live pass.
#[test]
fn g16_no_settle_move_does_not_trip_the_storm() {
    let mut t = Table::new(&["丰川祥子", "仓田真白", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // The storm sits on P1's livehouse tile with 2 crystals.
    let live = tile("Live House Galaxy");
    t.set_owner(live, Some(1));
    place_on_tile(&mut t, 1, "RAS:练习室里的风暴", live);
    t.set_crystals(1, "RAS:练习室里的风暴", 2);
    t.give(0, &["Mujica:人偶的箱庭"]);
    t.dice(&[3]);
    t.play(0, "Mujica:人偶的箱庭").unwrap();
    // P1 and P2 choose to move (no settle); queue their rolls.
    drain(&mut t);
    // The storm's card is still in place: a no-settle move did not trip it.
    assert!(
        t.field_ids(1).iter().any(|c| c.contains("练习室里的风暴")),
        "the storm is not consumed by a no-settle move: {:?}",
        t.field_ids(1)
    );
}

// =====================================================================
// G17. remote-settle x extra-settle: 立希's quarter-rent plus 轮符雨
// =====================================================================

// 规则书 (（立希）想认真去做): 「本次结算导致的所有[支付]变为原价的四分之一」.
// 规则书 (轮符雨): 「回合结束时额外进行一次[触发结算]」.
// Expect: the extra settle is a normal settle (full rent).
#[test]
fn g17_quarter_rent_remote_settle_then_full_extra_settle() {
    let mut t = Table::new(&["椎名立希", "仓田真白", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    let tile = tile("弦卷豪宅");
    let rent = data().tiles[tile].rent[0];
    t.set_owner(tile, Some(0));
    // P1 stands on P0's tile with a stay.
    t.set_pos(1, tile);
    t.set_state(1, "stay", 1);
    // P0 forces P1 to settle P0's tile at quarter rent (立希's 「本次结算」).
    t.give(0, &["MyGO:（立希）想认真去做"]);
    t.play(0, "MyGO:（立希）想认真去做").unwrap();
    for _ in 0..10 {
        if t.prompt().is_none() {
            break;
        }
        if t.asked().contains(&0) {
            if t.prompt().unwrap().kind == "tile" {
                let _ = t.answer_tile(0, tile);
            } else {
                t.decline();
            }
        } else {
            t.decline();
        }
    }
    drain(&mut t);
    let quarter = rent / 4;
    assert_eq!(
        t.money(1),
        10_000 - quarter,
        "the remote settle pays R/4 = {quarter} (R = {rent})"
    );
    // On P1's turn, 轮符雨 adds a stay and a turn-end extra settle. P1 cannot
    // move (stay), so the extra settle is of P0's tile at the FULL rent --
    // 立希's 「本次结算」 quarter modifier does not carry over.
    until_turn(&mut t, 1);
    t.give(1, &["MyGO:轮符雨"]);
    t.play(1, "MyGO:轮符雨").unwrap();
    drain(&mut t);
    assert_eq!(t.state(1, "stay"), 2, "轮符雨 adds a stay");
    pass(&mut t, 1);
    assert_eq!(
        t.money(1),
        10_000 - quarter - rent,
        "the extra settle pays the full rent {rent}"
    );
}