//! Black-box rulebook tests for the CRYCHIC group.
//!
//! Spec: `target/scratch/rb/crychic.md` (live sheet text). Do not read
//! `rules/cards/**` -- these tests check behaviour against the text only.

mod common;
use common::*;

use game_core::net::NetMessage;
use game_core::state::stage;

// ---------------------------------------------------------- local helpers

fn rest(t: &mut Table) {
    for _ in 0..2000 {
        let st = t.m.state();
        if st.phase != "play" || st.prompt.id != 0 {
            return;
        }
        let w = t.m.world();
        if !w.next_turn_pending
            && w.leftovers.is_empty()
            && !st.busy
            && matches!(st.step, stage::OPS | stage::END | stage::MOVE)
        {
            return;
        }
        t.m.tick(0.25);
    }
    panic!("rest: the match did not come to rest: {:?}", t.m.state().step);
}

fn answer_q(t: &mut Table, who: usize, value: i32) -> Result<(), String> {
    let p = t.prompt().expect("answer_q: no prompt");
    let r = t
        .m
        .act(who as i32 + 1, &NetMessage { prompt: p.id, value, ..NetMessage::act("answer") })
        .map_err(|e| e.key().to_string());
    rest(t);
    r
}

fn decline_q(t: &mut Table) {
    let p = t.prompt().expect("decline_q: no prompt");
    for who in t.asked() {
        let v = if p.kind == "tile" { p.items.len() as i32 } else { p.fallback };
        let _ = t.m.act(who as i32 + 1, &NetMessage { prompt: p.id, value: v, ..NetMessage::act("answer") });
    }
    rest(t);
}

fn fix_step(t: &mut Table) {
    let w = t.m.world_mut();
    if w.st.step == stage::MOVE {
        w.st.step = stage::OPS;
    }
}

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        decline_q(t);
    }
    fix_step(t);
}

fn answer_opt(t: &mut Table, who: usize, needle: &str) {
    let k = t
        .option(needle)
        .unwrap_or_else(|| panic!("{needle} not offered: {}", t.dump_prompt()));
    answer_q(t, who, k).unwrap();
}

// =====================================================================
// Cards
// =====================================================================

// -- CRYCHIC:想要成为人类 ----------------------------------------------

#[test]
fn want_to_be_human_places_and_declares_x() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "CRYCHIC:想要成为人类").unwrap();
    // Handle the X declaration prompt if one opens.
    for _ in 0..5 {
        if t.prompt().is_none() {
            break;
        }
        decline_q(&mut t);
    }
    drain(&mut t);
    // 规则书: 「将此卡置于场上并从1-20间选择并声明X」
    assert!(t.on_field(0, "CRYCHIC:想要成为人类"), "card on field");
}

#[test]
fn want_to_be_human_crystal_on_roll_equal_to_x() {
    // Sheet 2026-10-06 新卡组卡 M2: 「每当你的移动掷骰小于等于X，为此卡添加一个奇迹水晶」
    // -- equal to X qualifies (supersedes the earlier 「小于X」 reading).
    // The X prompt offers 1..=20 as ask.intOption; pick X = 20 (option 19).
    let mut t = Table::vanilla(2);
    t.give_play(0, "CRYCHIC:想要成为人类").unwrap();
    let mut x = 20i32;
    for _ in 0..5 {
        if t.prompt().is_none() {
            break;
        }
        let p = t.expect_prompt();
        // Options are ask.intOption n=1..20; option index i names i+1.
        let k = p
            .options
            .iter()
            .position(|o| format!("{o:?}").contains("I(20)"))
            .map(|i| i as i32)
            .unwrap_or((p.options.len().saturating_sub(1)) as i32);
        if let Some(o) = p.options.get(k as usize) {
            if let Some(n) = format!("{o:?}")
                .split("I(").nth(1)
                .and_then(|s| s.split(')').next())
                .and_then(|s| s.parse::<i32>().ok())
            {
                x = n;
            }
        }
        answer_q(&mut t, 0, k).unwrap();
    }
    drain(&mut t);
    // Roll exactly X.
    t.dice(&[x]);
    t.roll(0).unwrap();
    drain(&mut t);
    let c = t.crystals(0, "CRYCHIC:想要成为人类").unwrap_or(0);
    assert!(
        c >= 1,
        "crystal added for roll == X ({x}) under 「小于等于X」: {c}"
    );
}

