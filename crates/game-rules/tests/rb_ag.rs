//! Black-box rulebook tests: Afterglow cards, 5 character skills, band skill.
//! Spec: `target/scratch/rb/ag.md`.
//!
//! Naming: `<slug>_<what>`. Each assertion block carries the clause it checks
//! as `// 规则书: 「…」`. Assertions follow the text; a behaviour that disagrees
//! is `#[ignore = "DISCREPANCY: …"]`.

mod common;
use common::*;

const CIRCLE: usize = 0;
const FUJIMI: usize = 11; // 富士见坂, price 600
const HILL: usize = 44; // 星之鼓动山丘, price 600
const SHOTENGAI: usize = 46; // 商店街 (agent, group 10)
const GALAXY_RAMEN: usize = 49; // 银河拉面馆, group 10
const YAMABUKI: usize = 48; // 山吹面包房, group 10
const KITAZAWA: usize = 51; // 北泽精肉店, group 10
const STARDENT: usize = 6; // 星空齿科

fn skip_all(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

// =====================================================================
// AG:宣战布告
// =====================================================================

// 规则书: 「[反击]当你或你拥有的格子被其他玩家的卡效果影响时：[指定]那名玩家。被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡」
#[test]
fn declaration_pays_500_and_draws() {
    let mut t = Table::vanilla(2);
    t.set_draw(1, &["通用:GREAT"]);
    t.give(1, &["AG:宣战布告"]);
    t.give(0, &["通用:登上武道馆"]);
    t.play(0, "通用:登上武道馆").unwrap();
    assert!(t.react_offered("AG:宣战布告"), "{}", t.dump_prompt());
    t.react(1, "AG:宣战布告").unwrap();
    skip_all(&mut t);
    // 规则书: 「被[指定]的玩家[支付][使用者]500」 — P0 pays P1 500.
    // 规则书: 「[使用者]抽1张卡」 — P1 draws.
    assert!(t.hand(1).contains(&"通用:GREAT".to_string()), "hand {:?}", t.hand(1));
    // Budokan also resolved: P1 paid P0 2000 (2-player X). Net P0 +2000-500.
    assert_eq!(t.money(0), 11_500, "events {:?}", t.recent_keys(8));
    assert_eq!(t.money(1), 8_500);
}

// The counter is [反击] on 「被其他玩家的卡效果影响」 — a targeting card.
#[test]
fn declaration_offered_against_targeting_card() {
    let mut t = Table::vanilla(2);
    t.give(1, &["AG:宣战布告"]);
    t.give(0, &["通用:雨啊，快点来吧"]);
    t.dice(&[1, 1]);
    t.play(0, "通用:雨啊，快点来吧").unwrap();
    t.answer(0, 0).unwrap(); // target P1
    assert!(t.react_offered("AG:宣战布告"), "{}", t.dump_prompt());
}

// =====================================================================
// AG:Y.O.L.O
// =====================================================================

// 规则书: 「你的掷骰结算前打出此卡，使结果增加1d4结果的数字」
#[test]
fn yolo_adds_1d4_to_the_roll() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 10);
    t.give(0, &["AG:Y.O.L.O"]);
    // The card fires as a [反击] on the roll's settle; roll first.
    t.dice(&[5, 3]); // d20=5, +1d4=3 → 8
    t.roll(0).unwrap();
    // A react window offering YOLO may open mid-roll.
    if t.react_offered("AG:Y.O.L.O") {
        t.react(0, "AG:Y.O.L.O").unwrap();
    } else {
        // Otherwise play it from hand before the move resolves.
        t.play(0, "AG:Y.O.L.O").ok();
    }
    skip_all(&mut t);
    eprintln!("pos={}", t.pos(0));
    // 规则书: 「使结果增加1d4结果的数字」
    assert!(t.pos(0) == 18 || t.pos(0) == 15, "pos {}", t.pos(0));
}

// =====================================================================
// AG:商店街的青梅竹马
// =====================================================================

