//! G03 review regression: is a roll window's `actor` the mover or the
//! 幻觉来了 substitute? no_expectation is `actor == owner` on DiceRollAdd.

mod common;
use common::*;

use game_core::state::stage;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..60 {
        if t.turn() == who && t.step() == stage::OPS {
            return;
        }
        let cur = t.turn();
        if t.turn() == who {
            t.end(who).unwrap();
            drain(t);
            continue;
        }
        t.m.world_mut().st.skip_move = true;
        t.end(cur).unwrap();
        drain(t);
    }
    panic!("never reached turn {who}");
}

fn draw_event_quiet(t: &mut Table, who: usize, event: &str) {
    let mut deck: Vec<String> = t.event_deck().into_iter().filter(|e| e != event).collect();
    deck.insert(0, event.to_string());
    let deck: Vec<&str> = deck.iter().map(String::as_str).collect();
    t.set_event_deck(&deck);
    until_turn(t, who);
    t.set_pos(who, tile("CiRCLE 咖啡厅") - 1);
    t.dice(&[1]);
    t.roll(who).unwrap();
    drain(t);
}

const NE: &str = "PP:不要背负期待";

fn fired(t: &Table, mark: i32) -> Vec<String> {
    t.events_since(mark)
        .into_iter()
        .map(|e| format!("{:?}", e.msg))
        .filter(|s| s.contains("no_expectation_roll"))
        .collect()
}

fn sub_log(t: &Table, mark: i32) -> Vec<String> {
    t.events_since(mark)
        .into_iter()
        .map(|e| format!("{:?}", e.msg))
        .filter(|s| s.contains("rolled_by"))
        .collect()
}

fn base(hallucination: bool) -> Table {
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    if hallucination {
        draw_event_quiet(&mut t, 0, "幻觉来了");
    }
    until_turn(&mut t, 1);
    t
}

/// Control: without the event, the mover's own card shaves.
#[test]
fn zz_control_no_event_mover_card_fires() {
    let mut t = base(false);
    t.place_raw(1, NE);
    let mark = t.mark();
    t.dice(&[10]);
    t.roll(1).unwrap();
    drain(&mut t);
    println!("PROBE control: shave={:?}", fired(&t, mark));
    assert!(!fired(&t, mark).is_empty(), "control: card must fire at all");
    assert!(sub_log(&t, mark).is_empty(), "control: no substitution");
}

/// Control: the substitute's card fires on its own roll.
#[test]
fn zz_control_substitute_own_roll_fires() {
    let mut t = base(false);
    t.place_raw(0, NE);
    until_turn(&mut t, 0);
    let mark = t.mark();
    t.dice(&[10]);
    t.roll(0).unwrap();
    drain(&mut t);
    println!("PROBE control2: shave={:?}", fired(&t, mark));
    assert!(!fired(&t, mark).is_empty(), "control2: card fires on own roll");
}

#[test]
fn zz_hallucination_actor_is_the_mover() {
    let mut t = base(true);
    t.place_raw(1, NE);
    let mark = t.mark();
    t.dice(&[10]);
    t.roll(1).unwrap();
    drain(&mut t);
    let subs = sub_log(&t, mark);
    println!("PROBE mover-owned: shave={:?} sub={subs:?}", fired(&t, mark));
    assert!(!subs.is_empty(), "P0 must be recorded as the stand-in roller");
    assert!(!fired(&t, mark).is_empty(), "the mover's own card must apply");
}

#[test]
fn zz_hallucination_actor_is_not_the_substitute() {
    let mut t = base(true);
    t.place_raw(0, NE);
    let mark = t.mark();
    t.dice(&[10]);
    t.roll(1).unwrap();
    drain(&mut t);
    let subs = sub_log(&t, mark);
    println!("PROBE substitute-owned: shave={:?} sub={subs:?}", fired(&t, mark));
    assert!(!subs.is_empty(), "P0 must be recorded as the stand-in roller");
    assert!(fired(&t, mark).is_empty(), "the substitute's card must NOT apply");
}