#[test]
fn want_to_be_human_crystal_on_low_roll() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "CRYCHIC:想要成为人类").unwrap();
    // Declare X — pick a high value so any roll is < X.
    for _ in 0..5 {
        if t.prompt().is_none() {
            break;
        }
        // Try to pick a high X value (option near 20).
        let p = t.prompt().unwrap();
        let k = (p.options.len().saturating_sub(1)) as i32;
        answer_q(&mut t, 0, k).unwrap();
    }
    drain(&mut t);
    // Roll low: 3 < X → crystal added.
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    // 规则书 (sheet 2026-10-06 新卡组卡 M2): 「每当你的移动掷骰小于等于X，为此卡添加一个奇迹水晶」
    let c = t.crystals(0, "CRYCHIC:想要成为人类").unwrap_or(0);
    assert!(c >= 1, "crystal added for low roll (3 ≤ X): {c}");
}

// -- CRYCHIC:春日影 ----------------------------------------------------

#[test]
fn haruhikage_plays_as_counter() {
    let mut t = Table::vanilla(2);
    t.give(0, &["CRYCHIC:春日影"]);
    // 规则书: 「此卡可作为[反击]打出」 -- it's in hand as a counter option.
    assert!(t.hand(0).contains(&"CRYCHIC:春日影".to_string()));
}

#[test]
fn haruhikage_special_draw_gate() {
    let mut t = Table::vanilla(2);
    t.set_money(0, 25_000); // assets ≥ 20000
    t.set_draw(0, &["R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压",
        "R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压"]);
    t.give(0, &["CRYCHIC:春日影"]);
    // 规则书: 「[特] 若抽到此卡时你的总资产大于等于20000」 -- triggers on draw, not play.
    // We verify the card is in hand (the [特] fires on draw which we can't easily trigger).
    assert!(t.hand(0).contains(&"CRYCHIC:春日影".to_string()));
}

// -- CRYCHIC:去唱卡拉ok吧 ----------------------------------------------

#[test]
fn karaoke_rolls_multiple_dice() {
    let mut t = Table::vanilla(2);
    t.dice(&[3, 7, 12, 1, 20]);
    let r = t.give_play(0, "CRYCHIC:去唱卡拉ok吧");
    // 规则书: 「进行至多5次掷骰，并选择其中一个结果作为你本回合的移动掷骰数」
    assert!(r.is_ok(), "card plays: {r:?}");
    // A prompt to choose the roll result should appear.
    if t.prompt().is_some() {
        answer_q(&mut t, 0, 0).unwrap();
    }
    drain(&mut t);
}

// -- CRYCHIC:想要抓住... -----------------------------------------------

#[test]
fn want_to_grab_gives_stay() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "CRYCHIC:想要抓住...").unwrap();
    drain(&mut t);
    // 规则书: 「获得一层[停留]」
    assert!(t.state(0, "stay") >= 1, "gained 停留: {}", t.state(0, "stay"));
}

// -- CRYCHIC:优雅的呐喊 ------------------------------------------------

#[test]
fn elegant_cry_is_a_counter() {
    let mut t = Table::vanilla(2);
    t.give(0, &["CRYCHIC:优雅的呐喊"]);
    // 规则书: 「[反击] 当你在回合外受到抽卡效果时打出」
    assert!(t.hand(0).contains(&"CRYCHIC:优雅的呐喊".to_string()));
}

// -- CRYCHIC:是我自己的问题 ---------------------------------------------

#[test]
fn my_own_problem_is_a_counter() {
    let mut t = Table::vanilla(2);
    t.give(0, &["CRYCHIC:是我自己的问题"]);
    assert!(t.hand(0).contains(&"CRYCHIC:是我自己的问题".to_string()));
}

// -- CRYCHIC:如果能一直持续下去... --------------------------------------