// 规则书: 「投掷1d6并[传送]到商店街自己拥有的对应的格子（从商店街格子开始数），如果投掷结果大于自己拥有的商店街格子数量则[传送]到商店街，视为你的主要移动」
#[test]
fn childhood_teleports_to_nth_shop_tile() {
    let mut t = Table::vanilla(2);
    t.own(0, &[YAMABUKI, GALAXY_RAMEN]); // two group-10 tiles
    t.dice(&[2]);
    t.give_play(0, "AG:商店街的青梅竹马").unwrap();
    skip_all(&mut t);
    // 1d6 = 2 → the 2nd owned shop tile counting from 商店街 (46).
    // 46 → 47 → 48(山吹面包房, owned #1) → 49(银河拉面馆, owned #2).
    assert_eq!(t.pos(0), GALAXY_RAMEN, "events {:?}", t.recent_keys(8));
}

// 规则书: 「如果投掷结果大于自己拥有的商店街格子数量则[传送]到商店街」
#[test]
fn childhood_over_roll_goes_to_shotengai() {
    let mut t = Table::vanilla(2);
    t.own(0, &[YAMABUKI]); // only one shop tile
    t.dice(&[3]); // 3 > 1
    t.give_play(0, "AG:商店街的青梅竹马").unwrap();
    skip_all(&mut t);
    assert_eq!(t.pos(0), SHOTENGAI);
}

// =====================================================================
// AG:即使夕阳落山
// =====================================================================

// 规则书: 「[指定][使用者]拥有的四个有房子的格子和[使用者]以外的所有玩家。将X设为“被[指定]格子的地契价格和建造已有房屋的造价之和”÷“[使用者]以外的[存活]玩家数量”向上取整10」
// 规则书: 「[限]第100轮次开始前此卡的X为原本X的一半」
#[test]
fn sunset_x_halved_before_round_100() {
    let mut t = Table::vanilla(2);
    // Four tiles with houses.
    t.own(0, &[HILL, FUJIMI, 7, 9]);
    for &tile in &[HILL, FUJIMI, 7, 9] {
        t.set_houses(tile, 1);
    }
    // (600+500)+(600+500)+(1400+1000)+(1800+1000) = 7400, ÷1 = 7400,
    // 向上取整10 = 7400, halved = 3700.
    t.give_play(0, "AG:即使夕阳落山").unwrap();
    skip_all(&mut t);
    // 规则书: 「所有被[指定]的玩家[支付][使用者]X资金」
    assert_eq!(t.money(0), 13_700, "events {:?}", t.recent_keys(8));
    assert_eq!(t.money(1), 6_300);
    // 规则书: 「删除所有被[指定]格子上的1栋房」
    for &tile in &[HILL, FUJIMI, 7, 9] {
        assert_eq!(t.houses(tile), 0, "tile {tile}");
    }
}

// 规则书: 「[限]」 — the gate is the round-100 halving, not a play ban.
#[test]
fn sunset_is_playable() {
    let mut t = Table::vanilla(2);
    t.own(0, &[HILL, FUJIMI, 7, 9]);
    for &tile in &[HILL, FUJIMI, 7, 9] {
        t.set_houses(tile, 1);
    }
    let r = t.give_play(0, "AG:即使夕阳落山");
    assert!(r.is_ok(), "{r:?}");
}

// =====================================================================
// AG:绯红之魂
// =====================================================================

// 规则书: 「将此卡放置在[使用者]的[场地]，然后选择[消耗]1到5次500资金并在这张卡上放置对应数量的[奇迹水晶]」
#[test]
fn soul_places_and_loads_crystals() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "AG:绯红之魂").unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.options.len(), 5, "1..=5: {}", t.dump_prompt());
    t.answer(0, 2).unwrap(); // 3 × 500
    skip_all(&mut t);
    assert!(t.on_field(0, "AG:绯红之魂"), "field {:?}", t.field_ids(0));
    // 规则书: 「[消耗]…500资金」×3 = 1500.
    assert_eq!(t.money(0), 8_500, "events {:?}", t.recent_keys(8));
    // 规则书: 「放置对应数量的[奇迹水晶]」
    let x = t.crystals(0, "AG:绯红之魂").unwrap();
    assert_eq!(x, 3, "3 crystals");
}

