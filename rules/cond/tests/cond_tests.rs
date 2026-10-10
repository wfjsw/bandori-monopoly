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
            main: true,
            dir: 1,
            tags: Vec::new(),
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
        trigger_card: 0,
        name: String::new(),
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
        circle_tiles: vec![0],
        ring_tiles: vec![],
        live_house_tiles: vec![],
        buyable_tiles: vec![],
        tile_count: 12,
        tile_owners: vec![0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
        plan_fixed_roll: None,
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
        slots: [("asUsualTurn".to_string(), 3)].into_iter().collect(),
        tok_names: [("水母标记".to_string(), 2)].into_iter().collect(),
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

    // Unregistered names answer -1 (the guest's `ctx::tile_named`), never 0 --
    // tile 0 is a real tile (CiRCLE).
    let cond = compile("tile_named('nowhere') == -1").unwrap();
    assert!(cond.eval(&win(), &cand(0)));

    let cond = compile("tile_named('nowhere') == 9").unwrap();
    assert!(!cond.eval(&win(), &cand(0)));
}

#[test]
fn tile_kind_predicates() {
    // `win()` fills circle_tiles with tile 0.
    assert!(compile("is_circle(0)").unwrap().eval(&win(), &cand(0)));
    assert!(!compile("is_circle(1)").unwrap().eval(&win(), &cand(0)));
    assert!(!compile("is_ring(0)").unwrap().eval(&win(), &cand(0)));
}

#[test]
fn move_main() {
    // `win()` carries `main: true`.
    assert!(compile("move.main").unwrap().eval(&win(), &cand(0)));
    assert!(!compile("!move.main").unwrap().eval(&win(), &cand(0)));
    let mut w = win();
    w.mv.main = false;
    assert!(!compile("move.main").unwrap().eval(&w, &cand(0)));
}

#[test]
fn tok_lookup() {
    // `tok('name')` is the owner's named token counter (the guest's
    // `ctx::tok(owner, name)`); missing names answer 0.
    let cond = compile("tok('水母标记') >= 1").unwrap();
    assert!(cond.eval(&win(), &cand(0)));

    let cond = compile("tok('星星贴纸') >= 1").unwrap();
    assert!(!cond.eval(&win(), &cand(0)));

    let mut c = cand(0);
    c.tok_names.insert("星星贴纸".to_string(), 3);
    assert!(cond.eval(&win(), &c));
}

#[test]
fn move_dir() {
    // `win()` carries `dir: 1` (forward). `move.dir < 0` is the backward filter
    // kanon_march uses.
    assert!(!compile("move.dir < 0").unwrap().eval(&win(), &cand(0)));
    assert!(compile("move.dir > 0").unwrap().eval(&win(), &cand(0)));
    let mut w = win();
    w.mv.dir = -1;
    assert!(compile("move.dir < 0").unwrap().eval(&w, &cand(0)));
}

#[test]
fn move_tag_lookup() {
    // `move.tag('name')` is the move's per-card tag counter
    // (`trigger::move_tag(name)`); missing names answer 0. Both the dotted
    // CEL spelling and the flat id are callable.
    let cond = compile("move.tag('fireRoll') != 0").unwrap();
    assert!(!cond.eval(&win(), &cand(0)));

    let cond_flat = compile("move_tag('fireRoll') != 0").unwrap();
    assert!(!cond_flat.eval(&win(), &cand(0)));

    let mut w = win();
    w.mv.tags.push(("fireRoll".to_string(), 1));
    assert!(cond.eval(&w, &cand(0)));
    assert!(cond_flat.eval(&w, &cand(0)));
    assert!(!compile("move.tag('other') != 0").unwrap().eval(&w, &cand(0)));
}

#[test]
fn trigger_card_matches_card_id() {
    // `trigger_card` shares `card.id`'s encoding: the self-check form is
    // `trigger_card == card.id`.
    let mut w = win();
    w.trigger_card = 42;
    assert!(compile("trigger_card == card.id").unwrap().eval(&w, &cand(0)));
    // No card on the trigger -> 0, never a real card id.
    assert!(!compile("trigger_card == card.id").unwrap().eval(&win(), &cand(0)));

    // A card-id literal is the same `id_of` hash.
    let lit = rules_cond::id_of("CRYCHIC:春日影");
    let mut w = win();
    w.trigger_card = lit;
    assert!(compile(&format!("trigger_card == {lit}")).unwrap().eval(&w, &cand(0)));
}

