//! Shipped-card behaviour through the ruleset loader seam
//! (`dist/cards`): manifest contents, dice-dependent prompts, per-card
//! counteract scope, cross-module nested play, fire-up cost. The
//! loader-mechanism tests (handles, hash, duplicate ids, traps) live in
//! `crates/game-rules/tests/ruleset.rs`.
//!
//! Build the modules first: `node tools/build-ruleset.mjs`.

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
        r.cards()[yolo as usize].counteracts_to(TriggerKind::DiceRollResult)
            && !r.cards()[yolo as usize].has_play()
    );
    assert!(r.cards()[hagumi as usize].has_play());
    assert!(r.card("nope").is_none());
}

/// Exactly the twenty song-title cards declare `prop::SONG` (Kizuna Music's
/// 「卡名为歌名的卡」). The count is the X the event rolls; a drift here is a
/// rulebook disagreement.
#[test]
fn exactly_twenty_song_cards_declare_song() {
    let r = ruleset();
    let songs: Vec<&str> = r
        .cards()
        .iter()
        .filter(|c| r.card_prop(&c.id, "song") != 0)
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(songs.len(), 20, "SONG cards: {songs:?}");
    for id in [
        "PPP:Returns",
        "PPP:STAR BEAT!",
        "AG:宣战布告",
        "AG:Y.O.L.O",
        "AG:ONE OF US",
        "PP:TITLE IDOL",
        "R:（ykn）louder",
        "R:Fire bird",
        "R:（燐子）Ringing Bloom",
        "R:Sprechchor",
        "R:轨迹",
        "RAS:R. I. O. T.",
        "RAS:狂乱Hey Kids!!",
        "RAS:UNSTOPPABLE",
        "MyGO:壱雫空",
        "MyGO:无路矢",
        "MyGO:轮符雨",
        "Mujica:黑色生日",
        "Mujica:骰子已经掷下",
        "CRYCHIC:春日影",
    ] {
        assert!(songs.contains(&id), "missing SONG on {id}");
    }
}