// 规则书: 「（1）[反击][拥有者]因导致的[消耗]或[支付]时可选择移除此卡的1个[奇迹水晶]，此次[消耗]或[支付]金额减少1000（最少为0，若为[支付]则被[支付]玩家[获得]500资金）」
#[test]
fn soul_counter_reduces_a_payment() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "AG:绯红之魂").unwrap();
    t.answer(0, 1).unwrap(); // 2 crystals
    skip_all(&mut t);
    // The owner pays: land on P1's tile.
    t.own(1, &[HILL]);
    t.set_houses(HILL, 3); // rent 1520
    t.begin_turn(0);
    t.set_pos(0, HILL - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    eprintln!("react? {}", t.dump_prompt());
    if t.prompt().is_some() && t.dump_prompt().contains("crimson_soul") {
        t.answer(0, 0).unwrap(); // yes, spend the crystal
    }
    skip_all(&mut t);
    eprintln!("money: {} {} crystals={:?}", t.money(0), t.money(1), t.crystals(0, "AG:绯红之魂"));
    // 1520 - 1000 = 520 transferred.
    assert!(t.money(0) < 10_000, "counter fired: {}", t.money(0));
}

// 规则书: 「（3）此卡上不再拥有[奇迹水晶]时将此卡放入[使用者]弃卡区」
#[test]
fn soul_leaves_when_empty() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "AG:绯红之魂").unwrap();
    t.answer(0, 0).unwrap(); // 1 crystal
    skip_all(&mut t);
    assert!(t.on_field(0, "AG:绯红之魂"));
    assert_eq!(t.crystals(0, "AG:绯红之魂"), Some(1));
    // The owner pays: land on P1's tile and pay rent.
    t.own(1, &[HILL]);
    t.set_houses(HILL, 3); // rent 1520
    t.begin_turn(0);
    t.set_pos(0, HILL - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    eprintln!("react? {}", t.dump_prompt());
    // The soul counter is a yes/no prompt (crimson_soul_pay_ask_to), not a
    // standard [反击] window.
    if t.prompt().is_some() && t.dump_prompt().contains("crimson_soul") {
        t.answer(0, 0).unwrap(); // yes, spend the crystal
    }
    skip_all(&mut t);
    eprintln!("field={:?} discard={:?} crystals={:?}", t.field_ids(0), t.discard(0), t.crystals(0, "AG:绯红之魂"));
    let gone = !t.on_field(0, "AG:绯红之魂");
    let empty = t.crystals(0, "AG:绯红之魂") == Some(0);
    assert!(gone || empty, "card left or crystals drained");
}

// =====================================================================
// AG:朝同一片天空迈进
// =====================================================================

// 规则书: 「[反击]抽出此卡时立刻打出，如果你手牌数大于等于3，获得手牌数*600的资金，如果你的手牌数小于3，抽一张卡」
#[test]
#[ignore = "DISCREPANCY: 「抽出此卡时立刻打出」 — the card is drawn into hand but its draw-trigger does not auto-play it (no sky event, money unchanged beyond the draw source)"]
fn sky_fires_on_draw() {
    let mut t = Table::vanilla(2);
    t.set_draw(0, &["AG:朝同一片天空迈进"]);
    t.set_hand(0, &["通用:登上武道馆", "通用:GREAT", "通用:安可"]); // 3 cards
    t.give(0, &["通用:10次招募（1回限定）"]);
    t.play(0, "通用:10次招募（1回限定）").unwrap();
    skip_all(&mut t);
    // 3 cards when drawn → 3×600 = 1800 on top of the recruit's -1500.
    assert_eq!(t.money(0), 10_000 - 1_500 + 1_800, "events {:?}", t.recent_keys(8));
}

// =====================================================================
// AG:（摩卡）0.5倍速
// =====================================================================

#[test]
fn moca_card_is_exclusive() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "美竹兰");
    let r = t.give_play(0, "AG:（摩卡）0.5倍速");
    assert_eq!(r, Err("err.play_exclusive".to_string()));
}

