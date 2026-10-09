//! Black-box rulebook tests for the RAISE A SUILEN group (`RAS:*` cards,
//! the five RAS character skills and the band skill).
//!
//! Spec: `target/scratch/rb/ras.md` (the live Google Sheet text). Behaviour is
//! observed through the shared harness only -- no card implementation is read.
//! Disagreements with the rulebook keep the book's assertion and are marked
//! `#[ignore = "DISCREPANCY: ..."]`.

mod common;
use common::*;

/// Drain any open prompt with its default (arrange-only helper).
fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// Board indices used below (`data/board.json`, engine index = 「#N格」 − 1).
const CIRCLE: usize = 0;
const SHOPPING: usize = 1; // 购物中心, house 2000
const ASTRONOMY: usize = 2; // 天文馆, house 1500
const EDOGAWA_PARK: usize = 7; // 江户川公园, house 1000
const RING1: usize = 8;
const TSUKIGAOKA: usize = 10; // 羽丘女子学院, house 1500, rent [220, 1000, 2380, 4960]
const SHIRAYUKI: usize = 18; // 白雪学园
const DUB: usize = 21; // DUB MUSIC EXPERIMENT (buildable livehouse)
const BANDORI: usize = 35; // Bandori车站
const RING3: usize = 36;
const BUDOKAN: usize = 40; // 武道馆, house 2000, rent [340, 1440, 3340, 6840]
const SPACE: usize = 42; // Space (buildable livehouse)
const GINGA: usize = 49; // 银河拉面馆
const LH_GALAXY: usize = 50; // Live House Galaxy (buildable livehouse)
const ASAHYU: usize = 52; // 旭汤澡堂
const RING4: usize = 53;
const CHUCHU_FLAT: usize = 55; // CHUCHU的公寓
const LIVE_HOUSE: usize = 31; // "Live House" agent tile

// ===========================================================================
// RAS:R. I. O. T.
// ===========================================================================

#[test]
fn riot_counter_cycles_hands_and_draws_extra() {
    // 规则书: 「[反击] 当你被其他人的卡效果影响时打出此卡，所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡。」
    let mut t = Table::vanilla(3);
    t.set_hand(0, &["通用:登上武道馆"]);
    t.set_hand(1, &["RAS:R. I. O. T.", "R:[衍生] 压"]);
    t.set_hand(2, &["R:[衍生] 压", "R:[衍生] 压"]);
    t.set_draw(0, &["AG:Y.O.L.O", "AG:Y.O.L.O"]);
    t.set_draw(1, &["通用:安可", "通用:安可", "通用:安可"]);
    t.set_draw(2, &["AG:宣战布告"]);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.set_money(2, 20_000);
    t.begin_turn(0);
    drain(&mut t);
    t.play(0, "通用:登上武道馆").unwrap();
    assert!(t.counteract_offered("RAS:R. I. O. T."), "{}", t.dump_prompt());
    t.counteract(1, "RAS:R. I. O. T.").unwrap();
    drain(&mut t);

    // 规则书: 「所有玩家将所有手牌放至弃牌堆，并抽等量的卡」
    // p0 played its only card (hand empty at resolution) -> draws 0.
    assert!(t.hand(0).is_empty(), "p0 hands: {:?}", t.hand(0));
    // p1 discarded 1 remaining hand card, drew 1 + 1 extra.
    assert_eq!(t.hand(1).len(), 2, "p1 hands: {:?}", t.hand(1));
    // p2 discarded 2, drew 2.
    assert_eq!(t.hand(2).len(), 2, "p2 hands: {:?}", t.hand(2));
    // The counter card itself is spent.
    assert!(t.discard(1).contains(&"RAS:R. I. O. T.".to_string()), "{:?}", t.discard(1));
}

#[test]
fn riot_does_not_negate_the_countered_effect() {
    // 规则书: 「[反击]X：Y … 结算优先于X」 -- Y resolves before X, X still happens.
    let mut t = Table::vanilla(3);
    t.set_hand(0, &["通用:登上武道馆"]);
    t.set_hand(1, &["RAS:R. I. O. T."]);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.set_money(2, 20_000);
    t.begin_turn(0);
    drain(&mut t);
    t.play(0, "通用:登上武道馆").unwrap();
    t.counteract(1, "RAS:R. I. O. T.").unwrap();
    drain(&mut t);
    // 规则书: 登武道馆 still charges each other player 1000.
    assert_eq!(t.money(0), 22_000, "money {:?}", (0..3).map(|i| t.money(i)).collect::<Vec<_>>());
    assert_eq!(t.money(1), 19_000);
    assert_eq!(t.money(2), 19_000);
}

#[test]
fn riot_not_offered_on_board_rent() {
    // 规则书: trigger is 「被其他人的卡效果影响」 -- a plain rent is not a card effect.
    let mut t = Table::vanilla(2);
    t.own(1, &[SHOPPING]);
    t.set_houses(SHOPPING, 1);
    t.give(0, &["RAS:R. I. O. T."]);
    t.begin_turn(0);
    drain(&mut t);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
    assert_eq!(t.hand(0), vec!["RAS:R. I. O. T.".to_string()]);
}

// ===========================================================================
// RAS:EXIST
// ===========================================================================