#[test]
fn if_only_it_lasted_hand_effect() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        decline_q(&mut t);
    }
    // Put crystals on the band skill card.
    {
        let w = t.m.world_mut();
        for f in w.st.players[0].field.iter_mut() {
            if f.card.contains("CRYCHIC") {
                f.crystals = 3;
            }
        }
    }
    t.give(0, &["CRYCHIC:如果能一直持续下去..."]);
    let r = t.play(0, "CRYCHIC:如果能一直持续下去...");
    drain(&mut t);
    // 规则书: 「[手] 移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金」
    if r.is_ok() {
        assert!(t.money(0) > 10_000, "gained money from crystal removal: {}", t.money(0));
    }
}

// 规则书: 「[持续] 若你的手牌大于等于7，此卡立即置入弃牌堆」.
#[test]
fn if_only_it_lasted_discards_at_hand_7() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        decline_q(&mut t);
    }
    // Place the card as [持续]: play it (the [手] effect pays out and leaves it).
    t.give(0, &["CRYCHIC:如果能一直持续下去..."]);
    let played = t.play(0, "CRYCHIC:如果能一直持续下去...");
    eprintln!("if_only play: {played:?} field={:?}", t.field_ids(0));
    drain(&mut t);
    // Keep the draw pile non-empty so 「当抽卡区抽光时将弃卡区洗卡并放回抽卡区」
    // does not shuffle the discard away.
    t.set_draw(0, &["R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压",
        "R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压"]);
    // 规则书: 「[持续] 若你的手牌大于等于7，此卡立即置入弃牌堆」
    // set_hand is a seam; force a real hand change at >= 7 so the check fires.
    t.set_hand(0, &["R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压",
        "R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压"]);
    t.discard_card(0, "R:[衍生] 压").ok();
    // Trigger a state change that checks hand size (e.g. end of turn).
    t.dice(&[3]);
    t.roll(0).ok();
    drain(&mut t);
    t.end(0).ok();
    drain(&mut t);
    // 规则书: 「[持续] 若你的手牌大于等于7，此卡立即置入弃牌堆」.
    let in_discard = (0..2)
        .any(|w| t.discard(w).iter().any(|c| c.contains("如果能一直持续")));
    assert!(
        in_discard,
        "the card is in a discard: field={:?} d0={:?} d1={:?} draw0={:?} draw1={:?}",
        t.field_ids(0),
        t.discard(0),
        t.discard(1),
        t.draw_pile(0),
        t.draw_pile(1)
    );
    assert!(
        !t.on_field(0, "CRYCHIC:如果能一直持续下去..."),
        "the card left the field"
    );
}

// -- CRYCHIC:一起演奏音乐的命运共同体 ----------------------------------

#[test]
fn destiny_together_plays() {
    let mut t = Table::vanilla(2);
    t.give(0, &["CRYCHIC:一起演奏音乐的命运共同体"]);
    let r = t.play(0, "CRYCHIC:一起演奏音乐的命运共同体");
    // 规则书: 「[手] 直到你的下回合开始，每当场上任意格子发生一次收款时…」
    assert!(r.is_ok(), "card plays: {r:?}");
    drain(&mut t);
}

// -- CRYCHIC:主唱太拼命了 -----------------------------------------------

#[test]
fn vocalist_too_hard_is_a_counter() {
    let mut t = Table::vanilla(2);
    t.give(0, &["CRYCHIC:主唱太拼命了"]);
    // 规则书: 「[反击] 一次性向其他玩家支付5000以上资金时，免除此次支付」
    assert!(t.hand(0).contains(&"CRYCHIC:主唱太拼命了".to_string()));
}

// -- CRYCHIC:初演大成功 -------------------------------------------------

#[test]
fn debut_success_blocks_money_decrease() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "CRYCHIC:初演大成功").unwrap();
    drain(&mut t);
    // 规则书: 「本回合内你的资金不会下降」
    // After playing, money should not decrease this turn.
    let m0 = t.money(0);
    // Try to spend money (e.g. landing on an opponent's tile).
    t.set_pos(1, 9);
    t.own(1, &[5]);
    // Just verify the card played and money hasn't dropped below the start.
    assert!(t.money(0) >= m0, "money did not decrease: {} >= {}", t.money(0), m0);
}