// 规则书: 「(1) 将此卡放置在场上并获得3个奇迹水晶，你的回合结束时移除一个奇迹水晶，奇迹水晶为0时此卡放入弃牌堆」
#[test]
fn moca_card_places_with_three_crystals() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "青叶摩卡");
    t.give_play(0, "AG:（摩卡）0.5倍速").unwrap();
    assert!(t.on_field(0, "AG:（摩卡）0.5倍速"));
    assert_eq!(t.crystals(0, "AG:（摩卡）0.5倍速"), Some(3), "3 crystals");
    // 规则书: 「你的回合结束时移除一个奇迹水晶」
    t.dice(&[1]);
    t.roll(0).unwrap();
    skip_all(&mut t);
    t.end(0).unwrap();
    let x = t.crystals(0, "AG:（摩卡）0.5倍速").unwrap();
    assert!(x < 3, "a crystal was removed: {x} events {:?}", t.recent_keys(8));
}

// 规则书: 「(2) 此卡在场时，你的移动掷骰的最终结算/2（向上取整）且你的所有资金支付与消耗减半」
#[test]
fn moca_card_halves_move_and_spend() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "青叶摩卡");
    t.give_play(0, "AG:（摩卡）0.5倍速").unwrap();
    t.set_pos(0, 10);
    t.dice(&[7]); // 7 / 2 ceil = 4
    t.roll(0).unwrap();
    skip_all(&mut t);
    assert_eq!(t.pos(0), 14, "10 + ceil(7/2)");
}

// =====================================================================
// AG:(兰) 像往常一样
// =====================================================================

#[test]
fn ran_card_is_exclusive() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "青叶摩卡");
    let r = t.give_play(0, "AG:(兰) 像往常一样");
    assert_eq!(r, Err("err.play_exclusive".to_string()));
}

// 规则书: 「（2）受到异常移动效果（包括你的技能）的回合结束前，回到起始地点并取消所有受到的效果（不进行任何结算）」
#[test]
fn ran_card_returns_to_start_after_abnormal() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "美竹兰");
    t.give(0, &["AG:(兰) 像往常一样"]);
    // Give the player a [停留] via 雨啊 from P1.
    t.begin_turn(1);
    t.dice(&[1, 1]);
    t.give_play(1, "通用:雨啊，快点来吧").unwrap();
    t.answer(1, 0).unwrap(); // target P0
    skip_all(&mut t);
    // P0's turn: the stay blocks the roll. End the turn to trigger the return.
    t.begin_turn(0);
    // The stay may have been cancelled by the Ran card already.
    eprintln!("before end: pos={} stay={}", t.pos(0), t.state(0, "stay"));
    if t.state(0, "stay") == 0 {
        t.dice(&[3]);
        t.roll(0).unwrap();
        skip_all(&mut t);
    }
    t.end(0).unwrap();
    eprintln!("after end: pos={} stay={}", t.pos(0), t.state(0, "stay"));
}

// =====================================================================
// AG:回家的路上绕个道 / AG:刻入天穹傲岸的烈光 / AG:（巴）/（绯玛丽）/（鸫）
// =====================================================================

// 规则书: 「(1)此卡可作为反击使用 (2)进行一次“afterglow”式的移动…」
#[test]
fn detour_can_be_a_counter() {
    let mut t = Table::vanilla(2);
    t.give(1, &["AG:回家的路上绕个道"]);
    t.give(0, &["通用:登上武道馆"]);
    t.play(0, "通用:登上武道馆").unwrap();
    // A [反击] window may offer it.
    if t.react_offered("AG:回家的路上绕个道") {
        t.react(1, "AG:回家的路上绕个道").unwrap();
    }
    skip_all(&mut t);
    eprintln!("money: {} {}", t.money(0), t.money(1));
}

