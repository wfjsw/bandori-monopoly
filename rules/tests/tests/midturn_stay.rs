//! C-Q7 (MOVE-05): a [停留] gained *during* a move applies at once.
//!
//! Glossary 47 「[无法移动]：处于该状态时[主要移动]的[移动终点]视为当前所在格子
//! 且不[结算]」 + user ruling 2026-11-20: the layer stops the running walk on the
//! square the mover stands on, a main move withholds its settle (it runs at that
//! turn's end), and the layer wears off at the same turn end so it never costs
//! the next turn as well.
//!
//! `TEST:passStay` lands the layer on the mover's first `pass` of the walk --
//! the in-move span the ruling is about.

mod common;
use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// P0 stands on 购物中心 (1); the first step lands 天文馆 (2, P1's, rent 260).
fn armed() -> Table {
    let mut t = Table::new(&["户山香澄", "lock"]);
    t.strip_skills();
    t.clean();
    t.own(1, &[2]);
    t.set_pos(0, 1);
    t.give(0, &["TEST:passStay"]);
    t.play(0, "TEST:passStay").unwrap();
    drain(&mut t);
    assert_eq!(t.state(0, "stay"), 0, "the play alone grants nothing");
    t
}

#[test]
fn mid_walk_stay_stops_the_walk_and_defers_the_settle() {
    let mut t = armed();
    t.dice(&[5]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(0),
        2,
        "the walk stops on the square it reached when the stay landed"
    );
    assert_eq!(
        t.money(0),
        10_000,
        "the stopped square's settle is withheld"
    );
    assert!(
        t.recent_keys(24).iter().any(|k| k == "log.stay_stop"),
        "the stop is logged: {:?}",
        t.recent_keys(24)
    );
    t.end(0).unwrap();
    drain(&mut t);
    assert!(
        t.money(0) < 10_000,
        "the withheld settle runs at the turn end (money {})",
        t.money(0)
    );
    assert_eq!(
        t.state(0, "stay"),
        0,
        "the mid-turn layer wears off at this turn's end"
    );
}

#[test]
fn unstoppable_walks_through_the_mid_walk_stay() {
    let mut t = armed();
    t.set_state(0, "unstoppable", 1);
    t.dice(&[5]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 6, "the whole walk ran");
    // One layer per passed tile: [不可阻挡] 「无视」s the gate, it does not
    // refuse stay layers (rules.txt 51 refuses 晕眩 / 除外 only).
    assert_eq!(t.state(0, "stay"), 5, "the layers still landed");
    assert!(
        t.recent_keys(24).iter().all(|k| k != "log.stay_stop"),
        "no stop under [不可阻挡]: {:?}",
        t.recent_keys(24)
    );
}
