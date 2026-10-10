//! Integration tests against the real `dist/ruleset.wasm`.
//! Build it first: `tools/build-ruleset.sh`.
//!
//! The `TEST:pre*` guard-condition tests live in `pre_conditions.rs` (their
//! own binary): they assert on the process-global `guard_cost` counters,
//! which every test here would otherwise race.

#[path = "testworld.rs"]
mod testworld;
use testworld::*;


#[test]
fn loads_and_validates() {
    let r = ruleset();
    assert_eq!(r.sha256().len(), 64);
    // The shipped artifact is one unified module (tools/rules_aggregate.py);
    // the per-band layout lives in the source crates, not in what we publish.
    assert!(r.module_count() >= 1, "at least one shipped module");
    let yolo = r.card("AG:Y.O.L.O").expect("yolo in manifest");
    let hagumi = r.card("HHW:（育美）").expect("hagumi in manifest");
    assert!(
        r.card("R:[衍生] 压").is_some(),
        "official cards ship in family modules"
    );
    assert!(
        r.cards()[yolo as usize].counteracts_to(TriggerKind::MoveRoll)
            && !r.cards()[yolo as usize].has_play()
    );
    assert!(r.cards()[hagumi as usize].has_play());
    assert!(r.card("nope").is_none());
}

#[test]
fn hagumi_marks_replays_through_a_dice_dependent_prompt() {
    let r = ruleset();
    let card = r.card("HHW:（育美）").unwrap();
    let world = TestWorld::new(20261003);
    let call = Call::Play { card, player_id: 1 };

    // Pass 1: no answers -> blocked on the tile prompt, nothing committed.
    let Outcome::NeedInput(p) = r.run(&world, call, &[], None).unwrap() else {
        panic!("expected a prompt")
    };
    assert_eq!(p.kind, PromptKind::Tile);
    assert_eq!(p.player_id, 1);
    assert_eq!(p.answer_slot, 0);
    assert_eq!(p.title.key(), "cards:card-hhw.hagumi_marks_ask_title");
    assert!(!p.options.is_empty() && p.options.len() <= 2);

    // The prompt is reproducible: same snapshot -> same options.
    let Outcome::NeedInput(again) = r.run(&world, call, &[], None).unwrap() else {
        panic!()
    };
    assert_eq!(p, again);

    // Pass 2: answer with the last option -> commits a mark on exactly that tile.
    let pick = p.options.len() as i32 - 1;
    let PromptOption::Int(expected_tile) = p.options[pick as usize] else {
        panic!("tile options are ints")
    };
    let Outcome::Done(after) = r.run(&world, call, &[pick], None).unwrap() else {
        panic!("expected Done")
    };
    assert_eq!(
        after.marks,
        vec![(
            expected_tile,
            1,
            Msg::new("cards:card-hhw.hagumi_marks_mark_note").n("money", 2000)
        )]
    );
    assert_eq!(
        after
            .events
            .iter()
            .filter(|e| e.starts_with("dice"))
            .count(),
        2
    );
    assert_eq!(
        after.events.last().unwrap(),
        &format!("log player_id=1 cards:card-hhw.hagumi_marks_placed{{tile=tile:{expected_tile}}}")
    );

    // Determinism: replaying the full answer log gives an identical world.
    let Outcome::Done(after2) = r.run(&world, call, &[pick], None).unwrap() else {
        panic!()
    };
    assert_eq!(after, after2);
}

#[test]
fn yolo_counteracts_only_to_rolls_and_adds_1d4() {
    let r = ruleset();
    let card = r.card("AG:Y.O.L.O").unwrap();
    let mut world = TestWorld::new(7);

    assert!(
        !r.can_counteract(&world, card, 0).unwrap(),
        "no trigger -> cannot counteract"
    );

    world.trigger = Trigger {
        kind: TriggerKind::MoveRoll,
        // 「**你的**掷骰结算前」 -- the user's own roll (sheet revision).
        player_id: 0,
        mv: Some(game_rules::TriggerMove {
            kind: game_rules::MoveKind::Walk,
            resolve: true,
            tags: Vec::new(),
            main: true,
            dir: game_core::engine::rules::Dir::Forward,
            from: 0,
            remaining: 0,
            total: 0,
            roll: Some(12),
        }),
        ..Default::default()
    };
    assert!(r.can_counteract(&world, card, 0).unwrap());

    let Outcome::Done(after) = r
        .run(&world, Call::Counteract { card, player_id: 0 }, &[], None)
        .unwrap()
    else {
        panic!()
    };
    let roll = after.trigger.mv.and_then(|m| m.roll).unwrap();
    assert!((13..=16).contains(&roll), "12 + 1d4, got {roll}");
    let boost = Msg::new("cards:card-ag.yolo_boost")
        .player_id("who", 0)
        .i("n", (roll - 12) as i64)
        .i("total", roll as i64);
    assert_eq!(
        after.events.last().unwrap(),
        &format!("log player_id=0 {boost}")
    );
    assert_eq!(
        world.trigger.mv.and_then(|m| m.roll),
        Some(12),
        "input world is never modified"
    );
}

