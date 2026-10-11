//! Ruleset-loader integration tests (engine-universal): handles, hash
//! stability, duplicate ids, fuel / nesting traps. Build the modules first:
//! `node tools/build-ruleset.mjs`.
//!
//! The `TEST:pre*` guard-condition tests live in `pre_conditions.rs` (their
//! own binary): they assert on the process-global `guard_cost` counters,
//! which every test here would otherwise race.
//!
//! Shipped-card behaviour through the same loader seam lives in
//! `rules/tests/tests/ruleset.rs`.

#[path = "testworld.rs"]
mod testworld;
use testworld::*;


#[test]
fn bad_handle_is_rejected() {
    let r = ruleset();
    // Derived from the loaded set: a fixed handle goes stale as cards are ported.
    let bad = r.cards().len() as i32 + 7;
    assert!(matches!(
        r.run(
            &TestWorld::new(1),
            Call::Play {
                card: bad,
                player_id: 0
            },
            &[], None),
        Err(RuleError::NoSuchCard(_))
    ));
}

#[test]
fn ruleset_hash_is_independent_of_load_order() {
    let mods = modules("cards");
    let mut fwd = Ruleset::builder();
    let mut rev = Ruleset::builder();
    for m in &mods {
        fwd.add(m).unwrap();
    }
    for m in mods.iter().rev() {
        rev.add(m).unwrap();
    }
    assert_eq!(fwd.build().unwrap().sha256(), rev.build().unwrap().sha256());
}

#[test]
fn duplicate_card_ids_are_rejected() {
    let m = &modules("cards")[0];
    let mut b = Ruleset::builder();
    b.add(m).unwrap();
    b.add(m).unwrap();
    assert!(matches!(b.build(), Err(RuleError::DuplicateCard(_))));
}


#[test]
fn runaway_nesting_is_stopped() {
    let r = load(&["cards", "fixtures"]);
    let card = r.card("TEST:recurse").unwrap();
    let err = r
        .run(&TestWorld::new(1), Call::Play { card, player_id: 0 }, &[], None)
        .unwrap_err();
    assert!(
        matches!(err, RuleError::Trap(ref m) if m.contains("nested deeper")),
        "{err}"
    );
}


