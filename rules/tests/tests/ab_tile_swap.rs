//! ABI-13 -- the temporary **tile-rule swap** (`ctx::tile_swap_rules` /
//! `ctx::tile_clear_rule_swap`, `docs/rulebook/spec/abi/ab-tile-swap.md`).
//!
//! A rule swap rebinds which rule instances govern a board square, applied
//! immediately and reversed when the swapping instance leaves play. The
//! square's **identity** stays put: `tile_is(tile.id, "circle")` and
//! `ctx::is_start` keep reading the static board, and `prop::START` stays
//! stamped on the start square's own row.

mod common;

use common::*;
use game_core::engine::Match;
use game_core::state::{stage, TileTextEntry};

// ---------------------------------------------------------- local helpers

fn drain(t: &mut Table) {
    while let Some(p) = t.prompt() {
        if p.kind == "auction" {
            for who in t.asked() {
                let _ = t.m.act(
                    who as i32 + 1,
                    &game_core::net::NetMessage {
                        prompt: p.id,
                        value: -1,
                        ..game_core::net::NetMessage::act("answer")
                    },
                );
            }
            t.settle();
        } else {
            t.decline();
        }
    }
}

fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..60 {
        if t.turn() == who && t.step() == stage::OPS {
            return;
        }
        let cur = t.turn();
        if t.turn() == who {
            t.end(who).unwrap();
            drain(t);
            continue;
        }
        pass(t, cur);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

/// Put `event` on top of the event deck and land `who` on the cafe (the
/// `tile:event` draw), declining everything.
fn draw_event_quiet(t: &mut Table, who: usize, event: &str) {
    let mut deck: Vec<String> = t.event_deck().into_iter().filter(|e| e != event).collect();
    deck.insert(0, event.to_string());
    let deck: Vec<&str> = deck.iter().map(String::as_str).collect();
    t.set_event_deck(&deck);
    until_turn(t, who);
    let corner = tile("CiRCLE 咖啡厅");
    t.set_pos(who, (corner + 59) % 60);
    t.dice(&[1]);
    t.roll(who).unwrap();
    drain(t);
}

/// Play `card` from `who`'s hand onto their current square.
fn place(t: &mut Table, who: usize, card: &str) {
    t.give(who, &[card]);
    t.play(who, card).unwrap();
    drain(t);
}

/// The identity probe's log rows: `(tile, is_circle, is_start)`. A player
/// card's log rides `log.card_effect` with the real message in `what`.
fn identity_rows(t: &Table) -> Vec<(i32, i64, i64)> {
    t.st()
        .events
        .iter()
        .filter_map(|e| {
            if log_key(e) != "cards:fixture-test-cards.rule_identity_seen" {
                return None;
            }
            let m = if e.msg.key() == "log.card_effect" {
                match e.msg.args.get("what") {
                    Some(game_core::msg::Arg::Msg(m)) => m.as_ref(),
                    _ => return None,
                }
            } else {
                &e.msg
            };
            let num = |k: &str| match m.args.get(k) {
                Some(game_core::msg::Arg::Integer(v)) | Some(game_core::msg::Arg::Number(v)) => {
                    Some(*v)
                }
                _ => None,
            };
            match m.args.get("tile") {
                Some(game_core::msg::Arg::Tile(t)) => {
                    Some((*t, num("is_circle")?, num("is_start")?))
                }
                _ => None,
            }
        })
        .collect()
}

fn tile_text_winner<'a>(speakers: &'a [TileTextEntry], slot: &str) -> Option<&'a TileTextEntry> {
    speakers.iter().find(|e| e.slot == slot)
}

