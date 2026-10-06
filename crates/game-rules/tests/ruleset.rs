//! Integration tests against the real `dist/ruleset.wasm`.
//! Build it first: `tools/build-ruleset.sh`.

use game_rules::{
    Call, CardWorld, Msg, Outcome, PromptKind, PromptOption, RuleError, Ruleset, Trigger,
    TriggerKind,
};

fn dist(dir: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../dist")
        .join(dir)
}

/// Every module listed in `dist/<dir>/index.json`, in index order.
fn modules(dir: &str) -> Vec<Vec<u8>> {
    let index = dist(dir).join("index.json");
    let text = std::fs::read_to_string(&index)
        .unwrap_or_else(|_| panic!("{} missing -- run tools/build-ruleset.sh", index.display()));
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| std::fs::read(dist(dir).join(m["file"].as_str().unwrap())).unwrap())
        .collect()
}

fn load(sets: &[&str]) -> Ruleset {
    let mut b = Ruleset::builder();
    for dir in sets {
        for m in modules(dir) {
            b.add(&m).expect("module should load");
        }
    }
    b.build().expect("ruleset should build")
}

/// Shipped cards only.
fn ruleset() -> Ruleset {
    load(&["cards"])
}

/// Minimal world. The RNG lives here so replays draw the same numbers.
#[derive(Clone, Debug, PartialEq)]
struct TestWorld {
    rng: u64,
    money: Vec<i32>,
    marks: Vec<(i32, i32, Msg)>,
    events: Vec<String>,
    trigger: Trigger,
}

impl TestWorld {
    fn new(seed: u64) -> Self {
        Self {
            rng: seed,
            money: vec![10_000; 4],
            marks: vec![],
            events: vec![],
            trigger: Trigger::default(),
        }
    }
    fn d(&mut self, sides: i32) -> i32 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.rng >> 33) % sides as u64) as i32 + 1
    }
}

