//! G03 review regression: the turn-end withheld settle must run the tile rule
//! (`tile:edogawa` draws; the built-in body only logs).

mod common;
use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

#[test]
fn zz_blocked_turn_end_settle_runs_the_tile_rule() {
    let mut t = Table::new(&["户山香澄", "lock"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    let edogawa = tile("江户川乐器店");
    t.give(0, &["R:选择自己的舞台", "通用:雨啊，快点来吧"]);
    drain(&mut t);
    t.set_pos(0, edogawa);
    t.dice(&[1, 1]);
    t.play(0, "通用:雨啊，快点来吧").unwrap();
    t.answer(0, 0).unwrap();
    assert!(
        t.counteract_offered("R:选择自己的舞台"),
        "{}",
        t.dump_prompt()
    );
    t.counteract(0, "R:选择自己的舞台").unwrap();
    t.answer_one(0).unwrap(); // yes
    drain(&mut t);
    assert_eq!(t.state(0, "stay"), 1);
    assert_eq!(t.pos(0), edogawa, "blocked");
    let before = t.hand(0).len();
    let mark = t.mark();
    t.end(0).unwrap();
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k == "log.land_draw"),
        "the tile rule's settle ran: {keys:?}"
    );
    assert_eq!(t.hand(0).len(), before + 1, "the settle drew a card");
}