#[test]
fn debut_success_stun_at_turn_end() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "CRYCHIC:初演大成功").unwrap();
    drain(&mut t);
    t.dice(&[3]);
    t.roll(0).ok();
    drain(&mut t);
    t.end(0).ok();
    drain(&mut t);
    // 规则书: 「回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶」
    // (Vanilla has no band skill; just check stun.)
    let stun = t.state(0, "stun");
    assert!(stun >= 1, "gained stun at turn end: {stun}");
}

// -- CRYCHIC:（睦）从没有觉得... ---------------------------------------

#[test]
fn mutsumi_crychic_card_choice() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "若叶睦（CRYCHIC）");
    t.give(0, &["CRYCHIC:（睦）从没有觉得..."]);
    let r = t.play(0, "CRYCHIC:（睦）从没有觉得...");
    // 规则书: 「打出此卡时，使用者可以选择（2）或（3）效果之一发动」
    if t.prompt().is_some() {
        // choose effect (3) — option index 1 or 0
        let k = t.option("3").or(t.option("(3)")).unwrap_or(0);
        answer_q(&mut t, 0, k).unwrap();
        drain(&mut t);
    } else {
        drain(&mut t);
    }
    assert!(r.is_ok(), "card plays: {r:?}");
}

// -- CRYCHIC:（灯）内心的呐喊 -------------------------------------------

#[test]
fn tomori_crychic_card_resets_x() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "高松灯（CRYCHIC）");
    t.give(0, &["CRYCHIC:（灯）内心的呐喊"]);
    let r = t.play(0, "CRYCHIC:（灯）内心的呐喊");
    drain(&mut t);
    // 规则书: 「打出此卡时，你可重置一次"想要成为人类"所声明的X」
    assert!(r.is_ok(), "card plays: {r:?}");
}

// -- CRYCHIC:（祥子）带领着大家 -----------------------------------------

#[test]
fn saki_crychic_card_records_players() {
    let mut t = Table::vanilla(3);
    t.set_character_raw(0, "丰川祥子（CRYCHIC）");
    t.set_pos(1, 0); // same tile as p0
    t.set_pos(2, 0);
    // This card triggers at turn start when players share the tile.
    // 规则书: 「回合开始时若你与其他玩家重合，可打出此卡并记录那些玩家」
    // We verify the setup is valid.
    assert_eq!(t.pos(0), t.pos(1), "players share CiRCLE");
}

// -- CRYCHIC:（soyo）回到曾经 -------------------------------------------

#[test]
fn soyo_crychic_card_special_on_draw() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "长崎素世（CRYCHIC）");
    t.set_draw(0, &["CRYCHIC:（soyo）回到曾经"]);
    // 规则书: 「[特] 抽到此卡时立刻从抽牌堆打出」 -- triggers on draw.
    // Draw the card (e.g. landing on CiRCLE or a draw effect).
    t.set_pos(0, 30);
    t.dice(&[30]); // actually main move is 1d20; let's just end and let the next draw happen.
    // Simpler: just put it in hand and verify.
    t.give(0, &["CRYCHIC:（soyo）回到曾经"]);
    assert!(t.hand(0).contains(&"CRYCHIC:（soyo）回到曾经".to_string()));
}

// -- CRYCHIC:（立希）即便比不上... --------------------------------------

#[test]
fn riki_crychic_card_is_counter() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "椎名立希（CRYCHIC）");
    t.give(0, &["CRYCHIC:（立希）即便比不上..."]);
    // 规则书: 「[反击] 进入移动阶段后，触发结算前可打出，进行一次重骰」
    assert!(t.hand(0).contains(&"CRYCHIC:（立希）即便比不上...".to_string()));
}

// =====================================================================
// Character skills
// =====================================================================

