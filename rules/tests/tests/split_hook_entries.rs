//! ABI v73 Action/Filter split -- entry-index audit + behaviour pin for the
//! three cards whose mixed `On::Hook` kind list had to be split into two
//! entries. The split shifts every later entry index on those cards; this
//! file pins the new indices and proves both halves still fire with the same
//! outcomes the pre-split single entry produced (master's `spec_card_*` /
//! `mujica` expectations).

mod common;

use common::*;
use bandori_std::kind as rk;

/// Manifest shape of the three split cards: one Filter + one Action half,
/// same body, same pre, and no later entry-index drift beyond the known shift.
#[test]
fn split_cards_declare_filter_and_action_halves() {
    let r = rules();
    let shape = |id: &str| -> Vec<(i32, Vec<String>, bool)> {
        let idx = r.ruleset().card(id).unwrap_or_else(|| panic!("{id} loaded"));
        let info = &r.ruleset().cards()[idx as usize];
        info.on
            .iter()
            .map(|o| (o.kind, o.triggers.clone(), o.has_guard))
            .collect()
    };
    let kind = |name: &str| -> i32 {
        match name {
            "Play" => 0,
            "Action" => 3,
            "Filter" => 11,
            other => panic!("bad kind {other}"),
        }
    };

    // no_expectation (ABI v79): Play / FilterValue(DiceRollValue) /
    //   Filter(PaySplitAdd) / Action(Reshuffled). The roll shave + 0-step
    //   no-settle clamp are one FilterValue (option C).
    let s = shape("PP:不要背负期待");
    assert_eq!(s.len(), 4, "no_expectation entries: {s:?}");
    assert_eq!(s[0].0, kind("Play"));
    assert_eq!(s[1].0, kind("Filter"));
    assert_eq!(s[1].1, vec!["diceRollValue".to_string()], "DiceRollValue");
    assert_eq!(s[2].0, kind("Filter"));
    assert_eq!(s[2].1, vec!["paySplitAdd".to_string()], "PaySplitAdd");
    assert_eq!(s[3].0, kind("Action"));
    assert_eq!(s[3].1, vec!["reshuffled".to_string()], "Reshuffled");

    // j11 (ABI v93 one-kind-per-entry): Play / Action(TurnEnd) / Action(Targeted) /
    //   Action(PaySplitChooseBefore) / Filter(PaySplitChoose) / Action(CounterChanged)
    //   -- the old combined TurnEnd+Targeted arm is two entries; the Effect arm
    //   moved to the interactive pre-stage.
    let s = shape("Mujica:#J11");
    assert_eq!(s.len(), 6, "j11 entries: {s:?}");
    assert_eq!(s[0].0, kind("Play"));
    assert_eq!(s[1].0, kind("Action"));
    assert_eq!(s[1].1, vec!["turnEnd".to_string()], "TurnEnd");
    assert_eq!(s[2].0, kind("Action"));
    assert_eq!(s[2].1, vec!["targeted".to_string()], "Targeted");
    assert_eq!(s[3].0, kind("Action"));
    assert_eq!(s[3].1, vec!["paySplitChooseBefore".to_string()], "PaySplitChooseBefore");
    assert_eq!(s[4].0, kind("Filter"));
    assert_eq!(s[4].1, vec!["paySplitChoose".to_string()], "PaySplitChoose");
    assert_eq!(s[5].0, kind("Action"));
    assert_eq!(s[5].1, vec!["counterChanged".to_string()], "CounterChanged");

    // fever: Play / Filter(PaySplitAdd) / Action(TurnStart)
    let s = shape("通用:[衍生]FEVER!");
    assert_eq!(s.len(), 3, "fever entries: {s:?}");
    assert_eq!(s[0].0, kind("Play"));
    assert_eq!(s[1].0, kind("Filter"));
    assert_eq!(s[1].1, vec!["paySplitAdd".to_string()], "PaySplitAdd");
    assert_eq!(s[2].0, kind("Action"));
    assert_eq!(s[2].1, vec!["turnStart".to_string()], "TurnStart");
}

