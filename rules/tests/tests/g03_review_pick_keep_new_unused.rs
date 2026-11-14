//! G03 review regression: production `roll_pick` on keep-new (index n-1,
//! already chosen by the recast) must still record the old face as unused.

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

const LAYER_KEEP: &str = "RAS:（和奏瑞依）寄于指尖的执念";

#[test]
fn probe_layer_keep_keep_new_banks_the_old_face() {
    let mut t = Table::new(&["和奏瑞依", "户山香澄"]);
    t.strip_skills();
    t.clean();
    t.place_raw(0, LAYER_KEEP);
    t.place_raw(0, "TEST:fireReroll");
    drain(&mut t);
    t.set_pos(0, 0);
    t.set_fire(0, 1, 1);
    t.dice(&[5, 3]);
    t.roll(0).unwrap();
    // The fire recast raises its own Before with source 1; the keep window opens.
    assert!(t.counteract_offered(LAYER_KEEP), "{}", t.dump_prompt());
    t.counteract(0, LAYER_KEEP).unwrap();
    // Keep the NEW face (index 1 == the recast's own cast, hence already chosen).
    let mark = t.mark();
    pick(&mut t, 0, "fire_keep_new");
    drain(&mut t);
    // The fixture logs the `roll_reroll` outcome (`unused`).
    let logged = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.msg.key().ends_with("fire_recast_note"))
        .find_map(|e| match e.msg.args.get("unused") {
            Some(game_core::msg::Arg::Integer(v)) => Some(*v),
            _ => None,
        })
        .expect("the reroll outcome logged");
    assert_eq!(t.pos(0), 3, "the new face settles");
    assert_eq!(logged, 5, "keep-new recorded the old face 5 as unused");
    assert_eq!(
        t.state(0, "layer_keep_0"),
        5,
        "the card banks the unpicked old face on keep-new"
    );
    assert_eq!(
        t.state(0, "layer_keep_count"),
        2,
        "one die kept (stored 1-based)"
    );
}