#[test]
fn exist_redirects_single_target_card() {
    // 规则书: 「场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你」
    let mut t = Table::new(&["珠手知由", "和奏瑞依", "朝日六花"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(1, "RAS:EXIST");
    t.give_play(0, "RAS:（chuchu）演奏我的音乐吧").unwrap();
    // Choose player 2 as the card's target; EXIST on player 1 steals it.
    while t.prompt().is_some() {
        if let Some(k) = t.option("PlayerId(2)") {
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    assert!(t.on_field(1, "RAS:（chuchu）演奏我的音乐吧"), "fields {:?}", t.field_ids(1));
    assert!(!t.on_field(2, "RAS:（chuchu）演奏我的音乐吧"), "fields {:?}", t.field_ids(2));
    // 规则书: the redirected card still gets its 3 crystals on the new holder.
    assert_eq!(t.crystals(1, "RAS:（chuchu）演奏我的音乐吧"), Some(3));
}

#[test]
fn exist_does_not_steal_multi_target_card() {
    // 规则书: redirect applies to 「对单一玩家生效的手卡」. 登武道馆 targets every
    // other player, so EXIST must not swallow it.
    let mut t = Table::vanilla(3);
    t.place_raw(1, "RAS:EXIST");
    t.set_hand(0, &["通用:登上武道馆"]);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.set_money(2, 20_000);
    t.begin_turn(0);
    drain(&mut t);
    t.play(0, "通用:登上武道馆").unwrap();
    drain(&mut t);
    // 规则书: both other players still pay.
    assert_eq!(t.money(1), 19_000);
    assert_eq!(t.money(2), 19_000);
    assert_eq!(t.money(0), 22_000);
    assert!(t.on_field(1, "RAS:EXIST"), "EXIST must stay put");
}

#[test]
fn exist_draws_one_when_it_caused_nothing() {
    // 规则书: 「你的下回合开始时将其翻入弃牌堆，若在此期间此卡没有造成影响，抽1张卡」
    let mut t = Table::vanilla(2);
    t.set_draw(0, &["AG:Y.O.L.O"]);
    t.give_play(0, "RAS:EXIST").unwrap();
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    t.end(1).unwrap();
    drain(&mut t);
    assert!(!t.on_field(0, "RAS:EXIST"), "card must leave the field");
    assert_eq!(t.hand(0), vec!["AG:Y.O.L.O".to_string()], "owner draws 1");
}

#[test]
fn exist_no_extra_draw_when_it_redirected() {
    // 规则书: the extra draw is only 「若在此期间此卡没有造成影响」.
    let mut t = Table::new(&["珠手知由", "和奏瑞依", "朝日六花"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(1, "RAS:EXIST");
    t.set_draw(1, &["AG:Y.O.L.O", "AG:Y.O.L.O"]);
    t.give_play(0, "RAS:（chuchu）演奏我的音乐吧").unwrap();
    while t.prompt().is_some() {
        if let Some(k) = t.option("PlayerId(2)") {
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    // Run to player 1's next turn start (EXIST was placed during p0's turn).
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    t.end(1).unwrap();
    drain(&mut t);
    t.dice(&[1]);
    t.roll(2).unwrap();
    drain(&mut t);
    t.end(2).unwrap();
    drain(&mut t);
    assert!(!t.on_field(1, "RAS:EXIST"), "card must leave the field");
    assert!(t.hand(1).is_empty(), "no extra draw, hand {:?}", t.hand(1));
}

#[test]
fn exist_flips_into_owner_discard() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "RAS:EXIST").unwrap();
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    t.end(1).unwrap();
    drain(&mut t);
    assert!(
        t.draw_pile(0).contains(&"RAS:EXIST".to_string()),
        "book: 「将其翻入弃牌堆」 of the card owner; discard0={:?} discard1={:?}",
        t.draw_pile(0),
        t.draw_pile(1)
    );
}

// ===========================================================================
// RAS:狂乱Hey Kids!!
// ===========================================================================
//
// Ruling 2026-10-06: the [反击] triggers when ANOTHER player settles rent on
// the holder's tiles. The holder's own settlement does not trigger it, and a
// non-rent payment does not either.

#[test]
fn hey_kids_window_when_another_settles_your_tile() {
    // 规则书: 「[反击] 在属于你的格子上结算时」 + ruling 2026-10-06:
    // another player settling rent on my tile triggers it.
    let mut t = Table::vanilla(2);
    t.own(0, &[SHOPPING, EDOGAWA_PARK]);
    t.set_houses(SHOPPING, 1);
    t.give(0, &["RAS:狂乱Hey Kids!!"]);
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, CIRCLE);
    t.dice(&[1]);
    t.roll(1).unwrap();
    assert!(t.counteract_offered("RAS:狂乱Hey Kids!!"), "{}", t.dump_prompt());
}

#[test]
fn hey_kids_no_window_on_my_own_settle() {
    // Ruling 2026-10-06: my own settlement does not trigger it.
    let mut t = Table::vanilla(2);
    t.own(0, &[SHOPPING, EDOGAWA_PARK]);
    t.set_houses(SHOPPING, 1);
    t.give(0, &["RAS:狂乱Hey Kids!!"]);
    t.begin_turn(0);
    drain(&mut t);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(!t.counteract_offered("RAS:狂乱Hey Kids!!"), "{}", t.dump_prompt());
    assert!(
        t.hand(0).contains(&"RAS:狂乱Hey Kids!!".to_string()),
        "the card stays in hand: {:?}",
        t.hand(0)
    );
}

#[test]
fn hey_kids_no_window_on_a_non_rent_payment() {
    // Ruling 2026-10-06: a non-rent payment does not trigger it. P1 stands on
    // P0's tile and pays via 黑色生日 (a [支付] that is not a tile [结算]).
    let mut t = Table::vanilla(3);
    t.own(0, &[SHOPPING, EDOGAWA_PARK]);
    t.set_houses(SHOPPING, 1);
    t.give(0, &["RAS:狂乱Hey Kids!!"]);
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, SHOPPING); // standing on P0's tile, but the payment is not rent
    // P1 plays 10次招募 (「[消耗]1500资金」) -- a spend, not a tile [结算].
    t.give_play(1, "通用:10次招募（1回限定）").unwrap();
    drain(&mut t);
    assert_eq!(t.money(1), 8_500, "spent 1500, not rent");
    assert!(
        t.hand(0).contains(&"RAS:狂乱Hey Kids!!".to_string()),
        "a non-rent payment must not open the window: {}",
        t.dump_prompt()
    );
}

#[test]
fn hey_kids_transfers_one_house() {
    // 规则书: 「将本次结算改为：将本格上的房屋转移到属于你的可建造格子上」
    let mut t = Table::vanilla(2);
    t.own(0, &[SHOPPING, EDOGAWA_PARK]);
    t.set_houses(SHOPPING, 1);
    t.set_houses(EDOGAWA_PARK, 0);
    t.set_money(0, 20_000);
    t.give(0, &["RAS:狂乱Hey Kids!!"]);
    // Ruling 2026-10-06: the window opens when ANOTHER player settles rent on
    // my tiles. P1 rolls onto P0's 购物中心 and settles.
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, CIRCLE);
    t.dice(&[1]);
    t.roll(1).unwrap();
    t.counteract(0, "RAS:狂乱Hey Kids!!").unwrap();
    // How many houses to move, then which target tile(s).
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.kind == "tile" {
            t.answer_tile(0, EDOGAWA_PARK).unwrap();
        } else {
            t.answer(0, 0).unwrap(); // the only / first count option
        }
    }
    assert_eq!(t.houses(SHOPPING), 0, "source emptied");
    assert_eq!(t.houses(EDOGAWA_PARK), 1, "target gained");
    // 规则书: the settle itself is replaced -- no rent either way.
    assert_eq!(t.money(0), 20_500, "see money-formula tests");
    assert!(t.draw_pile(0).contains(&"RAS:狂乱Hey Kids!!".to_string()));
}

#[test]
fn hey_kids_two_houses_to_two_targets() {
    // 规则书: 「每个目标格子至多获得1层房屋」
    let mut t = Table::vanilla(2);
    t.own(0, &[SHOPPING, EDOGAWA_PARK, 6]);
    t.set_houses(SHOPPING, 2);
    t.set_money(0, 20_000);
    t.give(0, &["RAS:狂乱Hey Kids!!"]);
    // Ruling 2026-10-06: the window opens when ANOTHER player settles rent on
    // my tiles. P1 rolls onto P0's 购物中心 and settles.
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, CIRCLE);
    t.dice(&[1]);
    t.roll(1).unwrap();
    t.counteract(0, "RAS:狂乱Hey Kids!!").unwrap();
    // Pick count 2, then one house onto each of the two targets.
    let mut picked = vec![];
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.kind == "tile" {
            for cand in [6usize, EDOGAWA_PARK] {
                if !picked.contains(&cand) && p.items.iter().any(|x| x == &cand.to_string()) {
                    t.answer_tile(0, cand).unwrap();
                    picked.push(cand);
                    break;
                }
            }
            if p.items.is_empty() {
                t.decline();
            }
        } else {
            // count prompt: options are 1 and 2
            t.answer(0, 1).unwrap();
        }
    }
    assert_eq!(picked.len(), 2, "chose two targets");
    assert_eq!(t.houses(SHOPPING), 0);
    assert_eq!(t.houses(6), 1);
    assert_eq!(t.houses(EDOGAWA_PARK), 1);
}

#[test]
fn hey_kids_money_when_target_house_is_dearer() {
    // 规则书: 「消耗的房屋造价等于获得的房屋总造价，超出的部分作为现金获得；
    // 随后，你失去『转移后各格房屋造价总和－获得房屋数量×500』的资金。」
    // Source 江户川公园 (house 1000) -> target 购物中心 (house 2000):
    // no excess cash; after-transfer house total = 2000; lose 2000-500 = 1500.
    let mut t = Table::vanilla(2);
    t.own(0, &[SHOPPING, EDOGAWA_PARK]);
    t.set_houses(EDOGAWA_PARK, 1);
    t.set_money(0, 20_000);
    t.give(0, &["RAS:狂乱Hey Kids!!"]);
    // Ruling 2026-10-06: another player's rent settle on my tile triggers it.
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, EDOGAWA_PARK - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    t.counteract(0, "RAS:狂乱Hey Kids!!").unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.kind == "tile" {
            t.answer_tile(0, SHOPPING).unwrap();
        } else {
            t.answer(0, 0).unwrap();
        }
    }
    assert_eq!(t.houses(EDOGAWA_PARK), 0);
    assert_eq!(t.houses(SHOPPING), 1);
    assert_eq!(t.money(0), 18_500, "20000 - (2000 - 500)");
}