#[test]
fn counter_name_and_counter_is() {
    // `counter_name` is the `id_of` hash of `Trigger.name`; `counter_is('…')`
    // is the string spelling (a `CounterChanged` counter name, or an
    // `On::Message` message name).
    let mut w = win();
    w.name = "cp".to_string();
    assert!(compile("counter_is('cp')").unwrap().eval(&w, &cand(0)));
    assert!(!compile("counter_is('crystals')").unwrap().eval(&w, &cand(0)));
    // The hash form mirrors `trigger_card`.
    let h = rules_cond::id_of("cp");
    assert!(compile(&format!("counter_name == {h}")).unwrap().eval(&w, &cand(0)));
    // Empty name -> 0, and `counter_is` is false for every spelling.
    assert!(!compile("counter_is('cp')").unwrap().eval(&win(), &cand(0)));
}

#[test]
fn card_counter_lookup() {
    // `card.counter('name')` is the candidate instance's named counter:
    // `"cp"` / `"crystals"` map to the two on-card wire names, anything else
    // to `FieldCard::counters`. Missing = 0. The CEL spelling is
    // `card.counter('…')`; the flat `card_counter('…')` is the same call.
    let mut c = cand(0);
    c.card_counters.insert("cp".to_string(), 6);
    c.card_counters.insert("crystals".to_string(), 2);
    c.card_counters.insert("星星贴纸".to_string(), 3);
    assert!(compile("card.counter('cp') == 6").unwrap().eval(&win(), &c));
    assert!(compile("card.counter('crystals') == 2").unwrap().eval(&win(), &c));
    assert!(compile("card.counter('星星贴纸') == 3").unwrap().eval(&win(), &c));
    assert!(compile("card_counter('cp') == 6").unwrap().eval(&win(), &c));
    // Missing names answer 0 (same as `tok`).
    assert!(compile("card.counter('抹茶芭菲') == 0").unwrap().eval(&win(), &c));
}

#[test]
fn band_is_name_form() {
    // The name form hashes through `id_of` and compares to the view's
    // `band(seat)` -- the same name `ctx::in_band` matches.
    let name = "Pastel✽Palettes";
    let mut w = win();
    w.players[0].band = rules_cond::id_of(name);
    w.players[1].band = rules_cond::id_of("Afterglow");
    let mut c = cand(0);
    c.owner_band = rules_cond::id_of(name);

    let cond = compile("band_is(owner, 'Pastel✽Palettes')").unwrap();
    assert!(cond.eval(&w, &c));

    let mut c1 = cand(1);
    c1.owner_band = rules_cond::id_of("Afterglow");
    assert!(!cond.eval(&w, &c1));

    // The int form still compares against the raw id.
    let mut w2 = win();
    w2.players[0].band = 7;
    assert!(compile("band_is(0, 7)").unwrap().eval(&w2, &cand(0)));
}

#[test]
fn character_is_name_form() {
    let name = "三角初华（Sumimi）";
    let mut w = win();
    w.players[0].character = rules_cond::id_of(name);
    let mut c = cand(0);
    c.owner_character = rules_cond::id_of(name);

    let cond = compile("character_is(owner, '三角初华（Sumimi）')").unwrap();
    assert!(cond.eval(&w, &c));
    assert!(!compile("character_is(owner, '纯田真奈')").unwrap().eval(&w, &c));
}

#[test]
fn card_fields() {
    let cond = compile("card.id == 42 && !card.placed").unwrap();
    assert!(cond.eval(&win(), &cand(0)));

    let mut c = cand(0);
    c.card_placed = true;
    assert!(!cond.eval(&win(), &c));
}

#[test]
fn owner_overlay_fields() {
    // Every `owner.*` dotted name binds through the candidate overlay.
    let cond = compile("owner.money >= 500 && owner.fire == 2 && owner.hand == 3").unwrap();
    assert!(cond.eval(&win(), &cand(0)));
    assert!(!cond.eval(&win(), &cand(1)));

    let cond = compile("owner.pos == 7 && owner.out == 0 && owner.no_hand == 0").unwrap();
    assert!(cond.eval(&win(), &cand(0)));

    let cond = compile("owner.character == 3 && owner.band == 1 && owner.tiles == 1").unwrap();
    assert!(cond.eval(&win(), &cand(0)));
    assert!(!cond.eval(&win(), &cand(1)));

    // `owner.id` is the seat, same as bare `owner`.
    let cond = compile("owner.id == owner").unwrap();
    assert!(cond.eval(&win(), &cand(0)));
}

