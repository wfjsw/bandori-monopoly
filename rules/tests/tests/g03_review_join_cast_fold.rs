//! G03 review regression: a `do_move_roll` join cast must pass the Add/Value fold,
//! and `diceRollSettled` must carry the *settled* face.

mod common;
use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn fresh(chars: &[&str]) -> Table {
    let mut t = Table::new(chars);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t
}

/// sports rerolls through `do_move_roll` (a windowless join cast) during the
/// After window; the new cast must settle through half-speed's Value fold and
/// the min_roll floor. 3 settles to 2, reroll 12 settles to 6, floor 10.
#[test]
fn zz_probe_sports_reroll_settles_through_half_speed() {
    let mut t = fresh(&["青叶摩卡", "户山香澄"]);
    t.place_raw(0, "HHW:运动的天赋");
    t.place_raw(0, "AG:（摩卡）0.5倍速");
    t.m.world_mut().set_slot(0, "sports_talent_next", 700);
    drain(&mut t);
    t.set_pos(0, 0);
    t.dice(&[3, 12]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(0),
        10,
        "rerolled 12 must halve to 6 then floor to 10: {:?}",
        t.recent_keys(40)
    );
}

/// A Result-window write (Repaint's cut) must be re-folded before
/// `diceRollSettled`, so the event/final_face carries the settled face, not
/// the raw cut. Face 12 → +1, ×2 = 26; Repaint cuts 15 owned path tiles to 11;
/// the Settled event must say 24 (11 → +1 → ×2) and the walk must be 24.
#[test]
fn zz_probe_settled_carries_the_folded_face() {
    let mut t = fresh(&["高松灯（CRYCHIC）", "户山香澄"]);
    t.give(0, &["TEST:rollTrace"]);
    t.play(0, "TEST:rollTrace").unwrap();
    t.give(1, &["RAS:Repaint"]);
    t.own(1, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
    drain(&mut t);
    t.set_pos(0, 0);
    let mark = t.mark();
    t.dice(&[12]);
    t.roll(0).unwrap();
    if t.counteract_offered("RAS:Repaint") {
        t.counteract(1, "RAS:Repaint").unwrap();
    } else {
        panic!("Repaint not offered: {}", t.dump_prompt());
    }
    drain(&mut t);
    let mut settled: Option<i64> = None;
    let mut seen = Vec::new();
    for e in t.events_since(mark) {
        let msg = if e.msg.key() == "log.card_effect" {
            match e.msg.args.get("what") {
                Some(game_core::msg::Arg::Msg(m)) => m.as_ref(),
                _ => &e.msg,
            }
        } else {
            &e.msg
        };
        if msg.args.get("kind").is_none() || msg.args.get("face").is_none() {
            continue;
        }
        let kind = match msg.args.get("kind") {
            Some(game_core::msg::Arg::Integer(n)) => *n,
            _ => -1,
        };
        let face = match msg.args.get("face") {
            Some(game_core::msg::Arg::Integer(n)) => *n,
            _ => -1,
        };
        seen.push((kind, face));
        if kind == 124 {
            settled = Some(face);
        }
    }
    assert_eq!(t.pos(0), 24, "walk = fold(11) = 24; trace {seen:?}");
    assert_eq!(
        settled,
        Some(24),
        "Settled must carry the folded face: {seen:?}"
    );
}