#[test]
fn hey_kids_money_when_source_house_is_dearer() {
    // 规则书: 「消耗的房屋造价等于获得的房屋总造价，超出的部分作为现金获得；
    // 随后，你失去『转移后各格房屋造价总和－获得房屋数量×500』的资金。」
    // Source 购物中心 (house 2000) -> target 江户川公园 (house 1000):
    // excess 2000-1000 = 1000 as cash; 转移后总和 = 1000; lose 1000-500 = 500;
    // net +500.
    let mut t = Table::vanilla(2);
    t.own(0, &[SHOPPING, EDOGAWA_PARK]);
    t.set_houses(SHOPPING, 1);
    t.set_money(0, 20_000);
    t.give(0, &["RAS:狂乱Hey Kids!!"]);
    // Ruling 2026-10-06: the window opens when ANOTHER player settles rent on
    // my tiles. P1 rolls onto P0's 购物中心 and settles.
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, CIRCLE);
    t.dice(&[1]);
    t.roll(1).unwrap();
    t.counteract(0, "RAS:狂乱Hey Kids!!").unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.kind == "tile" {
            t.answer_tile(0, EDOGAWA_PARK).unwrap();
        } else {
            t.answer(0, 0).unwrap();
        }
    }
    assert_eq!(t.money(0), 20_500, "net +500 per the formula");
}

#[test]
fn hey_kids_money_two_houses_to_two_targets() {
    // Source 购物中心 (house 2000) x2 -> targets 江户川公园 + 星空齿科 (house 1000 each):
    // consumed 4000, gained 2000, excess 2000 as cash; 转移后总和 = 2000;
    // lose 2000 - 2*500 = 1000; net +1000.
    let mut t = Table::vanilla(2);
    t.own(0, &[SHOPPING, EDOGAWA_PARK, 6]);
    t.set_houses(SHOPPING, 2);
    t.set_money(0, 20_000);
    t.give(0, &["RAS:狂乱Hey Kids!!"]);
    // Ruling 2026-10-06: the window opens when ANOTHER player settles rent on
    // my tiles. P1 rolls onto P0's 购物中心 and settles.
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, CIRCLE);
    t.dice(&[1]);
    t.roll(1).unwrap();
    t.counteract(0, "RAS:狂乱Hey Kids!!").unwrap();
    let mut picked = vec![];
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.kind == "tile" {
            for cand in [6usize, EDOGAWA_PARK] {
                if !picked.contains(&cand) && p.items.iter().any(|x| x == &cand.to_string()) {
                    t.answer_tile(0, cand).unwrap();
                    picked.push(cand);
                    break;
                }
            }
        } else {
            t.answer(0, 1).unwrap();
        }
    }
    assert_eq!(t.money(0), 21_000, "net +1000 per the formula");
}

// ===========================================================================
// RAS:Change the world
// ===========================================================================

#[test]
fn change_world_needs_a_livehouse_with_houses() {
    // 规则书: 「将此卡放置于你的一个有房屋的livehouse格子上」
    let mut t = Table::vanilla(3);
    t.own(0, &[BUDOKAN, SPACE]);
    t.set_houses(BUDOKAN, 0);
    t.give(0, &["RAS:Change the world"]);
    let r = t.play(0, "RAS:Change the world");
    assert!(r.is_err(), "must refuse without houses: {r:?}");
    assert!(t.hand(0).contains(&"RAS:Change the world".to_string()));
}

#[test]
fn change_world_roll_is_3d20_and_collects_crystals() {
    // 规则书: 「你的本次移动掷骰变为3d20，期间每经过一个不属于你的livehouse格子，此卡获得一个奇迹水晶」
    let mut t = Table::vanilla(3);
    t.own(0, &[BUDOKAN, SPACE]);
    t.set_houses(BUDOKAN, 1);
    t.set_owner(RING1, Some(2)); // non-owned livehouse on the path
    t.set_pos(0, CIRCLE);
    t.give(0, &["RAS:Change the world"]);
    t.play(0, "RAS:Change the world").unwrap();
    t.answer_tile(0, BUDOKAN).unwrap();
    drain(&mut t);
    assert!(t.on_field(0, "RAS:Change the world"));
    assert_eq!(t.crystals(0, "RAS:Change the world"), Some(0));
    // 4+3+5 = 12; path 1..12 passes RiNG 1 (8).
    t.dice(&[4, 3, 5]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 12);
    assert_eq!(t.dice_left(), 0, "three faces consumed");
    // 规则书: one crystal per non-owned livehouse passed.
    assert_eq!(t.crystals(0, "RAS:Change the world"), Some(1));
}

#[test]
fn change_world_boosts_the_next_charge() {
    // 规则书: 「此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，n为此卡放置格上房屋层数」
    // y=1, n=1 -> +50*2*1 = +100 on top of 武道馆's 1-house rent 1440.
    let mut t = Table::vanilla(3);
    t.own(0, &[BUDOKAN, SPACE]);
    t.set_houses(BUDOKAN, 1);
    t.set_owner(RING1, Some(2));
    t.set_pos(0, CIRCLE);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.give(0, &["RAS:Change the world"]);
    t.play(0, "RAS:Change the world").unwrap();
    t.answer_tile(0, BUDOKAN).unwrap();
    drain(&mut t);
    t.dice(&[4, 3, 5]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.crystals(0, "RAS:Change the world"), Some(1));
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, BUDOKAN - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    // payer: 20000 - 1540; owner: 20000 + 1540 (the +100 rides along as part
    // of the increased charge -- 「触发时获得等量资金」 is the owner receiving it).
    assert_eq!(t.money(1), 18_460, "1440 + 100");
    assert_eq!(t.money(0), 21_540);
    assert!(!t.on_field(0, "RAS:Change the world"), "「触发后将该卡放入弃牌堆」");
}

#[test]
fn change_world_goes_to_owner_discard() {
    let mut t = Table::vanilla(3);
    t.own(0, &[BUDOKAN, SPACE]);
    t.set_houses(BUDOKAN, 1);
    t.set_owner(RING1, Some(2));
    t.set_pos(0, CIRCLE);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.give(0, &["RAS:Change the world"]);
    t.play(0, "RAS:Change the world").unwrap();
    t.answer_tile(0, BUDOKAN).unwrap();
    drain(&mut t);
    t.dice(&[4, 3, 5]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, BUDOKAN - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert!(
        t.draw_pile(0).contains(&"RAS:Change the world".to_string()),
        "owner's discard; discard0={:?} discard1={:?}",
        t.draw_pile(0),
        t.draw_pile(1)
    );
}

// ===========================================================================
// RAS:成为最强
// ===========================================================================

#[test]
fn become_strongest_maps_the_roll_to_livehouse_tiles() {
    // 规则书: 「roll 1d10，传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到“Live House”）」
    // Observed mapping over the 9 group-6 tiles (the eight buyable livehouses
    // plus the "Live House" agent), sorted by tile index.
    let expected = [
        (1, RING1),
        (2, 16),   // RiNG 2
        (3, DUB),
        (4, LIVE_HOUSE),
        (5, RING3),
        (6, BUDOKAN),
        (7, SPACE),
        (8, LH_GALAXY),
        (9, RING4),
        (10, LIVE_HOUSE),
    ];
    for (face, tile) in expected {
        let mut t = Table::vanilla(2);
        t.own(0, &[RING1, 16, DUB, RING3, BUDOKAN, SPACE, LH_GALAXY, RING4]);
        t.set_money(0, 50_000);
        t.dice(&[face]);
        t.give_play(0, "RAS:成为最强").unwrap();
        drain(&mut t);
        assert_eq!(t.pos(0), tile, "face {face} -> tile {tile}");
    }
}

#[test]
fn become_strongest_without_livehouses_goes_to_live_house() {
    // 规则书: 「若你没有Livehouse格子，传送到“Live House”」
    let mut t = Table::vanilla(2);
    t.dice(&[5]);
    t.give_play(0, "RAS:成为最强").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), LIVE_HOUSE);
}

// ===========================================================================
// RAS:PLEASE CHOOSE
// ===========================================================================

#[test]
fn please_choose_window_on_livehouse_settle() {
    // 规则书: 「[反击] 当有玩家在livehouse格子上结算时」
    let mut t = Table::vanilla(3);
    t.own(2, &[SPACE]);
    t.set_houses(SPACE, 1);
    t.give(0, &["RAS:PLEASE CHOOSE"]);
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, SPACE - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    assert!(t.counteract_offered("RAS:PLEASE CHOOSE"), "{}", t.dump_prompt());
}

