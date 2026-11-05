//! Throwaway probe: take a board-owned tile rule row off the board into a hand
//! and into a deck, and show where the name lands.

mod common;

use common::Table;

fn board_uids(t: &Table) -> Vec<(i32, String)> {
    t.m.world()
        .field_instances(game_core::state::BOARD_OWNER)
}

fn dump(t: &Table, tag: &str) {
    eprintln!("{tag}");
    for (i, h) in t.m.world().hidden.iter().enumerate() {
        eprintln!("  p{i} hand={:?} deck={:?} gy={:?}", h.hand, h.deck, h.graveyard);
    }
    eprintln!("  board_discard={:?}", t.m.world().st.board_discard);
    eprintln!("  board={:?}", board_uids(t));
}

#[test]
fn probe_one_of_us_leaves_a_deed_name() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "美竹兰");
    t.set_character_raw(1, "青叶摩卡");
    t.own(0, &[44]);
    t.own(1, &[11]);
    t.give_play(0, "AG:ONE OF US").unwrap();
    let k = t.option("PlayerId(1)").expect("partner prompt");
    t.answer(0, k).unwrap();
    t.answer_tile(0, 44).unwrap();
    t.answer_tile(0, 11).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    eprintln!("one_of_us hand0={:?}", t.hand(0));
    eprintln!(
        "attached={:?}",
        t.m.world()
            .field_instances(0)
            .into_iter()
            .find(|(_, c)| c == "AG:ONE OF US")
            .map(|(u, _)| t.m.world().attached_cards(u))
    );
}

#[test]
fn probe_board_row_to_hand() {
    for dest in [1, 4] {
        let mut t = Table::vanilla(2);
        t.give(0, &["TEST:drop"]);
        let uid = board_uids(&t)[0].0;
        eprintln!("== dest {dest}, taking board uid {uid}");
        t.set_state(0, "TEST.drop.uid", uid);
        t.set_state(0, "TEST.drop.dest", dest);
        t.play(0, "TEST:drop").unwrap();
        dump(&t, "AFTER");
    }
}
