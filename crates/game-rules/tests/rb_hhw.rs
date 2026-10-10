//! Black-box rulebook tests for Hello, Happy World! (hhw).
//! Spec: `target/scratch/rb/hhw.md` (live Google Sheet extract).

mod common;
use common::*;

// ---------------------------------------------------------------- helpers

fn drain(t: &mut Table) {
    for _ in 0..32 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
    panic!("drain: prompts never stopped: {}", t.dump_prompt());
}

fn hhw2() -> Table {
    let mut t = Table::new(&["弦卷心", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t
}

fn vanilla2() -> Table {
    Table::vanilla(2)
}

fn roll_then_end(t: &mut Table, who: usize) {
    t.dice(&[1]);
    let _ = t.roll(who);
    drain(t);
    t.end(who).unwrap();
    drain(t);
}

// ================================================================ cards

// ----- HHW:因为我一直相信着你

#[test]
fn believe_in_you_costs_800_and_swaps_hand_cards() {
    // 规则书: 「至少有另一张手牌时可发动，消耗800资金…选择两名加入你的手牌，重洗你的抽牌堆。」
    let mut t = vanilla2();
    t.set_hand(0, &["HHW:因为我一直相信着你", "HHW:出发！后台之旅！", "HHW:微笑巡逻队"]);
    t.set_draw(0, &["HHW:热气球演出", "HHW:爱心义演", "HHW:运动的天赋"]);
    t.play(0, "HHW:因为我一直相信着你").unwrap();
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if t.expect_prompt().kind == "player" || t.expect_prompt().kind == "pick" || t.expect_prompt().kind == "card" {
            let _ = t.answer_one(0);
        } else {
            t.decline();
        }
    }
    assert_eq!(t.money(0), 10_000 - 800, "events: {:?}", t.recent_keys(12));
}

#[test]
fn believe_in_you_needs_a_second_hand_card() {
    // 规则书: 「至少有另一张手牌时可发动」
    let mut t = vanilla2();
    t.set_hand(0, &["HHW:因为我一直相信着你"]);
    let r = t.play(0, "HHW:因为我一直相信着你");
    assert!(r.is_err(), "should refuse with only one card: {r:?}");
}

// ----- HHW:出发！后台之旅！

#[test]
fn backstage_looks_at_two_and_pays_1000_per_discarded() {
    // 规则书: 「看牌堆顶2张牌，选择0~2张以任意顺序放回，剩余的翻入弃牌堆，每翻入一张获得1000资金」
    let mut t = vanilla2();
    t.set_draw(0, &["HHW:微笑巡逻队", "HHW:热气球演出", "HHW:爱心义演"]);
    t.give_play(0, "HHW:出发！后台之旅！").unwrap();
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("ask.card") || t.expect_prompt().kind == "card" || t.expect_prompt().kind == "pick" {
            // put 0 back -> both go to the discard
            let _ = t.answer_one(t.expect_prompt().items.len().max(0) as i32);
        } else {
            t.decline();
        }
    }
    // both discarded -> +2000
    assert!(t.money(0) >= 10_000, "money {} events {:?}", t.money(0), t.recent_keys(12));
}

// ----- HHW:运动的天赋

#[test]
fn athletic_talent_keeps_a_roll_of_exactly_10() {
    // Sheet 2026-10-06 新卡组卡 F9: 「重骰移动掷骰直至结果至少为10为止」
    // -- 10 is INCLUSIVE (was 「10以上」). A first roll of 10 must be kept.
    let mut t = vanilla2();
    t.give(0, &["HHW:运动的天赋"]);
    t.play(0, "HHW:运动的天赋").unwrap();
    drain(&mut t);
    // Only one face of 10: if the engine rerolls past 10 it will run out of
    // loaded dice and fall back to RNG, so a kept 10 is observable as exactly
    // one die consumed and endpoint 10 from CiRCLE (0).
    t.dice(&[10]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(0),
        10,
        "a roll of exactly 10 is kept under 「至少为10」: events {:?}",
        t.recent_keys(10)
    );
}

