//! G03 review regression: the fixed_roll pin must be readable by the ladder's
//! residual guards (`ctx::fixed_roll()`), so it is read rather than taken.

mod common;
use common::*;
use game_core::engine::FixedFace;
use game_core::engine::rules::RollSource;

fn pin(t: &mut Table, v: i32) {
    t.m.world_mut().turn.fixed_roll = Some(FixedFace { value: v, source: RollSource::Unattributed });
}

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

const STARRY: &str = "Mor:蝴蝶飞舞的星月夜";

#[test]
fn probe_starry_over_pinned_face() {
    let mut t = Table::new(&["仓田真白", "户山香澄"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, STARRY);
    t.set_crystals(0, STARRY, 3);
    pin(&mut t, 18);
    t.dice(&[1, 1, 1, 1, 1]);
    t.roll(0).unwrap();
    let open = t.prompt();
    println!("starry: prompt after roll = {:?}", open.as_ref().map(|p| p.title.clone()));
    println!("starry: dump = {}", t.dump_prompt());
    drain(&mut t);
    println!("starry: pos = {}", t.pos(0));
}

#[test]
fn probe_eve_over_pinned_face() {
    let mut t = Table::new(&["仓田真白", "若宫伊芙"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_state(0, "ppFansUp", 2);
    t.set_state(0, "ppFansDown", 0);
    pin(&mut t, 5);
    t.dice(&[1, 1, 1, 1]);
    t.roll(0).unwrap();
    let open = t.prompt();
    println!("eve: prompt after roll = {:?}", open.as_ref().map(|p| p.title.clone()));
    println!("eve: dump = {}", t.dump_prompt());
    drain(&mut t);
    println!("eve: pos = {}", t.pos(0));
}

#[test]
fn probe_layer_over_pinned_face() {
    let mut t = Table::new(&["和奏瑞依", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 2, 2);
    let fire = t.charge(0);
    println!("layer: fire at start = {fire}");
    pin(&mut t, 5);
    t.dice(&[1, 1]);
    t.roll(0).unwrap();
    let open = t.prompt();
    println!("layer: prompt after roll = {:?}", open.as_ref().map(|p| p.title.clone()));
    println!("layer: dump = {}", t.dump_prompt());
    drain(&mut t);
    println!("layer: pos = {}", t.pos(0));
}
