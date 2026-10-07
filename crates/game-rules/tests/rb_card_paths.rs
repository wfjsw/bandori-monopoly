//! Black-box rulebook tests for four card paths that landed without a test
//! exercising them: `AG:（巴）商店街的救世主` (tomoe_savior),
//! `CRYCHIC:（睦）从没有觉得...` (mutsumi_never), `Mujica:（海铃）` (umiri_card),
//! `RAS:（PAREO）渐渐远去的你` (pareo_far).
//!
//! Spec: the cards sheet (`docs/rulebook/cards-sheet.csv`) plus the skill text
//! in `data/bands.json` / `data/characters.json`. Behaviour is observed through
//! the shared harness only -- no card implementation is read. Disagreements
//! with the rulebook keep the book's assertion and are marked
//! `#[ignore = "DISCREPANCY: ..."]`.

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
        .act(
            who as i32 + 1,
            &NetMessage {
                prompt: p.id,
                value,
                ..NetMessage::act("answer")
            },
        )
        .map_err(|e| e.key().to_string());
    rest(t);
    r
}

fn decline_q(t: &mut Table) {
    let p = t.prompt().expect("decline_q: no prompt");
    for who in t.asked() {
        let v = if p.kind == "tile" {
            p.items.len() as i32
        } else {
            p.fallback
        };
        let _ = t.m.act(
            who as i32 + 1,
            &NetMessage {
                prompt: p.id,
                value: v,
                ..NetMessage::act("answer")
            },
        );
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

/// Answer the first option matching `needle`, or panic with the dump.
fn answer_opt(t: &mut Table, who: usize, needle: &str) {
    let k = t
        .option(needle)
        .unwrap_or_else(|| panic!("{needle} not offered: {}", t.dump_prompt()));
    answer_q(t, who, k).unwrap();
}

/// Answer every prompt that comes up until the match is at rest again, taking
/// the first option whose text matches `take` and declining the rest.
fn drain_taking(t: &mut Table, who: usize, take: &str) {
    for _ in 0..20 {
        let Some(p) = t.prompt() else { return };
        let hit = t.option(take);
        if let Some(k) = hit {
            answer_q(t, who, k).unwrap();
            let _ = p;
            continue;
        }
        decline_q(t);
    }
    drain(t);
}

// =====================================================================
// AG:（巴）商店街的救世主 (tomoe_savior)
// =====================================================================

// 规则书 (cards-sheet, Afterglow 巴):
// 「【反击】当其他玩家抵押商店街地契时，你可以打出此卡，立刻支付常规收购价一半的
// 价格从该玩家处收购该地契。」
//
// 「常规收购价」 is the deed's regular purchase price (规则书 游戏流程 uses
// 「地契购买价格」 for that number: 「抵押后[获得]地契购买价格50%的资金」), so
// half of it is the price/2 the [反击] pays. The deed is a 商店街-group one
// (山吹面包房, group 10).
#[test]
fn tomoe_savior_hands_the_deed_over_at_half_price() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "宇田川巴");
    let shop = tile("山吹面包房"); // 商店街 group (10), price 2000
    t.own(1, &[shop]);
    t.set_money(0, 10_000);
    t.set_money(1, 10_000);
    t.give(0, &["AG:（巴）商店街的救世主"]);
    t.begin_turn(1);
    drain(&mut t);
    t.mortgage(1, shop).unwrap();
    assert!(
        t.counteract_offered("AG:（巴）商店街的救世主"),
        "【反击】 on 「当其他玩家抵押商店街地契时」: {}",
        t.dump_prompt()
    );
    t.counteract(0, "AG:（巴）商店街的救世主").unwrap();
    drain(&mut t);
    // 「从该玩家处收购该地契」
    assert_eq!(
        t.owner(shop),
        Some(0),
        "the deed is handed over: events {:?}",
        t.recent_keys(12)
    );
    // 「支付常规收购价一半的价格」 -- half of 山吹面包房's 2000. The mortgager
    // keeps their own 「抵押后[获得]地契购买价格50%的资金」 payout on top.
    assert_eq!(t.money(0), 10_000 - 1_000, "paid 2000/2 = 1000");
    assert_eq!(
        t.money(1),
        10_000 + 1_000 + 1_000,
        "mortgage 50% + the sale: events {:?}",
        t.recent_keys(12)
    );
}

