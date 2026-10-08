//! G1 unit tests: parse errors, unknown vars, float rejection, kind rejection,
//! truth tables for the sample conditions, function-backed vars.

use rules_cond::{
    compile, CandidateCtx, ChainLink, Cond, CondError, MoveSnap, PlayerSnap, TileSnap, WindowCtx,
    WindowScope,
};

fn win() -> WindowCtx {
    WindowCtx {
        kind: 0,
        actor: 1,
        target: -1,
        tile: TileSnap {
            id: 3,
            owner: 0,
            houses: 2,
            mortgaged: 0,
            price: 400,
        },
        value: 6000,
        step: 4,
        by: 1,
        pay_is_rent: false,
        mv: MoveSnap {
            roll: Some(5),
            kind: Some(rules_cond::mv::Walk),
            remaining: 5,
        },
        roll_source: 0,
        abnormal: false,
        chain: vec![
            ChainLink {
                kind: rules_cond::trig::Pay,
                from: 1,
                hits: 0,
            },
            ChainLink {
                kind: rules_cond::trig::Effect,
                from: 2,
                hits: -1,
            },
        ],
        turn_player: 1,
        turn_key: 7,
        players: vec![
            PlayerSnap {
                money: 1200,
                character: 3,
                band: 1,
                ..PlayerSnap::default()
            },
            PlayerSnap {
                money: 400,
                character: 4,
                band: 2,
                ..PlayerSnap::default()
            },
            PlayerSnap::default(),
            PlayerSnap::default(),
        ],
        tile_ids: [("festival".to_string(), 9)].into_iter().collect(),
    }
}

fn cand(owner: i64) -> CandidateCtx {
    CandidateCtx {
        owner,
        owner_money: if owner == 0 { 1200 } else { 400 },
        owner_fire: 2,
        owner_crystals: 1,
        owner_hand: 3,
        owner_pos: 7,
        owner_no_hand: 0,
        owner_character: if owner == 0 { 3 } else { 4 },
        owner_band: if owner == 0 { 1 } else { 2 },
        owner_tiles: 1,
        card_id: 42,
        card_placed: false,
        card_cp: 1,
        slots: [("asUsualTurn".to_string(), 3)].into_iter().collect(),
        toks: [(rules_cond::trig::FireSpent, 2)].into_iter().collect(),
        blocked_bands: vec![2],
        ..CandidateCtx::default()
    }
}

// ---------------------------------------------------------------------------
// parse errors
// ---------------------------------------------------------------------------

#[test]
fn parse_error_on_garbage() {
    let err = compile("actor ==").unwrap_err();
    assert!(matches!(err, CondError::Parse(_)), "{err:?}");
}

#[test]
fn parse_error_on_empty() {
    assert!(matches!(compile("").unwrap_err(), CondError::Parse(_)));
}

// ---------------------------------------------------------------------------
// unknown vars / fns
// ---------------------------------------------------------------------------

#[test]
fn unknown_variable_rejected() {
    let err = compile("bogus == 1").unwrap_err();
    assert!(matches!(err, CondError::UnknownVar(ref s) if s == "bogus"), "{err:?}");
}

#[test]
fn unknown_field_rejected() {
    let err = compile("owner.bogus >= 1").unwrap_err();
    assert!(matches!(err, CondError::UnknownVar(ref s) if s == "owner.bogus"), "{err:?}");
}

#[test]
fn unknown_function_rejected() {
    let err = compile("frobnicate(owner)").unwrap_err();
    assert!(matches!(err, CondError::UnknownFn(ref s) if s == "frobnicate"), "{err:?}");
}

// ---------------------------------------------------------------------------
// float rejection
// ---------------------------------------------------------------------------

#[test]
fn float_literal_rejected() {
    let err = compile("value >= 5000.5").unwrap_err();
    assert!(matches!(err, CondError::FloatLiteral(_)), "{err:?}");
}

#[test]
fn float_literal_zero_rejected() {
    assert!(matches!(
        compile("value >= 0.0").unwrap_err(),
        CondError::FloatLiteral(_)
    ));
}

#[test]
fn double_call_rejected() {
    let err = compile("value >= double(1)").unwrap_err();
    assert!(matches!(err, CondError::FloatOp(ref s) if s == "double"), "{err:?}");
}

#[test]
fn uint_literal_rejected() {
    assert!(matches!(
        compile("value >= 5u").unwrap_err(),
        CondError::FloatLiteral(_)
    ));
}

#[test]
fn int_division_allowed() {
    // `tile.price/2` is a real pattern (tomoe_savior).
    assert!(compile("value >= tile.price / 2").is_ok());
}

// ---------------------------------------------------------------------------
// kind is an ordinary window variable (per-kind clauses on multi-kind entries)
// ---------------------------------------------------------------------------

#[test]
fn kind_is_a_window_variable() {
    let c = compile("kind == MoveRoll && move.roll < 6 || kind != MoveRoll && value >= 500").unwrap();
    let mut w = win();
    w.kind = rules_cond::trig::MoveRoll;
    w.mv.roll = Some(3);
    assert!(c.eval(&w, &cand(0)));
    w.kind = rules_cond::trig::MoveRoll + 1;
    w.value = 100;
    assert!(!c.eval(&w, &cand(0)));
}

