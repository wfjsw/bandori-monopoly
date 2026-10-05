//! Integration tests against the real `dist/ruleset.wasm`.
//! Build it first: `tools/build-ruleset.sh`.

use game_rules::{CardWorld, Call, Msg, Outcome, PromptKind, PromptOption, RuleError, Ruleset, Trigger, TriggerKind};

fn dist(dir: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist").join(dir)
}

/// Every module listed in `dist/<dir>/index.json`, in index order.
fn modules(dir: &str) -> Vec<Vec<u8>> {
    let index = dist(dir).join("index.json");
    let text = std::fs::read_to_string(&index)
        .unwrap_or_else(|_| panic!("{} missing -- run tools/build-ruleset.sh", index.display()));
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["modules"].as_array().unwrap().iter()
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
        Self { rng: seed, money: vec![10_000; 4], marks: vec![], events: vec![], trigger: Trigger::default() }
    }
    fn d(&mut self, sides: i32) -> i32 {
        self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.rng >> 33) % sides as u64) as i32 + 1
    }
}

impl CardWorld for TestWorld {
    fn roll(&mut self, seat: i32, count: i32, sides: i32) -> i32 {
        let t = (0..count).map(|_| self.d(sides)).sum();
        self.events.push(format!("dice seat={seat} {count}d{sides}={t}"));
        t
    }
    fn log(&mut self, seat: i32, msg: Msg) {
        self.events.push(format!("log seat={seat} {msg}"));
    }
    fn tile_count(&self) -> i32 {
        60
    }
    fn add_mark(&mut self, tile: i32, seat: i32, _kind: &str, note: Msg) {
        self.marks.push((tile, seat, note));
    }
    fn money(&self, seat: i32) -> i32 {
        self.money[seat as usize]
    }
    fn gain(&mut self, seat: i32, amount: i32, _: Msg) -> i32 {
        self.money[seat as usize] += amount;
        amount
    }
    fn pay(&mut self, seat: i32, amount: i32, _: Msg) -> i32 {
        self.money[seat as usize] -= amount;
        amount
    }
    fn tile_named(&self, _: &str) -> i32 {
        -1
    }
    fn tile_owner(&self, _: i32) -> i32 {
        -1
    }
    fn seat_pos(&self, _: i32) -> i32 {
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
    fn seat_count(&self) -> i32 {
        4
    }
    fn seat_out(&self, _: i32) -> i32 {
        0
    }
    fn others_count(&self, _: i32) -> i32 {
        3
    }
    fn others_at(&self, seat: i32, index: i32) -> i32 {
        [0, 1, 2, 3].into_iter().filter(|&i| i != seat).nth(index.max(0) as usize).unwrap_or(-1)
    }
    fn draw(&mut self, _: i32, _: i32) -> i32 {
        0
    }
    fn add_to_hand(&mut self, _: i32, _: &str) {}
    fn add_to_deck(&mut self, _: i32, _: &str, _: bool) {}
    fn add_to_deck_at(&mut self, _: i32, _: &str, _: i32) {}
    fn to_discard(&mut self, _: i32, _: &str) {}
    fn place_card(&mut self, _: i32, _: &str, _: game_core::msg::Msg) {}
    fn set_dest(&mut self, _: i32) {}
    fn ring_multiplier(&self) -> i32 {
        10
    }
    fn add_ring_bonus(&mut self, _: i32) -> i32 {
        0
    }
    fn teleport_to(&mut self, _: i32, _: i32) {}
    fn unplace_card(&mut self, _: i32) -> bool {
        false
    }
    fn is_placed(&self, _: i32) -> i32 {
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
    fn seats_on_count(&self, _: i32, _: i32) -> i32 {
        0
    }
    fn seats_on_at(&self, _: i32, _: i32, _: i32) -> i32 {
        -1
    }
    fn hand_count(&self, _: i32, _: &str) -> i32 {
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
    fn sweep_to_deck(&mut self, _: i32) -> i32 {
        0
    }
    fn can_pay(&self, _: i32) -> i32 {
        1
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
    fn turn_seat(&self) -> i32 {
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
    assert!(r.card("R:[衍生] 压").is_some(), "official cards ship in family modules");
    assert!(r.cards()[yolo as usize].react && !r.cards()[yolo as usize].play);
    assert!(r.cards()[hagumi as usize].play);
    assert!(r.card("nope").is_none());
}

#[test]
fn hagumi_marks_replays_through_a_dice_dependent_prompt() {
    let r = ruleset();
    let card = r.card("HHW:（育美）").unwrap();
    let world = TestWorld::new(20261003);
    let call = Call::Play { card, seat: 1 };

    // Pass 1: no answers -> blocked on the tile prompt, nothing committed.
    let Outcome::NeedInput(p) = r.run(&world, call, &[]).unwrap() else { panic!("expected a prompt") };
    assert_eq!(p.kind, PromptKind::Tile);
    assert_eq!(p.seat, 1);
    assert_eq!(p.answer_slot, 0);
    assert_eq!(p.title.key(), "cards:card-hhw.hagumi_marks_ask_title");
    assert!(!p.options.is_empty() && p.options.len() <= 2);

    // The prompt is reproducible: same snapshot -> same options.
    let Outcome::NeedInput(again) = r.run(&world, call, &[]).unwrap() else { panic!() };
    assert_eq!(p, again);

    // Pass 2: answer with the last option -> commits a mark on exactly that tile.
    let pick = p.options.len() as i32 - 1;
    let PromptOption::Int(expected_tile) = p.options[pick as usize] else { panic!("tile options are ints") };
    let Outcome::Done(after) = r.run(&world, call, &[pick]).unwrap() else { panic!("expected Done") };
    assert_eq!(after.marks, vec![(expected_tile, 1, Msg::new("cards:card-hhw.hagumi_marks_mark_note").n("money", 2000))]);
    assert_eq!(after.events.iter().filter(|e| e.starts_with("dice")).count(), 2);
    assert_eq!(after.events.last().unwrap(), &format!("log seat=1 cards:card-hhw.hagumi_marks_placed{{tile=tile:{expected_tile}}}"));

    // Determinism: replaying the full answer log gives an identical world.
    let Outcome::Done(after2) = r.run(&world, call, &[pick]).unwrap() else { panic!() };
    assert_eq!(after, after2);
}

#[test]
fn yolo_reacts_only_to_rolls_and_adds_1d4() {
    let r = ruleset();
    let card = r.card("AG:Y.O.L.O").unwrap();
    let mut world = TestWorld::new(7);

    assert!(!r.can_react(&world, card, 0).unwrap(), "no trigger -> cannot react");

    world.trigger = Trigger { kind: TriggerKind::MoveRoll, seat: 2, move_roll: Some(12), ..Default::default() };
    assert!(r.can_react(&world, card, 0).unwrap());

    let Outcome::Done(after) = r.run(&world, Call::React { card, seat: 0 }, &[]).unwrap() else { panic!() };
    let roll = after.trigger.move_roll.unwrap();
    assert!((13..=16).contains(&roll), "12 + 1d4, got {roll}");
    let boost = Msg::new("cards:card-ag.yolo_boost").seat("who", 2).i("n", (roll - 12) as i64).i("total", roll as i64);
    assert_eq!(after.events.last().unwrap(), &format!("log seat=0 {boost}"));
    assert_eq!(world.trigger.move_roll, Some(12), "input world is never modified");
}

#[test]
fn runaway_effect_is_stopped_by_fuel() {
    let r = ruleset().with_fuel(200);
    let card = r.card("HHW:（育美）").unwrap();
    let err = r.run(&TestWorld::new(1), Call::Play { card, seat: 0 }, &[]).unwrap_err();
    assert!(matches!(err, RuleError::Trap(ref m) if m.contains("fuel")), "{err}");
}

#[test]
fn bad_handle_is_rejected() {
    let r = ruleset();
    // Derived from the loaded set: a fixed handle goes stale as cards are ported.
    let bad = r.cards().len() as i32 + 7;
    assert!(matches!(r.run(&TestWorld::new(1), Call::Play { card: bad, seat: 0 }, &[]), Err(RuleError::NoSuchCard(_))));
}

#[test]
fn ruleset_hash_is_independent_of_load_order() {
    let mods = modules("cards");
    let mut fwd = Ruleset::builder();
    let mut rev = Ruleset::builder();
    for m in &mods { fwd.add(m).unwrap(); }
    for m in mods.iter().rev() { rev.add(m).unwrap(); }
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
    assert_ne!(r.module_sha256(relay), r.module_sha256(r.card("HHW:（育美）").unwrap()), "different modules");
    let world = TestWorld::new(42);
    let call = Call::Play { card: relay, seat: 2 };

    // The prompt raised inside the *nested* card aborts the whole outer run.
    let Outcome::NeedInput(p) = r.run(&world, call, &[]).unwrap() else { panic!("expected a prompt") };
    assert_eq!((p.kind, p.seat, p.answer_slot), (PromptKind::Tile, 2, 0));

    // Replay with the answer: outer and inner effects both complete, in order.
    let Outcome::Done(after) = r.run(&world, call, &[0]).unwrap() else { panic!("expected Done") };
    let kinds: Vec<&str> = after.events.iter().map(|e| e.split(' ').next().unwrap()).collect();
    assert_eq!(kinds, ["log", "dice", "dice", "log", "log"]);
    assert!(after.events[0].ends_with("cards:fixture-test-cards.relay_start") && after.events[4].ends_with("cards:fixture-test-cards.relay_end"));
    let PromptOption::Int(t) = p.options[0] else { panic!() };
    assert_eq!(after.marks[0].0, t);

    // Determinism across modules: replaying the same answers gives an identical world.
    let Outcome::Done(after2) = r.run(&world, call, &[0]).unwrap() else { panic!() };
    assert_eq!(after, after2);
}

#[test]
fn runaway_nesting_is_stopped() {
    let r = load(&["cards", "fixtures"]);
    let card = r.card("TEST:recurse").unwrap();
    let err = r.run(&TestWorld::new(1), Call::Play { card, seat: 0 }, &[]).unwrap_err();
    assert!(matches!(err, RuleError::Trap(ref m) if m.contains("nested deeper")), "{err}");
}
