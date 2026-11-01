//! G03: the roll transaction ladder and the card bodies that ride its
//! windows. Design and row ids: `docs/rulebook/spec/abi/G03-roll-and-prompts.md`
//! §9.
//!
//! Every assertion is the 规则书 text observed through the shared harness.

mod common;
use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// A clean 2-seat board with `chars` as the personas, player 0's turn open.
fn fresh(chars: &[&str]) -> Table {
    let mut t = Table::new(chars);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t
}

/// Answer the open prompt with the option whose message names `needle`.
fn pick(t: &mut Table, who: usize, needle: &str) {
    let k = t
        .option(needle)
        .unwrap_or_else(|| panic!("{needle} not offered: {}", t.dump_prompt()));
    t.answer(who, k).unwrap();
}

/// Play `card` from `who`'s hand during the open [反击] window.
fn play_counter(t: &mut Table, who: usize, card: &str) {
    assert!(
        t.counteract_offered(card),
        "{card} not offered: {}",
        t.dump_prompt()
    );
    t.counteract(who, card).unwrap();
}

const STARRY: &str = "Mor:蝴蝶飞舞的星月夜";
const HALF_SPEED: &str = "AG:（摩卡）0.5倍速";
const SPORTS: &str = "HHW:运动的天赋";
const TAKI: &str = "CRYCHIC:（立希）即便比不上...";
const WANT_HUMAN: &str = "CRYCHIC:想要成为人类";
const HAGUMI: &str = "HHW:（育美）";
const MISAKI: &str = "HHW:（美咲）";
const NO_EXPECTATION: &str = "PP:不要背负期待";
const LAYER_KEEP: &str = "RAS:（和奏瑞依）寄于指尖的执念";
const PLEASE_CHOOSE: &str = "RAS:PLEASE CHOOSE";
const DOLL_GARDEN: &str = "Mujica:人偶的箱庭";
const OWN_STAGE: &str = "R:选择自己的舞台";
const RAIN: &str = "通用:雨啊，快点来吧";
const OVERLAP: &str = "PP:[衍生]重叠的声音";

// 规则书 (`starry_night` (1) + `moca_half` (2)): the reroll is a new cast of
// the same transaction, so 「最终结算」 modifiers reach it too -- the half
// applies to the kept rerolled face.
#[test]
fn g03_starry_reroll_reaches_the_settlement() {
    let mut t = fresh(&["桐谷透子", "户山香澄"]);
    t.place_raw(0, STARRY);
    t.place_raw(0, HALF_SPEED);
    t.set_crystals(0, STARRY, 2);
    drain(&mut t);
    t.set_pos(0, 0);
    t.dice(&[7, 7]);
    t.roll(0).unwrap();
    pick(&mut t, 0, "starry_night_keep_new");
    drain(&mut t);
    assert_eq!(t.pos(0), 4, "the kept 7 settles through the half: ceil(7/2)");
}

// 规则书 (`sports_talent`): 「此卡位于场上时，每次掷骰获得一次资金」 -- every
// cast pays, effect rolls included, and the old main-move-only guard is gone.
#[test]
fn g03_athletic_pays_on_every_cast() {
    let mut t = fresh(&["北泽育美", "户山香澄"]);
    t.m.world_mut().set_slot(0, "sports_talent_next", 700);
    t.place_raw(0, SPORTS);
    drain(&mut t);
    let money = t.money(0);
    t.give(0, &[HAGUMI]);
    // 4d20 x2 -- effect rolls. 70 wraps onto tile 9; both casts pay.
    t.dice(&[20, 20, 20, 10, 3, 3, 2, 2]);
    t.play(0, HAGUMI).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), money + 700 + 600, "one payout per cast");
    // The forced (card-driven) walk's own cast pays as well. The card's own
    // 「重骰直至至少10」 may cast again (and pay again), so only the first
    // payout is pinned.
    let money = t.money(0);
    t.set_pos(0, 0);
    t.dice(&[4]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert!(
        t.money(0) >= money + 500,
        "the walk cast paid ({} → {})",
        money,
        t.money(0)
    );
}