#[test]
fn trigger_kind_is_an_alias() {
    let c = compile("trigger.kind == MoveRoll").unwrap();
    let mut w = win();
    w.kind = rules_cond::trig::MoveRoll;
    assert!(c.eval(&w, &cand(0)));
}

#[test]
fn other_trigger_fields_are_unknown() {
    let err = compile("trigger.foo == 1").unwrap_err();
    assert!(matches!(err, CondError::UnknownVar(_)), "{err:?}");
}

#[test]
fn move_kind_allowed() {
    // `move.kind` is the move's kind, not the trigger category.
    assert!(compile("move.kind == Walk && move.roll != null").is_ok());
}

// ---------------------------------------------------------------------------
// truth tables -- the GUARDS.md sample conditions
// ---------------------------------------------------------------------------

#[test]
fn actor_eq_owner() {
    let cond = compile("actor == owner").unwrap();
    let w = win();
    assert!(cond.eval(&w, &cand(1)));
    assert!(!cond.eval(&w, &cand(0)));
}

#[test]
fn actor_ne_owner_and_money() {
    let cond = compile("actor != owner && owner.money >= 500").unwrap();
    let w = win();
    // actor=1; owner=0 has 1200 -> true
    assert!(cond.eval(&w, &cand(0)));
    // owner=1 equals actor -> false
    assert!(!cond.eval(&w, &cand(1)));
}

#[test]
fn effect_hits_and_value() {
    let cond = compile("effect.hits(owner) && value >= 5000").unwrap();
    let w = win();
    // chain hits seat 0; value = 6000
    assert!(cond.eval(&w, &cand(0)));
    // seat 1 is not hit
    assert!(!cond.eval(&w, &cand(1)));

    let mut w2 = win();
    w2.value = 100;
    assert!(!cond.eval(&w2, &cand(0)));
}

#[test]
fn tile_owner_and_houses() {
    let cond = compile("tile.owner == owner && tile.houses > 0").unwrap();
    let w = win();
    assert!(cond.eval(&w, &cand(0)));
    assert!(!cond.eval(&w, &cand(1)));

    let mut w2 = win();
    w2.tile.houses = 0;
    assert!(!cond.eval(&w2, &cand(0)));
}

#[test]
fn move_kind_walk_and_roll_present() {
    let cond = compile("move.kind == Walk && move.roll != null").unwrap();
    assert!(cond.eval(&win(), &cand(0)));

    let mut w = win();
    w.mv.roll = None;
    assert!(!cond.eval(&w, &cand(0)));

    let mut w = win();
    w.mv.kind = Some(rules_cond::mv::Teleport);
    assert!(!cond.eval(&w, &cand(0)));
}

#[test]
fn effect_has_pay() {
    let cond = compile("effect.has(Pay)").unwrap();
    assert!(cond.eval(&win(), &cand(0)));

    let mut w = win();
    w.chain.clear();
    assert!(!cond.eval(&w, &cand(0)));
}

#[test]
fn slot_ne_turn_key() {
    let cond = compile("slot('asUsualTurn') != turn_key").unwrap();
    // slot = 3, turn_key = 7 -> true
    assert!(cond.eval(&win(), &cand(0)));

    let mut c = cand(0);
    c.slots.insert("asUsualTurn".into(), 7);
    assert!(!cond.eval(&win(), &c));
}

#[test]
fn money_function() {
    let cond = compile("money(owner) >= 500").unwrap();
    assert!(cond.eval(&win(), &cand(0)));
    assert!(!cond.eval(&win(), &cand(1)));

    // money() of a non-owner seat reads the window player table.
    let cond = compile("money(actor) < 500").unwrap();
    // actor = 1, players[1].money = 400
    assert!(cond.eval(&win(), &cand(0)));
}

#[test]
fn character_is_and_blocked() {
    let cond = compile("character_is(owner, 3) && blocked(2)").unwrap();
    // owner 0 is character 3; band 2 is blocked on the candidate
    assert!(cond.eval(&win(), &cand(0)));
    assert!(!cond.eval(&win(), &cand(1)));

    let cond = compile("blocked(1)").unwrap();
    assert!(!cond.eval(&win(), &cand(0)));
}

#[test]
fn neighbor() {
    let cond = compile("actor == neighbor(owner, -1)").unwrap();
    // actor = 1; neighbor(0, -1) in a 4-seat game = 3 -> false
    assert!(!cond.eval(&win(), &cand(0)));
    // actor = 1; neighbor(1, -1) = 0 -> false
    assert!(!cond.eval(&win(), &cand(1)));

    let cond = compile("actor == neighbor(owner, 1)").unwrap();
    // neighbor(0, 1) = 1 == actor
    assert!(cond.eval(&win(), &cand(0)));
}

