//! G03 review regression: with three casts on the transaction (starry's
//! reroll, then Taki's counter-reroll), the keep-old/new answer is an option
//! index but `roll_pick` reads it as an absolute cast index.

mod common;
use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

const STARRY: &str = "Mor:蝴蝶飞舞的星月夜";
const TAKI: &str = "CRYCHIC:（立希）即便比不上...";

#[test]
fn zz_three_casts_keep_new_uses_the_newest() {
    let mut t = Table::new(&["桐谷透子", "户山香澄"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, STARRY);
    t.set_crystals(0, STARRY, 2);
    t.give(0, &[TAKI]);
    drain(&mut t);
    t.set_pos(0, 0);
    t.dice(&[7, 3, 9]);
    t.roll(0).unwrap();
    assert!(
        t.counteract_offered(TAKI),
        "taki offered at the recast's Result: {}",
        t.dump_prompt()
    );
    t.counteract(0, TAKI).unwrap();
    drain_until_choice(&mut t);
    let opts = t.dump_prompt();
    let k = t
        .option("starry_night_keep_new")
        .unwrap_or_else(|| panic!("keep-new not offered: {opts}"));
    t.answer(0, k).unwrap();
    drain(&mut t);
    let keys = t.recent_keys(40);
    assert_eq!(
        t.pos(0),
        9,
        "keep-new must use the newest cast 9, not the middle 3: {keys:?}"
    );
}

#[test]
fn zz_half_speed_unused_face_is_raw() {
    let mut t = Table::new(&["和奏瑞依", "户山香澄"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, "AG:（摩卡）0.5倍速");
    t.place_raw(0, "RAS:（和奏瑞依）寄于指尖的执念");
    t.place_raw(0, "TEST:fireReroll");
    drain(&mut t);
    t.set_pos(0, 0);
    t.set_fire(0, 1, 1);
    t.dice(&[7, 3]);
    t.roll(0).unwrap();
    assert!(
        t.counteract_offered("RAS:（和奏瑞依）寄于指尖的执念"),
        "layer keep offered at the fire recast's Before: {}",
        t.dump_prompt()
    );
    t.counteract(0, "RAS:（和奏瑞依）寄于指尖的执念").unwrap();
    // The pick prompt: print the offered labels (the shown faces are settled).
    drain_until(&mut t, "fire_keep_new");
    eprintln!("PICK PROMPT: {}", t.dump_prompt());
    let k = t.option("fire_keep_new").unwrap();
    t.answer(0, k).unwrap();
    drain(&mut t);
    // initial raw 7 settles to ceil(7/2)=4; the kept new 3 settles to 2.
    assert_eq!(t.pos(0), 2, "the kept new face settles through the half");
    assert_eq!(
        t.state(0, "layer_keep_0"),
        7,
        "the banked unused face (probe: raw 7, settled would be 4)"
    );
    // And the shown old face in the prompt:
    // (dumped above; starry-style messages put the settled face in the text)
}

/// Decline any non-choice prompt (yes/no) until a choice offering `needle` is open.
fn drain_until(t: &mut Table, needle: &str) {
    loop {
        let Some(p) = t.prompt() else { return };
        if p.kind == "choice" && p.options.iter().any(|o| format!("{o:?}").contains(needle)) {
            return;
        }
        t.decline();
    }
}

/// Decline any non-choice prompt (yes/no) until the starry pick is open.
fn drain_until_choice(t: &mut Table) {
    loop {
        let Some(p) = t.prompt() else { return };
        if p.kind == "choice"
            && p.options
                .iter()
                .any(|o| format!("{o:?}").contains("starry_night_keep_new"))
        {
            return;
        }
        t.decline();
    }
}

#[test]
fn zz_four_casts_keep_old_uses_the_pair_old() {
    let mut t = Table::new(&["桐谷透子", "户山香澄"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, STARRY);
    t.set_crystals(0, STARRY, 2);
    t.give(0, &[TAKI]);
    drain(&mut t);
    t.set_pos(0, 0);
    // 4 (initial), 3 (starry), 3 (taki, repeats -> ask again), 8 (taki again)
    t.dice(&[4, 3, 3, 8]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered(TAKI), "{}", t.dump_prompt());
    t.counteract(0, TAKI).unwrap();
    assert_eq!(
        t.prompt().unwrap().title.key(),
        "cards:card-crychic.taki_again_title"
    );
    t.answer(0, 0).unwrap(); // reroll again
    drain_until(&mut t, "starry_night_keep_new");
    let k = t.option("starry_night_keep_old").unwrap();
    t.answer(0, k).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(0),
        3,
        "keep-old must be the pair's old cast 3, not cast 0 (4)"
    );
}

#[test]
fn zz_starry_prompt_faces_are_settled() {
    let mut t = Table::new(&["桐谷透子", "户山香澄"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, STARRY);
    t.place_raw(0, "AG:（摩卡）0.5倍速");
    t.set_crystals(0, STARRY, 2);
    drain(&mut t);
    t.set_pos(0, 0);
    t.dice(&[7, 7]);
    t.roll(0).unwrap();
    drain_until(&mut t, "starry_night_keep_new");
    eprintln!("STARRY PROMPT: {}", t.dump_prompt());
    let k = t.option("starry_night_keep_new").unwrap();
    t.answer(0, k).unwrap();
    drain(&mut t);
    eprintln!("POS: {}", t.pos(0));
    assert_eq!(t.pos(0), 4);
}

/// Layer's own fire reroll opens the pair; a counteraction (taki) then recasts
/// on the same `Roll` link, so three casts exist by the time the player picks.
/// The keep answer is an option index, `roll_pick` an absolute cast index:
/// 「keep new」 must name the newest cast, not cast 1.
#[test]
fn zz_layer_fire_reroll_keep_new_uses_the_newest() {
    let mut t = Table::new(&["和奏瑞依", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 1, 1);
    t.give(0, &[TAKI]);
    drain(&mut t);
    t.set_pos(0, 0);
    // 7 (initial), 3 (layer's fire recast), 9 (taki's counter recast).
    t.dice(&[7, 3, 9]);
    t.roll(0).unwrap();
    // Layer's reroll ask is a yes/no, which `drain_until` (choice-only) would
    // walk past: decline anything else until it is the open prompt.
    loop {
        let p = t.prompt().expect("layer reroll prompt");
        if p.title.key().ends_with("layer_again_title") {
            break;
        }
        t.decline();
    }
    // `ask_yes` answers 0.
    t.answer_one(0).unwrap();
    // The recast runs its own ladder; decline whatever else surfaces until
    // taki's Result window is the open prompt.
    let mut dumped = String::new();
    while !t.counteract_offered(TAKI) {
        let Some(p) = t.prompt() else { break };
        dumped = t.dump_prompt();
        if p.kind == "choice" && dumped.contains("layer_keep_new") {
            break;
        }
        t.decline();
    }
    assert!(
        t.counteract_offered(TAKI),
        "taki offered at the fire recast's Result: {dumped}"
    );
    t.counteract(0, TAKI).unwrap();
    drain_until(&mut t, "layer_keep_new");
    let k = t
        .option("layer_keep_new")
        .unwrap_or_else(|| panic!("keep-new not offered: {}", t.dump_prompt()));
    t.answer(0, k).unwrap();
    drain(&mut t);
    let keys = t.recent_keys(40);
    assert_eq!(
        t.pos(0),
        9,
        "keep-new must use the newest cast 9, not cast 1 (3): {keys:?}"
    );
}
