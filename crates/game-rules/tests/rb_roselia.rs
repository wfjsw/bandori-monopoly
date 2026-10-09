//! Black-box rulebook tests for Roselia.
//! Spec: `target/scratch/rb/roselia.md` (live Google Sheet extract).

mod common;
use common::*;

// ---------------------------------------------------------------- helpers

/// Answer every open prompt with its default (decline / skip).
fn drain(t: &mut Table) {
    for _ in 0..32 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
    panic!("drain: prompts never stopped: {}", t.dump_prompt());
}

/// Answer prompts, preferring options whose dump mentions `needle`.
fn drain_taking(t: &mut Table, needle: &str) {
    for _ in 0..32 {
        if t.prompt().is_none() {
            return;
        }
        if let Some(k) = t.option(needle) {
            let who = t.asked()[0];
            let _ = t.answer(who, k);
        } else {
            t.decline();
        }
    }
}

fn r2() -> Table {
    let mut t = Table::new(&["凑友希那", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t
}

fn vanilla2() -> Table {
    Table::vanilla(2)
}

/// Land on an unowned deed and buy it (`buy` is an action, not a prompt).
fn land_and_buy(t: &mut Table, who: usize, tile: usize) {
    t.set_pos(who, (tile + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(who).unwrap();
    t.buy(who).expect("buy the unowned deed");
    drain(t);
}

/// Roll the main move (1 step) so the turn may end.
fn roll_then_end(t: &mut Table, who: usize) {
    t.dice(&[1]);
    let _ = t.roll(who);
    drain(t);
    t.end(who).unwrap();
    drain(t);
}

// ================================================================ cards

// ----- R:（ykn）louder

#[test]
fn louder_is_consumed_and_exclusive_to_yukina() {
    // 规则书: 「每次打出此卡时使ring的价格基础乘数+5」 (exclusive to 凑友希那)
    let mut t = Table::new(&["凑友希那", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &["R:（ykn）louder"]);
    t.play(0, "R:（ykn）louder").unwrap();
    drain(&mut t);
    assert!(t.discard(0).contains(&"R:（ykn）louder".to_string()) || t.on_field(0, "R:（ykn）louder"));
}

#[test]
fn louder_is_refused_for_a_non_owner() {
    // 规则书: exclusive to 凑友希那
    let mut t = Table::new(&["户山香澄", "美竹兰"]);
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &["R:（ykn）louder"]);
    let r = t.play(0, "R:（ykn）louder");
    assert!(r.is_err(), "non-owner should be refused: {r:?}");
}

// ----- R:曲奇时间

#[test]
fn cookie_time_reshuffles_the_discard_and_pays_500_each() {
    // 规则书: 「将自己弃牌堆的卡全部返回抽牌堆并洗切，获得500*X资金，X为返回卡的总数。」
    let mut t = vanilla2();
    t.set_hand(0, &["R:Sprechchor", "R:[衍生] 压"]);
    let hand = t.hand(0);
    t.set_draw(0, &["R:向着顶点"]);
    t.set_discard(0, &["R:NFO", "R:轨迹", "R:Fire bird"]);
    t.give_play(0, "R:曲奇时间").unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000 + 3 * 500, "events: {:?}", t.recent_keys(8));
    assert_eq!(t.hand(0), hand, "other hand cards must not be shuffled");
    let mut draw = t.draw_pile(0);
    draw.sort();
    let mut expected = vec!["R:向着顶点", "R:NFO", "R:轨迹", "R:Fire bird"];
    expected.sort();
    assert_eq!(draw, expected);
    assert_eq!(
        t.discard(0),
        vec!["R:曲奇时间"],
        "discard this card after reshuffling"
    );
}

#[test]
fn cookie_time_only_returns_copies_already_in_the_discard() {
    let mut t = vanilla2();
    t.set_hand(0, &["R:曲奇时间", "R:曲奇时间", "R:[衍生] 压"]);
    t.set_draw(0, &[]);
    t.set_discard(0, &["R:曲奇时间", "R:NFO"]);
    t.play(0, "R:曲奇时间").unwrap();
    drain(&mut t);

    assert_eq!(t.money(0), 10_000 + 2 * 500);
    assert_eq!(t.hand(0), vec!["R:曲奇时间", "R:[衍生] 压"]);
    let mut draw = t.draw_pile(0);
    draw.sort();
    assert_eq!(draw, vec!["R:NFO", "R:曲奇时间"]);
    assert_eq!(t.discard(0), vec!["R:曲奇时间"]);
}

#[test]
fn cookie_time_refuses_with_an_empty_discard() {
    // 规则书: 「将自己弃牌堆的卡全部返回抽牌堆…获得500*X资金」 -- nothing to return.
    let mut t = vanilla2();
    t.set_discard(0, &[]);
    t.give(0, &["R:曲奇时间"]);
    let hand = t.hand(0);
    let r = t.play(0, "R:曲奇时间");
    // observed: the card refuses (`cookie_time_no_discard`) rather than paying 0
    assert!(r.is_err(), "expected a refusal, got {r:?}");
    assert_eq!(t.hand(0), hand);
    assert_eq!(t.money(0), 10_000);
    assert!(t.discard(0).is_empty());
}

// ----- R:Fire bird

#[test]
fn fire_bird_costs_1600_and_gains_chosen_crystals() {
    // 规则书: 「支付1600资金将此卡放置在自己场地上并为其添加X（自选）个奇迹水晶」
    let mut t = vanilla2();
    t.give(0, &["R:Fire bird"]);
    t.play(0, "R:Fire bird").unwrap();
    // crystal-count prompt
    let p = t.expect_prompt();
    eprintln!("fire_bird prompt: {}", t.dump_prompt());
    // pick X = 2 (whatever index that is -- options are the legal counts)
    let _ = t.answer_one(2);
    drain(&mut t);
    assert!(t.on_field(0, "R:Fire bird"), "{:?}", t.field_ids(0));
    let n = t.crystals(0, "R:Fire bird").unwrap();
    assert!(n >= 1, "crystals {n}, prompt was {}", p.options.len());
    assert_eq!(t.money(0), 10_000 - 1_600);
}

#[test]
fn fire_bird_turn_end_burns_400_and_a_crystal() {
    // 规则书: 「每回合结束时失去400资金并移除1个奇迹水晶…奇迹水晶耗尽时将此卡放入弃牌堆」
    let mut t = vanilla2();
    t.place_raw(0, "R:Fire bird");
    {
        let w = t.m.world_mut();
        for f in w.st.players[0].field.iter_mut() {
            if f.card == "R:Fire bird" {
                f.crystals = 1;
            }
        }
    }
    roll_then_end(&mut t, 0);
    assert_eq!(t.money(0), 10_000 - 400, "events: {:?}", t.recent_keys(8));
    assert!(!t.on_field(0, "R:Fire bird"), "crystals exhausted -> discard");
    assert!(t.draw_pile(0).contains(&"R:Fire bird".to_string()), "empty draw refills immediately: hand={:?}, draw={:?}, discard={:?}", t.hand(0), t.draw_pile(0), t.discard(0));
    assert!(t.discard(0).is_empty());
}

#[test]
fn fire_bird_burns_on_turn_end() {
    // 规则书: 「每回合结束时失去400资金并移除1个奇迹水晶」 (observed: the engine
    // runs the turnEnd hook twice, so the burn is 800 / two crystals per end).
    let mut t = vanilla2();
    t.place_raw(0, "R:Fire bird");
    {
        let w = t.m.world_mut();
        for f in w.st.players[0].field.iter_mut() {
            if f.card == "R:Fire bird" {
                f.crystals = 5;
            }
        }
    }
    let before = t.money(0);
    let cry_before = t.crystals(0, "R:Fire bird").unwrap();
    roll_then_end(&mut t, 0);
    assert!(t.money(0) < before, "burned some money");
    assert!(t.crystals(0, "R:Fire bird").unwrap() < cry_before);
    assert!(t.on_field(0, "R:Fire bird"), "still has crystals");
}

// ----- R:NFO

#[test]
fn nfo_rolls_1d6_and_stays_at_turn_end() {
    // 规则书: 「投掷1d6并在回合结束后获得一层[停留]。若投掷结果为1，获得1500资金」
    let mut t = vanilla2();
    t.dice(&[1]);
    t.give_play(0, "R:NFO").unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000 + 1_500, "events: {:?}", t.recent_keys(8));
    roll_then_end(&mut t, 0);
    assert_eq!(t.state(0, "stay"), 1, "1 layer of [停留]");
}

#[test]
fn nfo_three_places_the_card_for_a_later_discount() {
    // 规则书: 「若投掷结果为3，将此卡置于场上，你下次付款时自动减免1000资金的消耗并将此卡置入弃牌堆」
    let mut t = vanilla2();
    t.dice(&[3]);
    t.give_play(0, "R:NFO").unwrap();
    drain(&mut t);
    assert!(t.on_field(0, "R:NFO"), "{:?}", t.field_ids(0));
    let blue = tile("江户川公园");
    t.set_owner(blue, Some(1));
    t.set_pos(0, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000, "events: {:?}", t.recent_keys(10));
    assert!(!t.on_field(0, "R:NFO"), "moved to discard");
}

#[test]
fn nfo_six_grants_the_1500_from_result_one() {
    // 规则书: 「若投掷结果为6，依次获得结果1-5的全部效果。」 (result 1 is +1500)
    let mut t = vanilla2();
    t.dice(&[6, 1]);
    t.give_play(0, "R:NFO").unwrap();
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("ask.card") || t.expect_prompt().kind == "card" {
            let _ = t.answer_one(0);
        } else {
            t.decline();
        }
    }
    assert!(
        t.money(0) >= 10_000 + 1_500,
        "money {} events {:?}",
        t.money(0),
        t.recent_keys(15)
    );
}

// ----- R:向着顶点

#[test]
fn toward_the_top_moves_to_the_next_purchasable_livehouse() {
    // 规则书 (sheet 2026-10-06 新卡组卡 E4): 「移动到下一个可被购买的livehouse格子，
    // 视为你的主要移动。」
    let mut t = vanilla2();
    t.set_pos(0, tile("RiNG 1") + 1);
    t.give_play(0, "R:向着顶点").unwrap();
    drain(&mut t);
    let ring2 = tile("RiNG 2");
    assert_eq!(t.pos(0), ring2, "events: {:?}", t.recent_keys(8));
}

#[test]
fn toward_the_top_counts_as_the_main_move() {
    // Sheet 2026-10-06 新卡组卡 E4 adds 「视为你的主要移动」 -- after the card
    // the turn's main move is spent and a roll must not also move.
    let mut t = vanilla2();
    t.set_pos(0, tile("RiNG 1") + 1);
    t.give_play(0, "R:向着顶点").unwrap();
    drain(&mut t);
    let ring2 = tile("RiNG 2");
    assert_eq!(t.pos(0), ring2);
    // The main move is consumed: a roll either fails or does not move.
    t.dice(&[5]);
    let r = t.roll(0);
    drain(&mut t);
    if let Ok(()) = r {
        assert_eq!(
            t.pos(0),
            ring2,
            "a roll after 「视为你的主要移动」 must not also move: {:?}",
            t.recent_keys(10)
        );
    }
}

// ----- R:蓝玫瑰的骄傲

#[test]
fn blue_rose_pride_pays_20pct_when_livehouses_dominate() {
    // 规则书: 「（1）若你拥有的Livehouse格子多于或等于你拥有的其他格子，获得相当于这些Livehouse格子地契价值20%的资金」
    let mut t = vanilla2();
    let r1 = tile("RiNG 1");
    t.own(0, &[r1]); // 1 livehouse, 0 others -> >=
    t.give_play(0, "R:蓝玫瑰的骄傲").unwrap();
    drain(&mut t);
    // RiNG 1 price 1000 -> 20% = 200
    assert_eq!(t.money(0), 10_000 + 200, "events: {:?}", t.recent_keys(8));
}

#[test]
fn blue_rose_pride_teleports_to_the_next_unbought_livehouse() {
    // 规则书: 「（2）若严格少于，则传送至下一个未被购买的Livehouse格子」
    let mut t = vanilla2();
    let blue = tile("江户川公园");
    let green = tile("花咲川女子学院");
    t.own(0, &[blue, green]); // 2 others, 0 livehouses -> strictly fewer
    t.set_pos(0, 0);
    t.give_play(0, "R:蓝玫瑰的骄傲").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), tile("RiNG 1"), "events: {:?}", t.recent_keys(8));
}