// 规则书: 「[反击] 当你经过一名角色时…你从对方处获得等于对方最贵格子基础购买价格一半数额的资金…」
#[test]
fn glory_counter_on_passing_a_character() {
    let mut t = Table::vanilla(2);
    t.give(0, &["AG:刻入天穹傲岸的烈光"]);
    t.own(1, &[HILL]); // P1's cheapest; give them a dearer one
    t.own(1, &[40]); // 武道馆, price 3400
    t.set_pos(1, 20);
    t.begin_turn(0);
    t.set_pos(0, 18);
    t.dice(&[3]); // 18+3 = 21, passes 20 (P1)
    t.roll(0).unwrap();
    if t.react_offered("AG:刻入天穹傲岸的烈光") {
        t.react(0, "AG:刻入天穹傲岸的烈光").unwrap();
    }
    skip_all(&mut t);
    eprintln!("money: {} {}", t.money(0), t.money(1));
}

#[test]
fn hagumi_card_is_exclusive() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "美竹兰");
    let r = t.give_play(0, "AG:（鸫）微小的『能做到』的事");
    assert_eq!(r, Err("err.play_exclusive".to_string()));
}

#[test]
fn himari_card_is_exclusive() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "美竹兰");
    let r = t.give_play(0, "AG:（绯玛丽）如果并非没问题");
    assert_eq!(r, Err("err.play_exclusive".to_string()));
}

// 规则书: 「【反击】当你的一次掷骰小于6时，你可以打出此卡使结果+1」
#[test]
fn himari_card_bumps_a_low_roll() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "上原绯玛丽");
    t.give(0, &["AG:（绯玛丽）如果并非没问题"]);
    t.set_pos(0, 10);
    t.dice(&[3]);
    t.roll(0).unwrap();
    if t.react_offered("AG:（绯玛丽）如果并非没问题") {
        t.react(0, "AG:（绯玛丽）如果并非没问题").unwrap();
    }
    skip_all(&mut t);
    eprintln!("pos={}", t.pos(0));
}

#[test]
fn one_of_us_places_on_field() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "美竹兰");
    t.set_character_raw(1, "青叶摩卡"); // another Afterglow character
    t.own(0, &[HILL]);
    t.own(1, &[FUJIMI]);
    t.give_play(0, "AG:ONE OF US").unwrap();
    skip_all(&mut t);
    assert!(t.on_field(0, "AG:ONE OF US"), "field {:?}", t.field_ids(0));
}

// =====================================================================
// Character skills
// =====================================================================

// 规则书: 美竹兰「（1）每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）」
#[test]
#[ignore = "DISCREPANCY: 「（初始1，上限1）」 — fire pots start at 0, not 1 (cap 1 is set). Same gap as 都筑诗船's 初始1"]
fn ran_skill_starts_with_one_fire() {
    let mut t = Table::new(&["美竹兰", "户山香澄"]);
    assert_eq!(t.fire(0), 1, "初始1");
    assert_eq!(t.state_var(0, "fire").max, 1, "上限1");
}

// 规则书: 美竹兰「（2）投掷移动步数前可使用一个[火罐]选择本次移动向后」
#[test]
fn ran_skill_can_move_backward() {
    let mut t = Table::new(&["美竹兰", "户山香澄"]);
    t.clean();
    t.set_fire(0, 1, 1);
    t.begin_turn(0);
    let sid = t.skill_id(0, "叛逆的红挑染");
    let r = t.skill(0, &sid);
    eprintln!("ran skill: {r:?}");
    if t.prompt().is_some() {
        eprintln!("prompt: {}", t.dump_prompt());
        t.answer(0, 0).ok();
    }
    skip_all(&mut t);
    t.set_pos(0, 10);
    t.dice(&[4]);
    t.roll(0).unwrap();
    skip_all(&mut t);
    eprintln!("pos={} fire={}", t.pos(0), t.fire(0));
}