// After the handover the acquisition is a 「购买」: band:Afterglow:商店街的宠儿
// 「你购买商店街的或价值小于等于1200的格子时自动免费在上面加盖一栋房子」 must hear
// it. The listener runs (`cards:skill-bands.afterglow_free_house` is in the
// event stream) -- 「must hear it」 is the clause under test here.
#[test]
fn tomoe_savior_buy_listener_hears_the_acquisition() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "宇田川巴");
    t.place_raw(0, "skill:Afterglow:商店街的宠儿");
    let shop = tile("山吹面包房"); // 商店街 group (10), price 2000
    t.own(1, &[shop]);
    t.give(0, &["AG:（巴）商店街的救世主"]);
    t.begin_turn(1);
    drain(&mut t);
    let mk = t.mark();
    t.mortgage(1, shop).unwrap();
    t.counteract(0, "AG:（巴）商店街的救世主").unwrap();
    drain(&mut t);
    assert_eq!(t.owner(shop), Some(0), "events {:?}", t.keys_since(mk));
    // 「购买」 listener: 商店街的宠儿 hears the handover.
    let keys = t.keys_since(mk);
    assert!(
        keys.iter().any(|k| k.contains("afterglow_free_house")),
        "the Afterglow 「购买」 listener heard the handover: {keys:?}"
    );
}

// 规则书 band:Afterglow:商店街的宠儿: 「自动免费在上面加盖一栋房子」 -- the free
// house itself, as part of the same handover.
#[test]
fn tomoe_savior_buy_listener_places_the_free_house() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "宇田川巴");
    t.place_raw(0, "skill:Afterglow:商店街的宠儿");
    let shop = tile("山吹面包房"); // 商店街 group (10), price 2000
    t.own(1, &[shop]);
    t.give(0, &["AG:（巴）商店街的救世主"]);
    t.begin_turn(1);
    drain(&mut t);
    t.mortgage(1, shop).unwrap();
    t.counteract(0, "AG:（巴）商店街的救世主").unwrap();
    drain(&mut t);
    assert_eq!(t.owner(shop), Some(0), "events {:?}", t.recent_keys(12));
    // 「购买」 listener: 商店街的宠儿 「自动免费在上面加盖一栋房子」.
    assert_eq!(
        t.houses(shop),
        1,
        "the Afterglow 「购买」 listener's free house: events {:?}",
        t.recent_keys(12)
    );
}