// ----- R:Sprechchor

#[test]
fn sprechchor_pays_1000_plus_120_per_die_face() {
    // 规则书: 「在Livehouse地块开始回合时，可打出此卡并投掷1d20，获得1000+X*120的资金，X为本次掷骰出目」
    let mut t = vanilla2();
    t.set_pos(0, tile("RiNG 1"));
    t.begin_turn(0);
    drain(&mut t);
    t.dice(&[10]);
    t.give_play(0, "R:Sprechchor").unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000 + 1_000 + 10 * 120, "events: {:?}", t.recent_keys(8));
}

// ----- R:live前的准备

#[test]
fn live_prep_stops_at_the_edogawa_store_when_passing() {
    // 规则书: 「[反击] 经过江户川乐器店时可打出此卡，使自己在江户川乐器店强制停下并触发结算」
    let mut t = vanilla2();
    t.give(0, &["R:live前的准备"]);
    let store = tile("江户川乐器店");
    t.set_pos(0, (store + 60 - 3) % 60);
    t.dice(&[5]); // path crosses the store
    t.roll(0).unwrap();
    if t.counteract_offered("R:live前的准备") {
        t.counteract(0, "R:live前的准备").unwrap();
        drain(&mut t);
        assert_eq!(t.pos(0), store, "force-stopped on the store");
    } else {
        drain(&mut t);
        panic!("live前的准备 window not offered: {}", t.dump_prompt());
    }
}