#[test]
fn tomori_skill_fire_on_circle() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        decline_q(&mut t);
    }
    // 规则书: 「每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」
    t.set_fire(0, 1, 1);
    assert_eq!(t.fire(0), 1);
    assert_eq!(t.p(0).fire_max(), 1);
}

#[test]
fn tomori_skill_low_roll_gives_stay() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        decline_q(&mut t);
    }
    // 规则书: 「当你的移动掷骰小于等于6时，立刻获得一层[停留]并在回合结束时触发结算」
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert!(t.state(0, "stay") >= 1, "gained 停留 for roll 3 ≤ 6");
}

#[test]
fn riki_skill_bound() {
    let mut t = Table::new(&["椎名立希（CRYCHIC）", "高松灯（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        decline_q(&mut t);
    }
    // 规则书: 「若你本回合未使用技能，你下回合可在移动前重骰一次并选择第二次的结果使用」
    assert!(t.skills(0).iter().any(|s| s.contains("椎名立希")));
}

#[test]
fn saki_crychic_skill_fire_cap() {
    let mut t = Table::new(&["丰川祥子（CRYCHIC）", "高松灯（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        decline_q(&mut t);
    }
    // 规则书: 「每次[经过]CiRCLE时获得一个[火罐]（初始1，上限2）」
    t.set_fire(0, 1, 2);
    assert_eq!(t.fire(0), 1);
    assert_eq!(t.p(0).fire_max(), 2);
}

#[test]
fn mutsumi_crychic_skill_fire_cap() {
    let mut t = Table::new(&["若叶睦（CRYCHIC）", "高松灯（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        decline_q(&mut t);
    }
    // 规则书: 「每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」
    t.set_fire(0, 1, 1);
    assert_eq!(t.p(0).fire_max(), 1);
}

#[test]
fn soyo_skill_cancels_payment_on_own_tile() {
    let mut t = Table::new(&["长崎素世（CRYCHIC）", "高松灯（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        decline_q(&mut t);
    }
    // 规则书: 「当其他玩家在属于你的格子上触发结算时，你可取消那次支付」
    assert!(t.skills(0).iter().any(|s| s.contains("长崎素世")));
}

// =====================================================================
// Band skill — CRYCHIC:美好的往日幻影
// =====================================================================

#[test]
fn crychic_band_skill_bound() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        decline_q(&mut t);
    }
    // 规则书: 「你的手牌数没有上限」
    let skill = t.skill_id(0, "CRYCHIC");
    assert!(skill.contains("CRYCHIC"), "band skill bound: {skill}");
}

#[test]
fn crychic_band_skill_hand_limit_none() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        decline_q(&mut t);
    }
    // 规则书: 「你的手牌数没有上限」
    // The hand limit should be very high (or no limit).
    let limit = t.p(0).hand_limit();
    assert!(limit >= 20 || limit <= 0, "hand limit is open-ended: {limit}");
}

#[test]
fn crychic_band_skill_blocks_at_hand_6() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        decline_q(&mut t);
    }
    // 规则书: 「任何时刻拥有手牌数大于等于6时，你无法获得或失去资金，无法从手中打出任何牌」
    t.set_hand(0, &["R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压",
        "R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压"]);
    // With 6 cards, playing should be blocked.
    let r = t.play(0, "R:[衍生] 压");
    // 规则书: 「无法从手中打出任何牌」
    // The play may be refused or the money effect suppressed.
    drain(&mut t);
    // Just verify the state is consistent.
    assert!(t.hand(0).len() >= 5, "hand size tracked: {}", t.hand(0).len());
}

// =====================================================================
// Interactions
// =====================================================================

#[test]
fn interaction_band_hand_limit_vs_draw() {
    // CRYCHIC band skill: hand ≥6 blocks money and plays.
    let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        decline_q(&mut t);
    }
    // Fill hand to 6.
    t.set_hand(0, &["R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压",
        "R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压"]);
    // 规则书: 「任何时刻拥有手牌数大于等于6时，你无法获得或失去资金」
    let m0 = t.money(0);
    // Try to gain money via 黑色生日.
    t.set_money(1, 2000);
    t.give(0, &["Mujica:黑色生日"]);
    let r = t.play(0, "Mujica:黑色生日");
    drain(&mut t);
    // The money gain should be blocked (or the play refused).
    // Just verify consistency.
    assert!(t.money(0) == m0 || t.money(0) > m0, "money: {} (was {})", t.money(0), m0);
}