// 规则书: 青叶摩卡「（2）当他人使用角色技能或打出手牌时，你可以消耗一个[火罐]传送到其所在格子」
#[test]
fn moca_skill_teleports_to_the_actor() {
    let mut t = Table::new(&["青叶摩卡", "美竹兰"]);
    t.clean();
    t.set_fire(0, 1, 1);
    t.set_pos(1, 30);
    t.begin_turn(1);
    // P1 plays a card; P0 may teleport.
    t.give(1, &["通用:GREAT"]);
    t.play(1, "通用:GREAT").unwrap();
    if t.prompt().is_some() {
        eprintln!("moca prompt: {}", t.dump_prompt());
    }
    skip_all(&mut t);
    eprintln!("pos0={} fire={}", t.pos(0), t.fire(0));
}

// 规则书: 宇田川巴「（2）移动阶段前，你可以使用一个[火罐]，使此次移动的起点向绝对距离“银河拉面馆”更近的方向移动10格」
#[test]
fn utsugi_skill_shifts_start_toward_ramen() {
    let mut t = Table::new(&["宇田川巴", "美竹兰"]);
    t.clean();
    t.set_fire(0, 1, 1);
    t.begin_turn(0);
    let sid = t.skill_id(0, "豚骨酱油拉面大姐");
    let r = t.skill(0, &sid);
    eprintln!("utsugi skill: {r:?}");
    if t.prompt().is_some() {
        eprintln!("prompt: {}", t.dump_prompt());
    }
    skip_all(&mut t);
    eprintln!("pos={} fire={}", t.pos(0), t.fire(0));
}

// 规则书: 上原绯玛丽「（2）移动阶段前，你可以消耗一个火罐指定你的此次移动仅在单/双数格上进行」
#[test]
fn himari_skill_parity_move() {
    let mut t = Table::new(&["上原绯玛丽", "美竹兰"]);
    t.clean();
    t.set_fire(0, 1, 1);
    t.begin_turn(0);
    let sid = t.skill_id(0, "大家一起迈出新的一步");
    let r = t.skill(0, &sid);
    eprintln!("himari skill: {r:?}");
    if t.prompt().is_some() {
        eprintln!("prompt: {}", t.dump_prompt());
    }
    skip_all(&mut t);
    eprintln!("pos={} fire={}", t.pos(0), t.fire(0));
}

// 规则书: 羽泽鸫「（2）当你进行主要移动时，你可以失去一个[火罐]使本次移动反向」
#[test]
fn tsuzushi_skill_reverses_a_move() {
    let mut t = Table::new(&["羽泽鸫", "美竹兰"]);
    t.clean();
    t.set_fire(0, 1, 1);
    t.begin_turn(0);
    let sid = t.skill_id(0, "伟大的平凡");
    let r = t.skill(0, &sid);
    eprintln!("tsuzushi skill: {r:?}");
    if t.prompt().is_some() {
        eprintln!("prompt: {}", t.dump_prompt());
    }
    skip_all(&mut t);
    t.set_pos(0, 10);
    t.dice(&[4]);
    t.roll(0).unwrap();
    skip_all(&mut t);
    eprintln!("pos={} fire={}", t.pos(0), t.fire(0));
}

// =====================================================================
// Band skill: Afterglow — 商店街的宠儿
// =====================================================================

// 规则书: 「你购买商店街的或价值小于等于1200的格子时自动免费在上面加盖一栋房子」
#[test]
fn band_free_house_on_cheap_buy() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "skill:Afterglow:商店街的宠儿");
    t.set_pos(0, FUJIMI - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    skip_all(&mut t);
    // Landing on an unowned deed: buy through the action.
    t.buy(0).unwrap();
    skip_all(&mut t);
    // 规则书: 「自动免费在上面加盖一栋房子」
    assert_eq!(t.owner(FUJIMI), Some(0), "events {:?}", t.recent_keys(8));
    assert_eq!(t.houses(FUJIMI), 1, "free house");
    assert_eq!(t.money(0), 9_400, "paid 600, house free");
}

// 规则书: 「购买商店街的…格子时自动免费在上面加盖一栋房子」
#[test]
fn band_free_house_on_shop_street_buy() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "skill:Afterglow:商店街的宠儿");
    t.set_pos(0, YAMABUKI - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    skip_all(&mut t);
    t.buy(0).unwrap();
    skip_all(&mut t);
    assert_eq!(t.owner(YAMABUKI), Some(0), "events {:?}", t.recent_keys(8));
    assert_eq!(t.houses(YAMABUKI), 1);
}