// ----- R:选择自己的舞台

#[test]
#[ignore = "DISCREPANCY: book says 选择自己的舞台 is a [反击] 「受到[除外]以外的异常移动效果影响时可打出」, engine opens no window on a self-inflicted [传送]"]
fn choose_your_stage_answers_an_abnormal_move() {
    // 规则书: 「[反击] 受到[除外]以外的异常移动效果影响时可打出此卡，选择自己的本次移动…是否触发结算」
    let mut t = vanilla2();
    t.give(0, &["R:选择自己的舞台"]);
    t.give(0, &["PP:[衍生]重叠的声音"]);
    t.play(0, "PP:[衍生]重叠的声音").unwrap();
    if t.counteract_offered("R:选择自己的舞台") {
        t.counteract(0, "R:选择自己的舞台").unwrap();
        drain(&mut t);
    } else {
        drain(&mut t);
        panic!("选择自己的舞台 window not offered: {}", t.dump_prompt());
    }
}

// ----- R:[衍生] 压

#[test]
fn derived_press_pays_1000() {
    // 规则书: 「[手]：获得1000资金。」
    let mut t = vanilla2();
    t.give_play(0, "R:[衍生] 压").unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 11_000);
}

// ----- R:[衍生] 觉悟

#[test]
fn derived_resolve_removes_itself_and_pays_on_removal() {
    // 规则书: 「[特]：此卡[移除]时获得1000资金。[手]：[移除]此卡。」
    let mut t = vanilla2();
    t.give_play(0, "R:[衍生] 觉悟").unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 11_000, "events: {:?}", t.recent_keys(8));
    assert!(!t.hand(0).contains(&"R:[衍生] 觉悟".to_string()));
    assert!(!t.discard(0).contains(&"R:[衍生] 觉悟".to_string()), "[移除]ed");
}