// 规则书 (`taki`): 「本回合内你骰出过的所有骰点」 -- only the owner's own casts
// count, not the opponent's rolls of the same face.
#[test]
fn g03_taki_counts_only_the_owners_casts() {
    let mut t = fresh(&["椎名立希（CRYCHIC）", "高松灯（CRYCHIC）"]);
    t.give(0, &[TAKI]);
    // The owner's own first cast of the turn is a 3; their turn ends.
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    // The opponent rolls a 5 on their turn.
    t.dice(&[5]);
    t.roll(1).unwrap();
    drain(&mut t);
    t.end(1).unwrap();
    drain(&mut t);
    // The owner rolls 3 again and rerolls to 5 -- the opponent's 5 is not in
    // the owner's history, so 5 ends the reroll.
    t.set_pos(0, 0);
    t.dice(&[3, 5]);
    t.roll(0).unwrap();
    play_counter(&mut t, 0, TAKI);
    drain(&mut t);
    assert!(
        !t.recent_keys(12).iter().any(|k| k.contains("taki_again")),
        "the opponent's 5 does not count: {:?}",
        t.recent_keys(12)
    );
    assert_eq!(t.pos(0), 5, "the rerolled 5 settles");
}

// 规则书 (`taki` + 鸫's armed extreme): the reroll inherits the transaction's
// forced face, so it settles at the theoretical maximum.
#[test]
fn g03_taki_reroll_settles_at_the_armed_extreme() {
    let mut t = fresh(&["椎名立希（CRYCHIC）", "高松灯（CRYCHIC）"]);
    t.give(0, &[TAKI]);
    t.set_pos(0, 0);
    t.set_extreme(1);
    t.dice(&[5]);
    t.roll(0).unwrap();
    play_counter(&mut t, 0, TAKI);
    drain(&mut t);
    assert_eq!(t.pos(0), 20, "the reroll settles at the armed max");
}

// 规则书 (`want_human` (2)): the armed 「额外增加20-X」 is part of *this*
// roll's face, so it rides a reroll instead of dying with the first cast.
#[test]
fn g03_want_human_boost_survives_a_reroll() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "户山香澄"]);
    t.strip_skills();
    t.clean();
    let wh = t.place_raw(0, WANT_HUMAN);
    t.place_raw(0, STARRY);
    t.m.world_mut().set_prop_at(wh, "want_human_x", 15);
    t.set_crystals(0, WANT_HUMAN, 2);
    t.set_crystals(0, STARRY, 2);
    // Turn start drains the 2 crystals and arms +5 (20 - 15).
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 0);
    t.dice(&[7, 3]);
    t.roll(0).unwrap();
    pick(&mut t, 0, "starry_night_keep_new");
    drain(&mut t);
    assert_eq!(t.pos(0), 3 + 5, "the kept rerolled 3 still carries +5");
    assert_eq!(
        t.m.world().prop_at(wh, "want_human_boost"),
        0,
        "one-shot boost consumed"
    );
    assert_eq!(
        t.crystals(0, WANT_HUMAN),
        Some(2),
        "both casts were ≤ X: one crystal each"
    );
}

// 规则书 (`want_human` (1)): 「每当你的移动掷骰小于等于X」 checks *every* cast,
// rerolls included -- the low first face banks, the high reroll does not.
#[test]
fn g03_want_human_banks_every_low_cast() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "户山香澄"]);
    t.strip_skills();
    t.clean();
    let wh = t.place_raw(0, WANT_HUMAN);
    t.place_raw(0, STARRY);
    t.m.world_mut().set_prop_at(wh, "want_human_x", 10);
    t.set_crystals(0, STARRY, 2);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 0);
    t.dice(&[5, 20]);
    t.roll(0).unwrap();
    pick(&mut t, 0, "starry_night_keep_new");
    drain(&mut t);
    assert_eq!(
        t.crystals(0, WANT_HUMAN),
        Some(1),
        "5 ≤ 10 banks, 20 > 10 does not"
    );
}

// (G04) 规则书 (`detour`): the counteraction's plan writes reach the walk.
#[test]
#[ignore = "G04: the plan-stage writes land in the walk"]
fn g04_detour_counter_plan_writes_reach_the_walk() {}