#[test]
fn hagumi_marks_replays_through_a_dice_dependent_prompt() {
    let r = ruleset();
    let card = r.card("HHW:（育美）").unwrap();
    let world = TestWorld::new(20261003);
    let call = Call::Play {
        card,
        player_id: 1,
        arg: Vec::new(),
    };
    // The two 4d20 rolls are host answers (a roll is a `NeedHost` request in
    // this harness): 15 -> tile 14, 48 -> tile 47.
    let faces = [15, 48];

    // Pass 1: the rolls answered, the pick not -> blocked on the choice.
    let Outcome::NeedInput(p, _) = r.run(&world, call.clone(), &faces, None).unwrap() else {
        panic!("expected a prompt")
    };
    assert_eq!(p.kind, PromptKind::Choice);
    assert_eq!(p.player_id, 1);
    // Two roll answers are already on the log; the pick is the third slot.
    assert_eq!(p.answer_slot, 2);
    assert_eq!(p.title.key(), "cards:card-hhw.hagumi_marks_ask_title");
    assert_eq!(p.options.len(), 2, "one option per roll: {:?}", p.options);

    // The prompt is reproducible: same rolls -> same options.
    let Outcome::NeedInput(again, _) = r.run(&world, call.clone(), &faces, None).unwrap() else {
        panic!()
    };
    assert_eq!(p, again);

    // Pass 2: answer with the last option -> commits a mark on that roll's tile.
    let pick: i32 = 1;
    // Every option is a labelled roll (`hagumi_marks_opt`), carrying its tile.
    let PromptOption::Str(opt) = &p.options[pick as usize] else {
        panic!("roll options are labelled")
    };
    let Some(game_core::msg::Arg::Tile(expected_tile)) = opt.a.get("tile") else {
        panic!("the option carries its tile: {opt:?}")
    };
    let expected_tile = *expected_tile;
    assert_eq!(expected_tile, 47, "the 48 roll maps to tile 47");
    let answers = [faces[0], faces[1], pick];
    let Outcome::Done(after) = r.run(&world, call.clone(), &answers, None).unwrap() else {
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
        after.events.last().unwrap(),
        &format!("log player_id=1 cards:card-hhw.hagumi_marks_placed{{tile=tile:{expected_tile}}}")
    );

    // Determinism: replaying the full answer log gives an identical world.
    let Outcome::Done(after2) = r.run(&world, call.clone(), &answers, None).unwrap() else {
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
        kind: TriggerKind::DiceRollResult,
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
            by_skill: false,
            settle_redirect: false,
        }),
        ..Default::default()
    };
    assert!(r.can_counteract(&world, card, 0).unwrap());

    // The 1d4 is a host request (`roll_direct`): answer it with a 3.
    let Outcome::Done(after) = r
        .run(
            &world,
            Call::Counteract {
                card,
                player_id: 0,
                arg: Vec::new(),
            },
            &[3],
            None,
        )
        .unwrap()
    else {
        panic!()
    };
    let roll = after.trigger.mv.and_then(|m| m.roll).unwrap();
    assert_eq!(roll, 15, "12 + 1d4(3)");
    let boost = Msg::new("cards:card-ag.yolo_boost")
        .player_id("who", 0)
        .i("n", 3)
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
        .run(
            &TestWorld::new(1),
            Call::Play {
                card,
                player_id: 0,
                arg: Vec::new(),
            },
            &[],
            None,
        )
        .unwrap_err();
    assert!(
        matches!(err, RuleError::Trap(ref m) if m.contains("fuel")),
        "{err}"
    );
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
        arg: Vec::new(),
    };

    // The prompt raised inside the *nested* card aborts the whole outer run.
    // The inner card's two 4d20 rolls are host answers and come first
    // (15 -> tile 14, 48 -> tile 47); the pick is the third slot.
    let faces = [15, 48];
    let Outcome::NeedInput(p, _) = r.run(&world, call.clone(), &faces, None).unwrap() else {
        panic!("expected a prompt")
    };
    assert_eq!(
        (p.kind, p.player_id, p.answer_slot),
        (PromptKind::Choice, 2, 2)
    );

    // Replay with the answer: outer and inner effects both complete, in order.
    let answers = [faces[0], faces[1], 0];
    let Outcome::Done(after) = r.run(&world, call.clone(), &answers, None).unwrap() else {
        panic!("expected Done")
    };
    let kinds: Vec<&str> = after
        .events
        .iter()
        .map(|e| e.split(' ').next().unwrap())
        .collect();
    // The harness answers a roll from the log without running the engine's
    // roll routine, so the faces leave no `dice` line here; the two logs bound
    // the inner card's own line.
    assert_eq!(kinds, ["log", "log", "log"]);
    assert!(
        after.events[0].ends_with("cards:fixture-test-cards.relay_start")
            && after.events[1].ends_with("cards:card-hhw.hagumi_marks_placed{tile=tile:14}")
            && after.events[2].ends_with("cards:fixture-test-cards.relay_end")
    );
    let PromptOption::Str(opt) = &p.options[0] else {
        panic!()
    };
    let Some(game_core::msg::Arg::Tile(t)) = opt.a.get("tile") else {
        panic!()
    };
    assert_eq!(after.marks[0].0, *t);
    assert_eq!(*t, 14, "the 15 roll maps to tile 14");

    // Determinism across modules: replaying the same answers gives an identical world.
    let Outcome::Done(after2) = r.run(&world, call.clone(), &answers, None).unwrap() else {
        panic!()
    };
    assert_eq!(after, after2);
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
    let call = Call::Play {
        card,
        player_id: 0,
        arg: Vec::new(),
    };
    // One cold run to pay first-time costs, and to prove the entry really runs.
    // `R:[衍生] 压` gains money, which is now a pipeline entry that pauses for the
    // host (`NeedHost`); any of Done / NeedInput / NeedHost proves the entry ran.
    assert!(matches!(
        r.run(&world, call.clone(), &[], None),
        Ok(Outcome::Done(_)) | Ok(Outcome::NeedInput(_, _)) | Ok(Outcome::NeedHost(_, _))
    ));

    let n = 100;
    let t = Instant::now();
    for _ in 0..n {
        let _ = r.run(&world, call.clone(), &[], None);
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
    let call = Call::Play {
        card,
        player_id: 0,
        arg: Vec::new(),
    };
    let _ = r.run(&world, call.clone(), &[], None);
    let n = 200;

    let t = Instant::now();
    for _ in 0..n {
        let _ = r.run(&world, call.clone(), &[], None);
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
    let call = Call::Play {
        card,
        player_id: 0,
        arg: Vec::new(),
    };
    let _ = r2.run(&world, call.clone(), &[], None);
    let n = 500;
    let t = Instant::now();
    for _ in 0..n {
        let _ = r2.run(&world, call.clone(), &[], None);
    }
    println!("  small check on warm ruleset: {:?}/call", t.elapsed() / n);
}