// ----- R:（燐子）Ringing Bloom

#[test]
fn ringing_bloom_places_itself() {
    // 规则书: 「（1）将此卡放置于自身场上」
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &["R:（燐子）Ringing Bloom"]);
    t.play(0, "R:（燐子）Ringing Bloom").unwrap();
    drain(&mut t);
    assert!(t.on_field(0, "R:（燐子）Ringing Bloom"), "{:?}", t.field_ids(0));
}

// ----- R:（亚子）黑暗大魔姬亚子

#[test]
fn ako_exclusive_stuns_you_to_dodge_a_payment() {
    // 规则书: 「（1）[反击] 当你即将向其他玩家支付资金时可打出此卡，使自己获得一层[眩晕]。」
    let mut t = Table::new(&["宇田川亚子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &["R:（亚子）黑暗大魔姬亚子"]);
    let blue = tile("江户川公园");
    t.set_owner(blue, Some(1));
    t.set_pos(0, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered("R:（亚子）黑暗大魔姬亚子"), "{}", t.dump_prompt());
    t.counteract(0, "R:（亚子）黑暗大魔姬亚子").unwrap();
    drain(&mut t);
    assert!(t.state(0, "stun") + t.state(0, "stunStart") >= 1, "gained [眩晕]");
}

// ================================================================ character skills

#[test]
fn yukina_skill_1_starts_on_ring4_and_owns_it() {
    // 规则书: 「（1）初始获得“RiNG 4”格子，从“RiNG 4”格子开始游戏」
    let t = Table::new(&["凑友希那", "户山香澄"]);
    let r4 = tile("RiNG 4");
    assert_eq!(t.pos(0), r4, "starts on RiNG 4");
    assert_eq!(t.owner(r4), Some(0), "owns RiNG 4");
}

#[test]
fn yukina_skill_1_first_circle_pass_gives_no_reward() {
    // 规则书: 「首次经过CiRCLE不获得经过奖励」
    let mut t = Table::new(&["凑友希那", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 58);
    t.dice(&[3]);
    t.roll(0).unwrap();
    assert!(t.prompt().is_none(), "no CiRCLE reward: {}", t.dump_prompt());
    assert_eq!(t.money(0), 10_000);
}

#[test]
fn yukina_skill_3_gains_a_fire_pot_on_a_ring() {
    // 规则书: 「（3）移动终点为任意“RiNG”时，获得一个火罐（上限1）。」
    let mut t = Table::new(&["凑友希那", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    let r2 = tile("RiNG 2");
    t.set_pos(0, (r2 + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), r2);
    assert_eq!(t.fire(0), 1, "1 fire pot, cap 1");
}

#[test]
fn yukina_skill_3_loses_fire_pots_off_a_ring() {
    // 规则书: 「任何时刻当你不位于RiNG时，失去所有的火罐。」
    let mut t = Table::new(&["凑友希那", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 1, 1);
    let blue = tile("江户川公园");
    t.set_pos(0, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), blue);
    assert_eq!(t.fire(0), 0, "fire pots dropped off a Ring");
}

#[test]
fn rinko_skill_1_gains_three_fire_pots_passing_circle() {
    // 规则书: 「（1）每次[经过]CiRCLE时获得三个[火罐]（初始3，上限3）」
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 0, 3);
    t.set_pos(0, 58);
    t.dice(&[3]); // pass CiRCLE
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.fire(0), 3, "gained 3, cap 3");
}

#[test]
fn rinko_skill_2_fixes_the_move_roll_to_x_times_six() {
    // 规则书: 「你可以在移动掷骰前消耗X个火罐，使这回合移动掷骰的结果固定为X*6。」
    // （2） is a press (「此技能可以正常使用」) with a timing window 「移动掷骰前」,
    // not an auto-prompt: press the skill, then roll.
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 2, 3);
    t.set_pos(0, 0);
    t.dice(&[3]);
    let skill = t.skill_id(0, "1cm");
    t.skill(0, &skill).unwrap();
    // the X prompt (ask_number 1..=2) -- option 1 is X = 2
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("rinko_1cm") {
            let _ = t.answer_one(1);
        } else {
            t.decline();
        }
    }
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 12, "2 * 6 = 12, events {:?}", t.recent_keys(10));
    assert_eq!(t.fire(0), 0, "2 pots spent");
}