// 规则书 (`hagumi_marks` (1)): one option per *roll* -- two faces that wrap
// onto one tile stay two options, and each keeps its own outcome.
#[test]
fn g03_hagumi_offers_one_option_per_cast() {
    let mut t = fresh(&["北泽育美", "户山香澄"]);
    t.give(0, &[HAGUMI]);
    // 60 tiles: 70 (>60) and 10 (≤60) both land on tile 9.
    t.dice(&[20, 20, 20, 10, 3, 3, 2, 2]);
    t.play(0, HAGUMI).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.options.len(), 2, "one option per roll: {}", t.dump_prompt());
    let money = t.money(0);
    t.answer(0, 1).unwrap(); // the ≤60 roll: a mark, no money
    drain(&mut t);
    assert_eq!(t.money(0), money, "the ≤60 pick pays nothing");
    assert!(!t.marks_on(9).is_empty(), "it places its mark on tile 9");
}

// 规则书 (`misaki_card`): 「使用火罐进行移动掷骰后，触发结算前可打出此卡」 --
// the window is the Result of a *fire* cast; a plain roll offers nothing.
#[test]
fn g03_misaki_only_after_a_fire_cast() {
    // A plain roll: no fire source, no offer.
    let mut t = fresh(&["奥泽美咲", "户山香澄"]);
    t.give(0, &[MISAKI]);
    t.set_pos(0, 10);
    t.set_pos(1, 12);
    t.dice(&[5]);
    t.roll(0).unwrap();
    assert!(
        !t.counteract_offered(MISAKI),
        "a plain roll: {}",
        t.dump_prompt()
    );
    drain(&mut t);
}

#[test]
fn g03_misaki_offered_when_the_result_is_a_fire_recast() {
    // A fire recast: the Result carries source 1 and the offer appears.
    let mut t = fresh(&["奥泽美咲", "户山香澄"]);
    t.give(0, &[MISAKI]);
    t.place_raw(0, "TEST:fireRecast");
    t.set_fire(0, 1, 1);
    t.set_pos(0, 10);
    t.set_pos(1, 12);
    t.dice(&[5, 3]);
    t.roll(0).unwrap();
    // `play_counter` itself fails the test if the card is not offered.
    play_counter(&mut t, 0, MISAKI);
    // 「传送至你选择的一名…玩家所在的格子」 -- the only rival in the span.
    t.answer_one(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), t.pos(1), "teleported onto the rival");
}

// 规则书 (`moca_half` (2) + `no_expectation` (1)): the shave is additive and
// the half is the final settlement, in that order (7 − 2 = 5 → ceil(5/2) = 3).
#[test]
fn g03_half_speed_settles_after_the_shave() {
    for order in [0, 1] {
        let mut t = fresh(&["青叶摩卡", "户山香澄"]);
        let first = if order == 0 { NO_EXPECTATION } else { HALF_SPEED };
        let second = if order == 0 { HALF_SPEED } else { NO_EXPECTATION };
        t.place_raw(0, first);
        t.place_raw(0, second);
        drain(&mut t);
        t.set_pos(0, 0);
        t.dice(&[7]);
        t.roll(0).unwrap();
        drain(&mut t);
        assert_eq!(t.pos(0), 3, "order {order}: 7 − 2 = 5, then ceil(5/2)");
    }
}

// 规则书 (`no_expectation` (1)): 「非回合开始时进行投掷的投掷结果减少2」 --
// effect rolls lose 2 with no floor; a move 0 forgoes the settle; the
// turn-start step is untouched.
#[test]
fn g03_no_expectation_shaves_effect_rolls() {
    let mut t = fresh(&["北泽育美", "户山香澄"]);
    t.place_raw(0, NO_EXPECTATION);
    drain(&mut t);
    t.give(0, &[HAGUMI]);
    // Effect 4d20s: 80 → 78 (still > 60) and 62 → 60 (≤ 60, a mark).
    t.dice(&[20, 20, 20, 20, 20, 20, 20, 2]);
    t.play(0, HAGUMI).unwrap();
    let p = t.expect_prompt();
    let labels: Vec<String> = p.options.iter().map(|o| format!("{o:?}")).collect();
    assert!(
        labels.iter().any(|l| l.contains("78")),
        "the 80 was shaved to 78: {labels:?}"
    );
    assert!(
        labels.iter().any(|l| l.contains("60")),
        "the 62 was shaved to 60 with no floor below 60: {labels:?}"
    );
    t.answer(0, 0).unwrap();
    drain(&mut t);
    // A move cast of 1 shaves to 0: no step, and the settle is forgone.
    t.set_pos(0, 8);
    t.own(1, &[10]);
    let money = t.money(0);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 8, "0 steps");
    assert_eq!(t.money(0), money, "no settle at 0");
    assert!(
        t.recent_keys(12)
            .iter()
            .any(|k| k.contains("no_expectation_no_resolve")),
        "the resolve is cancelled: {:?}",
        t.recent_keys(12)
    );
}

