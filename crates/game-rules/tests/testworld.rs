//! Shared test world + ruleset loaders for the `ruleset` / `pre_conditions`
//! integration tests. Included via `#[path = "testworld.rs"] mod testworld;`
//! so each test binary compiles its own copy (integration tests are separate
//! crates) without dragging in `common/mod.rs`'s full-match harness.
//!
//! The world is a minimal [`CardWorld`]: an RNG, per-seat money, marks and an
//! event log, plus the one [`Trigger`] the guard-condition tests inspect.

pub use game_rules::{
    Call, CardWorld, Msg, Outcome, PromptKind, PromptOption, RuleError, Ruleset, Trigger,
    TriggerKind,
};

pub fn dist(dir: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../dist")
        .join(dir)
}

/// Every module listed in `dist/<dir>/index.json`, in index order.
pub fn modules(dir: &str) -> Vec<Vec<u8>> {
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

pub fn load(sets: &[&str]) -> Ruleset {
    let mut b = Ruleset::builder();
    for dir in sets {
        for m in modules(dir) {
            b.add(&m).expect("module should load");
        }
    }
    b.build().expect("ruleset should build")
}

/// Shipped cards only.
pub fn ruleset() -> Ruleset {
    load(&["cards"])
}

/// Minimal world. The RNG lives here so replays draw the same numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct TestWorld {
    pub rng: u64,
    pub money: Vec<i32>,
    pub marks: Vec<(i32, i32, Msg)>,
    pub events: Vec<String>,
    pub trigger: Trigger,
}

impl TestWorld {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: seed,
            money: vec![10_000; 4],
            marks: vec![],
            events: vec![],
            trigger: Trigger::default(),
        }
    }
    pub fn d(&mut self, sides: i32) -> i32 {
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
    fn place_mark(
        &mut self,
        tile: i32,
        _kind: &str,
        _category: &str,
        owner: i32,
        _src: i32,
        _count: i32,
        note: Msg,
        _fresh: bool,
    ) -> i32 {
        self.marks.push((tile, owner, note));
        1
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
    fn count_marks(&self, _: i32, _: &game_core::state::MarkFilter<'_>) -> i32 {
        0
    }
    fn remove_marks(&mut self, _: i32, _: &game_core::state::MarkFilter<'_>) -> i32 {
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
    fn set_trigger_price(&mut self, v: i32) {
        self.trigger.price = v;
    }
    fn set_trigger_deal_owner(&mut self, v: i32) {
        self.trigger.deal_owner = v;
    }
    fn set_trigger_deal_houses(&mut self, v: i32) {
        self.trigger.deal_houses = v;
    }
    fn set_trigger_deal_mortgaged(&mut self, v: i32) {
        self.trigger.deal_mortgaged = v != 0;
    }
    fn set_trigger_reason(&mut self, reason: &str) {
        self.trigger.reason = reason.to_string();
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