// The same handover is a 「购买」 for every listener: RAS:（chuchu）演奏我的音乐吧
// 「场上存在此卡的玩家下次购买地契时，[使用者]获得100资金」 must pay its user.
#[test]
fn tomoe_savior_buy_listener_pays_the_chuchu_bonus() {
    let mut t = Table::vanilla(3);
    t.set_character_raw(0, "宇田川巴");
    t.set_character_raw(2, "珠手知由");
    let shop = tile("山吹面包房");
    t.own(1, &[shop]);
    t.give(0, &["AG:（巴）商店街的救世主"]);
    // P2 puts the chuchu card on P0's field (「将此卡放置于你以外的一名玩家场上」).
    t.begin_turn(2);
    drain(&mut t);
    t.give_play(2, "RAS:（chuchu）演奏我的音乐吧").unwrap();
    // The placement asks which player; pick P0.
    for _ in 0..6 {
        let Some(p) = t.prompt() else { break };
        if t.option("PlayerId(0)").is_some() {
            answer_opt(&mut t, 2, "PlayerId(0)");
            continue;
        }
        if p.kind == "player" {
            let k = t
                .option("P0")
                .or_else(|| t.option("宇田川巴"))
                .unwrap_or(0);
            answer_q(&mut t, 2, k).unwrap();
            continue;
        }
        decline_q(&mut t);
    }
    drain(&mut t);
    assert!(
        t.on_field(0, "RAS:（chuchu）演奏我的音乐吧"),
        "the chuchu card is on P0: {:?}",
        t.field_ids(0)
    );
    // P1 mortgages; P0 buys the deed back through tomoe_savior.
    t.begin_turn(1);
    drain(&mut t);
    t.mortgage(1, shop).unwrap();
    assert!(
        t.counteract_offered("AG:（巴）商店街的救世主"),
        "{}",
        t.dump_prompt()
    );
    t.counteract(0, "AG:（巴）商店街的救世主").unwrap();
    drain(&mut t);
    assert_eq!(t.owner(shop), Some(0), "events {:?}", t.recent_keys(16));
    // 「场上存在此卡的玩家下次购买地契时，[使用者]获得100资金」
    assert_eq!(
        t.money(2),
        10_000 + 100,
        "the chuchu 「购买」 listener paid its user: events {:?}",
        t.recent_keys(16)
    );
}

// 规则书: same card, the [反击] is only for 「商店街地契」 -- a deed outside the
// 商店街 group must not open the window.
#[test]
fn tomoe_savior_only_offered_on_a_shop_street_deed() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "宇田川巴");
    let land = tile("购物中心"); // group 4, not 商店街
    t.own(1, &[land]);
    t.give(0, &["AG:（巴）商店街的救世主"]);
    t.begin_turn(1);
    drain(&mut t);
    t.mortgage(1, land).unwrap();
    assert!(
        !t.counteract_offered("AG:（巴）商店街的救世主"),
        "not a 商店街地契 -- no window: {}",
        t.dump_prompt()
    );
}

// 规则书: 「当你抵押商店街地契时，你可以打出此卡，额外获得一份抵押收益并将地契翻回」
// -- the self-mortgage branch pays the mortgage value twice and un-mortgages.
#[test]
fn tomoe_savior_on_your_own_mortgage_pays_twice_and_unmortgages() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "宇田川巴");
    let shop = tile("山吹面包房"); // price 2000 -> mortgage 1000
    t.own(0, &[shop]);
    t.set_money(0, 10_000);
    t.give(0, &["AG:（巴）商店街的救世主"]);
    t.begin_turn(0);
    drain(&mut t);
    t.mortgage(0, shop).unwrap();
    assert!(
        t.counteract_offered("AG:（巴）商店街的救世主"),
        "【反击】 on 「当你抵押商店街地契时」: {}",
        t.dump_prompt()
    );
    t.counteract(0, "AG:（巴）商店街的救世主").unwrap();
    drain(&mut t);
    // 「额外获得一份抵押收益并将地契翻回」
    assert!(!t.mortgaged(shop), "the deed is flipped back: events {:?}", t.recent_keys(12));
    assert_eq!(t.money(0), 10_000 + 1_000 + 1_000, "two mortgage payouts (2000/2 each)");
}

// =====================================================================
// CRYCHIC:（睦）从没有觉得... (mutsumi_never)
// =====================================================================