// 规则书 (`no_expectation` (1)): 「非回合开始时」 -- a roll taken during the
// turn start (a `turnStart` hook's die, step 1) is not shaved.
#[test]
fn g03_no_expectation_leaves_a_turn_start_roll_alone() {
    let mut t = fresh(&["户山香澄", "花园多惠"]);
    t.place_raw(0, NO_EXPECTATION);
    drain(&mut t);
    // Finish the owner's turn, then the opponent's, back to the owner's turn
    // start. The rain rolls on its owner's turn start.
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    t.place_raw(0, "MyGO:那天的雨");
    t.set_crystals(0, "MyGO:那天的雨", 5);
    t.dice(&[3]);
    t.roll(1).unwrap();
    drain(&mut t);
    t.dice(&[5]);
    let mark = t.mark();
    t.end(1).unwrap();
    drain(&mut t);
    let rolled = t
        .events_since(mark)
        .into_iter()
        .find(|e| e.msg.key() == "that_day_rain_log" || e.msg.key().ends_with("that_day_rain_log"))
        .and_then(|e| match e.msg.args.get("roll") {
            Some(game_core::msg::Arg::Integer(v)) => Some(*v),
            _ => None,
        })
        .expect("the turn-start roll logged");
    assert_eq!(rolled, 5, "the 1d10 was not shaved: {rolled}");
}

// 规则书 (`layer_keep`): 「保留（写下）未被选择的另一个骰点」 -- the pick
// records the face it did not take, the settle banks it, and `use_die`
// offers it afterwards.
#[test]
fn g03_layer_keep_banks_the_unchosen_face() {
    let mut t = fresh(&["和奏瑞依", "户山香澄"]);
    t.place_raw(0, LAYER_KEEP);
    t.place_raw(0, "TEST:fireReroll");
    drain(&mut t);
    t.set_pos(0, 0);
    t.set_fire(0, 1, 1);
    t.dice(&[5, 3]);
    t.roll(0).unwrap();
    // The fire recast raises its own Before with source 1: the keep opens.
    play_counter(&mut t, 0, LAYER_KEEP);
    // Keep the old face; the rerolled 3 is left behind.
    pick(&mut t, 0, "fire_keep_old");
    drain(&mut t);
    assert_eq!(t.pos(0), 5, "the old face settles");
    assert_eq!(t.state(0, "layer_keep_0"), 3, "the unchosen face is banked");
    assert_eq!(t.state(0, "layer_keep_count"), 1, "one die kept");
}

// 规则书 (`please_choose` (1)): option (1) exists only while the opponent can
// actually play a designating card; the pick plays it for them.
#[test]
fn g03_please_choose_opt1_offers_only_with_a_targeting_card() {
    // Empty-handed opponent: only the teleport option.
    let mut t = fresh(&["户山香澄", "lock"]);
    t.give(1, &[PLEASE_CHOOSE]);
    drain(&mut t);
    t.set_pos(0, 0);
    t.dice(&[8]); // onto RiNG 1, a livehouse
    t.roll(0).unwrap();
    assert!(
        t.counteract_offered(PLEASE_CHOOSE),
        "livehouse settle: {}",
        t.dump_prompt()
    );
    t.counteract(1, PLEASE_CHOOSE).unwrap();
    let p = t.expect_prompt();
    assert_eq!(
        p.options.len(),
        1,
        "no targeting card in hand: {}",
        t.dump_prompt()
    );
    t.answer_one(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(1), t.pos(0), "option (2): teleported onto the settler");
    // With a designating card the option appears and plays it.
    // The mask sits in the *settler's* hand: the forced player plays one of
    // their own cards designating the PLEASE CHOOSE holder.
    let mut t = fresh(&["户山香澄", "lock"]);
    t.give(1, &[PLEASE_CHOOSE]);
    t.give(0, &["PP:[白鹭千圣]微笑的铁假面"]);
    drain(&mut t);
    t.set_pos(0, 0);
    t.dice(&[8]);
    t.roll(0).unwrap();
    t.counteract(1, PLEASE_CHOOSE).unwrap();
    let p = t.expect_prompt();
    assert_eq!(
        p.options.len(),
        2,
        "the mask is offered: {}",
        t.dump_prompt()
    );
    let k = t.option("微笑的铁假面").expect("the mask names the card");
    t.answer(0, k).unwrap();
    drain(&mut t);
    assert!(
        t.on_field(0, "PP:[白鹭千圣]微笑的铁假面"),
        "the pick played it for the forced player"
    );
}