impl CardWorld for TestWorld {
    fn place_card_on(&mut self, _: i32, _: i32, _: &str, _: Msg) -> i32 {
        -1
    }
    // keyed state: the stub holds nothing, so every column reads 0 and writes
    // go nowhere. The real storage lives on `MatchPlayer::state`.
    fn state_var(&self, _: i32, _: &str) -> game_core::state::StateVar {
        game_core::state::StateVar::default()
    }
    fn state_get(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn state_min(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn state_max(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn state_expires(&self, _: i32, _: &str) -> Option<game_core::state::Tick> {
        None
    }
    fn state_set(&mut self, _: i32, _: &str, value: i32) -> i32 {
        value
    }
    fn state_add(&mut self, _: i32, _: &str, delta: i32) -> i32 {
        delta
    }
    fn state_set_bounds(&mut self, _: i32, _: &str, _: i32, _: i32) {}
    fn state_set_expires(&mut self, _: i32, _: &str, _: Option<game_core::state::Tick>) {}
    fn tick_state(&mut self, _: i32, _: game_core::state::Tick) -> Vec<(String, i32)> {
        Vec::new()
    }

    fn roll(&mut self, player_id: i32, count: i32, sides: i32) -> i32 {
        let t = (0..count).map(|_| self.d(sides)).sum();
        self.events
            .push(format!("dice player_id={player_id} {count}d{sides}={t}"));
        t
    }
    fn log(&mut self, player_id: i32, msg: Msg) {
        self.events.push(format!("log player_id={player_id} {msg}"));
    }
    fn tile_count(&self) -> i32 {
        60
    }
    fn add_mark(&mut self, tile: i32, player_id: i32, _kind: &str, note: Msg) {
        self.marks.push((tile, player_id, note));
    }
    fn money(&self, player_id: i32) -> i32 {
        self.money[player_id as usize]
    }
    fn gain(&mut self, player_id: i32, amount: i32, _: Msg) -> i32 {
        self.money[player_id as usize] += amount;
        amount
    }
    fn pay(&mut self, player_id: i32, amount: i32, _: Msg) -> i32 {
        self.money[player_id as usize] -= amount;
        amount
    }
    fn tile_named(&self, _: &str) -> i32 {
        -1
    }
    fn tile_owner(&self, _: i32) -> i32 {
        -1
    }
    fn player_pos(&self, _: i32) -> i32 {
        -1
    }
    fn tile_steps_ahead(&self, _: i32, _: i32) -> i32 {
        -1
    }
    fn rent_of(&self, _: i32) -> i32 {
        0
    }
    fn buy_price(&self, _: i32) -> i32 {
        0
    }
    fn build_cost(&self, _: i32) -> i32 {
        0
    }
    fn mortgage_value(&self, _: i32) -> i32 {
        0
    }
    fn owned_count(&self, _: i32) -> i32 {
        0
    }
    fn owned_at(&self, _: i32, _: i32) -> i32 {
        -1
    }
    fn player_count(&self) -> i32 {
        4
    }
    fn player_out(&self, _: i32) -> i32 {
        0
    }
    fn others_count(&self, _: i32) -> i32 {
        3
    }
    fn others_at(&self, player_id: i32, index: i32) -> i32 {
        [0, 1, 2, 3]
            .into_iter()
            .filter(|&i| i != player_id)
            .nth(index.max(0) as usize)
            .unwrap_or(-1)
    }
    fn draw(&mut self, _: i32, _: i32) -> i32 {
        0
    }
    fn add_to_hand(&mut self, _: i32, _: &str) {}
    fn add_to_deck(&mut self, _: i32, _: &str, _: bool) {}
    fn add_to_deck_at(&mut self, _: i32, _: &str, _: i32) {}
    fn to_discard(&mut self, _: i32, _: &str) {}
    fn place_card(&mut self, _: i32, _: &str, _: game_core::msg::Msg) -> i32 {
        -1
    }
    fn set_dest(&mut self, _: i32) {}
    fn set_transfer_to_dest(&mut self, _: i32, _: i32) {}
    fn send_to_dest(&mut self, _: i32) -> Option<i32> {
        None
    }
    fn transfer_to_dest(&mut self, _: i32, _: i32) -> Option<i32> {
        None
    }
    fn ring_multiplier(&self) -> i32 {
        10
    }
    fn add_ring_bonus(&mut self, _: i32) -> i32 {
        0
    }
    fn teleport_to(&mut self, _: i32, _: i32) {}
    fn unplace_card(&mut self) -> i32 {
        0
    }
    fn is_placed(&self) -> i32 {
        0
    }
    fn count_marks(&self, _: i32, _: &str, _: i32) -> i32 {
        0
    }
    fn remove_marks(&mut self, _: i32, _: &str, _: i32) -> i32 {
        0
    }
    fn tok(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn set_tok(&mut self, _: i32, _: &str, _: i32) {}
    fn add_tok(&mut self, _: i32, _: &str, _: i32, _: i32) -> i32 {
        0
    }
    fn slot(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn set_slot(&mut self, _: i32, _: &str, _: i32) {}
    fn inc_slot(&mut self, _: i32, _: &str, _: i32) -> i32 {
        0
    }
    fn band_crystals(&self, _: i32) -> i32 {
        0
    }
    fn add_band_crystals(&mut self, _: i32, _: i32, _: i32) -> i32 {
        0
    }
    fn fire(&self, _: i32) -> i32 {
        0
    }
    fn fire_max(&self, _: i32) -> i32 {
        0
    }
    fn gain_fire(&mut self, _: i32, _: i32, _: Msg) -> i32 {
        0
    }
    fn give_stay(&mut self, _: i32, _: i32) {}
    fn give_stun(&mut self, _: i32, _: i32) {}
    fn give_exile(&mut self, _: i32, _: i32, _: i32) {}
    fn give_extra_turn(&mut self, _: i32) {}
    fn trigger(&self) -> Trigger {
        self.trigger.clone()
    }
    fn set_trigger_move_roll(&mut self, roll: i32) {
        self.trigger.move_roll = Some(roll);
    }
    fn set_trigger_value(&mut self, value: i32) {
        self.trigger.value = value;
    }
    fn crystals(&self) -> i32 {
        0
    }
    fn set_crystals(&mut self, _: i32) -> i32 {
        0
    }
    fn add_crystals(&mut self, _: i32, _: i32) -> i32 {
        0
    }
    fn take_card(&mut self, _: i32, _: game_rules::CardPile, _: &str) -> bool {
        false
    }
    fn cards_in(&self, _: i32, _: game_rules::CardPile) -> Vec<String> {
        Vec::new()
    }
    fn set_trigger_target(&mut self, to: i32) {
        self.trigger.target = to;
    }
    fn set_trigger_cancelled(&mut self) {
        self.trigger.negate_activation();
    }
    fn set_trigger_negate_effect(&mut self) {
        self.trigger.negate_effect();
    }
    fn set_trigger_spare(&mut self, seat: i32) {
        self.trigger.spare(seat);
    }
    fn declare_trigger_effect(&mut self, kind: i32, target: i32, from: i32, tile: i32, value: i32) {
        self.trigger.declare(game_core::engine::rules::Effect {
            kind: card_sdk::abi::TriggerKind::from_i32(kind).as_str(),
            target,
            from,
            tile,
            value,
        });
    }
    fn trig_card_is(&self, id: &str) -> i32 {
        (self.trigger.card == id) as i32
    }
    // The test world models money, marks and dice only -- board/hand queries are
    // neutral until a test needs them.
    fn is_buyable(&self, _: i32) -> i32 {
        0
    }
    fn is_shop(&self, _: i32) -> i32 {
        0
    }
    fn is_ring(&self, _: i32) -> i32 {
        0
    }
    fn is_circle(&self, _: i32) -> i32 {
        0
    }
    fn is_live_house(&self, _: i32) -> i32 {
        0
    }
    fn tile_group(&self, _: i32) -> i32 {
        -1
    }
    fn tile_price(&self, _: i32) -> i32 {
        0
    }
    fn houses_of(&self, _: i32) -> i32 {
        0
    }
    fn set_houses(&mut self, _: i32, _: i32) {}
    fn add_house(&mut self, _: i32, _: i32) -> i32 {
        0
    }
    fn mortgaged_of(&self, _: i32) -> i32 {
        0
    }
    fn set_mortgaged(&mut self, _: i32, _: i32) {}
    fn set_owner(&mut self, _: i32, _: i32) {}
    fn dist(&self, _: i32, _: i32) -> i32 {
        0
    }
    fn tile_forward(&self, _: i32, _: i32) -> i32 {
        0
    }
    fn neighbor(&self, _: i32, _: i32) -> i32 {
        -1
    }
    fn players_on_count(&self, _: i32, _: i32) -> i32 {
        0
    }
    fn players_on_at(&self, _: i32, _: i32, _: i32) -> i32 {
        -1
    }
    fn hand_count(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn hand_size(&self, _: i32) -> i32 {
        0
    }
    fn discard_count(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn deck_count(&self, _: i32) -> i32 {
        0
    }
    fn discard_size(&self, _: i32) -> i32 {
        0
    }
    fn discard_from_hand(&mut self, _: i32, _: &str) -> i32 {
        0
    }
    fn shuffle_into_deck(&mut self, _: i32, _: bool, _: bool) -> i32 {
        0
    }
    fn can_pay(&self, _: i32) -> i32 {
        1
    }
    fn cant_move(&self, _: i32) -> i32 {
        0
    }
    fn spend_fire(&mut self, _: i32, _: i32, _: Msg) -> i32 {
        0
    }
    fn stay_of(&self, _: i32) -> i32 {
        0
    }
    fn stun_of(&self, _: i32) -> i32 {
        0
    }
    fn turn_player(&self) -> i32 {
        0
    }
    fn round_no(&self) -> i32 {
        0
    }
    fn turn_key(&self) -> i32 {
        1
    }
    fn character_is(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn in_band(&self, _: i32, _: &str) -> i32 {
        0
    }
}

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
    let Outcome::NeedInput(p) = r.run(&world, call, &[]).unwrap() else {
        panic!("expected a prompt")
    };
    assert_eq!(p.kind, PromptKind::Tile);
    assert_eq!(p.player_id, 1);
    assert_eq!(p.answer_slot, 0);
    assert_eq!(p.title.key(), "cards:card-hhw.hagumi_marks_ask_title");
    assert!(!p.options.is_empty() && p.options.len() <= 2);

    // The prompt is reproducible: same snapshot -> same options.
    let Outcome::NeedInput(again) = r.run(&world, call, &[]).unwrap() else {
        panic!()
    };
    assert_eq!(p, again);

    // Pass 2: answer with the last option -> commits a mark on exactly that tile.
    let pick = p.options.len() as i32 - 1;
    let PromptOption::Int(expected_tile) = p.options[pick as usize] else {
        panic!("tile options are ints")
    };
    let Outcome::Done(after) = r.run(&world, call, &[pick]).unwrap() else {
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
    let Outcome::Done(after2) = r.run(&world, call, &[pick]).unwrap() else {
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
        move_roll: Some(12),
        ..Default::default()
    };
    assert!(r.can_counteract(&world, card, 0).unwrap());

    let Outcome::Done(after) = r
        .run(&world, Call::Counteract { card, player_id: 0 }, &[])
        .unwrap()
    else {
        panic!()
    };
    let roll = after.trigger.move_roll.unwrap();
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
        world.trigger.move_roll,
        Some(12),
        "input world is never modified"
    );
}

#[test]
fn runaway_effect_is_stopped_by_fuel() {
    let r = ruleset().with_fuel(200);
    let card = r.card("HHW:（育美）").unwrap();
    let err = r
        .run(&TestWorld::new(1), Call::Play { card, player_id: 0 }, &[])
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
            &[]
        ),
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
    let Outcome::NeedInput(p) = r.run(&world, call, &[]).unwrap() else {
        panic!("expected a prompt")
    };
    assert_eq!(
        (p.kind, p.player_id, p.answer_slot),
        (PromptKind::Tile, 2, 0)
    );

    // Replay with the answer: outer and inner effects both complete, in order.
    let Outcome::Done(after) = r.run(&world, call, &[0]).unwrap() else {
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
    let Outcome::Done(after2) = r.run(&world, call, &[0]).unwrap() else {
        panic!()
    };
    assert_eq!(after, after2);
}

#[test]
fn runaway_nesting_is_stopped() {
    let r = load(&["cards", "fixtures"]);
    let card = r.card("TEST:recurse").unwrap();
    let err = r
        .run(&TestWorld::new(1), Call::Play { card, player_id: 0 }, &[])
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
    assert!(matches!(
        r.run(&world, call, &[]),
        Ok(Outcome::Done(_)) | Ok(Outcome::NeedInput(_))
    ));

    let n = 100;
    let t = Instant::now();
    for _ in 0..n {
        let _ = r.run(&world, call, &[]);
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
    let _ = r.run(&world, call, &[]);
    let n = 200;

    let t = Instant::now();
    for _ in 0..n {
        let _ = r.run(&world, call, &[]);
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
    let _ = r2.run(&world, call, &[]);
    let n = 500;
    let t = Instant::now();
    for _ in 0..n {
        let _ = r2.run(&world, call, &[]);
    }
    println!("  small check on warm ruleset: {:?}/call", t.elapsed() / n);
}
