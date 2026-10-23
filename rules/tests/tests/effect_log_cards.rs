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

/// `RAS:成为最强` rolls 1d10 only when the player already owns a livehouse;
/// otherwise it logs 「没有Livehouse格子」 and teleports to Live House with no
/// roll. The 4x4x40 seed-0 delta (master dice 18, ours 17) is this branch:
/// master's multi-pass re-run sees the deed this card's own agent-settle buys
/// and rolls; the check at play time says no livehouse, so we skip the roll.
/// Destinations coincide (roll 10 and the no-livehouse branch both pick Live
/// House), which is why the state hashes match.
#[test]
fn be_strongest_rolls_only_with_a_livehouse() {
    // No livehouse: no dice, and the 「没有」 line is logged.
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
    let mark = t.mark();
    t.give_play(0, "RAS:成为最强").ok();
    drain(&mut t);
    let dice = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "dice")
        .count();
    assert_eq!(dice, 0, "no livehouse -> no 1d10 roll");

    // Owns a livehouse (RiNG 3 is group 6): the 1d10 must land exactly once.
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_hand(1, &[]);
    t.set_money(0, 50_000);
    t.set_money(1, 50_000);
    t.set_owner(36, Some(0));
    let mark = t.mark();
    t.give_play(0, "RAS:成为最强").ok();
    drain(&mut t);
    let dice = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "dice")
        .count();
    assert_eq!(dice, 1, "owns a livehouse -> exactly one 1d10 roll");
}