// 规则书 (`doll_garden`): the victim's roll is a real move roll of their own --
// their modifiers apply (no expectation shaves it), their After hook runs, and
// it is not the main move.
#[test]
fn g03_doll_garden_victim_roll_takes_the_victim_view() {
    let mut t = fresh(&["户山香澄", "花园多惠"]);
    t.give(0, &[DOLL_GARDEN]);
    t.place_raw(1, NO_EXPECTATION);
    drain(&mut t);
    t.set_pos(0, 0);
    t.set_pos(1, 20);
    t.dice(&[6, 5, 4, 3]);
    t.play(0, DOLL_GARDEN).unwrap();
    // P1 chooses to move: their own 6 is shaved to 4 and walks 4.
    t.answer_one(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(1), 24, "the victim's 6 was shaved by their own card");
    assert!(
        t.recent_keys(20)
            .iter()
            .any(|k| k.contains("no_expectation_roll")),
        "the victim's modifier ran: {:?}",
        t.recent_keys(20)
    );
}

#[test]
fn g03_doll_garden_victim_is_not_the_main_move() {
    // 运动的天赋 rerolls a low *main* move (its After pre is `move.main`); a
    // victim's roll must not reach it, while its cast entry still pays.
    let mut t = fresh(&["户山香澄", "北泽育美"]);
    t.give(0, &[DOLL_GARDEN]);
    t.place_raw(1, SPORTS);
    t.m.world_mut().set_slot(1, "sports_talent_next", 700);
    drain(&mut t);
    t.set_pos(0, 0);
    t.set_pos(1, 20);
    let money = t.money(1);
    t.dice(&[6, 5, 4, 3]);
    t.play(0, DOLL_GARDEN).unwrap();
    t.answer_one(0).unwrap();
    drain(&mut t);
    // The cast entry still pays (「每次掷骰」 -- the reward is a `log.gain`
    // whose `src` is the card's message, not a key of its own).
    assert_eq!(t.money(1), money + 700, "the victim's cast entry paid");
    // The After reroll did not: the 6 settles as 6 (a reroll would log and,
    // with `min_roll` 10, the walk would be 10).
    assert_eq!(t.pos(1), 26, "the victim's 6 walked unrerolled");
    assert!(
        !t.recent_keys(30)
            .iter()
            .any(|k| k.contains("sports_talent_reroll")),
        "the main-move reroll did not run: {:?}",
        t.recent_keys(30)
    );
}

// 规则书 (`own_stage`): 「选择自己的本次移动（或无法移动的回合结束时）是否
// 触发结算」 -- exactly one settle, only where the answer says.
#[test]
fn g03_own_stage_no_suppresses_the_landing_settle() {
    // A forced [传送] that is 「不[结算]」 by itself -- only the answer can make
    // it settle, and the answer says no.
    let mut t = fresh(&["户山香澄", "花园多惠"]);
    t.give(0, &[OWN_STAGE, OVERLAP]);
    drain(&mut t);
    t.play(0, OVERLAP).unwrap();
    play_counter(&mut t, 0, OWN_STAGE);
    t.answer_one(1).unwrap(); // no
    let mut settled = false;
    while let Some(p) = t.prompt() {
        if p.title.key().contains("buy") {
            settled = true;
        }
        t.decline();
    }
    assert_eq!(t.pos(0), tile("Bandori车站"), "the teleport happened");
    assert!(!settled, "no settle offer");
}

#[test]
fn g03_own_stage_default_settles() {
    let mut t = fresh(&["户山香澄", "lock"]);
    t.own(1, &[10]);
    t.set_pos(0, 8);
    t.dice(&[2]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 10);
    assert!(t.money(0) < 10_000, "the landing settle ran");
}