#[test]
fn runaway_effect_is_stopped_by_fuel() {
    let r = ruleset().with_fuel(200);
    let card = r.card("HHW:（育美）").unwrap();
    let err = r
        .run(&TestWorld::new(1), Call::Play { card, player_id: 0 }, &[], None)
        .unwrap_err();
    assert!(
        matches!(err, RuleError::Trap(ref m) if m.contains("fuel")),
        "{err}"
    );
}

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
fn cross_module_play_card_replays_the_inner_prompt() {
    let r = load(&["cards", "fixtures"]);
    let relay = r.card("TEST:relay").unwrap();
    assert_ne!(
        r.module_sha256(relay),
        r.module_sha256(r.card("HHW:（育美）").unwrap()),
        "different modules"
    );
    let world = TestWorld::new(42);
    let call = Call::Play {
        card: relay,
        player_id: 2,
    };

    // The prompt raised inside the *nested* card aborts the whole outer run.
    let Outcome::NeedInput(p) = r.run(&world, call, &[], None).unwrap() else {
        panic!("expected a prompt")
    };
    assert_eq!(
        (p.kind, p.player_id, p.answer_slot),
        (PromptKind::Tile, 2, 0)
    );

    // Replay with the answer: outer and inner effects both complete, in order.
    let Outcome::Done(after) = r.run(&world, call, &[0], None).unwrap() else {
        panic!("expected Done")
    };
    let kinds: Vec<&str> = after
        .events
        .iter()
        .map(|e| e.split(' ').next().unwrap())
        .collect();
    assert_eq!(kinds, ["log", "dice", "dice", "log", "log"]);
    assert!(
        after.events[0].ends_with("cards:fixture-test-cards.relay_start")
            && after.events[4].ends_with("cards:fixture-test-cards.relay_end")
    );
    let PromptOption::Int(t) = p.options[0] else {
        panic!()
    };
    assert_eq!(after.marks[0].0, t);

    // Determinism across modules: replaying the same answers gives an identical world.
    let Outcome::Done(after2) = r.run(&world, call, &[0], None).unwrap() else {
        panic!()
    };
    assert_eq!(after, after2);
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

/// How long the rules submodule takes to fire up for a small check: build
/// (compile + validate + manifest) and then one `run` (store + linker +
/// instantiate + entry). Prints the breakdown; the assertion is only a
/// "not catastrophically slow" tripwire.
#[test]
fn fire_up_cost() {
    use std::time::Instant;
    let wasm = modules("cards");
    let kb: usize = wasm.iter().map(|w| w.len()).sum::<usize>() / 1024;

    let t = Instant::now();
    let mut b = Ruleset::builder();
    for m in &wasm {
        b.add(m).unwrap();
    }
    let r = b.build().unwrap();
    let build = t.elapsed();

    // A real entry point (Play), so the run reaches instantiate + call.
    let card = r
        .card("R:[衍生] 压")
        .or_else(|| r.card("AG:Y.O.L.O"))
        .unwrap();
    let world = TestWorld::new(1);
    let call = Call::Play { card, player_id: 0 };
    // One cold run to pay first-time costs, and to prove the entry really runs.
    // `R:[衍生] 压` gains money, which is now a pipeline entry that pauses for the
    // host (`NeedHost`); any of Done / NeedInput / NeedHost proves the entry ran.
    assert!(matches!(
        r.run(&world, call, &[], None),
        Ok(Outcome::Done(_)) | Ok(Outcome::NeedInput(_)) | Ok(Outcome::NeedHost(_, _))
    ));

    let n = 100;
    let t = Instant::now();
    for _ in 0..n {
        let _ = r.run(&world, call, &[], None);
    }
    let each = t.elapsed() / n;

    println!("fire_up: {} modules, {kb} KiB", wasm.len());
    println!("fire_up: build (compile+inspect+manifest) {build:?}");
    println!("fire_up: one real run (store+linker+instantiate+entry) {each:?} x{n}");
    assert!(
        each.as_millis() < 50,
        "a small check should fire up in single-digit ms, got {each:?}"
    );
}

/// Where a single fire-up's ~2ms goes: the linker is rebuilt per call
/// (`linker()` walks 159 `func_wrap` registrations), then the module is
/// instantiated into a fresh store.
#[test]
fn fire_up_breakdown() {
    use std::time::Instant;
    let wasm = modules("cards");
    let mut b = Ruleset::builder();
    for m in &wasm {
        b.add(m).unwrap();
    }
    let r = b.build().unwrap();
    let card = r.card("R:[衍生] 压").unwrap();
    let world = TestWorld::new(1);
    let call = Call::Play { card, player_id: 0 };
    let _ = r.run(&world, call, &[], None);
    let n = 200;

    let t = Instant::now();
    for _ in 0..n {
        let _ = r.run(&world, call, &[], None);
    }
    println!("breakdown: full run {:?}/call", t.elapsed() / n);
}

/// The "boot all rules to ask one small question" cost: loading the same
/// ruleset twice in one process. The engine is process-wide and caches
/// compilations, so the second load is the honest per-check boot.
#[test]
fn fire_up_reboot() {
    use std::time::Instant;
    let wasm = modules("cards");
    let boot = |label: &str| {
        let t = Instant::now();
        let mut b = Ruleset::builder();
        for m in &wasm {
            b.add(m).unwrap();
        }
        let r = b.build().unwrap();
        println!("  {label}: {:?}", t.elapsed());
        r
    };
    let r1 = boot("boot #1 (cold: compile)");
    let r2 = boot("boot #2 (warm: engine cache)");
    assert_eq!(r1.sha256(), r2.sha256());
    // And one small check on the warm ruleset.
    let card = r2.card("R:[衍生] 压").unwrap();
    let world = TestWorld::new(1);
    let call = Call::Play { card, player_id: 0 };
    let _ = r2.run(&world, call, &[], None);
    let n = 500;
    let t = Instant::now();
    for _ in 0..n {
        let _ = r2.run(&world, call, &[], None);
    }
    println!("  small check on warm ruleset: {:?}/call", t.elapsed() / n);
}
