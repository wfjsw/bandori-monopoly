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
                    .a
                    .get("why")
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
