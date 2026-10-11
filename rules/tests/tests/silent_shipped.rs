//! Shipped-card silent-entry bookkeeping: declare-cap / bounds hooks must
//! stay silent so a turn start does not flash them. The engine mechanism
//! (`CardDef::silent`) is pinned in `crates/game-rules/tests/silent_entry.rs`.

mod common;

use common::*;

/// Shipped declare-cap / bookkeeping entries must be silent so they never
/// flash. Checked by entry index against the CardDef tables.
#[test]
fn shipped_declare_cap_entries_are_silent() {
    let r = rules();
    // (card id, entry index) -- the fire-pot / bounds declaration entry.
    for (id, entry) in [
        ("skill:和奏瑞依:一次又一次竭尽全力", 0),
        ("skill:花园多惠:花园警察，出警！", 0),
        ("skill:宇田川巴:豚骨酱油拉面大姐", 1),
    ] {
        let idx = r.ruleset().card(id).unwrap_or_else(|| panic!("{id} loaded"));
        let info = &r.ruleset().cards()[idx as usize];
        assert!(
            info.on[entry].silent,
            "{id} entry {entry} must be silent, got {:?}",
            info.on.get(entry).map(|o| o.silent)
        );
    }
}

/// A turn start must not flash the raise_effort declare-cap entry.
#[test]
fn turn_start_does_not_flash_declare_cap() {
    let mut t = Table::vanilla(2);
    // Give player 0 the raise_effort skill via character.
    t.strip_skills();
    t.set_character_raw(0, "和奏瑞依");
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        t.decline();
    }
    let mark = t.mark();
    t.begin_turn(0);
    while t.prompt().is_some() {
        t.decline();
    }
    let fired: Vec<_> = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "card" && e.card.contains("竭尽全力"))
        .collect();
    assert!(
        fired.is_empty(),
        "declare_cap must not flash on turn start: {:?}",
        fired.iter().map(|e| e.msg.key()).collect::<Vec<_>>()
    );
}
