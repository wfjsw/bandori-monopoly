//! STACK-01: a hook body that nested-settles must run its post-settle tail
//! exactly once, and must not re-run earlier siblings in the same counteract
//! walk (resume must not replay completed bodies).

mod common;

use common::*;
use game_core::engine::CardRules;

#[test]
fn post_settle_tail_runs_once_and_siblings_not_replayed() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 3);
    t.dice(&[2]);
    place_on_tile(&mut t, 0, "TEST:settleTail", 5);
    place_on_tile(&mut t, 0, "TEST:settleSib", 5);
    let mark = t.mark();
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    let keys = t.keys_since(mark);
    let count = |k: &str| {
        keys.iter()
            .filter(|x| x.ends_with(k) || x.as_str() == k)
            .count()
    };
    assert_eq!(count("tail_before"), 1, "tail_before once, keys={keys:?}");
    assert_eq!(
        count("tail_after"),
        1,
        "tail_after once (post-settle tail), keys={keys:?}"
    );
    assert_eq!(
        count("sib_once"),
        1,
        "sibling not replayed on resume, keys={keys:?}"
    );
}

fn place_on_tile(t: &mut Table, who: usize, card: &str, tile: usize) {
    let d = data();
    let props = rules().card_props(card);
    t.m.world_mut().place_card_on(
        &d,
        who as i32,
        tile as i32,
        card,
        game_core::msg::Msg::default(),
        props,
    );
}