/// One-step landing on `at` (the walk's last step), then drain.
fn land_on(t: &mut Table, who: usize, at: usize) {
    t.set_pos(who, (at + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(who).unwrap();
    drain(t);
}

// ---------------------------------------------------------------- tests

/// The event's rule swap is a plain world write: both squares resolve through
/// the partner immediately, and `prop::START` stays stamped on the real
/// CiRCLE's own row (a square fact, never copied onto the partner).
#[test]
fn circle_rebuild_swap_binds_both_squares() {
    let mut t = Table::vanilla(2);
    draw_event_quiet(&mut t, 0, "协助CiRCLE重建");
    let circle = tile("CiRCLE") as i32;
    let cafe = tile("CiRCLE 咖啡厅") as i32;
    let w = t.m.world();
    assert_eq!(
        w.governing_square(circle),
        cafe,
        "CiRCLE runs the cafe's rule"
    );
    assert_eq!(
        w.governing_square(cafe),
        circle,
        "the cafe runs CiRCLE's rule"
    );
    let homed_start = |tile: i32| {
        w.st.board_field
            .iter()
            .find(|f| f.tile == tile)
            .and_then(|f| f.props.get("start").copied())
            .unwrap_or(0)
    };
    assert_eq!(
        homed_start(circle),
        1,
        "START stays on the start square's row"
    );
    assert_eq!(homed_start(cafe), 0, "never copied onto the partner");
}

/// The settle body follows the swap; the square's identity does not. CiRCLE's
/// body (the start-square draw) runs on the fixture's square, while a settle
/// of CiRCLE itself runs that square's rule, and `tile_is(tile.id, "circle")` /
/// `ctx::is_start` still answer for the physical square only.
#[test]
fn rule_swap_moves_the_settle_body_and_keeps_identity() {
    let mut t = Table::vanilla(2);
    let e = tile("偶像经纪公司");
    let circle = tile("CiRCLE") as i32;
    t.set_pos(0, e);
    place(&mut t, 0, "TEST:rule_identity_probe");
    place(&mut t, 0, "TEST:rule_swap");
    assert_eq!(t.m.world().governing_square(e as i32), circle);

    // Settling the fixture's square runs CiRCLE's settle (「抽取一张手卡」 --
    // its `log.land_start`).
    let mark = t.mark();
    t.set_pos(0, e - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k == "log.land_start"),
        "CiRCLE's settle body ran on the square: {keys:?}"
    );
    let row = identity_rows(&t)
        .into_iter()
        .find(|(tile, _, _)| *tile == e as i32)
        .expect("the probe heard the settle");
    assert_eq!(row.1, 0, "the square is not a circle by kind");
    assert_eq!(row.2, 0, "the square is not the start square");

    // Settling CiRCLE (by the other player: the turn's roll is spent) runs the
    // fixture square's own rule instead. Its buy offer names the fixture
    // square -- the property body reads its own tile (`ctx::self_tile`).
    until_turn(&mut t, 1);
    let mark = t.mark();
    t.set_pos(1, 59);
    t.dice(&[1]);
    t.roll(1).unwrap();
    let d = t.dump_prompt();
    assert!(
        d.contains(&format!("Tile({})", e)),
        "the buy offer names the property square: {d}"
    );
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert!(
        !keys.iter().any(|k| k == "log.land_start"),
        "CiRCLE's own body moved away: {keys:?}"
    );
    let row = identity_rows(&t)
        .into_iter()
        .find(|(tile, _, _)| *tile == circle)
        .expect("the probe heard the settle");
    assert_eq!(row.1, 1, "CiRCLE is still a circle by kind");
    assert_eq!(row.2, 1, "CiRCLE is still the start square");
}

/// The [经过] half follows too: passing the swapped square pays CiRCLE's
/// [pass-reward], a pass of CiRCLE itself does not.
#[test]
fn rule_swap_moves_the_pass_reward() {
    let mut t = Table::vanilla(2);
    let e = tile("偶像经纪公司");
    t.set_pos(0, e);
    place(&mut t, 0, "TEST:rule_swap");

    // Three steps from two behind `e`: across it (a pass), landing past.
    let mark = t.mark();
    t.set_pos(0, e - 2);
    t.dice(&[3]);
    t.roll(0).unwrap();
    let d = t.dump_prompt();
    assert!(
        d.contains("pass_reward"),
        "passing the swapped square offers the reward: {d}"
    );
    let _ = t.answer_one(0);
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k == "log.pass_reward_money"),
        "the [pass-reward] paid: {keys:?}"
    );

    // A pass of CiRCLE pays nothing -- the reward moved off it.
    until_turn(&mut t, 1);
    let mark = t.mark();
    t.set_pos(1, 59);
    t.dice(&[2]);
    t.roll(1).unwrap();
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert!(
        !keys.iter().any(|k| k.contains("pass_reward")),
        "CiRCLE no longer pays: {keys:?}"
    );
}

