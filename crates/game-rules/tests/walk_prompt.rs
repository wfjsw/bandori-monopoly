//! A walked tile pick (`ctx::ask_tile_walk`) reaches the client as `walk`.

mod common;

use common::*;

#[test]
fn rimi_choco_prompt_is_a_walk_pick() {
    let mut t = Table::vanilla(2);
    t.set_persona_raw(0, "牛込里美");
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.give_play(0, "PPP:（里美）我的心就像巧克力螺").unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.kind, "tile");
    assert!(p.walk, "rimi_choco's tile pick must be flagged walk");
}
