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
        t.recent_keys(24).iter().any(|k| k == "log.stay_skip"),
        "the stop is logged through the ruleset key: {:?}",
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
        t.recent_keys(24).iter().all(|k| k != "log.stay_skip"),
        "no stop under [不可阻挡]: {:?}",
        t.recent_keys(24)
    );
}

#[test]
fn blocked_side_move_takes_no_step_and_settles_nothing() {
    // 47: 「无法使用非[传送]的移动效果（…则抵消该效果）」 -- a forced move onto a
    // staying player is cancelled outright, not walked one tile.
    let mut t = Table::new(&["户山香澄", "lock"]);
    t.strip_skills();
    t.clean();
    t.own(0, &[5]);
    t.set_pos(1, 5);
    t.set_state(1, "stay", 1);
    t.give(0, &["TEST:forcer"]);
    t.play(0, "TEST:forcer").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(1), 5, "the cancelled move leaves the target put");
    assert_eq!(t.money(1), 10_000, "and settles nothing");
}

#[test]
fn settle_answer_settle_keeps_the_stop_settle_once() {
    let mut t = armed();
    t.set_state(0, "resolveOverride", 1);
    t.dice(&[5]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 2, "still stops at the layer's square");
    assert_eq!(t.money(0), 10_000 - 260, "the answer keeps the settle here");
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.money(0),
        10_000 - 260,
        "and the turn end must not repeat it"
    );
}

#[test]
fn settle_answer_skip_settles_nowhere() {
    let mut t = armed();
    t.set_state(0, "resolveOverride", 2);
    t.dice(&[5]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000, "the answer pins the settle off");
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000, "and no turn-end copy either");
}

#[test]
fn a_follower_still_settles_when_the_mover_stops() {
    // `CRYCHIC:（祥子）带领着大家` replays the main move on the players sharing
    // the tile. The mover's withheld settle is its own: a follower walks the
    // whole move and settles its landing.
    let mut t = Table::new(&["丰川祥子（CRYCHIC）", "高松灯（CRYCHIC）"]);
    t.strip_skills();
    t.clean();
    t.set_pos(0, 1);
    t.set_pos(1, 1);
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &["CRYCHIC:（祥子）带领着大家", "TEST:passStay"]);
    t.play(0, "CRYCHIC:（祥子）带领着大家").unwrap();
    drain(&mut t);
    t.play(0, "TEST:passStay").unwrap();
    drain(&mut t);
    t.own(0, &[6]);
    t.dice(&[5]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 2, "the mover stops on the layer's square");
    assert_eq!(t.pos(1), 6, "the follower walks the whole move");
    // 6 is the mover's: the follower pays its rent (160, halved by the card to
    // 80). Had the mover's settle run at 2 as well, tile 2 would have charged
    // one of them on top.
    assert_eq!(
        t.money(1),
        10_000 - 80,
        "the follower still settles its landing"
    );
    assert_eq!(
        t.money(0),
        10_000 + 80,
        "the mover's own settle stayed withheld"
    );
}