#[test]
fn g03_own_stage_yes_on_a_blocked_stay_settles_at_turn_end() {
    let mut t = fresh(&["户山香澄", "lock"]);
    t.give(0, &[OWN_STAGE, RAIN]);
    t.own(1, &[10]);
    drain(&mut t);
    t.set_pos(0, 10);
    // Apply the [停留] to self; own_stage's window opens on it.
    t.dice(&[1, 1]);
    t.play(0, RAIN).unwrap();
    t.answer(0, 0).unwrap(); // the extra target
    play_counter(&mut t, 0, OWN_STAGE);
    t.answer_one(0).unwrap(); // yes
    drain(&mut t);
    assert_eq!(t.state(0, "stay"), 1, "the [停留] landed");
    // The blocked turn: no move, and nothing settles yet.
    assert_eq!(t.pos(0), 10, "the move is blocked");
    assert_eq!(t.money(0), 10_000, "nothing yet");
    t.end(0).unwrap();
    drain(&mut t);
    assert!(t.money(0) < 10_000, "the held settle ran at the turn end");
}

#[test]
fn g03_own_stage_no_skips_the_blocked_settle() {
    let mut t = fresh(&["户山香澄", "lock"]);
    t.give(0, &[OWN_STAGE, RAIN]);
    t.own(1, &[10]);
    drain(&mut t);
    t.set_pos(0, 10);
    t.dice(&[1, 1]);
    t.play(0, RAIN).unwrap();
    t.answer(0, 0).unwrap();
    play_counter(&mut t, 0, OWN_STAGE);
    t.answer_one(1).unwrap(); // no
    drain(&mut t);
    assert_eq!(t.pos(0), 10, "the move is blocked");
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000, "no settle at all");
}


/// 「或无法移动的回合结束时」 across seats: the answer is given in the
/// **applying** turn (the rain's target here), and must survive into the
/// blocked seat's own turn -- the engine may not read it on the declarer.
#[test]
fn g03_own_stage_cross_seat_skip_survives_the_turn() {
    let mut t = fresh(&["户山香澄", "lock"]);
    t.give(0, &[RAIN]);
    t.give(1, &[OWN_STAGE]);
    t.own(1, &[10]);
    t.set_pos(1, 10);
    t.dice(&[1, 1]);
    t.play(0, RAIN).unwrap();
    t.answer(0, 0).unwrap(); // the extra target: P1
    play_counter(&mut t, 1, OWN_STAGE);
    t.answer_one(1).unwrap(); // no
    drain(&mut t);
    assert_eq!(t.state(1, "stay"), 1, "the [停留] landed on P1");
    t.end(0).unwrap();
    drain(&mut t);
    for _ in 0..20 {
        if t.turn() == 1 {
            break;
        }
        let cur = t.turn();
        t.m.world_mut().st.skip_move = true;
        t.end(cur).unwrap();
        drain(&mut t);
    }
    assert_eq!(t.turn(), 1);
    assert_eq!(t.pos(1), 10, "the blocked turn could not move");
    t.end(1).unwrap();
    drain(&mut t);
    assert_eq!(t.money(1), 10_000, "P1's own answer suppressed the settle");
}

/// A [眩晕] closes both gates, so the turn auto-skips before any main move:
/// the default must already be stamped at the turn start (the old post-ladder
/// immobile stamp never ran for it).
#[test]
fn g03_own_stage_stun_turn_end_settles() {
    let mut t = Table::new(&["户山香澄", "lock"]);
    t.strip_skills();
    t.clean();
    // 江户川乐器店's tile rule draws a card -- unlike a rent, the draw does
    // not pass the pay gate the stun also closes.
    let edogawa = tile("江户川乐器店");
    t.set_pos(0, edogawa);
    t.set_state(0, "stun", 1);
    // One deck card: clean emptied both piles and the tile rule draws.
    t.m.world_mut().hidden[0].deck.push("PP:不要背负期待".into());
    let before = t.hand(0).len();
    let mark = t.mark();
    t.begin_turn(0);
    // The stun closes the act gate, so the turn auto-skips straight to its end.
    drain(&mut t);
    assert_eq!(t.pos(0), edogawa, "no move happened");
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k == "log.land_draw"),
        "the stunned turn's end settle ran the tile rule: {keys:?}"
    );
    assert_eq!(t.hand(0).len(), before + 1, "the settle drew a card");
}