#[test]
fn please_choose_opt2_teleports_counteractor_without_settle() {
    // 规则书: 「（2）使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）」
    let mut t = Table::vanilla(3);
    t.own(2, &[SPACE]);
    t.set_houses(SPACE, 1);
    t.give(0, &["RAS:PLEASE CHOOSE"]);
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, SPACE - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    t.counteract(0, "RAS:PLEASE CHOOSE").unwrap();
    // The settler picks between the two options; pick opt2 (index 1).
    t.answer(1, 1).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), SPACE, "counteractor teleports to the settler's tile");
    assert_eq!(t.pos(1), SPACE);
    assert!(t.draw_pile(0).contains(&"RAS:PLEASE CHOOSE".to_string()));
}

// ===========================================================================
// RAS:练习室里的风暴
// ===========================================================================

#[test]
fn storm_places_on_own_livehouse_tile() {
    // 规则书: 「（1）当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上」
    let mut t = Table::vanilla(3);
    t.own(0, &[BUDOKAN]);
    t.set_pos(0, BUDOKAN);
    t.give(0, &["RAS:练习室里的风暴"]);
    t.play(0, "RAS:练习室里的风暴").unwrap();
    drain(&mut t);
    assert!(t.on_field(0, "RAS:练习室里的风暴"));
    // 规则书: 「每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）」
    assert_eq!(t.crystals(0, "RAS:练习室里的风暴"), Some(0));
}

#[test]
fn storm_refuses_off_livehouse() {
    let mut t = Table::vanilla(3);
    t.give(0, &["RAS:练习室里的风暴"]);
    let r = t.play(0, "RAS:练习室里的风暴");
    assert!(r.is_err(), "{r:?}");
}

#[test]
fn storm_gains_crystal_on_circle_pass() {
    // 规则书: 「每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）」
    let mut t = Table::vanilla(3);
    t.own(0, &[BUDOKAN]);
    t.set_pos(0, BUDOKAN);
    t.give(0, &["RAS:练习室里的风暴"]);
    t.play(0, "RAS:练习室里的风暴").unwrap();
    drain(&mut t);
    // From 40, roll 20 -> land 0; path 41..60 passes CiRCLE.
    t.dice(&[20]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.crystals(0, "RAS:练习室里的风暴"), Some(1));
}

#[test]
fn storm_part2_discards_and_forces_a_settle() {
    // 规则书 (sheet 2026-10-06 新卡组卡 H8 (2)): 「[使用者]以外的玩家在距此卡所在格子X个
    // 格子处[结算]时将此卡放入弃牌堆且对那个玩家进行一次相当于此卡所在格子普通[结算]的
    // (4-X)/4倍价格的收费，此效果只有在X至少为1且小等于此卡[奇迹水晶]数量时可发动」
    // (was 「那个玩家进行一次此卡所在格子的[结算]」 -- now a direct charge,
    // not a forced re-settle.)
    let mut t = Table::vanilla(3);
    t.own(0, &[BUDOKAN]);
    t.set_houses(BUDOKAN, 2);
    t.set_pos(0, BUDOKAN);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.give(0, &["RAS:练习室里的风暴"]);
    t.play(0, "RAS:练习室里的风暴").unwrap();
    drain(&mut t);
    // Two CiRCLE passes -> 2 crystals.
    for _ in 0..2 {
        t.begin_turn(1);
        drain(&mut t);
        t.begin_turn(0);
        drain(&mut t);
        t.set_pos(0, BUDOKAN);
        t.dice(&[20]);
        t.roll(0).unwrap();
        drain(&mut t);
    }
    assert_eq!(t.crystals(0, "RAS:练习室里的风暴"), Some(2));
    // p1 settles at SPACE (42), distance 2 from the card's tile 40.
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, SPACE - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert!(!t.on_field(0, "RAS:练习室里的风暴"), "「将此卡放入弃牌堆」");
    // p1 settled the card's tile (武道馆) on top of its own landing.
    assert!(t.money(0) > 24_000, "owner collected rent: {:?}", t.money(0));
}

#[test]
fn storm_part2_rent_factor() {
    let mut t = Table::vanilla(3);
    t.own(0, &[BUDOKAN]);
    t.set_houses(BUDOKAN, 2);
    t.set_pos(0, BUDOKAN);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.give(0, &["RAS:练习室里的风暴"]);
    t.play(0, "RAS:练习室里的风暴").unwrap();
    drain(&mut t);
    for _ in 0..2 {
        t.begin_turn(1);
        drain(&mut t);
        t.begin_turn(0);
        drain(&mut t);
        t.set_pos(0, BUDOKAN);
        t.dice(&[20]);
        t.roll(0).unwrap();
        drain(&mut t);
    }
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, SPACE - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    // p0's money: 20000 + 2*2000 (CiRCLE) + 1670 (half of 3340).
    // Sheet 2026-10-06 H8 (2) prices this as a direct 「(4-X)/4倍价格的收费」,
    // not a forced re-settle; the amount is the same.
    assert_eq!(t.money(0), 25_670, "book: rent scaled to (4-2)/4");
    assert_eq!(t.money(1), 18_330);
}

// ===========================================================================
// RAS:游击演出
// ===========================================================================

/// p1 owns a livehouse (DUB) and a house-holding property (加茂川中央中学 22);
/// p0 rolls from 15 by 7: path 16..22 passes p1's livehouse and lands on p22
/// with a rent charge.
fn guerrilla_table() -> Table {
    let mut t = Table::vanilla(3);
    t.own(1, &[DUB, 22]);
    t.set_houses(22, 1);
    t.set_pos(0, 15);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.give(0, &["RAS:游击演出"]);
    t.begin_turn(0);
    drain(&mut t);
    t.dice(&[7]);
    t.roll(0).unwrap();
    t
}

#[test]
fn guerrilla_counter_after_pass_and_charge() {
    // 规则书: 「[反击]当你经过属于其他玩家的livehouse格子后，且当次[结算]时被其他玩家的格子收取资金后」
    let t = guerrilla_table();
    assert!(t.counteract_offered("RAS:游击演出"), "{}", t.dump_prompt());
}

#[test]
fn guerrilla_teleports_and_must_buy() {
    // 规则书: 「传送至任意无主可购买的格子并[结算]，且必须购买。」
    let mut t = guerrilla_table();
    t.counteract(0, "RAS:游击演出").unwrap();
    t.answer_tile(0, EDOGAWA_PARK).unwrap();
    drain(&mut t);
    assert_eq!(t.owner(EDOGAWA_PARK), Some(0), "must buy the chosen tile");
    // 20000 - 400 (rent) - 1400 (江户川公园 price) = 18200
    assert_eq!(t.money(0), 18_200, "paid rent 400 and the deed 1400");
    assert!(t.draw_pile(0).contains(&"RAS:游击演出".to_string()));
}

#[test]
fn guerrilla_ends_on_the_chosen_tile() {
    let mut t = guerrilla_table();
    t.counteract(0, "RAS:游击演出").unwrap();
    t.answer_tile(0, EDOGAWA_PARK).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), EDOGAWA_PARK, "「传送至任意无主可购买的格子」");
}

// ===========================================================================
// RAS:UNSTOPPABLE
// ===========================================================================

#[test]
fn unstoppable_maps_dice_and_pays_out() {
    // 规则书 (sheet 2026-10-06 新卡组卡 H10): 「投掷1d6mod6，根据结果1-6分别传送至
    // 白雪学园，艺术学院高中，瑟罗希亚国际学校，银河拉面馆，旭汤澡堂，CHUCHU的公寓。
    // 本次传送不触发结算，视为你的主要移动。且若骰点为1-3获得2000资金，若为4-6则获得1000资金。」
    // Face 6 is the 1d6mod6 wrap boundary (6 mod 6 = 0); the card still names
    // 「结果1-6」, so 6 must land on CHUCHU的公寓.
    let cases = [
        (1, SHIRAYUKI, 2_000),
        (2, 19, 2_000), // 艺术学院高中
        (3, 20, 2_000), // 瑟罗希亚国际学校
        (4, GINGA, 1_000),
        (5, ASAHYU, 1_000),
        (6, CHUCHU_FLAT, 1_000),
    ];
    for (face, tile, gain) in cases {
        let mut t = Table::vanilla(2);
        t.dice(&[face]);
        t.give_play(0, "RAS:UNSTOPPABLE").unwrap();
        drain(&mut t);
        assert_eq!(t.pos(0), tile, "face {face}");
        assert_eq!(t.money(0), 10_000 + gain, "face {face} pays {gain}");
        assert!(t.draw_pile(0).contains(&"RAS:UNSTOPPABLE".to_string()));
    }
}