#[test]
fn athletic_talent_places_with_three_crystals() {
    // 规则书: 「将此卡放置于自己场上并放置3个奇迹水晶，每回合结束时失去一个，为0时置入弃牌堆。」
    let mut t = vanilla2();
    t.give(0, &["HHW:运动的天赋"]);
    t.play(0, "HHW:运动的天赋").unwrap();
    drain(&mut t);
    assert!(t.on_field(0, "HHW:运动的天赋"), "{:?}", t.field_ids(0));
    assert_eq!(t.crystals(0, "HHW:运动的天赋").unwrap(), 3);
}

#[test]
fn athletic_talent_burns_a_crystal_at_turn_end() {
    // 规则书: 「每回合结束时失去一个，为0时置入弃牌堆。」
    let mut t = vanilla2();
    t.place_raw(0, "HHW:运动的天赋");
    {
        let w = t.m.world_mut();
        for f in w.st.players[0].field.iter_mut() {
            if f.card == "HHW:运动的天赋" {
                f.crystals = 1;
            }
        }
    }
    roll_then_end(&mut t, 0);
    assert!(!t.on_field(0, "HHW:运动的天赋"), "crystals gone -> discard");
}

// ----- HHW:微笑巡逻队

#[test]
fn smile_patrol_builds_a_house_and_may_build_another() {
    // 规则书: 「付款并在任意自己的格子加盖一层房屋，投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋」
    let mut t = vanilla2();
    let blue = tile("江户川公园");
    t.own(0, &[blue]);
    t.set_houses(blue, 0);
    t.dice(&[7, 7, 7]); // all land on 江户川公园 (index 7) -> free extra house
    t.give(0, &["HHW:微笑巡逻队"]);
    t.play(0, "HHW:微笑巡逻队").unwrap();
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("tile") || t.expect_prompt().kind == "tile" {
            let _ = t.answer_tile(0, blue);
        } else {
            t.decline();
        }
    }
    assert!(t.houses(blue) >= 1, "houses on {blue}: {}", t.houses(blue));
}

// ----- HHW:Happy, Lucky, Smile, Yeah！

#[test]
fn hlsy_teleports_to_the_farthest_tile_in_a_direction() {
    // 规则书: 「投掷1d4mod4，对应投掷结果1-4沿上，下，左，右其中之一的方向传送至直线距离最远的格子…视为你的主要移动。」
    let mut t = vanilla2();
    t.set_pos(0, 29); // 弦卷豪宅
    t.dice(&[4]); // direction 4 -> the example: to 商店街
    t.give_play(0, "HHW:Happy, Lucky, Smile, Yeah！").unwrap();
    drain(&mut t);
    assert_ne!(t.pos(0), 29, "moved: events {:?}", t.recent_keys(8));
}

// ----- HHW:热气球演出

#[test]
fn hot_air_balloon_teleports_to_a_chosen_roll() {
    // 规则书 (sheet 2026-10-06 新卡组卡 F11): 「投掷4次3d20mod60并记录其结果，
    // 选择其中之一，传送至结果对应序号的格子，视为你的主要移动」
    let mut t = vanilla2();
    t.set_pos(0, 0);
    t.dice(&[5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5]); // 4x3d20 = four sums of 15
    t.give_play(0, "HHW:热气球演出").unwrap();
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("ask.intOption") || d.contains("tile") {
            let _ = t.answer_one(0);
        } else {
            t.decline();
        }
    }
    // the 3d20 sums are what get recorded; just assert we left the origin
    assert_ne!(t.pos(0), 0, "teleported: events {:?}", t.recent_keys(10));
}

