//! The rulebook harness itself (`tests/common`): setup, loaded dice, turn
//! control. If these fail, every `rb_*` result is suspect.

mod common;
use common::*;

#[test]
fn vanilla_table_is_clean_and_at_player_0s_ops() {
    let t = Table::vanilla(3);
    assert_eq!(t.turn(), 0);
    assert_eq!(t.step(), game_core::state::stage::OPS);
    for who in 0..3 {
        assert_eq!(t.money(who), 10_000);
        assert_eq!(t.pos(who), 0);
        assert!(t.hand(who).is_empty());
        assert!(t.skills(who).is_empty(), "{:?}", t.field_ids(who));
    }
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
}

#[test]
fn a_full_table_binds_skills() {
    let t = Table::new(&["户山香澄", "美竹兰"]);
    assert!(t.skills(0).iter().any(|s| s.contains("户山香澄")), "{:?}", t.skills(0));
    assert!(t.skills(1).iter().any(|s| s.contains("美竹兰")), "{:?}", t.skills(1));
}

#[test]
fn loaded_dice_drive_the_main_roll() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 2);
    // A plain main move: the d20 shows 5. Land on 7 (江户川公园, unowned).
    t.dice(&[5]);
    t.roll(0).unwrap();
    assert_eq!(t.pos(0), 7, "events: {:?}", t.recent_keys(10));
    assert_eq!(t.dice_left(), 0);
}

#[test]
fn begin_turn_and_end_pass_the_turn() {
    let mut t = Table::vanilla(3);
    t.begin_turn(2);
    assert_eq!(t.turn(), 2);
    t.dice(&[1]);
    t.roll(2).unwrap();
    // Landing on an unowned deed offers nothing until we ask; end the turn.
    while t.prompt().is_some() {
        t.decline();
    }
    t.end(2).unwrap();
    assert_eq!(t.turn(), 0);
    assert_eq!(t.step(), game_core::state::stage::OPS);
}

#[test]
fn a_counter_window_is_offered_to_a_human_holding_a_counter() {
    // Every player is human, so holding an eligible [反击] card opens a
    // window. (Which card answers what is the rb_* files' business; this only
    // proves the harness surfaces the window and can answer it.)
    let mut t = Table::vanilla(2);
    t.give(1, &["AG:宣战布告"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    assert!(t.react_offered("AG:宣战布告"), "{}", t.dump_prompt());
    assert_eq!(t.asked(), vec![1]);
    t.decline();
    assert!(t.prompt().is_none(), "{}", t.dump_prompt());
}

#[test]
fn a_plain_card_plays() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "R:[衍生] 压").unwrap();
    assert_eq!(t.money(0), 11_000);
}
