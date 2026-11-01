//! G03 review regression: G03 pick vs one-shot boost.

mod common;
use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn pick(t: &mut Table, who: usize, needle: &str) {
    let k = t
        .option(needle)
        .unwrap_or_else(|| panic!("{needle} not offered: {}", t.dump_prompt()));
    t.answer(who, k).unwrap();
}

const STARRY: &str = "Mor:蝴蝶飞舞的星月夜";
const WANT_HUMAN: &str = "CRYCHIC:想要成为人类";

#[test]
fn probe_boost_keep_old() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "户山香澄"]);
    t.strip_skills();
    t.clean();
    t.place_raw(0, WANT_HUMAN);
    t.place_raw(0, STARRY);
    t.m.world_mut().set_slot(0, "want_human_x", 15);
    t.set_crystals(0, WANT_HUMAN, 2);
    t.set_crystals(0, STARRY, 2);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 0);
    t.dice(&[7, 3]);
    t.roll(0).unwrap();
    pick(&mut t, 0, "starry_night_keep_old");
    drain(&mut t);
    assert_eq!(t.pos(0), 7 + 5, "PROBE keep-old kept 7 (+5)");
}

#[test]
fn probe_boost_keep_new() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "户山香澄"]);
    t.strip_skills();
    t.clean();
    t.place_raw(0, WANT_HUMAN);
    t.place_raw(0, STARRY);
    t.m.world_mut().set_slot(0, "want_human_x", 15);
    t.set_crystals(0, WANT_HUMAN, 2);
    t.set_crystals(0, STARRY, 2);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 0);
    t.dice(&[7, 3]);
    t.roll(0).unwrap();
    pick(&mut t, 0, "starry_night_keep_new");
    drain(&mut t);
    assert_eq!(t.pos(0), 3 + 5, "PROBE keep-new kept 3 (+5)");
}
