//! G03 review regression: Add-stage floor between no_expectation and want_human.

mod common;
use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

const NO_EXPECTATION: &str = "PP:不要背负期待";
const WANT_HUMAN: &str = "CRYCHIC:想要成为人类";

fn run(noexp_first: bool) -> usize {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "户山香澄"]);
    t.strip_skills();
    t.clean();
    if noexp_first {
        t.place_raw(0, NO_EXPECTATION);
        t.place_raw(0, WANT_HUMAN);
    } else {
        t.place_raw(0, WANT_HUMAN);
        t.place_raw(0, NO_EXPECTATION);
    }
    t.m.world_mut().set_slot(0, "want_human_x", 15);
    t.set_crystals(0, WANT_HUMAN, 2);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 0);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.pos(0)
}

#[test]
fn probe_add_floor() {
    let a = run(true);
    let b = run(false);
    println!("noexp-first={a} want-first={b} (book: 1-2+5 = 4)");
}