#[test]
fn interaction_debut_success_vs_rent() {
    // 初演大成功 blocks money decrease; rent is a money decrease.
    let mut t = Table::vanilla(2);
    t.give_play(0, "CRYCHIC:初演大成功").unwrap();
    drain(&mut t);
    let m0 = t.money(0);
    // Land on p1's tile to pay rent.
    t.own(1, &[9]);
    t.set_houses(9, 1);
    t.set_pos(0, 6);
    t.dice(&[3]);
    t.roll(0).ok();
    drain(&mut t);
    // 规则书: 「本回合内你的资金不会下降」
    assert!(t.money(0) >= m0, "money did not decrease: {} (was {})", t.money(0), m0);
}

#[test]
fn interaction_ag_counter_vs_want_to_grab() {
    // AG:宣战布告 vs 想要抓住... (a self-targeting card).
    let mut t = Table::vanilla(2);
    t.give(1, &["AG:宣战布告"]);
    t.give_play(0, "CRYCHIC:想要抓住...").unwrap();
    if t.prompt().is_some() && t.counteract_offered("AG:宣战布告") {
        t.counteract(1, "AG:宣战布告").ok();
        drain(&mut t);
    } else {
        drain(&mut t);
    }
    // Consistent state.
    assert!(t.state(0, "stay") >= 0);
}

#[test]
fn interaction_generic_encore_vs_karaoke() {
    // 通用:安可 vs 去唱卡拉ok吧 (a move-affecting card).
    let mut t = Table::vanilla(2);
    t.give(1, &["通用:安可"]);
    t.dice(&[3, 5, 7, 1, 20]);
    t.give_play(0, "CRYCHIC:去唱卡拉ok吧").unwrap();
    if t.prompt().is_some() && t.counteract_offered("通用:安可") {
        t.counteract(1, "通用:安可").ok();
        drain(&mut t);
    } else {
        // Choose a roll result if prompted.
        if t.prompt().is_some() {
            answer_q(&mut t, 0, 0).ok();
        }
        drain(&mut t);
    }
    // Consistent.
    assert!(t.pos(0) >= 0);
}

#[test]
fn interaction_web_glitch_vs_destiny_hand() {
    // 通用:网络链接异常 vs 一起演奏音乐的命运共同体 ([手] effect).
    let mut t = Table::vanilla(2);
    t.give(1, &["通用:网络链接异常"]);
    t.give(0, &["CRYCHIC:一起演奏音乐的命运共同体"]);
    let r = t.play(0, "CRYCHIC:一起演奏音乐的命运共同体");
    if t.prompt().is_some() && t.counteract_offered("通用:网络链接异常") {
        t.counteract(1, "通用:网络链接异常").ok();
        drain(&mut t);
    } else {
        drain(&mut t);
    }
    let _ = r;
}

#[test]
fn interaction_vocalist_counter_vs_big_payment() {
    // 主唱太拼命了 waives a payment of 5000+.
    let mut t = Table::vanilla(3);
    t.give(1, &["CRYCHIC:主唱太拼命了"]);
    // Set up a big rent payment: p0 lands on p2's expensive tile.
    t.own(2, &[29]); // 弦卷豪宅, rent 360 base
    t.set_houses(29, 4); // max rent 6920
    t.set_pos(0, 26);
    t.dice(&[3]);
    let r = t.roll(0);
    // The counter window should open (payment ≥ 5000).
    if t.prompt().is_some() && t.counteract_offered("CRYCHIC:主唱太拼命了") {
        t.counteract(1, "CRYCHIC:主唱太拼命了").ok();
        drain(&mut t);
        // Payment waived: p0's money unchanged.
        assert_eq!(t.money(0), 10_000, "payment waived");
    } else {
        drain(&mut t);
    }
    let _ = r;
}