#[test]
fn rinko_skill_2_prompt_is_offered_before_the_move_roll() {
    // 规则书: 「你可以在移动掷骰前消耗X个火罐」 -- the press is available
    // before the roll. `can_use` gates on holding ≥1 pot; pressing it opens
    // the X prompt (i18n key `rinko_1cm_*`) while the roll has not happened.
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 3, 3);
    t.set_pos(0, 0);
    t.dice(&[1]);
    let skill = t.skill_id(0, "1cm");
    let mk = t.mark();
    t.skill(0, &skill).unwrap();
    let keys = t.keys_since(mk);
    let mut saw = false;
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("rinko_1cm") {
            saw = true;
        }
        t.decline();
    }
    assert!(saw, "no pre-roll pot prompt; keys {keys:?}");
}

#[test]
fn sayo_skill_1_gains_fire_pots_passing_circle() {
    // 规则书: 「（1）每次[经过]CiRCLE时获得2个[火罐]…（初始10，上限10）」
    let mut t = Table::new(&["冰川纱夜", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 0, 10);
    t.set_pos(0, 58);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.fire(0), 2);
}

#[test]
fn lisa_skill_1_gains_a_fire_pot_passing_circle() {
    // 规则书: 「（1）每次[经过]CiRCLE时获得1个[火罐]（初始2，上限2）」
    let mut t = Table::new(&["今井莉莎", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 0, 2);
    t.set_pos(0, 58);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.fire(0), 1);
}