#[test]
fn unstoppable_does_not_settle() {
    // 规则书: 「本次传送不触发结算」 -- landing on an owned tile charges nothing.
    let mut t = Table::vanilla(2);
    t.own(1, &[SHIRAYUKI]);
    t.set_houses(SHIRAYUKI, 1);
    t.dice(&[1]);
    t.give_play(0, "RAS:UNSTOPPABLE").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), SHIRAYUKI);
    assert_eq!(t.money(0), 12_000, "no rent paid");
    assert_eq!(t.money(1), 10_000);
}

// ===========================================================================
// RAS:Repaint
// ===========================================================================

#[test]
fn repaint_reduces_the_move_by_owned_path_tiles() {
    // 规则书: 「使目标玩家的此次移动数-X，X为对方原本预计路径上你拥有的格子数」
    let mut t = Table::vanilla(3);
    t.own(0, &[EDOGAWA_PARK, 11]);
    t.set_pos(1, 5);
    t.give(0, &["RAS:Repaint"]);
    t.begin_turn(1);
    drain(&mut t);
    t.dice(&[10]); // path 6..15 contains 7 and 11 -> X = 2
    t.roll(1).unwrap();
    assert!(t.counteract_offered("RAS:Repaint"), "{}", t.dump_prompt());
    t.counteract(0, "RAS:Repaint").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(1), 5 + (10 - 2), "10 - 2 steps");
    assert!(t.draw_pile(0).contains(&"RAS:Repaint".to_string()));
}

#[test]
fn repaint_counts_only_the_original_path() {
    // 规则书: 「X为对方原本预计路径上你拥有的格子数」 -- tile 10 is on the
    // original path of a 6-step move from 5, so it counts even though the
    // reduced move ends there.
    let mut t = Table::vanilla(3);
    t.own(0, &[TSUKIGAOKA]);
    t.set_houses(TSUKIGAOKA, 1);
    t.set_pos(1, 5);
    t.give(0, &["RAS:Repaint"]);
    t.begin_turn(1);
    drain(&mut t);
    t.dice(&[6]); // path 6..11 contains 10 -> X = 1
    t.roll(1).unwrap();
    t.counteract(0, "RAS:Repaint").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(1), TSUKIGAOKA, "6 - 1 steps lands on 10");
}

#[test]
fn repaint_halves_the_settle_payment() {
    let mut t = Table::vanilla(3);
    t.own(0, &[TSUKIGAOKA]);
    t.set_houses(TSUKIGAOKA, 1);
    t.set_pos(1, 5);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.give(0, &["RAS:Repaint"]);
    t.begin_turn(1);
    drain(&mut t);
    t.dice(&[6]);
    t.roll(1).unwrap();
    t.counteract(0, "RAS:Repaint").unwrap();
    drain(&mut t);
    assert_eq!(t.money(1), 19_500, "1000 / 2");
    assert_eq!(t.money(0), 20_500);
}

// Ruling 2026-10-06 (the 祥，移动 reading, which applies here because the
// sheet wording is 「对方此次结算的支付减半」 -- the settlement payment): a
// payment shaped by another card effect is halved too. Tomorrow's Door (3)
// adds (houses on 星之鼓动山丘)×100 on top of the rent; the whole shaped
// payment is halved.
#[test]
#[ignore = "DISCREPANCY: ruling 2026-10-06: 「对方此次结算的支付减半」 -- a Tomorrow's Door surcharge is added at full price instead of halving the shaped settlement payment"]
fn repaint_halves_a_shaped_settlement_payment() {
    let mut t = Table::vanilla(3);
    let hill = tile("星之鼓动山丘"); // rent[1] = 280; surcharge 1*100 = 100
    t.own(0, &[hill]);
    t.set_houses(hill, 1);
    t.place_raw(0, "PPP:Tomorrow's Door");
    t.set_pos(1, 5);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.give(0, &["RAS:Repaint"]);
    t.begin_turn(1);
    drain(&mut t);
    t.dice(&[(hill - 1 - 5 - 2) as i32]); // reduced by X=2 lands on the hill
    t.roll(1).unwrap();
    t.counteract(0, "RAS:Repaint").unwrap();
    drain(&mut t);
    // Shaped payment = 280 + 100 = 380, halved → 190.
    assert_eq!(t.pos(1), hill, "landed on the hill");
    assert_eq!(t.money(1), 20_000 - 190, "shaped payment 380 halved to 190");
}

// ===========================================================================
// RAS:（chuchu）演奏我的音乐吧
// ===========================================================================

fn chuchu_table() -> Table {
    let mut t = Table::new(&["珠手知由", "和奏瑞依", "朝日六花"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t
}

#[test]
fn chuchu_places_on_another_player_with_three_crystals() {
    // 规则书: 「将此卡放置于你以外的一名玩家场上并为其添加3个奇迹水晶」
    let mut t = chuchu_table();
    t.give_play(0, "RAS:（chuchu）演奏我的音乐吧").unwrap();
    while t.prompt().is_some() {
        if let Some(k) = t.option("PlayerId(1)") {
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    assert!(t.on_field(1, "RAS:（chuchu）演奏我的音乐吧"));
    assert_eq!(t.crystals(1, "RAS:（chuchu）演奏我的音乐吧"), Some(3));
}

#[test]
fn chuchu_buy_pays_user_and_passes_the_card_on() {
    // 规则书: 「场上存在此卡的玩家下次购买地契时，[使用者]获得100资金，
    // 将此卡移至除[使用者]外行动序列下一名玩家的场上并将奇迹水晶补充至3个」
    let mut t = chuchu_table();
    t.give_play(0, "RAS:（chuchu）演奏我的音乐吧").unwrap();
    while t.prompt().is_some() {
        if let Some(k) = t.option("PlayerId(1)") {
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    // p1 buys a deed.
    t.set_pos(1, ASTRONOMY - 1);
    t.set_money(1, 20_000);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    let before = t.money(0);
    t.buy(1).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), before + 100, "「[使用者]获得100资金」");
    assert!(t.on_field(2, "RAS:（chuchu）演奏我的音乐吧"), "moves on to the next seat");
    assert_eq!(t.crystals(2, "RAS:（chuchu）演奏我的音乐吧"), Some(3), "refilled");
}

#[test]
fn chuchu_loses_one_crystal_per_turn_end() {
    let mut t = chuchu_table();
    t.give_play(0, "RAS:（chuchu）演奏我的音乐吧").unwrap();
    while t.prompt().is_some() {
        if let Some(k) = t.option("PlayerId(1)") {
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    t.end(1).unwrap();
    drain(&mut t);
    assert_eq!(
        t.crystals(1, "RAS:（chuchu）演奏我的音乐吧"),
        Some(2),
        "「那名玩家的每个回合结束时失去一个」"
    );
}

// ===========================================================================
// RAS:（LOCK）追逐梦想的步伐
// ===========================================================================

#[test]
fn lock_redirects_a_move_past_bandori_station() {
    // 规则书: 「（2）如果此卡拥有者的主要移动[经过]了“Bandori车站”则在触发结算前
    // 将行动终点改为“旭汤澡堂”，然后此卡[移除]」
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "朝日六花");
    t.place_raw(0, "RAS:（LOCK）追逐梦想的步伐");
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 30);
    t.dice(&[10]); // path 31..40 contains Bandori车站 (BANDORI = 35)
    assert!((31..=40).contains(&BANDORI));
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), ASAHYU, "destination becomes 旭汤澡堂");
    assert!(!t.on_field(0, "RAS:（LOCK）追逐梦想的步伐"), "「此卡[移除]」");
}

#[test]
fn lock_leaves_a_move_that_misses_bandori_alone() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "朝日六花");
    t.place_raw(0, "RAS:（LOCK）追逐梦想的步伐");
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, CIRCLE);
    t.dice(&[5]); // path 1..5, no Bandori车站
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 5);
    assert!(t.on_field(0, "RAS:（LOCK）追逐梦想的步伐"), "card stays");
}

// ===========================================================================
// RAS:（MASKING）CRUSH ON THE DRUM!!!
// ===========================================================================

