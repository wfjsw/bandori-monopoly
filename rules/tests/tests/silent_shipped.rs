//! Shipped-card silent-entry bookkeeping: declare-cap / bounds hooks must
//! stay silent so a turn start does not flash them. The engine mechanism
//! (`On::silent`) is pinned in `crates/game-rules/tests/silent_entry.rs`.

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
        let idx = r
            .ruleset()
            .card(id)
            .unwrap_or_else(|| panic!("{id} loaded"));
        let info = &r.ruleset().cards()[idx as usize];
        assert_eq!(
            info.on[entry].presentation,
            card_sdk::abi::Presentation::Silent,
            "{id} entry {entry} must be silent, got {:?}",
            info.on.get(entry).map(|o| o.presentation)
        );
    }
}

/// `非凡之星` 「（1）」's fire-pot gain is owner-self-only, but the rulebook
/// describes it happening, so that clause carries `Flash::Always`. The policy
/// must ride the Settle hook that is 「（1）」 -- the old
/// `.flash(&[(2, Flash::Always)])` pointed at the silent `DeckAtGameStart`
/// `declare_cap` (entry 2) and was dead there, so the clause never flashed.
#[test]
fn kasumi_flashes_its_fire_pot_clause() {
    let r = rules();
    let id = "skill:户山香澄:非凡之星";
    let idx = r
        .ruleset()
        .card(id)
        .unwrap_or_else(|| panic!("{id} loaded"));
    let info = &r.ruleset().cards()[idx as usize];
    // 0 Play / 1 TurnStartBefore / 2 DeckAtGameStart / 3 Settle 「（1）」
    assert_eq!(info.on.len(), 4, "{id} entries");
    assert_eq!(info.on[1].presentation, card_sdk::abi::Presentation::Silent);
    assert_eq!(info.on[2].presentation, card_sdk::abi::Presentation::Silent);
    assert_eq!(
        info.on[3].flash,
        card_sdk::abi::Flash::Always,
        "「（1）」 (Settle) must be Flash::Always, got {:?}",
        info.on[3].flash
    );
    // The silent bookkeeping entries must not claim the policy.
    for e in [1, 2] {
        assert_eq!(
            info.on[e].flash,
            card_sdk::abi::Flash::Auto,
            "{id} entry {e} is bookkeeping and must stay Auto"
        );
    }
}

/// A turn start must not flash the layer declare-cap entry.
#[test]
fn turn_start_does_not_flash_declare_cap() {
    let mut t = Table::vanilla(2);
    // Give player 0 the layer skill via character.
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