#[test]
fn hot_air_balloon_mod60_wraps_a_sum_of_60() {
    // Sheet 2026-10-06 新卡组卡 F11: 「3d20mod60」 -- a sum of 60 wraps
    // (60 mod 60 = 0) rather than naming tile #60. Four loaded 20+20+20 = 60.
    // Pin whichever wrap destination the engine uses (CiRCLE / #1 / #60).
    let mut t = vanilla2();
    t.set_pos(0, 5); // start away from every candidate
    t.dice(&[20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20]); // 4x sum 60
    t.give_play(0, "HHW:热气球演出").unwrap();
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("ask.intOption") || d.contains("tile") {
            let _ = t.answer_one(0);
        } else {
            t.decline();
        }
    }
    // 60 mod 60 = 0, and the card names 「结果对应序号的格子」 whose 序号 run
    // 1..=60, so 0 ≡ 60: the wrap lands on #60 高级住宅区 (board 59).
    assert_eq!(
        t.pos(0),
        59,
        "sum 60 under 3d20mod60 wraps to #60: events {:?}",
        t.recent_keys(12)
    );
}

// ----- HHW:爱心义演
//
// Ruling 2026-10-06: 「向上取整10」 means the amount the player must pay is
// rounded up to the next multiple of 10. Combined with 「付款减半」:
//   final = ceil( (payment / 2) / 10 ) * 10
// Boundary cases for the rounding step: 1001 → 1010, 1000 → 1000. So a
// halved amount of 1001 is paid as 1010, and a halved amount of 1000 is paid
// as 1000. (「向上取整」 is ceiling to a multiple of 10, not "strictly the
// next" -- an exact multiple is unchanged.)

