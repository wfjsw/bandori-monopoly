//! 「效果适用」 for **shipped** cards: `PPP:Returns`' deckBeforeGame self-place
//! and `RAS:成为最强`'s livehouse-gated roll. The general activation-log
//! shape is `crates/game-rules/tests/effect_applied_log.rs`.

mod common;

use common::*;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// A `deckBeforeGame` hook that places itself announces exactly once per
/// source -- the pile->field transition must not dispatch it a second time.
/// `PPP:Returns` is in the opening deck and self-places (`returns_added` /
/// `returns_placed`); one `returns_added` line means one source.
#[test]
fn deck_before_game_self_place_announces_once() {
    let mut t = Table::vanilla(2);
    drain(&mut t);
    let events = t.events_since(0);
    let added = events
        .iter()
        .filter(|e| e.msg.key() == "cards:card-ppp.returns_added")
        .count();
    assert!(
        added >= 1,
        "the opening deck carries PPP:Returns: {}",
        t.recent_keys(30).join(", ")
    );
    let headers = events
        .iter()
        .filter(|e| {
            e.r#type == "card"
                && e.card == "PPP:Returns"
                && e.msg.key() == "log.effect_applied"
                && e.msg
                    .args
                    .get("reason")
                    .map(|w| format!("{w:?}").contains("deck_before"))
                    .unwrap_or(false)
        })
        .count();
    assert_eq!(
        headers,
        added,
        "one header per self-placing deckBeforeGame source (pile->field must not re-dispatch): {}",
        t.recent_keys(40).join(", ")
    );
}

/// `RAS:成为最强` casts its 1d10 before the 若 clause, so **both** branches
/// roll; only the destination differs. The fallback ignores the face and still
/// logs 「没有Livehouse格子」. (Old code branched on ownership first and never
/// rolled on the fallback.)
#[test]
fn be_strongest_always_rolls_and_ignores_the_face_on_the_fallback() {
    // No livehouse: one dice event, and the face is ignored -- roll 3 would
    // map to DUB MUSIC EXPERIMENT (21) but the fallback lands on Live House (31).
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.set_money(0, 50_000);
    t.set_money(1, 50_000);
    // Strip any opening deeds so neither seat owns a group-6 tile.
    {
        let w = t.m.world_mut();
        for p in w.st.players.iter_mut() {
            p.field.clear();
        }
    }
    t.dice(&[3]);
    let mark = t.mark();
    t.give_play(0, "RAS:成为最强").ok();
    drain(&mut t);
    let dice = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "dice")
        .count();
    assert_eq!(dice, 1, "no livehouse -> the 1d10 is still cast");
    assert_eq!(t.pos(0), 31, "the fallback ignores the face (Live House)");
    assert!(
        t.keys_since(mark)
            .iter()
            .any(|k| k.contains("be_strongest_none")),
        "the 「没有Livehouse格子」 line is logged: {:?}",
        t.recent_keys(20)
    );

    // Owns a livehouse (RiNG 3 is group 6): the face steers the destination --
    // roll 5 is the 5th entry ("RiNG 3", 36).
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.set_money(0, 50_000);
    t.set_money(1, 50_000);
    t.set_owner(36, Some(0));
    t.dice(&[5]);
    let mark = t.mark();
    t.give_play(0, "RAS:成为最强").ok();
    drain(&mut t);
    let dice = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "dice")
        .count();
    assert_eq!(dice, 1, "owns a livehouse -> exactly one 1d10 roll");
    assert_eq!(t.pos(0), 36, "the roll picks the 5th livehouse");
}

/// `HHW:运动的天赋`'s 「每次掷骰获得一次资金」 rides `diceRollCast`, one of the
/// roll windows. Its header why is the roll phrase (with the face), not the
/// generic fallback that once rendered the raw key `log.kind.diceRollCast`.
#[test]
fn sports_talent_cast_header_names_the_roll() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.m.world_mut().set_slot(0, "sports_talent_next", 700);
    t.give_play(0, "HHW:运动的天赋").unwrap();
    drain(&mut t);
    let mark = t.mark();
    t.set_pos(0, 0);
    t.dice(&[4]);
    t.roll(0).unwrap();
    drain(&mut t);
    let head = t
        .events_since(mark)
        .into_iter()
        .find(|e| {
            e.r#type == "card" && e.card == "HHW:运动的天赋" && e.msg.key() == "log.effect_applied"
        })
        .unwrap_or_else(|| panic!("the cast hook announces: {}", t.recent_keys(30).join(", ")));
    let reason = head
        .msg
        .args
        .get("reason")
        .expect("the header carries a why");
    let debug = format!("{reason:?}");
    assert!(
        debug.contains("log.reason.roll"),
        "the cast why is the roll phrase, not the generic fallback: {debug}"
    );
    assert!(debug.contains("value"), "the why names the face: {debug}");
}
