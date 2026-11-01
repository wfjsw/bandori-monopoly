//! G03 review regression: taki reroll: a brand-new face must end the loop.

mod common;
use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

const TAKI: &str = "CRYCHIC:（立希）即便比不上...";

fn setup() -> Table {
    let mut t = Table::new(&["椎名立希（CRYCHIC）", "高松灯（CRYCHIC）"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &[TAKI]);
    t.set_pos(0, 0);
    t
}

#[test]
fn zz_probe_taki_new_face_ends_the_loop() {
    let mut t = setup();
    // The first cast (3) is followed by a brand-new 5: no repeat, so the card
    // has nothing left to offer.
    t.dice(&[3, 5]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered(TAKI), "{}", t.dump_prompt());
    t.counteract(0, TAKI).unwrap();
    let dump = t.dump_prompt();
    println!("PROBE after recast to a new 5: {dump}");
    drain(&mut t);
    assert_eq!(t.pos(0), 5);
    assert!(
        !dump.contains("taki_again"),
        "PROBE: a brand-new face still offered a reroll: {dump}"
    );
}

#[test]
fn zz_probe_taki_accepted_new_face_ends_the_loop() {
    let mut t = setup();
    // 3 -> recast 3 (a repeat, so the prompt is owed) -> accepting recasts 9, a
    // brand-new face: the loop must end there without asking again.
    t.dice(&[3, 3, 9]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered(TAKI), "{}", t.dump_prompt());
    t.counteract(0, TAKI).unwrap();
    assert!(
        t.dump_prompt().contains("taki_again"),
        "{}",
        t.dump_prompt()
    );
    t.answer(0, 0).unwrap(); // yes: another recast (9)
    let dump = t.dump_prompt();
    assert!(
        !dump.contains("taki_again"),
        "PROBE: the loop must end on the brand-new 9: {dump}"
    );
    drain(&mut t);
    assert_eq!(t.pos(0), 9);
}

#[test]
fn zz_probe_taki_repeat_keeps_prompting() {
    let mut t = setup();
    // 3 again: a repeat, so the prompt is expected here.
    t.dice(&[3, 3]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered(TAKI), "{}", t.dump_prompt());
    t.counteract(0, TAKI).unwrap();
    let dump = t.dump_prompt();
    println!("PROBE after recast to a repeat 3: {dump}");
    assert!(
        dump.contains("taki_again"),
        "PROBE: a repeat must still offer the reroll: {dump}"
    );
    drain(&mut t);
    assert_eq!(t.pos(0), 3);
}