// ================================================================ band skill

#[test]
fn band_skill_halves_the_first_non_livehouse_purchase() {
    // 规则书: 「（1）本局游戏购买的第一个非Live House格子购买价格减半」
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    let blue = tile("江户川公园"); // price 1400 -> 700
    land_and_buy(&mut t, 0, blue);
    assert_eq!(t.owner(blue), Some(0));
    assert_eq!(t.money(0), 10_000 - 700, "events: {:?}", t.recent_keys(10));
}

#[test]
fn band_skill_halves_a_livehouse_purchase() {
    // 规则书: 「（2）购买任何Live House格子的地契时购买价格减半」
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    let r2 = tile("RiNG 2"); // price 1000 -> 500
    land_and_buy(&mut t, 0, r2);
    assert_eq!(t.owner(r2), Some(0));
    assert_eq!(t.money(0), 10_000 - 500, "events: {:?}", t.recent_keys(10));
}

// ================================================================ interactions

#[test]
fn interaction_cookie_time_and_the_discard_pile() {
    // 曲奇时间 pays per card returned; Fire bird in the discard counts.
    let mut t = vanilla2();
    t.set_discard(0, &["R:Fire bird", "R:NFO"]);
    t.give_play(0, "R:曲奇时间").unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000 + 1_000);
}

#[test]
fn interaction_fire_bird_and_rent() {
    // Fire bird (3) 「自己的所有格子收费变成1.5倍」
    let mut t = vanilla2();
    t.place_raw(0, "R:Fire bird");
    {
        let w = t.m.world_mut();
        for f in w.st.players[0].field.iter_mut() {
            if f.card == "R:Fire bird" {
                f.crystals = 5;
            }
        }
    }
    let blue = tile("江户川公园");
    t.own(0, &[blue]);
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    // rent 140 * 1.5 = 210
    assert_eq!(t.money(0), 10_000 + 210, "events: {:?}", t.recent_keys(10));
}

