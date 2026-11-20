//! A passive move reshape (an `On::Filter(Filter::DiceRollPlan(..))` body) is
//! bookkeeping at the plan pass: it is neither an abnormal movement effect nor
//! a clause the player has to be shown.
//!
//! * the walk is still shaped (2d20, reverse) -- the presentation change is
//!   not a behaviour change;
//! * no `"card"` event, no flash, no `plan.*` outcome line, no body log;
//! * nothing lands as 异常移动: `turn.abnormal` stays empty and `abnormalHit`
//!   stays 0 (仓田真白's reverse is a plan property, not a received effect);
//! * an **Action** at the same kind (the G05 plan offer) still announces,
//!   prompts and flashes -- the rule keys on the entry kind, not the stage.

mod common;
use common::*;

use game_core::state::MatchEvent;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn card_events(t: &Table, mark: i32, card: &str) -> Vec<MatchEvent> {
    t.events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "card" && e.card == card)
        .collect()
}

/// 仓田真白 (1)(2): the plan-pass reshape writes no header, no flash, no line.
#[test]
fn mashiro_plan_reshape_is_silent() {
    let mut t = Table::new(&["仓田真白", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.set_pos(0, 4);
    let mark = t.mark();
    // 2d20 = 3 + 3 = 6 backwards -> 58.
    t.dice(&[3, 3]);
    t.roll(0).unwrap();
    drain(&mut t);
    let skill = "skill:仓田真白:向后全速前进";
    let cards = card_events(&t, mark, skill);
    assert!(
        cards.is_empty(),
        "the plan filter posts no 「效果适用」 header: {cards:?}"
    );
    let plan_lines: Vec<_> = t
        .keys_since(mark)
        .into_iter()
        .filter(|k| k.starts_with("log.outcome.plan_") || k.contains("mashiro"))
        .collect();
    assert!(
        plan_lines.is_empty(),
        "no plan outcome and no body line: {plan_lines:?}"
    );
    // The shape itself is unchanged.
    assert_eq!(t.pos(0), 58, "6 backwards from 4");
}

/// The reverse is a plan property: nothing lands as an abnormal effect.
#[test]
fn mashiro_plan_reshape_is_not_abnormal() {
    let mut t = Table::new(&["仓田真白", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.set_pos(0, 4);
    t.dice(&[3, 3]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.state(0, "abnormalHit"), 0, "no abnormal hit latched");
    assert!(
        t.m.world().turn.abnormal.iter().all(|&v| v == 0),
        "no abnormal counter: {:?}",
        t.m.world().turn.abnormal
    );
}

/// The same holds for a conditional plan reshape (羽泽鸫's owed double-dice
/// reverse) -- the condition does not buy it a header.
#[test]
fn tsugumi_owed_plan_reshape_is_silent() {
    let mut t = Table::new(&["羽泽鸫", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.set_pos(0, 4);
    t.m.world_mut()
        .st
        .players[0]
        .state_set("skill.tsugumiPlain.owed", 1);
    let mark = t.mark();
    t.dice(&[3, 3]);
    t.roll(0).unwrap();
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert!(
        !keys.iter().any(|k| k.starts_with("log.outcome.plan_")),
        "the plan reshape posts no outcome line: {keys:?}"
    );
    // 2d20 backwards, and the one-shot is spent.
    assert_eq!(t.pos(0), 58);
    assert_eq!(
        t.state(0, "skill.tsugumiPlain.owed"),
        0,
        "the owed move is consumed"
    );
}

/// An `On::Action(Action::DiceRollPlan(..))` entry is an offer, not a passive
/// reshape: it still asks and still announces (its header's flash then follows
/// the ordinary scope classifier).
#[test]
fn action_plan_offer_still_announces() {
    let mut t = Table::vanilla(2);
    t.dice(&[1]);
    t.give_play(0, "TEST:planOffer").unwrap();
    drain(&mut t);
    let mark = t.mark();
    t.roll(0).unwrap();
    assert!(
        t.prompt().is_some(),
        "the offer asks: {}",
        t.dump_prompt()
    );
    t.answer_one(0).unwrap(); // yes
    drain(&mut t);
    let cards = card_events(&t, mark, "TEST:planOffer");
    assert!(
        !cards.is_empty(),
        "an Action at the plan pass announces: {:?}",
        t.recent_keys(30)
    );
    assert_eq!(t.pos(0), 3, "and its body still pins the walk");
}
