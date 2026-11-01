//! G03 review regression: dump the dice-shaped events of a kanon main move.

mod common;
use common::*;

#[test]
fn zz_probe_dice_log() {
    let mut t = Table::new(&["松原花音", "户山香澄"]);
    t.begin_turn(0);
    while t.prompt().is_some() {
        t.decline();
    }
    let mark = t.mark();
    t.dice(&[8, 3]);
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    for e in t.events_since(mark) {
        eprintln!("{} {:?}", e.r#type, e.msg);
    }
}