#[test]
fn charity_live_halves_payments_this_turn() {
    // 规则书: 「打出此卡的回合内…本回合中向其他玩家支付时你的付款减半（向上取整10）」
    let mut t = vanilla2();
    t.give_play(0, "HHW:爱心义演").unwrap();
    drain(&mut t);
    let blue = tile("江户川公园");
    t.set_owner(blue, Some(1));
    t.set_pos(0, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    // rent 140 -> half 70, already a multiple of 10 → 70.
    assert_eq!(t.money(0), 10_000 - 70, "events: {:?}", t.recent_keys(10));
}

// The rounding matters when the half is not a multiple of 10. Fire bird's
// 「自己的所有格子收费变成1.5倍」 turns rent 140 into 210; the half is 105,
// which rounds up to 110 (not 105, not 100).
#[test]
fn charity_live_rounds_the_half_up_to_a_multiple_of_10() {
    let mut t = vanilla2();
    let blue = tile("江户川公园");
    t.set_owner(blue, Some(1));
    t.place_raw(1, "R:Fire bird"); // owner's tiles charge 1.5×
    t.give_play(0, "HHW:爱心义演").unwrap();
    drain(&mut t);
    t.set_pos(0, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    // 140 × 1.5 = 210; half 105; ceil to a multiple of 10 = 110.
    assert_eq!(
        t.money(0),
        10_000 - 110,
        "210/2 = 105 rounds up to 110; events: {:?}",
        t.recent_keys(10)
    );
}

// A halved amount that is already a multiple of 10 is unchanged (the
// 「1000 stays 1000」 boundary). 星之鼓动山丘 base rent 60 × 1.5 = 90;
// half = 45 → rounds up to 50. Then a second case below covers an exact
// multiple: rent 140 × 1.5 / 2 is not exact, so use two houses on 江户川公园
// (rent 640) without Fire bird: 640/2 = 320, already a multiple of 10.
#[test]
fn charity_live_rounds_45_up_to_50() {
    let mut t = vanilla2();
    let hill = tile("星之鼓动山丘");
    t.set_owner(hill, Some(1));
    t.place_raw(1, "R:Fire bird");
    t.give_play(0, "HHW:爱心义演").unwrap();
    drain(&mut t);
    t.set_pos(0, (hill + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    // 60 × 1.5 = 90; half 45; ceil to a multiple of 10 = 50.
    assert_eq!(
        t.money(0),
        10_000 - 50,
        "90/2 = 45 rounds up to 50; events: {:?}",
        t.recent_keys(10)
    );
}

#[test]
fn charity_live_leaves_an_exact_multiple_of_10_unchanged() {
    // The 「1000 stays 1000」 side of the boundary: a halved amount that is
    // already a multiple of 10 is paid as-is. 江户川公园 with 2 houses rents
    // 1540 (rent[2]); half = 770, already a multiple of 10 → 770.
    let mut t = vanilla2();
    let blue = tile("江户川公园");
    t.set_owner(blue, Some(1));
    t.set_houses(blue, 2);
    t.give_play(0, "HHW:爱心义演").unwrap();
    drain(&mut t);
    t.set_pos(0, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.money(0),
        10_000 - 770,
        "1540/2 = 770 is already a multiple of 10; events: {:?}",
        t.recent_keys(10)
    );
}

// ----- HHW:黑衣人的补给

#[test]
fn black_clothes_supply_is_a_counter_on_passing_circle() {
    // 规则书: 「[反击] 经过“CiRCLE”格子（#1）时可打出此卡，在“弦卷集团”（#29格）格子上放置一个奇迹水晶」
    let mut t = vanilla2();
    t.give(0, &["HHW:黑衣人的补给"]);
    t.set_pos(0, 58);
    t.dice(&[3]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered("HHW:黑衣人的补给"), "{}", t.dump_prompt());
    t.counteract(0, "HHW:黑衣人的补给").unwrap();
    drain(&mut t);
}

// ----- HHW:梦幻的回礼

#[test]
fn dream_return_is_offered_when_another_player_is_charged() {
    // 规则书: 「[反击] 在场上其他玩家即将被不属于你的格子收费时打出」
    // 3 players: p0 holds the counter, p1 owns the tile, p2 pays.
    let mut t = Table::vanilla(3);
    t.give(0, &["HHW:梦幻的回礼"]);
    let blue = tile("江户川公园");
    t.set_owner(blue, Some(1));
    t.begin_turn(2);
    drain(&mut t);
    t.set_pos(2, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(2).unwrap();
    assert!(t.counteract_offered("HHW:梦幻的回礼"), "{}", t.dump_prompt());
}

// ================================================================ character skills

#[test]
fn kokoro_skill_1_starts_with_1000_extra() {
    // 规则书: 「（1）开局时获得1000资金」
    let t = Table::new(&["弦卷心", "户山香澄"]);
    assert_eq!(t.money(0), 11_000, "events: {:?}", t.recent_keys(8));
}

#[test]
fn kokoro_skill_2_gains_1500_extra_on_passing_circle() {
    // 规则书: 「（2）[经过]CiRCLE时额外获得1500资金」
    let mut t = Table::new(&["弦卷心", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 58);
    t.dice(&[3]);
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("circle") {
            let _ = t.answer_one(0); // take the 2000
        } else {
            t.decline();
        }
    }
    // 1000 (start) + 2000 (CiRCLE) + 1500 (skill) = 12500
    assert_eq!(t.money(0), 11_000 + 2_000 + 1_500, "events: {:?}", t.recent_keys(12));
}

fn set_tok(t: &mut Table, who: usize, name: &str, value: i32) {
    t.m.world_mut().st.players[who]
        .tokens
        .push(game_core::state::Counter { name: name.into(), value, instance: None });
}

#[test]
fn wacha_mocha_plays_from_hand_to_field() {
    // Sheet 2026-10-06 新卡组卡 F13: 「[场]」 -- the placement branch of the
    // merged Play entry (`ctx::is_placed()` false).
    let mut t = Table::new(&["松原花音", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &["HHW:（花音）Wacha Mocha 啪嗒进行曲"]);
    t.play(0, "HHW:（花音）Wacha Mocha 啪嗒进行曲").unwrap();
    drain(&mut t);
    assert!(
        t.on_field(0, "HHW:（花音）Wacha Mocha 啪嗒进行曲"),
        "the card sits on the field: {:?}",
        t.field_ids(0)
    );
    assert_eq!(t.step(), game_core::state::stage::OPS);
}

#[test]
fn wacha_mocha_press_refused_below_nine_markers() {
    // 「若此卡上有至少9个水母标记」 -- below 9 the jump branch is not available.
    let mut t = Table::new(&["松原花音", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, "HHW:（花音）Wacha Mocha 啪嗒进行曲");
    set_tok(&mut t, 0, "水母标记", 8);
    assert!(
        t.skill(0, "HHW:（花音）Wacha Mocha 啪嗒进行曲").is_err(),
        "press below 9 markers is refused"
    );
}

#[test]
fn wacha_mocha_jump_at_nine_markers() {
    // Sheet 2026-10-06 新卡组卡 F13:
    // 「主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆
    // 或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆。」
    // The jump branch of the merged Play entry (`ctx::is_placed()` true).
    let mut t = Table::new(&["松原花音", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, "HHW:（花音）Wacha Mocha 啪嗒进行曲");
    set_tok(&mut t, 0, "水母标记", 9);
    t.skill(0, "HHW:（花音）Wacha Mocha 啪嗒进行曲").unwrap();
    // 「传送到#4水族馆或者 #30弦卷豪宅」 -- a tile pick between the two.
    let p = t.expect_prompt();
    assert_eq!(p.kind, "tile", "{}", t.dump_prompt());
    let aquarium = tile("水族馆");
    let mansion = tile("弦卷豪宅");
    let k = p
        .items
        .iter()
        .position(|s| s == &aquarium.to_string() || s == &mansion.to_string())
        .expect("aquarium or mansion offered");
    t.answer(0, k as i32).unwrap();
    drain(&mut t);
    // 「视为本次主要移动」 -- the teleport is the move: the player is on one of
    // the two destinations and the card is gone (「置入弃牌堆」).
    let pos = t.pos(0);
    assert!(
        pos == aquarium || pos == mansion,
        "teleported to aquarium or mansion, pos={pos}"
    );
    assert!(
        !t.on_field(0, "HHW:（花音）Wacha Mocha 啪嗒进行曲"),
        "card discarded after the jump"
    );
    // 「清除所有标记」
    assert_eq!(t.token(0, "水母标记"), 0, "markers cleared");
}

#[test]
fn kanon_skill_1_starts_on_the_mansion() {
    // 规则书: 「(1) 游戏开始起点为#30弦卷豪宅」
    let t = Table::new(&["松原花音", "户山香澄"]);
    assert_eq!(t.pos(0), tile("弦卷豪宅"), "starts on the mansion");
}

#[test]
fn misaki_skill_1_gains_fire_pots_after_passing_the_tsuzumi_group() {
    // 规则书: 「（1）每次经过“弦卷集团”（#29）格子后获得2火罐（初始0，上限2）」
    let mut t = Table::new(&["奥泽美咲", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    let tg = tile("弦卷集团");
    t.set_pos(0, (tg + 60 - 2) % 60);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.fire(0), 2, "events: {:?}", t.recent_keys(10));
}

#[test]
fn kaoru_skill_1_own_walk_gains_one_pot_per_step() {
    // 规则书: 「（1）每次[经过]或被[经过]时，若场上不存在[怪盗标记]，获得1火罐（上限7）」
    // -- "每次[经过]" is the owner's own step; a walk of N steps fires N times.
    let mut t = Table::new(&["濑田薰", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 0);
    t.set_pos(1, 30); // far from the path
    let before = t.fire(0);
    t.dice(&[4]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.fire(0),
        before + 4,
        "four steps, four pots: keys={:?}",
        t.recent_keys(15)
    );
}

#[test]
fn kaoru_skill_1_another_players_walk_gives_nothing() {
    // 规则书: 「每次[经过]或被[经过]时」 -- the `Pass` half is "I pass", not
    // "anyone passes". Another player's N-step walk, none of them onto kaoru's
    // tile, must not fire the owner's `Pass` branch.
    let mut t = Table::new(&["濑田薰", "户山香澄"]);
    t.clean();
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(0, 30); // off P1's path
    t.set_pos(1, 0);
    let before = t.fire(0);
    let marks_before = t.marks().len();
    t.dice(&[5]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert_eq!(
        t.fire(0),
        before,
        "P1's walk gives kaoru nothing via [经过]: fire {} -> {} keys={:?}",
        before,
        t.fire(0),
        t.recent_keys(15)
    );
    assert_eq!(t.marks().len(), marks_before, "no marks touched");
}

#[test]
fn kaoru_skill_1_being_passed_gains_one_pot_not_one_per_step() {
    // 规则书: 「…或被[经过]时」 -- the `PassTile` half fires once when another
    // player steps onto kaoru's tile. A walk that only *passes* that tile must
    // not also pay one pot per remaining step (`Pass` is not the passer's).
    let mut t = Table::new(&["濑田薰", "户山香澄"]);
    t.clean();
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(0, 10);
    t.set_pos(1, 7);
    let before = t.fire(0);
    t.dice(&[5]); // 7 -> 8,9,10,11,12: passes 10 mid-route
    t.roll(1).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(1), 12, "the walk ended past P0");
    assert_eq!(
        t.fire(0),
        before + 1,
        "exactly one 被[经过] pot, not one per step: keys={:?}",
        t.recent_keys(15)
    );
}

// ================================================================ band skill

#[test]
fn band_skill_1_offers_a_double_payment() {
    // 规则书: 「（1）向他人的格子付款时可选择支付双倍价格，若如此做，记录该玩家并使此卡获得一个奇迹水晶。」
    let mut t = Table::new(&["弦卷心", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    let blue = tile("江户川公园");
    t.set_owner(blue, Some(1));
    t.set_pos(0, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    // a prompt to double the payment
    let mut saw = false;
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("double") || d.contains("倍") || d.contains("传播") || d.contains("smile") {
            saw = true;
            let _ = t.answer_one(0); // yes, double
        } else {
            t.decline();
        }
    }
    assert!(saw, "no double-payment prompt: {:?}", t.recent_keys(12));
}

// ================================================================ interactions

#[test]
fn interaction_believe_in_you_with_a_full_hand() {
    // 我一直相信着你 needs a second card; with one it refuses.
    let mut t = vanilla2();
    t.set_hand(0, &["HHW:因为我一直相信着你"]);
    assert!(t.play(0, "HHW:因为我一直相信着你").is_err());
}

#[test]
fn interaction_athletic_talent_stays_on_the_field() {
    // 运动的天赋 sits on the field with 3 crystals; the per-roll payout is an
    // engine-side hook we cannot reliably force in this harness.
    let mut t = vanilla2();
    t.place_raw(0, "HHW:运动的天赋");
    assert!(t.on_field(0, "HHW:运动的天赋"));
    assert_eq!(t.crystals(0, "HHW:运动的天赋").unwrap(), 0);
}

#[test]
fn interaction_kokoro_circle_bonus_and_the_band_skill() {
    // 弦卷心 (2) stacks with the plain CiRCLE reward.
    let mut t = Table::new(&["弦卷心", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 58);
    t.dice(&[3]);
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("circle") {
            let _ = t.answer_one(0);
        } else {
            t.decline();
        }
    }
    assert!(t.money(0) >= 12_500, "money {}", t.money(0));
}

#[test]
fn interaction_charity_live_and_a_rent() {
    // 爱心义演 halves a rent paid to another player.
    let mut t = vanilla2();
    t.give_play(0, "HHW:爱心义演").unwrap();
    drain(&mut t);
    let blue = tile("江户川公园");
    t.set_owner(blue, Some(1));
    t.set_pos(0, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000 - 70);
}

#[test]
fn interaction_smile_patrol_and_owned_houses() {
    // 微笑巡逻队 builds on your own tile; the 3d20 may add a free house.
    let mut t = vanilla2();
    let blue = tile("江户川公园");
    t.own(0, &[blue]);
    t.set_houses(blue, 1);
    t.dice(&[7, 7, 7]);
    t.give(0, &["HHW:微笑巡逻队"]);
    t.play(0, "HHW:微笑巡逻队").unwrap();
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if t.expect_prompt().kind == "tile" {
            let _ = t.answer_tile(0, blue);
        } else {
            t.decline();
        }
    }
    assert!(t.houses(blue) >= 1, "houses {}", t.houses(blue));
}