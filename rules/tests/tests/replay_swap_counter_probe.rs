//! The Two-in-One **counteraction** swap (after own roll): it is a guest
//! write followed by a host request, so it must land on live state and stay
//! there past the pause.

mod common;
use common::*;

use game_core::record::Replayer;

#[test]
fn play_swap_live() {
    let mut t = Table::new(&["三角初华（Sumimi）", "高松灯"]);
    t.clean();
    let before = t.m.state().players[0].persona.clone();
    t.give_play(0, "Sumimi:Sumimi是二人一体的").expect("play");
    let after = t.m.state().players[0].persona.clone();
    eprintln!("play: {before} -> {after}");
    assert_eq!(after, "纯田真奈", "the play's set_persona landed");
}

#[test]
fn counter_swap_live() {
    let mut t = Table::new(&["三角初华（Sumimi）", "高松灯"]);
    t.clean();
    t.give(0, &["Sumimi:Sumimi是二人一体的"]);
    let before = t.m.state().players[0].persona.clone();
    eprintln!("persona before roll: {before}");
    t.roll(0).expect("roll");
    eprintln!("prompt after roll: {}", t.dump_prompt());
    assert!(
        t.counteract_offered("Sumimi:Sumimi是二人一体的"),
        "counter offered: {}",
        t.dump_prompt()
    );
    t.counteract(0, "Sumimi:Sumimi是二人一体的")
        .expect("counteract");
    let after = t.m.state().players[0].persona.clone();
    let skills: Vec<String> = t.m.state().players[0]
        .field
        .iter()
        .map(|f| f.card.clone())
        .collect();
    eprintln!("persona after counter: {after} field={skills:?}");
    assert_eq!(after, "纯田真奈", "the counter's set_persona landed");
}