#[test]
fn tile_named() {
    let cond = compile("tile_named('festival') == 9").unwrap();
    assert!(cond.eval(&win(), &cand(0)));

    let cond = compile("tile_named('nowhere') == 9").unwrap();
    assert!(!cond.eval(&win(), &cand(0)));
}

#[test]
fn tok_lookup() {
    let cond = compile("tok(Pay) >= 1").unwrap();
    // cand toks has FireSpent=2, not Pay -> 0
    assert!(!cond.eval(&win(), &cand(0)));

    let mut c = cand(0);
    c.toks.insert(rules_cond::trig::Pay, 3);
    assert!(cond.eval(&win(), &c));
}

#[test]
fn card_fields() {
    let cond = compile("card.id == 42 && !card.placed").unwrap();
    assert!(cond.eval(&win(), &cand(0)));

    let mut c = cand(0);
    c.card_placed = true;
    assert!(!cond.eval(&win(), &c));
}

// ---------------------------------------------------------------------------
// context reuse
// ---------------------------------------------------------------------------

#[test]
fn window_scope_reuse_matches_oneshot() {
    let w = win();
    let cond = compile("actor != owner && owner.money >= 500").unwrap();
    let scope = WindowScope::new(&w);
    for owner in [0i64, 1, 2] {
        let c = cand(owner);
        assert_eq!(
            scope.eval(&cond, &c),
            cond.eval(&w, &c),
            "owner={owner}"
        );
    }
}

#[test]
fn eval_checked_reports_non_bool() {
    // A bare int is not a bool.
    let cond = compile("value").unwrap();
    let err = cond.eval_checked(&win(), &cand(0)).unwrap_err();
    assert!(matches!(err, rules_cond::EvalError::NotBool(_)), "{err:?}");
}
// ---------------------------------------------------------------------------
// serialized compiled form (postcard of cel::wire::Compiled)
// ---------------------------------------------------------------------------

/// Every sample condition in the GUARDS.md §8.3 table, so the round-trip
/// covers the shapes the guard prefilter will actually carry.
fn sample_conds() -> Vec<(&'static str, Cond)> {
    [
        "actor == owner",
        "actor != owner && owner.money >= 500",
        "effect.hits(owner) && value >= 5000",
        "tile.owner == owner && tile.houses > 0",
        "move.kind == Walk && move.roll != null",
        "effect.has(Pay)",
        "slot('asUsualTurn') != turn_key",
        "money(owner) >= 500",
    ]
    .into_iter()
    .map(|src| (src, compile(src).unwrap()))
    .collect()
}

#[test]
fn wire_round_trip_preserves_eval() {
    for (src, cond) in sample_conds() {
        for with_source in [true, false] {
            let bytes = cond.to_bytes(with_source);
            let loaded = rules_cond::Cond::from_bytes(&bytes)
                .unwrap_or_else(|e| panic!("{src} (with_source={with_source}): {e}"));
            assert_eq!(cond.expr(), loaded.expr(), "{src} tree");
            assert_eq!(cond.used_vars(), loaded.used_vars(), "{src} vars");
            assert_eq!(cond.used_fns(), loaded.used_fns(), "{src} fns");
            if with_source {
                assert_eq!(loaded.src(), src, "{src} source");
            } else {
                assert_eq!(loaded.src(), "", "{src} lean form keeps no source");
            }
            // Same truth table on both sides.
            let (w, c) = (win(), cand(0));
            assert_eq!(
                cond.eval(&w, &c),
                loaded.eval(&w, &c),
                "{src} (with_source={with_source})"
            );
            let scope = WindowScope::new(&w);
            assert_eq!(
                scope.eval(&cond, &c),
                scope.eval(&loaded, &c),
                "{src} via WindowScope"
            );
        }
    }
}

#[test]
fn wire_version_mismatch_fails_loudly() {
    let cond = compile("actor == owner").unwrap();
    let mut bytes = cond.to_bytes(false);
    // `version` is the first field of the wrapper: postcard writes one byte.
    assert_eq!(bytes[0], 1, "cel::wire::FORMAT_VERSION");
    bytes[0] = 2;
    match rules_cond::Cond::from_bytes(&bytes) {
        Err(CondError::WireVersion { found: 2, expected: 1 }) => {}
        other => panic!("expected WireVersion 2/1, got {other:?}"),
    }
}

#[test]
fn wire_rejects_garbage() {
    assert!(matches!(
        rules_cond::Cond::from_bytes(b"not postcard"),
        Err(CondError::WireDecode(_))
    ));
    assert!(matches!(
        rules_cond::Cond::from_bytes(b""),
        Err(CondError::WireDecode(_))
    ));
}

#[test]
fn print_wire_sizes() {
    // `cargo test -p rules-cond --test cond_tests print_wire_sizes -- --nocapture`
    println!("source_bytes lean_bytes with_source_bytes expr");
    for (src, cond) in sample_conds() {
        println!(
            "{} {} {} {src}",
            src.len(),
            cond.to_bytes(false).len(),
            cond.to_bytes(true).len()
        );
    }
}