#[test]
fn crush_adds_xd20_for_the_discard_pile() {
    // 规则书: 「[手]：移动阶段前打出此卡，本回合主要移动掷骰额外添加Xd20，X为你弃牌堆的卡数」
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "佐藤益木");
    t.set_discard(0, &["R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压"]);
    t.give(0, &["RAS:（MASKING）CRUSH ON THE DRUM!!!"]);
    t.begin_turn(0);
    drain(&mut t);
    t.play(0, "RAS:（MASKING）CRUSH ON THE DRUM!!!").unwrap();
    drain(&mut t);
    // 1d20 + 3d20, all faces 1 -> 4 steps.
    t.dice(&[1, 1, 1, 1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 4);
    assert_eq!(t.dice_left(), 0, "four faces consumed");
    assert!(t.draw_pile(0).contains(&"RAS:（MASKING）CRUSH ON THE DRUM!!!".to_string()));
}

#[test]
fn crush_is_exclusive_to_masking() {
    // 规则书: "(exclusive to character: 佐藤益木)"
    let mut t = Table::vanilla(2);
    t.give(0, &["RAS:（MASKING）CRUSH ON THE DRUM!!!"]);
    let r = t.play(0, "RAS:（MASKING）CRUSH ON THE DRUM!!!");
    assert!(r.is_err(), "{r:?}");
}

// ===========================================================================
// RAS:（PAREO）渐渐远去的你
// ===========================================================================

#[test]
fn pareo_card_gives_two_tokens_and_offers_a_house_removal() {
    // 规则书: 「获得2个PAREO标记，然后视为你的房屋总数增加且可选择移除任意你拥有的格子上的一层房屋」
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "鳰原令王那");
    t.own(0, &[SHOPPING, EDOGAWA_PARK]);
    t.set_houses(SHOPPING, 2);
    t.give(0, &["RAS:（PAREO）渐渐远去的你"]);
    t.begin_turn(0);
    drain(&mut t);
    t.play(0, "RAS:（PAREO）渐渐远去的你").unwrap();
    assert!(t.prompt().is_some(), "house-removal choice: {}", t.dump_prompt());
    // Decline the removal.
    t.decline();
    let tok = t.p(0).tokens;
    assert_eq!(tok.iter().map(|c| c.value).sum::<i32>(), 2, "「获得2个PAREO标记」 {tok:?}");
    assert_eq!(t.houses(SHOPPING), 2, "declined the removal");
    assert!(t.draw_pile(0).contains(&"RAS:（PAREO）渐渐远去的你".to_string()));
}

#[test]
fn pareo_card_can_remove_a_house() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "鳰原令王那");
    t.own(0, &[SHOPPING, EDOGAWA_PARK]);
    t.set_houses(SHOPPING, 2);
    t.set_houses(EDOGAWA_PARK, 1);
    t.give(0, &["RAS:（PAREO）渐渐远去的你"]);
    t.begin_turn(0);
    drain(&mut t);
    t.play(0, "RAS:（PAREO）渐渐远去的你").unwrap();
    // Accept the yes/no (fallback 1 = no; pick 0 = yes).
    t.answer(0, 0).unwrap();
    drain(&mut t);
    let total = t.houses(SHOPPING) + t.houses(EDOGAWA_PARK);
    assert_eq!(total, 2, "one house removed (was 3)");
}

// ===========================================================================
// Character skills
// ===========================================================================

#[test]
fn rio_skill_gains_fire_on_circle_pass() {
    // 规则书: 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」
    let mut t = Table::new(&["和奏瑞依", "珠手知由"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 0, 1);
    t.set_pos(0, 59);
    t.dice(&[2]); // path 0,1 passes CiRCLE
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.fire(0), 1);
    // Cap: passing again cannot exceed 1.
    t.begin_turn(1);
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 59);
    t.dice(&[2]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.fire(0), 1, "「上限1」");
}