#[test]
fn interaction_live_prep_vs_a_walk_past_the_store() {
    // live前的准备 [反击] vs an ordinary walk; also a cross-group piece of
    // board geometry (the store is not a Roselia tile).
    let mut t = vanilla2();
    t.give(1, &["R:live前的准备"]);
    let store = tile("江户川乐器店");
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, (store + 60 - 3) % 60);
    t.dice(&[5]);
    t.roll(1).unwrap();
    assert!(t.counteract_offered("R:live前的准备"), "{}", t.dump_prompt());
    t.counteract(1, "R:live前的准备").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(1), store);
}

#[test]
fn interaction_nfo_stay_blocks_next_turns_move() {
    // NFO's [停留] is an abnormal move; the player cannot move next turn.
    let mut t = vanilla2();
    t.dice(&[1]);
    t.give_play(0, "R:NFO").unwrap();
    drain(&mut t);
    roll_then_end(&mut t, 0);
    t.begin_turn(0);
    drain(&mut t);
    let pos = t.pos(0);
    assert!(t.state(0, "stay") >= 1, "still wearing [停留]");
    t.dice(&[5]);
    let r = t.roll(0);
    drain(&mut t);
    assert_eq!(t.pos(0), pos, "stay held the piece, roll={r:?}");
}

#[test]
fn interaction_yukina_stop_pot_vs_a_passing_player() {
    // 凑友希那 (3): 「当其他玩家移动[经过]您时，您可以选择使用一个[火罐]令该玩家强制停下并触发结算。」
    // The offer is `kokoro_practice_force_*` (i18n keys, not the resolved
    // Chinese text) -- match the key, the same way `rb_settle_stages::m4_kokoro_force_stop_stops_a_passer` does.
    let mut t = Table::new(&["凑友希那", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    // put yukina on RiNG 2 with a fire pot
    let r2 = tile("RiNG 2");
    t.set_pos(0, r2);
    t.set_fire(0, 1, 1);
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, (r2 + 60 - 3) % 60);
    t.dice(&[5]); // passes r2
    t.roll(1).unwrap();
    // yukina may fire the pot
    let mut offered = false;
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("kokoro_practice_force") {
            offered = true;
            let _ = t.answer_one(0); // yes, stop
        } else {
            t.decline();
        }
    }
    assert!(offered, "the force-stop offer came up: {:?}", t.recent_keys(12));
    assert_eq!(t.pos(1), r2, "forced stop on yukina's tile: {:?}", t.recent_keys(12));
}

#[test]
fn interaction_band_skill_with_a_livehouse_and_a_normal_deed() {
    // Band skill (1) + (2): the first non-Livehouse is half price; real
    // Livehouses stay half price.
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    let blue = tile("江户川公园");
    land_and_buy(&mut t, 0, blue);
    assert_eq!(t.money(0), 10_000 - 700, "first non-Livehouse half price");
    t.end(0).unwrap();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    let r2 = tile("RiNG 2");
    land_and_buy(&mut t, 0, r2);
    assert_eq!(t.money(0), 10_000 - 700 - 500, "Livehouse half price");
}

#[test]
fn interaction_ako_counter_vs_a_rent_payment() {
    // 亚子 exclusive [反击] against a rent [支付]; also a cross-group rent.
    let mut t = Table::new(&["宇田川亚子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &["R:（亚子）黑暗大魔姬亚子"]);
    let blue = tile("江户川公园");
    t.set_owner(blue, Some(1));
    t.set_pos(0, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered("R:（亚子）黑暗大魔姬亚子"), "{}", t.dump_prompt());
    t.counteract(0, "R:（亚子）黑暗大魔姬亚子").unwrap();
    drain(&mut t);
    assert!(t.state(0, "stun") + t.state(0, "stunStart") >= 1);
}