// 规则书 (cards-sheet, CRYCHIC 睦):
// 「（2）消耗乐队技能卡上的3个奇迹水晶（不足3个则改为全部消耗），立即执行乐队技能的
// （2）效果，然后弃一张卡。」
//
// Bound band skill: CRYCHIC:美好的往日幻影, whose （2） is
// 「你的回合结束时，若你的抽牌堆与弃牌堆中都没有卡，移除此卡与你所有区域的所有
// "CRYCHIC"卡，将你剩余的所有手牌放入抽牌堆，并向抽牌堆中加入角色对应的自选"MyGO"
// 或"Ave Mujica"卡至抽牌堆中总共有10张卡并洗切；获得角色对应的"MyGO"或"Ave Mujica"
// 乐队技能卡，然后抽2张卡。」
// 「立即执行…（2）效果」 runs that effect body now (the C# port is
// `BandCrychic.TransformNow()`), so the CRYCHIC band card goes away, a MyGO /
// Ave Mujica band skill arrives and two cards are drawn.
#[test]
fn mutsumi_never_2_runs_the_band_skill_2() {
    let mut t = Table::new(&["若叶睦（CRYCHIC）", "长崎素世（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    let band = t.skill_id(0, "CRYCHIC");
    assert!(band.contains("CRYCHIC"), "bound band skill: {band}");
    // 「消耗乐队技能卡上的3个奇迹水晶」 -- >= 3, so exactly 3 are spent.
    t.set_crystals(0, &band, 5);
    // The (2) body is written for 「回合结束时，若你的抽牌堆与弃牌堆中都没有卡」 --
    // 「立即执行」 takes the effect either way, and the condition holds here too.
    t.set_draw(0, &[]);
    t.set_discard(0, &[]);
    t.set_hand(0, &["CRYCHIC:（睦）从没有觉得..."]);
    let m0 = t.money(0);
    t.play(0, "CRYCHIC:（睦）从没有觉得...").unwrap();
    // 「（1）打出此卡时，使用者可以选择（2）或（3）效果之一发动」 -- pick (2).
    let p = t.expect_prompt();
    let k = t
        .option("opt2")
        .or_else(|| t.option("(2)"))
        .unwrap_or_else(|| panic!("a (2)/(3) choice: {}", t.dump_prompt()));
    assert!(
        p.options.len() >= 2,
        "the choice offers (2) and (3): {}",
        t.dump_prompt()
    );
    answer_q(&mut t, 0, k).unwrap();
    // The transform may ask for the MyGO / Ave Mujica successor; take whatever
    // it offers (the sheet says 「自选」) and decline the rest.
    drain_taking(&mut t, 0, "Mujica");
    drain(&mut t);
    // 「立即执行乐队技能的（2）效果」 -- the CRYCHIC (2) body ran:
    // 「获得角色对应的"MyGO"或"Ave Mujica"乐队技能卡」
    let skills = t.skills(0);
    assert!(
        skills.iter().any(|s| s.contains("MyGO") || s.contains("Mujica")),
        "gained the MyGO / Ave Mujica band skill: {skills:?}"
    );
    // 「向抽牌堆中加入角色对应的自选"MyGO"或"Ave Mujica"卡至抽牌堆中总共有10张卡」
    // then 「然后抽2张卡」, then mutsumi's 「然后弃一张卡」: 10 filled, 2 drawn,
    // 1 discarded -> 8 left in the draw pile, 1 in hand, 1 in the discard.
    assert_eq!(
        t.draw_pile(0).len(),
        8,
        "filled to 10 then drew 2: {:?}",
        t.draw_pile(0)
    );
    assert_eq!(t.hand(0).len(), 1, "drew 2 then discarded 1: {:?}", t.hand(0));
    assert_eq!(t.discard(0).len(), 1, "「然后弃一张卡」: {:?}", t.discard(0));
    assert!(
        t.draw_pile(0).iter().all(|c| c.contains("Mujica") || c.contains("MyGO")),
        "the fill is MyGO / Ave Mujica cards: {:?}",
        t.draw_pile(0)
    );
    assert_eq!(t.money(0), m0, "no money clause in (2)");
}

// 规则书 band:CRYCHIC:美好的往日幻影（2）: 「移除此卡与你所有区域的所有"CRYCHIC"卡」
// -- part of the (2) body mutsumi_never's (2) must run.
#[test]
fn mutsumi_never_2_band_2_removes_the_crychic_cards() {
    let mut t = Table::new(&["若叶睦（CRYCHIC）", "长崎素世（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    let band = t.skill_id(0, "CRYCHIC");
    t.set_crystals(0, &band, 5);
    t.set_draw(0, &[]);
    t.set_discard(0, &[]);
    t.set_hand(0, &["CRYCHIC:（睦）从没有觉得..."]);
    t.play(0, "CRYCHIC:（睦）从没有觉得...").unwrap();
    let k = t
        .option("opt2")
        .or_else(|| t.option("(2)"))
        .unwrap_or_else(|| panic!("a (2)/(3) choice: {}", t.dump_prompt()));
    answer_q(&mut t, 0, k).unwrap();
    drain_taking(&mut t, 0, "Mujica");
    drain(&mut t);
    // 「移除此卡」 -- the band skill card itself.
    assert!(
        !t.on_field(0, &band),
        "the CRYCHIC band card is [移除]ed: {:?}",
        t.field_ids(0)
    );
    // 「与你所有区域的所有"CRYCHIC"卡」
    assert!(
        !t.discard(0).iter().any(|c| c.starts_with("CRYCHIC:"))
            && !t.draw_pile(0).iter().any(|c| c.starts_with("CRYCHIC:"))
            && !t.hand(0).iter().any(|c| c.starts_with("CRYCHIC:")),
        "no CRYCHIC card left in any zone: h={:?} d={:?} dr={:?}",
        t.hand(0),
        t.discard(0),
        t.draw_pile(0)
    );
}

// 规则书: 「不足3个则改为全部消耗」 -- a shortfall must not refuse the (2).
#[test]
fn mutsumi_never_2_runs_the_band_skill_2_on_a_crystal_shortfall() {
    let mut t = Table::new(&["若叶睦（CRYCHIC）", "长崎素世（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    let band = t.skill_id(0, "CRYCHIC");
    t.set_crystals(0, &band, 1); // 「不足3个则改为全部消耗」
    t.set_draw(0, &[]);
    t.set_discard(0, &[]);
    t.set_hand(0, &["CRYCHIC:（睦）从没有觉得..."]);
    t.play(0, "CRYCHIC:（睦）从没有觉得...").unwrap();
    let k = t
        .option("opt2")
        .or_else(|| t.option("(2)"))
        .unwrap_or_else(|| panic!("a (2)/(3) choice: {}", t.dump_prompt()));
    answer_q(&mut t, 0, k).unwrap();
    drain_taking(&mut t, 0, "Mujica");
    drain(&mut t);
    // The (2) body still runs on the shortfall: the successor band skill arrives.
    let skills = t.skills(0);
    assert!(
        skills.iter().any(|s| s.contains("MyGO") || s.contains("Mujica")),
        "「不足3个则改为全部消耗」 -- the (2) ran with 1 crystal: {skills:?}"
    );
    assert_eq!(
        t.draw_pile(0).len(),
        8,
        "filled to 10 then drew 2: {:?}",
        t.draw_pile(0)
    );
}

// =====================================================================
// Mujica:（海铃） (umiri_card)
// =====================================================================

// 规则书 (cards-sheet, Ave Mujica 海铃):
// 「（2）使用者打出此卡时以及使用者的回合开始时，从卡堆拿取场上有此卡的玩家的所有
// 乐队技能卡（相同乐队技能卡的效果不可叠加），但不视为那个乐队的角色。」
//
// The taken copy works for the user: band:Afterglow:商店街的宠儿
// 「你购买商店街的或价值小于等于1200的格子时自动免费在上面加盖一栋房子」 must fire
// on the taker's own buy.
#[test]
fn umiri_take_gives_the_user_a_working_band_skill_copy() {
    // `Table::new` binds the band skills: P0 is Ave Mujica (八幡海铃) and P1
    // is Afterglow (美竹兰), so the Afterglow band skill on P1's field is a
    // real bound one to 「拿取」.
    let mut t = Table::new(&["八幡海铃", "美竹兰", "青叶摩卡"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.give_play(0, "Mujica:（海铃）").unwrap();
    drain(&mut t);
    // 「（1）将此卡放置于在此卡使用者下一名行动的玩家场上」
    assert!(
        t.on_field(1, "Mujica:（海铃）"),
        "the card sits on the next player's field: p1={:?}",
        t.field_ids(1)
    );
    // 「拿取场上有此卡的玩家的所有乐队技能卡」 -- P1 holds the card, so P0 takes
    // P1's Afterglow band skill.
    let ids = t.field_ids(0);
    assert!(
        ids.iter().any(|c| c.contains("Afterglow")),
        "the taken Afterglow band copy is on the user: {ids:?}"
    );
    // The copy works: a cheap (<=1200) buy gets the free house.
    let cheap = tile("富士见坂"); // price 600
    t.set_pos(0, cheap - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.buy(0).unwrap();
    drain(&mut t);
    assert_eq!(t.owner(cheap), Some(0), "events {:?}", t.recent_keys(10));
    assert_eq!(
        t.houses(cheap),
        1,
        "the taken copy's 「自动免费在上面加盖一栋房子」: events {:?}",
        t.recent_keys(10)
    );
}

// 规则书: 「相同乐队技能卡的效果不可叠加」 -- a second copy of the same band skill
// is not attached on top of the first.
#[test]
fn umiri_take_does_not_stack_the_same_band_skill() {
    // P1 and P2 are both Afterglow (美竹兰 / 青叶摩卡), so both hold a bound
    // copy of the same band skill to 「拿取」.
    let mut t = Table::new(&["八幡海铃", "美竹兰", "青叶摩卡"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.give_play(0, "Mujica:（海铃）").unwrap();
    drain(&mut t);
    // On play the card is on P1 -> P0 takes P1's Afterglow.
    assert!(t.on_field(1, "Mujica:（海铃）"));
    // At the user's next turn start the card moves one seat forward to P2 and
    // (2) takes again -- P2 holds the same band skill.
    t.begin_turn(0);
    drain(&mut t);
    assert!(
        t.on_field(2, "Mujica:（海铃）"),
        "the card moved one seat forward: p2={:?}",
        t.field_ids(2)
    );
    let copies: Vec<_> = t.field_ids(0).iter().filter(|c| c.contains("Afterglow")).cloned().collect();
    assert_eq!(
        copies.len(),
        1,
        "「相同乐队技能卡的效果不可叠加」 -- one copy only: {copies:?}"
    );
    // And the stacked pair would have fired twice; one buy gives one house.
    let cheap = tile("富士见坂");
    t.set_pos(0, cheap - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.buy(0).unwrap();
    drain(&mut t);
    assert_eq!(t.houses(cheap), 1, "one free house, not two: {:?}", t.recent_keys(10));
}

// 规则书: 「但不视为那个乐队的角色」 -- the taken band skill does not make the user
// a character of that band. AG:ONE OF US 「选择场上的另一个Afterglow角色或者拥有
// 商店街格子的角色」 must not offer the taker as an Afterglow character.
#[test]
fn umiri_take_does_not_make_the_user_that_bands_character() {
    // P0 is not Afterglow; P1 is (and holds the bound Afterglow band skill).
    let mut t = Table::new(&["八幡海铃", "美竹兰", "青叶摩卡"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // ONE OF US 「指定双方各一块地契」 -- both sides need deeds. P1's is a
    // 商店街 tile (so P1 is eligible twice over); P0 owns none, and in
    // particular no 商店街 tile, so the only way ONE OF US could offer P0 is
    // as 「另一个Afterglow角色」.
    t.own(1, &[tile("山吹面包房")]);
    t.own(2, &[tile("富士见坂")]);
    t.give_play(0, "Mujica:（海铃）").unwrap();
    drain(&mut t);
    assert!(
        t.field_ids(0).iter().any(|c| c.contains("Afterglow")),
        "the Afterglow copy is on P0: {:?}",
        t.field_ids(0)
    );
    // ONE OF US is a [手] card, so play it on P2's own turn.
    t.begin_turn(2);
    drain(&mut t);
    t.give(2, &["AG:ONE OF US"]);
    t.play(2, "AG:ONE OF US").unwrap();
    // The selection prompt must list P1 (a real Afterglow character) and must
    // not list P0.
    let dump = t.dump_prompt();
    assert!(
        t.option("PlayerId(1)").is_some() || t.option("美竹兰").is_some(),
        "P1 is an offered Afterglow character: {dump}"
    );
    assert!(
        t.option("PlayerId(0)").is_none() && t.option("八幡海铃").is_none(),
        "「不视为那个乐队的角色」 -- P0 is not offered as an Afterglow character: {dump}"
    );
}

// 规则书: 「（3）当此卡回到使用者场上时，使用者回合结束时将此卡与使用者拿取的所有
// 乐队技能卡置入弃牌堆，抽一张卡。」 -- the copies go away with the card.
#[test]
fn umiri_copies_go_away_when_the_card_returns() {
    let mut t = Table::new(&["八幡海铃", "美竹兰", "青叶摩卡"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.give_play(0, "Mujica:（海铃）").unwrap();
    drain(&mut t);
    assert!(t.on_field(1, "Mujica:（海铃）"));
    assert!(t.field_ids(0).iter().any(|c| c.contains("Afterglow")));
    // Keep a draw pile so 「抽一张卡」 has something to draw (`clean()` emptied
    // every pile).
    t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!", "通用:FEVER!"]);
    // Two more of the user's turn starts move the card P1 -> P2 -> P0.
    for _ in 0..2 {
        t.begin_turn(0);
        drain(&mut t);
    }
    // 「当此卡回到使用者场上时」
    assert!(
        t.on_field(0, "Mujica:（海铃）"),
        "the card is back on the user's field: {:?}",
        t.field_ids(0)
    );
    // 「使用者回合结束时将此卡与使用者拿取的所有乐队技能卡置入弃牌堆，抽一张卡。」
    let hand0 = t.hand(0).len();
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    assert!(
        !t.on_field(0, "Mujica:（海铃）"),
        "the card left the field: {:?}",
        t.field_ids(0)
    );
    assert!(
        !t.field_ids(0).iter().any(|c| c.contains("Afterglow")),
        "the taken band copy went away with it: {:?}",
        t.field_ids(0)
    );
    // 「抽一张卡」 -- one card was drawn at the turn end (the next turn may
    // have drawn its own, so look at the event stream rather than a bare
    // hand count).
    assert!(
        t.recent_keys(20).iter().any(|k| k.contains("draw")),
        "「抽一张卡」: hand was {hand0} now {} events {:?}",
        t.hand(0).len(),
        t.recent_keys(20)
    );
}

// =====================================================================
// RAS:（PAREO）渐渐远去的你 (pareo_far)
// =====================================================================

// 规则书 (cards-sheet, RAS PAREO):
// 「获得2个PAREO标记，然后视为你的房屋总数增加且可选择移除任意你拥有的格子上的一层房屋」
//
// 规则书 skill:鳰原令王那:梦幻可爱♪女仆（2）:
// 「你拥有格子上的房屋总数增加时可选择失去1PAREO标记（初始1，上限3），所有非自己的
// 玩家分摊支付你"格子的房屋造价×格子上的房屋数"四分之一的资金（所有你拥有的格子中
// 取最高值）」
//
// 「视为你的房屋总数增加」 is that increase, so with the skill bound the offer
// 「可选择失去1PAREO标记」 must come up; taking it moves a quarter of
// (house cost * houses) of the dearest tile. 山吹面包房: house cost 1000,
// 2 houses -> 1000*2/4 = 500, split over the one other player.
//
// `MatchPrompt` carries i18n *keys*, not resolved UI text: the offer's prompt is
// `cards:skill-characters.numazu_maid_title` / `numazu_maid_ask` (zh-CN
// 「失去 1 个 PAREO 标记，向其他玩家分摊收取 {{n}}？」), so the needle below
// matches that key rather than the rendered 「PAREO」/「失去」 text. The card's
// own house-removal prompts are `cards:card-ras.pareo_far_ask_*` /
// `pareo_far_pick` and stay declined.
#[test]
fn pareo_far_triggers_the_pareo_skill_offer() {
    let mut t = Table::new(&["鳰原令王那", "和奏瑞依"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    let shop = tile("山吹面包房"); // house cost 1000
    t.own(0, &[shop]);
    t.set_houses(shop, 2);
    t.give(0, &["RAS:（PAREO）渐渐远去的你"]);
    let m0 = t.money(0);
    let m1 = t.money(1);
    t.play(0, "RAS:（PAREO）渐渐远去的你").unwrap();
    // The skill's offer 「可选择失去1PAREO标记」 must be on the table.
    assert!(
        t.prompt().is_some(),
        "the house-count increase offers the skill: {}",
        t.dump_prompt()
    );
    // Take the offer (yes = option 0 for a yes/no), then decline the card's
    // own house-removal choice.
    let mut took_offer = false;
    for _ in 0..10 {
        let Some(p) = t.prompt() else { break };
        let hay = format!("{p:?}");
        if !took_offer
            && (hay.contains("numazu_maid_ask") || hay.contains("numazu_maid_title"))
            && !hay.contains("pareo_far_ask")
            && !hay.contains("pareo_far_pick")
        {
            let k = t.option("ask.yes").unwrap_or(0);
            answer_q(&mut t, 0, k).unwrap();
            took_offer = true;
            continue;
        }
        // 「可选择移除任意你拥有的格子上的一层房屋」 -- decline.
        decline_q(&mut t);
    }
    drain(&mut t);
    assert!(
        took_offer,
        "the skill's 「可选择失去1PAREO标记」 offer came up: {}",
        t.dump_prompt()
    );
    // 「所有非自己的玩家分摊支付你"格子的房屋造价×格子上的房屋数"四分之一的资金」
    assert_eq!(t.money(0), m0 + 500, "1000 * 2 / 4 = 500, one other player");
    assert_eq!(t.money(1), m1 - 500, "events {:?}", t.recent_keys(16));
    // 「获得2个PAREO标记」 -- the card's own marks land.
    let tok = t.p(0).tokens;
    assert_eq!(
        tok.iter().map(|c| c.value).sum::<i32>(),
        2,
        "「获得2个PAREO标记」 {tok:?}"
    );
}

// 规则书: without the bound skill there is no offer -- the same card played by a
// 鳰原令王那 whose skill is stripped (vanilla) only does the card's own text.
#[test]
fn pareo_far_offer_needs_the_bound_skill() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "鳰原令王那");
    let shop = tile("山吹面包房");
    t.own(0, &[shop]);
    t.set_houses(shop, 2);
    t.give(0, &["RAS:（PAREO）渐渐远去的你"]);
    let m0 = t.money(0);
    let m1 = t.money(1);
    t.play(0, "RAS:（PAREO）渐渐远去的你").unwrap();
    drain(&mut t);
    // No PAREO-mark offer to take, so no payment.
    assert_eq!(t.money(0), m0, "no skill offer without the bound skill");
    assert_eq!(t.money(1), m1, "events {:?}", t.recent_keys(16));
    // The card's own text still runs: 「获得2个PAREO标记」.
    let tok = t.p(0).tokens;
    assert_eq!(tok.iter().map(|c| c.value).sum::<i32>(), 2, "{tok:?}");
}