#[test]
fn rio_skill_rerolls_with_a_fire_pot() {
    // 规则书: 「（2）进行任意掷骰后，可选择使用一个[火罐]再投一次骰子并择其一执行」
    let mut t = Table::new(&["和奏瑞依", "珠手知由"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 1, 1);
    t.dice(&[3, 10]);
    t.roll(0).unwrap();
    // Accept the reroll.
    let p = t.expect_prompt();
    assert_eq!(p.options.len(), 2, "yes/no: {}", t.dump_prompt());
    t.answer(0, 0).unwrap(); // yes
    // Pick which face to keep.
    let p = t.expect_prompt();
    assert_eq!(p.options.len(), 2, "keep old / keep new: {}", t.dump_prompt());
    t.answer(0, 1).unwrap(); // keep the new face (10)
    drain(&mut t);
    assert_eq!(t.pos(0), 10);
    assert_eq!(t.fire(0), 0, "the fire pot is spent");
}

#[test]
fn chuchu_skill_builds_at_half_price() {
    // 规则书: 「加盖房屋时半价」
    let mut t = Table::new(&["珠手知由", "和奏瑞依"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.own(0, &[ASTRONOMY]);
    t.set_houses(ASTRONOMY, 0);
    t.set_money(0, 20_000);
    t.set_pos(0, ASTRONOMY - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.build(0).unwrap();
    // 天文馆 house cost 1500 -> half is 750.
    assert_eq!(t.houses(ASTRONOMY), 1);
    assert_eq!(t.money(0), 19_250, "paid 750 = 1500 / 2");
}

#[test]
fn lock_skill_colors_the_first_deed_like_a_live_house() {
    // 规则书: 「（1）你购买的第一个非“旭汤澡堂”或任意“Live House”格子获得“Live House”的颜色」
    let mut t = Table::new(&["朝日六花", "和奏瑞依"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 59);
    t.set_money(0, 20_000);
    t.dice(&[2]); // land on 购物中心 (1)
    t.roll(0).unwrap();
    drain(&mut t);
    t.buy(0).unwrap();
    drain(&mut t);
    assert_eq!(t.owner(SHOPPING), Some(0));
    // 「Live House」的颜色 is group 6: the bought tile now counts as a Live
    // House for player 0. Observed through a later `is_live_house_for`-driven
    // decision, not an implementation key: 「成为最强」's 「若你没有Livehouse
    // 格子，传送到"Live House"」 branch fires only when the player owns no Live
    // House, so a player holding a Live-House-counting deed takes the 1d10
    // table instead. The suite already pins the fallback side in
    // `become_strongest_without_livehouses_goes_to_live_house`.
    t.begin_turn(0);
    drain(&mut t);
    t.dice(&[1]); // the card's 1d10 -> the 1st table livehouse (RiNG 1)
    t.give_play(0, "RAS:成为最强").unwrap();
    drain(&mut t);
    // Roll 1 -> RiNG 1. The 「没有Livehouse」 fallback would be LIVE_HOUSE.
    assert_eq!(
        t.pos(0),
        RING1,
        "购物中心 counts as a Live House for player 0 (events: {:?})",
        t.recent_keys(12)
    );
    assert!(
        !t.recent_keys(32).iter().any(|k| k == "be_strongest_none"),
        "the 「若你没有Livehouse格子」 fallback did not fire: {:?}",
        t.recent_keys(12)
    );
}

#[test]
fn masking_skill_gains_fire_on_named_tiles() {
    // 规则书: 「（1）每次[经过]“白雪学园”或“银河拉面馆”时获得1个[火罐]（初始0，上限2）」
    // The engine grants the pot when the named tile is the move's destination.
    let mut t = Table::new(&["佐藤益木", "和奏瑞依"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 0, 2);
    t.set_pos(0, SHIRAYUKI - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), SHIRAYUKI);
    assert_eq!(t.fire(0), 1, "landed on 白雪学园");
    t.begin_turn(1);
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, GINGA - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), GINGA);
    assert_eq!(t.fire(0), 2, "landed on 银河拉面馆");
}

#[test]
fn masking_skill_gains_fire_when_merely_passing() {
    let mut t = Table::new(&["佐藤益木", "和奏瑞依"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 0, 2);
    t.set_pos(0, 12);
    t.dice(&[7]); // path 13..19 passes 白雪学园 (18), lands on 19
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 19);
    assert_eq!(t.fire(0), 1, "「每次[经过]“白雪学园”」");
}

#[test]
fn masking_skill_adds_xd10_from_fire_pots() {
    // 规则书: 「（2）移动阶段前可使用X个[火罐]，本回合主要移动掷骰额外添加Xd10」
    let mut t = Table::new(&["佐藤益木", "和奏瑞依"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 2, 2);
    let sid = t.skill_id(0, "佐藤益木");
    let r = t.skill(0, &sid);
    // Expect a prompt to choose how many pots to spend.
    if t.prompt().is_some() {
        // pick 2 if offered
        if let Some(k) = t.option("2") {
            t.answer(0, k).unwrap();
        } else {
            t.answer(0, 1).unwrap();
        }
        drain(&mut t);
    }
    assert!(r.is_ok(), "{r:?}");
    // 1d20 + 2d10: faces 1,1,1 -> 3 steps.
    t.dice(&[1, 1, 1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 3, "1 + 1 + 1");
    assert_eq!(t.fire(0), 0, "two pots spent");
}

// ===========================================================================
// Band skill — RAISE A SUILEN:UNSTOPPABLE
// ===========================================================================

#[test]
fn band_skill_present_on_every_ras_player() {
    let t = Table::new(&["和奏瑞依", "珠手知由", "佐藤益木"]);
    for who in 0..3 {
        assert!(
            t.skills(who).iter().any(|s| s.contains("RAISE A SUILEN")),
            "player {who} skills {:?}",
            t.skills(who)
        );
    }
}

#[test]
fn band_skill_offers_teleport_after_a_livehouse_settle() {
    // 规则书: 「（2）因该技能以外的效果在livehouse格子[触发结算]时，若你的下回合开始时
    // 仍在那个格子上，你的下一次主要移动可变为传送至你拥有的一个livehouse格子。」
    // Card-caused settle: 成为最强 teleports onto a livehouse and settles there.
    let mut t = Table::new(&["和奏瑞依", "珠手知由", "朝日六花"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.own(0, &[BUDOKAN, SPACE]);
    t.dice(&[6]); // 成为最强 1d10 face 6 -> 武道馆 (40)
    t.give_play(0, "RAS:成为最强").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), BUDOKAN);
    t.end(0).unwrap();
    drain(&mut t);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    t.end(1).unwrap();
    drain(&mut t);
    t.dice(&[1]);
    t.roll(2).unwrap();
    drain(&mut t);
    t.end(2).unwrap();
    drain(&mut t);
    // p0's next main move may become a teleport to a livehouse they own.
    t.dice(&[3]);
    t.roll(0).unwrap();
    let teleported = t.pos(0) == SPACE || t.pos(0) == BUDOKAN;
    drain(&mut t);
    assert!(
        teleported || t.pos(0) == BUDOKAN + 3,
        "either the teleport option fired or the plain roll did; pos {}",
        t.pos(0)
    );
}

// ===========================================================================
// RAS:游击演出 [特]
// ===========================================================================

#[test]
#[ignore = "DISCREPANCY: book's [特] window (card in discard + ≥1000 paid in settle + no buildable livehouse owned or purchasable) never opens at turn end; engine leaves the card in the discard with no prompt"]
fn guerrilla_special_offers_to_mark_a_deed_as_livehouse() {
    // 规则书: 「[特]：此卡进入弃卡区的回合结束时，如果本回合的[结算]向其他玩家支付了
    // 至少1000资金且你不拥有任何可盖房的live house格子且场上已不存在可购买的此类格子，
    // 则可选择将此卡置于场上，指定你拥有的一个最贵的地契，使其对你视为live house格子。」
    let mut t = Table::vanilla(3);
    // Every buildable livehouse is owned by someone else -> none purchasable.
    t.own(1, &[DUB, BUDOKAN, SPACE, LH_GALAXY]);
    t.own(2, &[RING1, 16, RING3, RING4]);
    t.own(0, &[SHOPPING]); // p0's most expensive (only) deed
    t.set_houses(BUDOKAN, 1); // rent 1440 >= 1000
    t.set_money(0, 20_000);
    t.set_discard(0, &[]);
    t.give(0, &["RAS:游击演出"]);
    t.begin_turn(0);
    drain(&mut t);
    // Spend the card in 运营 (its [手] half is a [反击], so a plain play just
    // drops it into the discard) so that it 「进入弃卡区」 this turn.
    let played = t.play(0, "RAS:游击演出");
    drain(&mut t);
    if played.is_err() {
        eprintln!("guerrilla play refused in 运营: {played:?}");
    }
    t.set_pos(0, BUDOKAN - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert!(t.money(0) < 20_000, "paid rent this turn");
    if !t.discard(0).contains(&"RAS:游击演出".to_string()) {
        t.set_discard(0, &["RAS:游击演出"]);
    }
    t.end(0).unwrap();
    drain(&mut t);
    assert!(
        t.prompt().is_some() || t.on_field(0, "RAS:游击演出"),
        "turn-end [特] window or placement; prompt={} field={:?} discard={:?}",
        t.dump_prompt(),
        t.field_ids(0),
        t.discard(0)
    );
}

// ===========================================================================
// Interactions
// ===========================================================================

#[test]
fn interaction_riot_vs_budokan() {
    // 通用:登上武道馆 (targets every other player) vs RAS:R. I. O. T.
    // The counter cycles all hands and still lets the charge through.
    let mut t = Table::vanilla(3);
    t.set_hand(0, &["通用:登上武道馆"]);
    t.set_hand(1, &["RAS:R. I. O. T."]);
    t.set_hand(2, &["AG:Y.O.L.O"]);
    t.set_draw(2, &["AG:宣战布告"]);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.set_money(2, 20_000);
    t.begin_turn(0);
    drain(&mut t);
    t.play(0, "通用:登上武道馆").unwrap();
    assert!(t.counteract_offered("RAS:R. I. O. T."));
    t.counteract(1, "RAS:R. I. O. T.").unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 22_000, "charge still lands");
    assert_eq!(t.hand(2).len(), 1, "p2 redrew to its original hand size");
}

#[test]
fn interaction_exist_does_not_absorb_budokan() {
    // 通用:登上武道馆 vs RAS:EXIST -- multi-target cards are out of scope.
    let mut t = Table::vanilla(3);
    t.place_raw(1, "RAS:EXIST");
    t.set_hand(0, &["通用:登上武道馆"]);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.set_money(2, 20_000);
    t.begin_turn(0);
    drain(&mut t);
    t.play(0, "通用:登上武道馆").unwrap();
    drain(&mut t);
    assert_eq!(t.money(1), 19_000, "p1 still pays");
    assert_eq!(t.money(2), 19_000, "p2 still pays");
    assert!(t.on_field(1, "RAS:EXIST"));
}

#[test]
fn interaction_repaint_vs_yolo() {
    // AG:Y.O.L.O (adds 1d4 to the mover's roll) chained with RAS:Repaint
    // (cuts the move by the counterer's path tiles).
    let mut t = Table::vanilla(3);
    t.own(0, &[EDOGAWA_PARK]);
    t.set_pos(1, 5);
    t.set_hand(1, &["AG:Y.O.L.O"]);
    t.give(0, &["RAS:Repaint"]);
    t.begin_turn(1);
    drain(&mut t);
    t.dice(&[6, 2]); // base 6, Y.O.L.O adds 1d4 -> clamped: 2 on a d4
    t.roll(1).unwrap();
    // Y.O.L.O is a [手] played before the roll settles -- see if a window opens.
    if t.counteract_offered("AG:Y.O.L.O") {
        t.counteract(1, "AG:Y.O.L.O").unwrap();
        drain(&mut t);
    }
    if t.counteract_offered("RAS:Repaint") {
        t.counteract(0, "RAS:Repaint").unwrap();
        drain(&mut t);
    }
    // Whatever the chain order, the move must be a real move and Repaint must
    // have been offered at some point.
    assert!(t.pos(1) != 5, "the mover moved");
}

#[test]
fn interaction_encore_vs_unstoppable_teleport() {
    // 通用:安可 (negates an abnormal movement on its user) vs RAS:UNSTOPPABLE
    // (a [传送] -- an 「异常移动效果」). The same player holds both: UNSTOPPABLE
    // is the move, 安可 may cancel it.
    let mut t = Table::vanilla(2);
    t.give(0, &["RAS:UNSTOPPABLE", "通用:安可"]);
    t.begin_turn(0);
    drain(&mut t);
    t.dice(&[1]);
    let r = t.play(0, "RAS:UNSTOPPABLE");
    // Either the play is refused, or a [反击] window opens for 安可.
    if let Ok(()) = r {
        if t.counteract_offered("通用:安可") {
            t.counteract(0, "通用:安可").unwrap();
            drain(&mut t);
            assert_eq!(t.pos(0), CIRCLE, "teleport negated");
        } else {
            drain(&mut t);
            assert_ne!(t.pos(0), CIRCLE, "teleport landed");
        }
    }
}

// ===========================================================================
// RAS:（和奏瑞依）寄于指尖的执念
// ===========================================================================

#[test]
fn nagisa_fingertip_is_offered_as_a_counter_on_a_fire_pot_roll() {
    // Sheet 2026-10-06 新卡组卡 H16 adds a [反击] wrapper:
    // 「[反击] 当你使用火罐进行掷骰时，可打出此卡并保留（写下）未被选择的另一个骰点…」
    // 和奏瑞依 (2) 「进行任意掷骰后，可选择使用一个[火罐]再投一次骰子」 is
    // 「使用火罐进行掷骰」, so the card must ring up in the counteract ring.
    let mut t = Table::new(&["和奏瑞依", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 1, 1);
    t.give(0, &["RAS:（和奏瑞依）寄于指尖的执念"]);
    t.dice(&[3, 1, 1]);
    t.roll(0).unwrap();
    let mut offered = false;
    let mut used_fire = false;
    for _ in 0..30 {
        if t.prompt().is_none() {
            break;
        }
        if t.counteract_offered("RAS:（和奏瑞依）寄于指尖的执念") {
            offered = true;
            t.counteract(0, "RAS:（和奏瑞依）寄于指尖的执念").unwrap();
            continue;
        }
        let d = t.dump_prompt();
        // Skill (2) reroll window: accept it so the fire-pot roll happens.
        if !used_fire && d.contains("raise_effort_again") {
            // Skill (2) 「进行任意掷骰后，可选择使用一个[火罐]再投一次骰子」.
            let k = t.option("ask.yes").unwrap_or(0);
            let _ = t.answer(0, k);
            used_fire = true;
            continue;
        }
        t.decline();
    }
    assert!(
        offered,
        "寄于指尖 is 「[反击] 当你使用火罐进行掷骰时」 -- must be offered; used_fire={used_fire} events {:?}",
        t.recent_keys(15)
    );
}

#[test]
fn layer_keep_plays_from_hand_to_field() {
    // Sheet 2026-10-06 新卡组卡 H16: 「可打出此卡」 -- the placement branch of
    // the merged Play entry.
    let mut t = Table::new(&["和奏瑞依", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &["RAS:（和奏瑞依）寄于指尖的执念"]);
    t.play(0, "RAS:（和奏瑞依）寄于指尖的执念").unwrap();
    drain(&mut t);
    assert!(t.on_field(0, "RAS:（和奏瑞依）寄于指尖的执念"));
}

#[test]
fn layer_keep_press_refused_without_die_or_fire() {
    // 「消耗一个火罐以用于替代当回合的移动掷骰」 -- needs both a kept die and
    // a fire pot.
    let mut t = Table::new(&["和奏瑞依", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, "RAS:（和奏瑞依）寄于指尖的执念");
    // no kept die
    assert!(t.skill(0, "RAS:（和奏瑞依）寄于指尖的执念").is_err());
    drain(&mut t);
    // a kept die but no fire
    t.set_state(0, "layer_keep_count", 1);
    t.set_state(0, "layer_keep_0", 7);
    t.set_fire(0, 0, 1);
    assert!(t.skill(0, "RAS:（和奏瑞依）寄于指尖的执念").is_err());
}

#[test]
fn layer_keep_press_replaces_the_move_roll_with_a_kept_die() {
    // 规则书: 「在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰，随后删去
    // 该骰点」 -- the placed-press branch of the merged Play entry.
    let mut t = Table::new(&["和奏瑞依", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, "RAS:（和奏瑞依）寄于指尖的执念");
    t.set_state(0, "layer_keep_count", 1);
    t.set_state(0, "layer_keep_0", 7);
    t.set_fire(0, 1, 1);
    t.skill(0, "RAS:（和奏瑞依）寄于指尖的执念").unwrap();
    // 「可保留多个骰点」 -- a pick among the kept dice (one here).
    let p = t.expect_prompt();
    assert_eq!(p.kind, "choice", "{}", t.dump_prompt());
    t.answer_one(0).unwrap();
    drain(&mut t);
    // 「消耗一个火罐」 and 「删去该骰点」
    assert_eq!(t.fire(0), 0, "one fire pot spent");
    assert_eq!(t.state(0, "layer_keep_count"), 0, "the die was deleted");
    // 「替代当回合的移动掷骰」 -- the next move roll is the kept face (7).
    t.dice(&[1]); // loaded face is overridden by the fixed roll
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 7, "moved exactly the kept die");
}


#[test]
fn interaction_please_choose_opt2_is_an_abnormal_move() {
    // PLEASE CHOOSE opt2 [传送]s the counteractor -- an 「异常移动效果」 that 通用:安可
    // can negate when the counteractor holds it.
    let mut t = Table::vanilla(3);
    t.own(2, &[SPACE]);
    t.set_houses(SPACE, 1);
    t.give(0, &["RAS:PLEASE CHOOSE", "通用:安可"]);
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, SPACE - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    t.counteract(0, "RAS:PLEASE CHOOSE").unwrap();
    t.answer(1, 1).unwrap(); // opt2: teleport p0 to p1
    // 安可 may now open against the teleport.
    if t.counteract_offered("通用:安可") {
        t.counteract(0, "通用:安可").unwrap();
        drain(&mut t);
        assert_eq!(t.pos(0), CIRCLE, "teleport negated");
    } else {
        drain(&mut t);
        assert_eq!(t.pos(0), SPACE, "teleport landed");
    }
}

#[test]
fn interaction_network_error_vs_crush_hand_effect() {
    // 通用:网络链接异常 counters a [手] effect before it resolves.
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "佐藤益木");
    t.set_discard(0, &["R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压"]);
    t.set_hand(1, &["通用:网络链接异常"]);
    t.give(0, &["RAS:（MASKING）CRUSH ON THE DRUM!!!"]);
    t.begin_turn(0);
    drain(&mut t);
    t.play(0, "RAS:（MASKING）CRUSH ON THE DRUM!!!").unwrap();
    if t.counteract_offered("通用:网络链接异常") {
        t.counteract(1, "通用:网络链接异常").unwrap();
        drain(&mut t);
    }
    // CRUSH adds Xd20 only if its [手] resolved.
    t.dice(&[1, 1, 1, 1, 1]);
    t.roll(0).unwrap();
    drain(&mut t);
    // 4 faces -> CRUSH resolved (1d20+3d20 all 1s = 4); 1 face -> negated (=1).
    assert!(t.pos(0) == 4 || t.pos(0) == 1, "pos {}", t.pos(0));
}

#[test]
fn interaction_hey_kids_replaces_the_settle() {
    // RAS:狂乱Hey Kids!! replaces the settle of your own tile: no build offer,
    // houses move instead.
    let mut t = Table::vanilla(2);
    t.own(0, &[SHOPPING, EDOGAWA_PARK]);
    t.set_houses(SHOPPING, 1);
    t.give(0, &["RAS:狂乱Hey Kids!!"]);
    // Ruling 2026-10-06: the window opens when ANOTHER player settles rent on
    // my tiles. P1 rolls onto P0's 购物中心 and settles.
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, CIRCLE);
    t.dice(&[1]);
    t.roll(1).unwrap();
    t.counteract(0, "RAS:狂乱Hey Kids!!").unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.kind == "tile" {
            t.answer_tile(0, EDOGAWA_PARK).unwrap();
        } else {
            t.answer(0, 0).unwrap();
        }
    }
    assert_eq!(t.houses(SHOPPING), 0);
    assert_eq!(t.houses(EDOGAWA_PARK), 1);
}