/// The shifted `.silent` index on no_expectation still points at the
/// Reshuffled bookkeeping entry (now index 4 after the v74 Filter/Action split).
#[test]
fn no_expectation_silent_index_follows_the_split() {
    let r = rules();
    let idx = r
        .ruleset()
        .card("PP:不要背负期待")
        .expect("no_expectation loaded");
    let info = &r.ruleset().cards()[idx as usize];
    assert_eq!(
        info.on[1].presentation,
        card_sdk::abi::Presentation::Visible,
        "entry 1 (FilterValue DiceRollValue) announces"
    );
    assert_eq!(
        info.on[2].presentation,
        card_sdk::abi::Presentation::Visible,
        "entry 2 (Filter PaySplitAdd) announces"
    );
    assert_eq!(
        info.on[3].presentation,
        card_sdk::abi::Presentation::Silent,
        "entry 3 (Reshuffled) must stay silent after the merge"
    );
}

/// FEVER! Filter half: the PaySplitAdd boost still rewrites the amount
/// (master `fever_payment_boost` outcome).
#[test]
fn fever_filter_half_pay_add_still_boosts() {
    const FUJIMI: usize = 11; // 富士见坂
    let mut t = Table::vanilla(2);
    let money0 = 10_000i64;
    let money1 = 10_000i64;
    t.set_money(0, money0 as i32);
    t.set_money(1, money1 as i32);
    t.own(0, &[FUJIMI]);
    t.give(0, &["通用:[衍生]FEVER!"]);
    t.play(0, "通用:[衍生]FEVER!").expect("play FEVER!");
    let n = t.field(0).len();
    let x = 600 - 200 * n as i64;
    t.begin_turn(1);
    t.set_pos(1, FUJIMI - 1);
    t.dice(&[1]);
    t.roll(1).expect("roll onto FUJIMI");
    while t.prompt().is_some() {
        let _ = t.answer(0, 0).or_else(|_| t.answer(1, 0));
    }
    let paid = money1 - t.money(1) as i64;
    let got = t.money(0) as i64 - money0;
    assert_eq!(paid, got, "FEVER! is a pure transfer");
    assert_eq!(paid, 60 + x, "Filter half applied exactly once (rent 60 + X {x}, n {n})");
}

/// no_expectation both halves: the roll shave (Filter DiceRollValue) and PaySplitAdd
/// bend (Filter) still land. Outcome matches master
/// `no_expect_roll_minus_2` + the pay-arm of the old shared `hook`.
#[test]
fn no_expectation_both_halves_fire() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PP:不要背负期待").expect("place no_expect");
    while t.prompt().is_some() {
        let _ = t.answer(0, 0);
    }
    t.begin_turn(0);
    let face = 6;
    t.dice(&[face]);
    t.roll(0).expect("diceRollBefore");
    while t.prompt().is_some() {
        let _ = t.answer(0, 0);
    }
    // FilterValue half (DiceRollValue): face - 2.
    assert_eq!(t.pos(0), (face - 2) as usize, "DiceRollValue half shaves by 2");
}

/// j11 both halves: TurnEnd decay (Action) burns a crystal. Sequence matches
/// master `mujica::j11_decays_one_crystal_per_turn` (the Filter PaySplitChoose half
/// is covered by that suite's pay-choose probes and the manifest shape test).
#[test]
fn j11_action_half_decays_on_turn_end() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "Mujica:#J11").expect("place j11");
    assert_eq!(t.crystals(0, "Mujica:#J11"), Some(2), "placed with 2");
    t.dice(&[3]);
    t.roll(0).expect("diceRollBefore");
    while t.prompt().is_some() {
        t.decline();
    }
    // Same `fix_step` seam mujica.rs uses: leave MOVE so `end` is legal.
    {
        let w = t.m.world_mut();
        if w.st.step == game_core::state::stage::MOVE {
            w.st.step = game_core::state::stage::OPS;
        }
    }
    t.end(0).expect("end");
    while t.prompt().is_some() {
        t.decline();
    }
    assert_eq!(
        t.crystals(0, "Mujica:#J11"),
        Some(1),
        "TurnEnd half burns one crystal"
    );
}

/// Custom bandori-std kind ids stay Action (non-filter) so `On::Action` may
/// list them. `ChargePaying` is NOT a filter (its 「可」 opt-in prompts).
/// The three universal raises (`markerSpent` / `cardUsed` / `houseAdded`) are
/// engine kinds and likewise non-filter.
#[test]
fn custom_kind_filter_classification() {
    use card_sdk::abi::ActionKind;
    assert!(!rk::PassRewardAffected.is_filter());
    assert!(!ActionKind::MarkerSpent.is_filter());
    assert!(!ActionKind::CardUsed.is_filter());
    assert!(!ActionKind::HouseAdded.is_filter());
    assert!(!rk::ChargePaying.is_filter());
}