/// The binding rides the instance: taking it off the field reverts both
/// squares.
#[test]
fn rule_swap_reverses_when_the_instance_leaves() {
    let mut t = Table::vanilla(2);
    let e = tile("偶像经纪公司");
    t.set_pos(0, e);
    place(&mut t, 0, "TEST:rule_swap");
    place(&mut t, 0, "TEST:rule_swap_off");
    assert_eq!(t.m.world().governing_square(e as i32), e as i32);
    assert_eq!(
        t.m.world().governing_square(tile("CiRCLE") as i32),
        tile("CiRCLE") as i32
    );

    // The fixture square's own rule runs again (a buy offer, no start draw).
    let mark = t.mark();
    t.set_pos(0, e - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    let d = t.dump_prompt();
    assert!(
        d.contains("ask.buy"),
        "the property settle offers the buy: {d}"
    );
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert!(
        !keys.iter().any(|k| k == "log.land_start"),
        "the property body runs again: {keys:?}"
    );

    // CiRCLE's own body is back (the other player's turn: the roll is spent).
    until_turn(&mut t, 1);
    let mark = t.mark();
    t.set_pos(1, 59);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k == "log.land_start"),
        "CiRCLE draws again: {keys:?}"
    );
}

/// The binding is a world write on the instance row: a save / restore round
/// trip keeps it in force.
#[test]
fn rule_swap_rides_save_and_restore() {
    let mut t = Table::vanilla(2);
    let e = tile("偶像经纪公司");
    t.set_pos(0, e);
    place(&mut t, 0, "TEST:rule_swap");
    let saved = t.m.save();
    t.m = Match::restore(data(), rules(), &saved).expect("restore");
    let circle = tile("CiRCLE") as i32;
    assert_eq!(t.m.world().governing_square(e as i32), circle);
    let mark = t.mark();
    land_on(&mut t, 0, e);
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k == "log.land_start"),
        "the restored binding still routes the body: {keys:?}"
    );
}

/// Deed-popup text composition follows the swap: the swapped-in rule's own
/// texts win on the square it now governs, and the square it left shows
/// nothing (the property rule declares no note).
#[test]
fn rule_swap_composes_tile_text() {
    let mut t = Table::vanilla(2);
    let e = tile("偶像经纪公司");
    t.set_pos(0, e);
    place(&mut t, 0, "TEST:rule_swap");
    let st = t.st();
    let at_e = &st.tile_texts[e];
    let settle = tile_text_winner(at_e, "settle").expect("a settle speaker on the square");
    assert_eq!(settle.from, "tile:circle");
    assert_eq!(settle.key, "deed.note.circleSettle");
    let p = tile_text_winner(at_e, "pass").expect("a pass speaker on the square");
    assert_eq!(p.from, "tile:circle");
    assert_eq!(p.key, "deed.note.circlePass");
    let at_circle = &st.tile_texts[tile("CiRCLE")];
    assert!(
        tile_text_winner(at_circle, "settle").is_none(),
        "the property rule carries no note"
    );
}

/// 笑容大游行 (1) 「当次移动的移动终点视为"弦卷集团"地产商」 -- the card's own
/// one-shot rule swap is dropped at that settle's `tileResolved`, even when
/// (2) is declined and the card never moves off 弦卷集团.
#[test]
fn smile_parade_one_shot_swap_ends_with_its_settle() {
    let mut t = Table::vanilla(2);
    let group = tile("弦卷集团");
    t.set_pos(0, group - 2);
    t.give(0, &["HHW:笑容大游行"]);
    t.dice(&[4]);
    t.roll(0).unwrap();
    let end = (group - 2 + 4) % 60;
    let mut placed = false;
    while t.prompt().is_some() {
        if t.counteract_offered("HHW:笑容大游行") {
            t.counteract(0, "HHW:笑容大游行").unwrap();
            placed = true;
        } else {
            // Decline (2) 「可将…放置于[移动终点]」 and every other ask.
            t.decline();
        }
    }
    assert!(placed, "（1）counteracted");
    let w = t.m.world();
    assert_eq!(
        w.governing_square(end as i32),
        end as i32,
        "the one-shot swap died with its settle"
    );
    assert_eq!(w.governing_square(group as i32), group as i32);
    // The card is still on 弦卷集团 (（2） declined), showing the (3) slot
    // binding only.
    let f = t
        .field(0)
        .into_iter()
        .find(|f| f.card.contains("笑容大游行"))
        .expect("the card stayed");
    assert_eq!(f.tile, group as i32);
    assert!(
        f.props.get("ruleSwapA").copied().unwrap_or(-1) < 0,
        "no rule swap left on the card: {:?}",
        f.props
    );
}