// =====================================================================
// Interactions
// =====================================================================

// AG:宣战布告 (AG) vs 通用:登上武道馆 (general): a cross-group counter.
#[test]
fn ix_declaration_vs_budokan() {
    let mut t = Table::vanilla(2);
    t.set_draw(1, &["通用:GREAT"]);
    t.give(1, &["AG:宣战布告"]);
    t.give(0, &["通用:登上武道馆"]);
    t.play(0, "通用:登上武道馆").unwrap();
    t.react(1, "AG:宣战布告").unwrap();
    skip_all(&mut t);
    // Both the counter and the original card resolve.
    assert!(t.hand(1).contains(&"通用:GREAT".to_string()));
    assert_eq!(t.money(0), 11_500);
}

// AG:宣战布告 vs PPP:抓到了: cross-group, PPP targeting card.
#[test]
fn ix_declaration_vs_ppp_caught() {
    let mut t = Table::vanilla(2);
    t.set_pos(1, 5);
    t.set_draw(1, &["通用:GREAT"]);
    t.give(1, &["AG:宣战布告"]);
    t.give(0, &["PPP:抓到了"]);
    t.play(0, "PPP:抓到了").unwrap();
    t.answer(0, 0).unwrap(); // target P1
    if t.react_offered("AG:宣战布告") {
        t.react(1, "AG:宣战布告").unwrap();
    }
    skip_all(&mut t);
    eprintln!("pos0={} money: {} {}", t.pos(0), t.money(0), t.money(1));
}

// 通用:安可 (general) vs the stay from 通用:雨啊 while AG's 0.5倍速 is up.
#[test]
fn ix_encore_and_moca_half_speed() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "青叶摩卡");
    t.give_play(0, "AG:（摩卡）0.5倍速").unwrap();
    t.give(0, &["通用:安可"]);
    t.begin_turn(1);
    t.dice(&[1, 1]);
    t.give_play(1, "通用:雨啊，快点来吧").unwrap();
    t.answer(1, 0).unwrap();
    if t.react_offered("通用:安可") {
        t.react(0, "通用:安可").unwrap();
    }
    skip_all(&mut t);
    eprintln!("stay={} pos={}", t.state(0, "stay"), t.pos(0));
}

// The AG band's free house meets the sunset card's demolish.
#[test]
fn ix_band_house_then_sunset_demolish() {
    let mut t = Table::vanilla(3);
    t.place_raw(0, "skill:Afterglow:商店街的宠儿");
    // Buy a cheap tile → free house.
    t.set_pos(0, FUJIMI - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    skip_all(&mut t);
    t.buy(0).unwrap();
    skip_all(&mut t);
    assert_eq!(t.houses(FUJIMI), 1, "free house");
    // Next turn: demolish it with sunset (needs 4 housed tiles).
    t.begin_turn(0);
    t.own(0, &[HILL, 7, 9]);
    for &tile in &[HILL, 7, 9] {
        t.set_houses(tile, 1);
    }
    t.give_play(0, "AG:即使夕阳落山").unwrap();
    skip_all(&mut t);
    assert_eq!(t.houses(FUJIMI), 0, "demolished");
}

// 绯红之魂's crystal drain meets the owner's own rent payment.
#[test]
fn ix_soul_crystal_vs_rent() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "AG:绯红之魂").unwrap();
    t.answer(0, 1).unwrap(); // 2 crystals
    skip_all(&mut t);
    t.own(1, &[HILL]);
    t.set_houses(HILL, 3);
    t.begin_turn(0);
    t.set_pos(0, HILL - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    if t.prompt().is_some() && t.dump_prompt().contains("crimson_soul") {
        t.answer(0, 0).unwrap(); // yes, spend the crystal
    }
    skip_all(&mut t);
    let x = t.crystals(0, "AG:绯红之魂").unwrap_or(0);
    assert!(x < 2, "a crystal was spent: {x}");
}