#[test]
fn effect_count_aliases() {
    // `chain.count` is an alias of `effect.count`; both flatten to
    // `effect_count` and read the chain length.
    let cond = compile("effect.count == 2").unwrap();
    assert!(cond.eval(&win(), &cand(0)));
    let cond = compile("chain.count == 2").unwrap();
    assert!(cond.eval(&win(), &cand(0)));
    let cond = compile("chain.count == 0").unwrap();
    assert!(!cond.eval(&win(), &cand(0)));
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

// ---------------------------------------------------------------------------
// vocabulary sync (docs/GUARDS.md: "how to add a condition name")
// ---------------------------------------------------------------------------

#[test]
fn vocab_matches_schema_lists() {
    use rules_cond::schema;
    use rules_cond::vocab::{by_cel, flat_idents, func_names, struct_roots, Scope, VOCAB};
    // Every VOCAB window/candidate name is accepted by the lint.
    for f in flat_idents() {
        assert!(
            schema::ident_known(f),
            "VOCAB name {f} missing from the schema lint"
        );
    }
    // Every VOCAB function is in the lint's function list.
    for fn_ in func_names() {
        assert!(
            schema::functions().contains(&fn_),
            "VOCAB fn {fn_} missing from the schema function list"
        );
    }
    // ...and every schema function is in VOCAB (or is CEL stdlib).
    for f in schema::functions() {
        if schema::CEL_STDLIB.contains(&f) {
            continue;
        }
        assert!(
            func_names().contains(&f),
            "schema function {f} missing from VOCAB"
        );
    }
    // The lint's struct roots agree with VOCAB's dotted spellings (and aliases),
    // and `by_cel` maps every spelling to the table's flat identifier.
    let roots = struct_roots();
    for n in VOCAB {
        if matches!(n.scope, Scope::Func { .. }) {
            continue;
        }
        for dotted in std::iter::once(n.cel).chain(n.aliases.iter().copied()) {
            if let Some((root, field)) = dotted.split_once('.') {
                assert!(
                    roots.iter().any(|(r, fs)| *r == root && fs.contains(&field)),
                    "VOCAB {dotted} missing from struct_roots"
                );
            }
            assert_eq!(
                by_cel(dotted).map(|n| n.flat),
                Some(n.flat),
                "by_cel({dotted}) must land on {}",
                n.flat
            );
        }
    }
    // Schema list helpers are pure projections of VOCAB.
    let window_bare: Vec<_> = VOCAB
        .iter()
        .filter(|n| n.scope == Scope::Window && !n.cel.contains('.'))
        .map(|n| n.flat)
        .collect();
    assert_eq!(schema::window_vars(), window_bare);
    let cand_bare: Vec<_> = VOCAB
        .iter()
        .filter(|n| n.scope == Scope::Candidate && !n.cel.contains('.'))
        .map(|n| n.flat)
        .collect();
    assert_eq!(schema::candidate_vars(), cand_bare);
}

#[test]
fn vocab_flats_are_unique() {
    use rules_cond::vocab::VOCAB;
    let mut seen = std::collections::BTreeSet::new();
    for n in VOCAB {
        assert!(seen.insert(n.flat), "duplicate flat name {}", n.flat);
    }
}

// ---------------------------------------------------------------------------
// native geometry / plan / counter names (2026-10-10)
// ---------------------------------------------------------------------------

/// A window with `n` players at the given positions on a `size`-tile ring.
fn geo_win(size: i64, pos: &[i64], owners: &[i64]) -> WindowCtx {
    let mut w = win();
    w.tile_count = size;
    w.tile_owners = owners.to_vec();
    w.players = pos
        .iter()
        .map(|&p| PlayerSnap {
            pos: p,
            ..PlayerSnap::default()
        })
        .collect();
    w
}

#[test]
fn plan_fixed_roll_null_when_unset() {
    // `plan.fixed_roll` binds `null` when no card fixed this turn's roll --
    // the sports_talent / kaoru_thief / soyo_clear / kanon_lost
    // `roll_plan_unfixed` residual is `plan.fixed_roll == null`.
    assert!(compile("plan.fixed_roll == null").unwrap().eval(&win(), &cand(0)));
    let mut w = win();
    w.plan_fixed_roll = Some(10);
    assert!(!compile("plan.fixed_roll == null").unwrap().eval(&w, &cand(0)));
    assert!(compile("plan.fixed_roll == 10").unwrap().eval(&w, &cand(0)));
}

#[test]
fn card_tile_binding() {
    // Raw `ctx::self_tile()` host value: -2 = not placed, -1 = with owner.
    // The rana_funny residual is `self_tile() == trigger.tile`.
    let mut w = win(); // trigger tile id = 3
    let mut c = cand(0);
    c.card_tile = Some(3);
    assert!(compile("card.tile == tile.id").unwrap().eval(&w, &c));
    c.card_tile = Some(-1);
    assert!(!compile("card.tile == tile.id").unwrap().eval(&w, &c));
    c.card_tile = None; // binds -2
    assert!(!compile("card.tile == tile.id").unwrap().eval(&w, &c));
    assert!(compile("card.tile == -2").unwrap().eval(&w, &c));
    // With-owner form: `card.tile == -1`.
    c.card_tile = Some(-1);
    assert!(compile("card.tile == -1").unwrap().eval(&w, &c));
    let _ = &mut w;
}

#[test]
fn dist_ring_distance() {
    // 12-tile ring: 0..6 is 6 either way; 0..1 is 1; 0..11 is 1 (wrap).
    let w = geo_win(12, &[0, 1, 6, 11], &[0, -1, -1, -1]);
    // `dist` is int-returning; ask it inside a comparison.
    assert!(compile("dist(0, 1) == 1").unwrap().eval(&w, &cand(0)));
    assert!(compile("dist(0, 6) == 6").unwrap().eval(&w, &cand(0)));
    assert!(compile("dist(0, 11) == 1").unwrap().eval(&w, &cand(0)));
    assert!(compile("dist(11, 0) == 1").unwrap().eval(&w, &cand(0)));
    assert!(compile("dist(1, 1) == 0").unwrap().eval(&w, &cand(0)));
    // your_light's shape: `dist(pos(owner), tile_named('学校')) > 20`.
    assert!(compile("dist(pos(owner), 0) <= 20").unwrap().eval(&w, &cand(0)));
}

#[test]
fn players_on_counts_present_standers() {
    // Two players on tile 5, one on tile 0. `players_on(5, 0)` counts the
    // *other* present stander on 5 (except seat 0 is not on 5 anyway).
    let mut w = geo_win(12, &[5, 5, 0, 5], &[-1; 12]);
    w.players[2].out = 1; // seat 2 is out -> not present
    // seats 0,1,3 on tile 5. players_on(5, 0) = {1, 3} = 2.
    assert!(compile("players_on(5, 0) == 2").unwrap().eval(&w, &cand(0)));
    // players_on(5, -1) excludes nobody: 3 present standers (0,1,3).
    assert!(compile("players_on(5, -1) == 3").unwrap().eval(&w, &cand(0)));
    // lisa_bond's shape: `players_on(owner.pos, owner) > 0`.
    let mut c = cand(0);
    c.owner_pos = 5;
    assert!(compile("players_on(owner.pos, owner) > 0").unwrap().eval(&w, &c));
}

#[test]
fn next_dist_to_nearest_rival() {
    // Seat 0 at 1; rivals at 3, 9, 6. Forward: 3-1=2. Backward: 1-9 wrap=4
    // (the 1->6 way is 7, the 1->3 way is 10).
    let w = geo_win(12, &[1, 3, 9, 6], &[-1; 12]);
    assert!(compile("next_dist(0, 1) == 2").unwrap().eval(&w, &cand(0)));
    assert!(compile("next_dist(0, -1) == 4").unwrap().eval(&w, &cand(0)));
    // No rival -> -1 (meet_again's `next_dist(player, dir) > 0` is false).
    let w = geo_win(12, &[1, 1, 1, 1], &[-1; 12]);
    assert!(compile("next_dist(0, 1) == -1").unwrap().eval(&w, &cand(0)));
}

#[test]
fn others_within_radius() {
    // Seat 0 at 0; rivals at 2, 5, 11. within(0, 2) = {11? dist 1, 1 at 2}
    // dist(0,2)=2, dist(0,5)=5, dist(0,11)=1. within r=2 -> seats at dist 1..=2
    // = seat 2 (dist 2) and seat 3 (dist 1) = 2.
    let w = geo_win(12, &[0, 2, 5, 11], &[-1; 12]);
    assert!(compile("others_within(0, 2) == 2").unwrap().eval(&w, &cand(0)));
    assert!(compile("others_within(0, 5) == 3").unwrap().eval(&w, &cand(0)));
    assert!(compile("others_within(0, 0) == 0").unwrap().eval(&w, &cand(0)));
    // haruhikage's shape.
    assert!(compile("others_within(owner, 5) > 0").unwrap().eval(&w, &cand(0)));
}

#[test]
fn owned_within_radius() {
    // Seat 0 at 0 owns tiles 1 and 6 (and 0). near(3) = dist <= 3 from pos 0:
    // tile 0 (d 0), tile 1 (d 1), tile 6 (d 6) -> tiles 0,1 = 2.
    let owners = [0, 0, -1, -1, -1, -1, 0, -1, -1, -1, -1, -1];
    let w = geo_win(12, &[0, 3, 6, 9], &owners);
    assert!(compile("owned_within(0, 3) == 2").unwrap().eval(&w, &cand(0)));
    assert!(compile("owned_within(0, 6) == 3").unwrap().eval(&w, &cand(0)));
    assert!(compile("owned_within(1, 0) == 0").unwrap().eval(&w, &cand(0)));
}

#[test]
fn on_path_counts_my_deeds() {
    // Seat 1 (them) at 2, roll 5 -> forward tiles 3,4,5,6,7. Seat 0 (me)
    // owns 4 and 7 -> 2.
    let owners = [-1, -1, -1, -1, 0, -1, -1, 0, -1, -1, -1, -1];
    let mut w = geo_win(12, &[0, 2, 5, 9], &owners);
    w.mv.roll = Some(5);
    assert!(compile("on_path(0, 1) == 2").unwrap().eval(&w, &cand(0)));
    // No face -> 0 (repaint's residual is `on_path > 0`).
    w.mv.roll = None;
    assert!(compile("on_path(0, 1) == 0").unwrap().eval(&w, &cand(0)));
}

#[test]
fn between_counts_rivals_in_span() {
    // Seat 0 at 0, roll 5, dir forward -> span 1..=5. Rivals at 2, 5, 11.
    // 11 is behind (fwd 11) -> out. 2 and 5 are in -> 2.
    let mut w = geo_win(12, &[0, 2, 5, 11], &[-1; 12]);
    w.mv.roll = Some(5);
    w.mv.dir = 1;
    assert!(compile("between(0) == 2").unwrap().eval(&w, &cand(0)));
    // Backward: span is 1..=5 behind -> 11 (bwd 1) is in, 2 (bwd 10) and 5
    // (bwd 7) are out -> 1.
    w.mv.dir = -1;
    assert!(compile("between(0) == 1").unwrap().eval(&w, &cand(0)));
}

#[test]
fn gains_and_targeted_counters() {
    let mut w = win();
    w.players[0].gains = 3;
    w.players[1].targeted = 2;
    assert!(compile("gains_this_turn(0) == 3").unwrap().eval(&w, &cand(0)));
    assert!(compile("gains_this_turn(owner) == 3").unwrap().eval(&w, &cand(0)));
    assert!(compile("targeted_count(1) == 2").unwrap().eval(&w, &cand(0)));
    assert!(compile("targeted_count(owner) >= 2").unwrap().eval(&w, &cand(1)));
    // hagumi_marks / centrifugal shapes.
    assert!(compile("gains_this_turn(owner) > 0").unwrap().eval(&w, &cand(0)));
    assert!(!compile("targeted_count(owner) >= 2").unwrap().eval(&w, &cand(0)));
}

#[test]
fn repeated_digits_predicate() {
    // no_breakup's `cant_play` filter: money with a repeated digit.
    assert!(compile("repeated_digits(44)").unwrap().eval(&win(), &cand(0)));
    assert!(compile("repeated_digits(-444)").unwrap().eval(&win(), &cand(0)));
    assert!(!compile("repeated_digits(1234)").unwrap().eval(&win(), &cand(0)));
    assert!(!compile("repeated_digits(123)").unwrap().eval(&win(), &cand(0)));
    assert!(!compile("repeated_digits(0)").unwrap().eval(&win(), &cand(0)));
    // no_breakup's shape: `!repeated_digits(money(owner))`.
    let mut w = win();
    w.players[0].money = 1223;
    assert!(compile("repeated_digits(money(owner))").unwrap().eval(&w, &cand(0)));
    w.players[0].money = 1234;
    assert!(!compile("repeated_digits(money(owner))").unwrap().eval(&w, &cand(0)));
}
