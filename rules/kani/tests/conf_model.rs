//! Model-conformance (trusted-base control) tests for the Kani symbolic host.
//!
//! Runs the same card entry / host op on
//!   (A) the real engine host (`game_rules::hostfns` over a thin `CardWorld`
//!       adapter -- the task's "thin adapter that calls hostfns with a mock
//!       HostCtx"), and
//!   (B) the symbolic model (`rules_kani::host::dispatch` + `world`).
//!
//! Compares resulting state + effect events. Any divergence is a **BUG IN THE
//! MODEL** unless the message says otherwise (engine bug candidate).
//!
//! Fail-closed: no `#[ignore]`, no allowlists. Unmodeled ops panic on the
//! model side and surface as test failures.
//!
//! `tests/conf_model.rs` is the entry point. The generated `SymOps` bridge
//! (HostOps -> dispatch) comes from `build.rs` via OUT_DIR.

use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, MutexGuard, OnceLock};

use card_sdk::kani_host::{Args, Op};
use game_rules::native_shims::{install_host, take_host, HostOps};
use game_rules::{
    with_inline_host, CardInfo, CardPile, CardWorld, CallOut, HostCtx, HostErr, HostRequest,
    HostState, InlineHost, Msg as EngineMsg, Prompt as EnginePrompt, RulesHandle,
};

use rules_kani::world::{self as kworld, World as KWorld, TILE_SHOP_NAME};

// ---------------------------------------------------------------------------
// Generated HostOps -> dispatch bridge
// ---------------------------------------------------------------------------
mod sym_ops {
    include!(concat!(env!("OUT_DIR"), "/conf_sym_ops.rs"));
}
use sym_ops::SymOps;

// ---------------------------------------------------------------------------
// Guest-arena helpers (names / messages cross the ABI as interned handles)
// ---------------------------------------------------------------------------

fn intern_bytes(b: &[u8]) -> (i32, i32) {
    card_sdk::native::intern(b)
}

fn intern_str(s: &str) -> (i32, i32) {
    intern_bytes(s.as_bytes())
}

fn intern_msg(key: &str) -> (i32, i32) {
    let m = card_sdk::msg::Msg::new(key);
    intern_bytes(&m.to_bytes())
}

/// The symbolic world is a process-global (`static mut`). Serialize every
/// model-side touch so parallel `cargo test` threads cannot clobber it. The
/// lock is REENTRANT per thread: every `#[test]` takes it at entry and helpers
/// may take it again (a plain `Mutex` deadlocks on that).
pub struct ModelGuard {
    _g: Option<MutexGuard<'static, ()>>,
}
impl Drop for ModelGuard {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get() - 1));
    }
}
thread_local! {
    static DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}
fn model_lock() -> ModelGuard {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let outer = DEPTH.with(|d| {
        let v = d.get();
        d.set(v + 1);
        v == 0
    });
    let g = if outer {
        Some(
            LOCK.get_or_init(|| Mutex::new(()))
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        )
    } else {
        None
    };
    ModelGuard { _g: g }
}

// ---------------------------------------------------------------------------
// Real-side world: a thin `CardWorld` that stores exactly the state we diff
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ConfWorld {
    pub n_players: i32,
    pub money: [i32; 4],
    pub pos: [i32; 4],
    pub out: [bool; 4],
    pub fire: [i32; 4],
    pub fire_max: [i32; 4],
    pub stay: [i32; 4],
    pub tiles_owner: [i32; 60],
    pub tiles_houses: [i32; 60],
    pub tiles_mortgaged: [bool; 60],
    /// Per-player hand card names (multiset).
    pub hands: Vec<Vec<String>>,
    pub decks: Vec<Vec<String>>,
    pub discards: Vec<Vec<String>>,
    /// Keyed state (slots). Separate from held, matching the real engine.
    pub state: BTreeMap<(i32, String), i32>,
    /// Held / tokens.
    pub held: BTreeMap<(i32, String), i32>,
    pub dest: i32,
    pub trig_value: i32,
    pub trig_target: i32,
    pub trig_cancelled: bool,
    pub turn_player: i32,
    pub events: Vec<String>,
    pub rng: u64,
    pub next_uid: i32,
    // ---- trigger payload (mirrors model `MoveSnap` / `BuySnap`) ----
    pub trig_main: bool,
    pub trig_roll_source: i32,
    pub trig_pay_is_rent: bool,
    pub move_present: bool,
    pub move_kind: i32,
    pub move_resolve: bool,
    pub move_dir: i32,
    pub move_from: i32,
    pub move_remaining: i32,
    pub move_total: i32,
    pub move_roll: i32,
    pub move_tags: BTreeMap<String, i32>,
    pub buy_present: bool,
    pub buy_kind: i32,
    pub buy_seller: i32,
    pub buy_price: i32,
    pub buy_deal_owner: i32,
    pub buy_deal_houses: i32,
    pub buy_deal_mortgaged: bool,
    // ---- field instances (uid → tile) for `tile_at` / `set_tile_at` ----
    pub field_uid: Vec<i32>,
    pub field_tile: Vec<i32>,
}

impl ConfWorld {
    pub fn new() -> Self {
        Self {
            n_players: 3,
            money: [0; 4],
            pos: [0; 4],
            out: [false; 4],
            fire: [0; 4],
            fire_max: [3; 4],
            stay: [0; 4],
            tiles_owner: [-1; 60],
            tiles_houses: [0; 60],
            tiles_mortgaged: [false; 60],
            hands: vec![Vec::new(); 4],
            decks: vec![Vec::new(); 4],
            discards: vec![Vec::new(); 4],
            state: BTreeMap::new(),
            held: BTreeMap::new(),
            dest: 0,
            trig_value: 0,
            trig_target: -1,
            trig_cancelled: false,
            turn_player: 0,
            events: Vec::new(),
            rng: 0x1234_5678_9abc_def0,
            next_uid: 1,
            trig_main: false,
            trig_roll_source: 0,
            trig_pay_is_rent: false,
            move_present: false,
            move_kind: -1,
            move_resolve: false,
            move_dir: 1,
            move_from: -1,
            move_remaining: 0,
            move_total: 0,
            move_roll: -1,
            move_tags: BTreeMap::new(),
            buy_present: false,
            buy_kind: 0,
            buy_seller: -1,
            buy_price: 0,
            buy_deal_owner: -1,
            buy_deal_houses: 0,
            buy_deal_mortgaged: false,
            field_uid: Vec::new(),
            field_tile: Vec::new(),
        }
    }

    fn d(&mut self, sides: i32) -> i32 {
        // Same mid-value formula as the model's native `roll_dice`.
        (1 * (sides.max(1) + 1)) / 2
    }

    fn pile_of(&mut self, pile: CardPile) -> &mut Vec<Vec<String>> {
        match pile {
            CardPile::Hand => &mut self.hands,
            CardPile::Discard => &mut self.discards,
            CardPile::Deck => &mut self.decks,
            _ => &mut self.hands,
        }
    }

    fn pile_ref(&self, pile: CardPile) -> &Vec<Vec<String>> {
        match pile {
            CardPile::Hand => &self.hands,
            CardPile::Discard => &self.discards,
            CardPile::Deck => &self.decks,
            _ => &self.hands,
        }
    }
}

/// Board static data comes from the same table the model uses, so tile_price /
/// tile_group / build_cost are definitionally equal and the test is about the
/// host op plumbing, not a second copy of board.json.
/// **0-based** engine tile index (see `board::tile_slot`).
fn board_price(tile: i32) -> i32 {
    let s = rules_kani::board::tile_slot(tile);
    if s < 0 {
        0
    } else {
        rules_kani::board::TILES[s as usize].price
    }
}
fn board_house_cost(tile: i32) -> i32 {
    let s = rules_kani::board::tile_slot(tile);
    if s < 0 {
        0
    } else {
        rules_kani::board::TILES[s as usize].house_cost
    }
}
fn board_group(tile: i32) -> i32 {
    let s = rules_kani::board::tile_slot(tile);
    if s < 0 {
        0
    } else {
        rules_kani::board::TILES[s as usize].group
    }
}

// A small name table for `tile_named`. The model currently resolves only
// 「江户川乐器店」; we mirror that so op-level tile_named is a plumbing check.
// Cards that look up other names will diverge (model gap) and fail -- correctly.
// 0-based engine index (`board.json` index 31 → position 30).
fn conf_tile_named(name: &str) -> i32 {
    if name == TILE_SHOP_NAME {
        30
    } else {
        -1
    }
}

impl CardWorld for ConfWorld {
    fn roll(&mut self, player_id: i32, count: i32, sides: i32) -> i32 {
        let mut t = 0;
        for _ in 0..count.max(0) {
            t += self.d(sides);
        }
        self.events.push(format!("dice {player_id} {count}d{sides}={t}"));
        t
    }
    fn log(&mut self, player_id: i32, msg: EngineMsg) {
        self.events.push(format!("log {player_id} {msg}"));
    }
    fn tile_count(&self) -> i32 {
        60
    }
    fn tile_named(&self, name: &str) -> i32 {
        conf_tile_named(name)
    }
    fn tile_owner(&self, tile: i32) -> i32 {
        let t = tile.max(0) as usize;
        if t < 60 {
            self.tiles_owner[t]
        } else {
            -1
        }
    }
    fn player_pos(&self, player_id: i32) -> i32 {
        self.pos[player_id.max(0) as usize % 4]
    }
    fn tile_steps_ahead(&self, player_id: i32, steps: i32) -> i32 {
        (self.player_pos(player_id) + steps).rem_euclid(60)
    }
    /// `WorldOps::rent_of` + `purchase::table_rent` (see `board::table_rent`).
    /// RiNG quotes the tile's base `price`; properties use the rent table at
    /// `rent_houses` (standing houses — ConfWorld has no RENT_HOUSES override).
    fn rent_of(&self, tile: i32) -> i32 {
        let s = rules_kani::board::tile_slot(tile);
        if s < 0 {
            return 0;
        }
        let t = &rules_kani::board::TILES[s as usize];
        if rules_kani::board::is_ring(t) {
            t.price
        } else {
            let h = self.tiles_houses[s as usize].max(0);
            rules_kani::board::table_rent(t, h)
        }
    }
    /// `purchase::quote_native` = land + houses × house_cost (then `.max(0)`).
    fn buy_price(&self, tile: i32) -> i32 {
        let s = rules_kani::board::tile_slot(tile);
        if s < 0 {
            return 0;
        }
        let t = &rules_kani::board::TILES[s as usize];
        let h = self.tiles_houses[s as usize].max(0);
        (t.price + h * t.house_cost).max(0)
    }
    fn build_cost(&self, tile: i32) -> i32 {
        board_house_cost(tile)
    }
    fn mortgage_value(&self, tile: i32) -> i32 {
        board_price(tile) / 2
    }
    fn owned_count(&self, player_id: i32) -> i32 {
        self.tiles_owner.iter().filter(|&&o| o == player_id).count() as i32
    }
    /// 0-based engine index (matches `WorldOps::owned_tiles`).
    fn owned_at(&self, player_id: i32, index: i32) -> i32 {
        self.tiles_owner
            .iter()
            .enumerate()
            .filter(|(_, &o)| o == player_id)
            .nth(index.max(0) as usize)
            .map(|(i, _)| i as i32)
            .unwrap_or(-1)
    }
    /// ABI v56: kind by wire spelling. Replaces `is_circle`/`is_ring`/`is_agent`.
    fn tile_is(&self, tile: i32, kind: &str) -> bool {
        let s = rules_kani::board::tile_slot(tile);
        if s < 0 {
            return false;
        }
        let t = &rules_kani::board::TILES[s as usize];
        match kind {
            "circle" => rules_kani::board::is_circle(t),
            "ring" => rules_kani::board::is_ring(t),
            "agent" => rules_kani::board::is_agent(t),
            "property" => rules_kani::board::is_property(t),
            _ => false,
        }
    }
    /// ABI v56: class membership. Keeps this model's quirks (is_buyable =
    /// unowned property; livehouse = the two hard-coded tiles).
    fn tile_in(&self, player_id: i32, tile: i32, class: &str) -> bool {
        match class {
            "buyable" => {
                let s = rules_kani::board::tile_slot(tile);
                s >= 0
                    && rules_kani::board::is_property(&rules_kani::board::TILES[s as usize])
                    && self.tile_owner(tile) < 0
            }
            "shop" => {
                let s = rules_kani::board::tile_slot(tile);
                s >= 0 && rules_kani::board::is_shop(&rules_kani::board::TILES[s as usize])
            }
            "livehouse" => {
                let _ = player_id;
                tile == 32 || tile == 51
            }
            _ => false,
        }
    }
    fn tile_group(&self, tile: i32) -> i32 {
        board_group(tile)
    }
    fn tile_price(&self, tile: i32) -> i32 {
        board_price(tile)
    }
    fn houses_of(&self, tile: i32) -> i32 {
        let t = tile.max(0) as usize;
        if t < 60 {
            self.tiles_houses[t]
        } else {
            0
        }
    }
    fn rent_houses_of(&self, tile: i32) -> i32 {
        self.houses_of(tile)
    }
    fn set_houses(&mut self, tile: i32, n: i32) {
        let t = tile.max(0) as usize;
        if t < 60 {
            self.tiles_houses[t] = n;
        }
        self.events.push(format!("set_houses {tile} {n}"));
    }
    fn add_house(&mut self, tile: i32, n: i32) -> i32 {
        let t = tile.max(0) as usize;
        if t < 60 {
            self.tiles_houses[t] = self.tiles_houses[t].saturating_add(n);
            self.events.push(format!("add_house {tile} {n}"));
            return self.tiles_houses[t];
        }
        0
    }
    fn mortgaged_of(&self, tile: i32) -> i32 {
        let t = tile.max(0) as usize;
        if t < 60 && self.tiles_mortgaged[t] {
            1
        } else {
            0
        }
    }
    fn set_mortgaged(&mut self, tile: i32, v: i32) {
        let t = tile.max(0) as usize;
        if t < 60 {
            self.tiles_mortgaged[t] = v != 0;
        }
    }
    fn set_owner(&mut self, tile: i32, player_id: i32) {
        let t = tile.max(0) as usize;
        if t < 60 {
            self.tiles_owner[t] = player_id;
        }
        self.events.push(format!("set_owner {tile} {player_id}"));
    }
    fn dist(&self, a: i32, b: i32) -> i32 {
        (b - a).rem_euclid(60)
    }
    fn tile_forward(&self, a: i32, b: i32) -> i32 {
        self.dist(a, b)
    }
    fn neighbor(&self, player_id: i32, dir: i32) -> i32 {
        let d = if dir == 0 { 1 } else { -1 };
        (self.player_pos(player_id) + d).rem_euclid(60)
    }
    fn players_on_count(&self, tile: i32, except: i32) -> i32 {
        (0..4)
            .filter(|&p| p as i32 != except && !self.out[p] && self.pos[p] == tile)
            .count() as i32
    }
    fn players_on_at(&self, tile: i32, except: i32, index: i32) -> i32 {
        (0..4)
            .filter(|&p| p as i32 != except && !self.out[p] && self.pos[p] == tile)
            .nth(index.max(0) as usize)
            .map(|p| p as i32)
            .unwrap_or(-1)
    }
    fn player_count(&self) -> i32 {
        self.n_players
    }
    fn player_out(&self, player_id: i32) -> i32 {
        self.out[player_id.max(0) as usize % 4] as i32
    }
    fn others_count(&self, player_id: i32) -> i32 {
        (0..self.n_players)
            .filter(|&i| i != player_id && !self.out[i as usize])
            .count() as i32
    }
    fn others_at(&self, player_id: i32, index: i32) -> i32 {
        (0..self.n_players)
            .filter(|&i| i != player_id && !self.out[i as usize])
            .nth(index.max(0) as usize)
            .unwrap_or(-1)
    }
    fn money(&self, player_id: i32) -> i32 {
        self.money[player_id.max(0) as usize % 4]
    }
    fn gain(&mut self, player_id: i32, amount: i32, _src: EngineMsg) -> i32 {
        let p = player_id.max(0) as usize % 4;
        self.money[p] = self.money[p].saturating_add(amount);
        self.events.push(format!("gain {player_id} {amount}"));
        amount
    }
    fn pay(&mut self, player_id: i32, amount: i32, _src: EngineMsg) -> i32 {
        let p = player_id.max(0) as usize % 4;
        self.money[p] = (self.money[p] - amount).max(0);
        self.events.push(format!("pay {player_id} {amount}"));
        amount
    }
    fn draw(&mut self, player_id: i32, n: i32) -> i32 {
        let p = player_id.max(0) as usize % 4;
        let n = n.max(0);
        for _ in 0..n {
            self.hands[p].push("<draw>".into());
        }
        self.events.push(format!("draw {player_id} {n}"));
        n
    }
    fn add_to_hand(&mut self, player_id: i32, card: &str) {
        let p = player_id.max(0) as usize % 4;
        self.hands[p].push(card.to_string());
        self.events.push(format!("add_to_hand {player_id} {card}"));
    }
    fn add_to_deck(&mut self, player_id: i32, card: &str, _shuffle: bool) {
        let p = player_id.max(0) as usize % 4;
        self.decks[p].push(card.to_string());
    }
    fn add_to_deck_at(&mut self, player_id: i32, card: &str, _pos: i32) {
        let p = player_id.max(0) as usize % 4;
        self.decks[p].push(card.to_string());
    }
    fn take_card(&mut self, player_id: i32, pile: CardPile, card: &str) -> bool {
        let p = player_id.max(0) as usize % 4;
        let v = self.pile_of(pile)[p].clone();
        if let Some(i) = v.iter().position(|c| c == card) {
            self.pile_of(pile)[p].remove(i);
            self.events.push(format!("take_card {player_id} {card}"));
            true
        } else {
            false
        }
    }
    fn cards_in(&self, player_id: i32, pile: CardPile) -> Vec<String> {
        let p = player_id.max(0) as usize % 4;
        self.pile_ref(pile)[p].clone()
    }
    fn to_discard(&mut self, player_id: i32, card: &str) {
        let p = player_id.max(0) as usize % 4;
        self.discards[p].push(card.to_string());
        self.events.push(format!("to_discard {player_id} {card}"));
    }
    fn hand_count(&self, player_id: i32, card: &str) -> i32 {
        let p = player_id.max(0) as usize % 4;
        self.hands[p].iter().filter(|c| c.as_str() == card).count() as i32
    }
    fn hand_size(&self, player_id: i32) -> i32 {
        let p = player_id.max(0) as usize % 4;
        self.hands[p].len() as i32
    }
    fn discard_count(&self, player_id: i32, card: &str) -> i32 {
        let p = player_id.max(0) as usize % 4;
        self.discards[p].iter().filter(|c| c.as_str() == card).count() as i32
    }
    fn deck_count(&self, player_id: i32) -> i32 {
        let p = player_id.max(0) as usize % 4;
        self.decks[p].len() as i32
    }
    fn discard_size(&self, player_id: i32) -> i32 {
        let p = player_id.max(0) as usize % 4;
        self.discards[p].len() as i32
    }
    fn discard_from_hand(&mut self, player_id: i32, card: &str) -> i32 {
        if self.take_card(player_id, CardPile::Hand, card) {
            self.to_discard(player_id, card);
            1
        } else {
            0
        }
    }
    fn shuffle_into_deck(&mut self, player_id: i32, hand: bool, discard: bool) -> i32 {
        let p = player_id.max(0) as usize % 4;
        let mut n = 0;
        if hand {
            let h = std::mem::take(&mut self.hands[p]);
            n += h.len();
            self.decks[p].extend(h);
        }
        if discard {
            let d = std::mem::take(&mut self.discards[p]);
            n += d.len();
            self.decks[p].extend(d);
        }
        n as i32
    }
    fn place_card(&mut self, player_id: i32, card: &str, _note: EngineMsg) -> i32 {
        self.events.push(format!("place_card {player_id} {card}"));
        let uid = self.next_uid;
        self.next_uid += 1;
        uid
    }
    fn place_card_on(&mut self, player_id: i32, tile: i32, card: &str, _note: EngineMsg) -> i32 {
        self.events.push(format!("place_card_on {player_id} {tile} {card}"));
        let uid = self.next_uid;
        self.next_uid += 1;
        uid
    }
    fn set_dest(&mut self, dest: i32) {
        self.dest = dest;
    }
    fn set_transfer_to_dest(&mut self, _to: i32, dest: i32) {
        self.dest = dest;
    }
    fn send_to_dest(&mut self, dest: i32) -> Option<i32> {
        let p = self.turn_player.max(0) as usize % 4;
        self.pos[p] = dest;
        self.events.push(format!("send_to_dest {dest}"));
        Some(1)
    }
    fn transfer_to_dest(&mut self, _to: i32, dest: i32) -> Option<i32> {
        self.send_to_dest(dest)
    }
    fn ring_multiplier(&self) -> i32 {
        10
    }
    fn add_ring_bonus(&mut self, _n: i32) -> i32 {
        0
    }
    fn teleport_to(&mut self, player_id: i32, tile: i32) {
        let p = player_id.max(0) as usize % 4;
        self.pos[p] = tile;
        self.events.push(format!("teleport_to {player_id} {tile}"));
    }
    fn unplace_card(&mut self) -> i32 {
        0
    }
    fn is_placed(&self) -> i32 {
        0
    }
    // keyed state (slots)
    fn state_var(&self, _player_id: i32, _key: &str) -> game_core::state::StateVar {
        game_core::state::StateVar::default()
    }
    fn state_get(&self, player_id: i32, key: &str) -> i32 {
        *self.state.get(&(player_id, key.to_string())).unwrap_or(&0)
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
    fn state_set(&mut self, player_id: i32, key: &str, value: i32) -> i32 {
        self.state.insert((player_id, key.to_string()), value);
        self.events.push(format!("set_slot {player_id} {key} {value}"));
        value
    }
    fn state_add(&mut self, player_id: i32, key: &str, delta: i32) -> i32 {
        let e = self.state.entry((player_id, key.to_string())).or_insert(0);
        *e = e.saturating_add(delta);
        *e
    }
    fn state_set_bounds(&mut self, _: i32, _: &str, _: i32, _: i32) {}
    fn state_set_expires(&mut self, _: i32, _: &str, _: Option<game_core::state::Tick>) {}
    fn tick_state(&mut self, _: i32, _: game_core::state::Tick) -> Vec<(String, i32)> {
        Vec::new()
    }
    // held / tokens
    fn count_held(&self, name: &str, player_id: i32) -> i32 {
        *self.held.get(&(player_id, name.to_string())).unwrap_or(&0)
    }
    fn add_held(&mut self, name: &str, player_id: i32, n: i32, max: i32) -> i32 {
        let e = self.held.entry((player_id, name.to_string())).or_insert(0);
        *e = (*e + n).clamp(0, max);
        self.events.push(format!("add_held {player_id} {name} {n}"));
        *e
    }
    fn set_held_name(&mut self, name: &str, player_id: i32, v: i32) {
        self.held.insert((player_id, name.to_string()), v);
        self.events.push(format!("set_held {player_id} {name} {v}"));
    }
    fn move_units(
        &mut self,
        _name: &str,
        _from_tile: i32,
        _from_player: i32,
        _to_tile: i32,
        _to_player: i32,
        _n: i32,
    ) -> i32 {
        0
    }
    // fire / stay
    fn fire(&self, player_id: i32) -> i32 {
        self.fire[player_id.max(0) as usize % 4]
    }
    fn fire_max(&self, player_id: i32) -> i32 {
        self.fire_max[player_id.max(0) as usize % 4]
    }
    fn gain_fire(&mut self, player_id: i32, n: i32, _why: EngineMsg) -> i32 {
        // Matches `game_core::engine::ops::gain_fire`: return the amount
        // actually gained (clamped under cap), not the new total.
        let p = player_id.max(0) as usize % 4;
        if n <= 0 {
            return 0;
        }
        let got = n.clamp(0, (self.fire_max[p] - self.fire[p]).max(0));
        if got <= 0 {
            return 0;
        }
        self.fire[p] += got;
        self.events.push(format!("gain_fire {player_id} {got}"));
        got
    }
    fn spend_fire(&mut self, player_id: i32, n: i32, _why: EngineMsg) -> i32 {
        let p = player_id.max(0) as usize % 4;
        if n <= 0 {
            return 1;
        }
        if self.fire[p] >= n {
            self.fire[p] -= n;
            1
        } else {
            0
        }
    }
    fn give_stay(&mut self, player_id: i32, n: i32) {
        let p = player_id.max(0) as usize % 4;
        self.stay[p] = self.stay[p].saturating_add(n);
        self.events.push(format!("give_stay {player_id} {n}"));
    }
    fn give_stun(&mut self, player_id: i32, n: i32) {
        let p = player_id.max(0) as usize % 4;
        self.stay[p] = self.stay[p].saturating_add(n);
        self.events.push(format!("give_stun {player_id} {n}"));
    }
    fn give_exile(&mut self, player_id: i32, n: i32, _to: i32) {
        self.events.push(format!("give_exile {player_id} {n}"));
    }
    fn give_extra_turn(&mut self, _player_id: i32) {}
    fn can_pay(&self, player_id: i32) -> i32 {
        (self.money(player_id) > 0) as i32
    }
    fn cant_move(&self, player_id: i32) -> i32 {
        (self.stay[player_id.max(0) as usize % 4] > 0) as i32
    }
    fn stay_of(&self, player_id: i32) -> i32 {
        self.stay[player_id.max(0) as usize % 4]
    }
    fn stun_of(&self, player_id: i32) -> i32 {
        self.stay[player_id.max(0) as usize % 4]
    }
    fn turn_player(&self) -> i32 {
        self.turn_player
    }
    fn round_no(&self) -> i32 {
        1
    }
    fn turn_key(&self) -> i32 {
        1
    }
    fn character_is(&self, _player_id: i32, _name: &str) -> i32 {
        0
    }
    fn in_band(&self, _player_id: i32, _name: &str) -> i32 {
        0
    }
    fn trigger(&self) -> game_rules::Trigger {
        use card_sdk::abi::BuyKind;
        let mut t = game_rules::Trigger::default();
        t.value = self.trig_value;
        t.target = self.trig_target;
        t.main = self.trig_main;
        t.roll_source = game_core::engine::rules::RollSource::from_i32(self.trig_roll_source);
        if self.move_present {
            t.mv = Some(game_rules::TriggerMove {
                kind: game_rules::MoveKind::from_i32(self.move_kind)
                    .unwrap_or(game_rules::MoveKind::Walk),
                resolve: self.move_resolve,
                tags: self
                    .move_tags
                    .iter()
                    .map(|(k, v)| (k.clone(), *v))
                    .collect(),
                main: self.trig_main,
                dir: game_core::engine::rules::Dir::from_i32(self.move_dir),
                from: self.move_from,
                remaining: self.move_remaining,
                total: self.move_total,
                roll: if self.move_roll >= 0 {
                    Some(self.move_roll)
                } else {
                    None
                },
            });
        }
        if self.trig_pay_is_rent {
            t.pay = Some(game_rules::TriggerPay { is_rent: true });
        }
        if self.buy_present {
            t.buy = Some(game_rules::TriggerBuy {
                kind: BuyKind::from_i32(self.buy_kind).unwrap_or(BuyKind::Land),
                seller: game_core::engine::rules::Payee::from_i32(self.buy_seller),
                price: self.buy_price,
                deal_owner: if self.buy_deal_owner >= 0 {
                    Some(self.buy_deal_owner)
                } else {
                    None
                },
                deal_houses: self.buy_deal_houses,
                deal_mortgaged: self.buy_deal_mortgaged,
            });
        }
        if self.trig_cancelled {
            t.negate_activation();
        }
        t
    }
    fn set_trigger_value(&mut self, value: i32) {
        self.trig_value = value;
    }
    fn set_trigger_target(&mut self, to: i32) {
        self.trig_target = to;
    }
    fn set_trigger_cancelled(&mut self) {
        self.trig_cancelled = true;
    }
    fn set_trigger_negate_effect(&mut self) {}
    fn set_trigger_spare(&mut self, _seat: i32) {}
    fn declare_trigger_effect(
        &mut self,
        _kind: i32,
        _target: i32,
        _from: i32,
        _tile: i32,
        _value: i32,
    ) {
    }
    fn trig_card_is(&self, _id: &str) -> i32 {
        0
    }
    fn set_trigger_price(&mut self, v: i32) {
        self.buy_price = v;
        self.buy_present = true;
    }
    fn set_trigger_deal_owner(&mut self, v: i32) {
        self.buy_deal_owner = v;
        self.buy_present = true;
    }
    fn set_trigger_deal_houses(&mut self, v: i32) {
        self.buy_deal_houses = v;
        self.buy_present = true;
    }
    fn set_trigger_deal_mortgaged(&mut self, v: i32) {
        self.buy_deal_mortgaged = v != 0;
        self.buy_present = true;
    }
    fn set_trigger_reason(&mut self, _reason: &str) {}
    fn set_trigger_move_roll(&mut self, roll: i32) {
        self.move_roll = roll;
        self.move_present = true;
    }
    fn paid_in_settle(&self) -> i32 {
        0
    }
    fn gains_this_turn(&self, _player: i32) -> i32 {
        0
    }
    fn targeted_count(&self, _player_id: i32) -> i32 {
        0
    }
    fn field_instances(&self, _player_id: i32) -> Vec<(i32, String)> {
        Vec::new()
    }
    fn tile_at(&self, uid: i32) -> i32 {
        self.field_uid
            .iter()
            .position(|&u| u == uid)
            .map(|i| self.field_tile[i])
            .unwrap_or(-1)
    }
    fn set_tile_at(&mut self, uid: i32, tile: i32) -> bool {
        match self.field_uid.iter().position(|&u| u == uid) {
            Some(i) => {
                self.field_tile[i] = tile;
                self.events.push(format!("set_tile_at {uid} {tile}"));
                true
            }
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Real-side HostCtx
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ConfRules;

impl RulesHandle for ConfRules {
    fn by_id(&self, _id: &str) -> Option<i32> {
        None
    }
    fn card(&self, _i: i32) -> Option<&CardInfo> {
        None
    }
}

pub struct ConfHost {
    pub state: HostState<ConfWorld, ConfRules>,
    pub fuel: u64,
}

impl ConfHost {
    pub fn new(world: ConfWorld, answers: Vec<i32>) -> Self {
        Self {
            state: HostState::new(ConfRules, world, answers, 0),
            fuel: 1_000_000,
        }
    }
}

impl HostCtx for ConfHost {
    type World = ConfWorld;
    type Rules = ConfRules;

    fn st(&self) -> &HostState<ConfWorld, ConfRules> {
        &self.state
    }
    fn st_mut(&mut self) -> &mut HostState<ConfWorld, ConfRules> {
        &mut self.state
    }
    fn read_guest(&mut self, ptr: i32, len: i32) -> Result<Vec<u8>, HostErr> {
        Ok(card_sdk::native::read(ptr, len.max(0) as usize))
    }
    fn write_guest(&mut self, ptr: i32, bytes: &[u8]) -> Result<(), HostErr> {
        card_sdk::native::write(ptr, bytes);
        Ok(())
    }
    fn fuel(&mut self) -> Result<u64, HostErr> {
        Ok(self.fuel)
    }
    fn set_fuel(&mut self, v: u64) -> Result<(), HostErr> {
        self.fuel = v;
        Ok(())
    }
    fn call_entry(
        &mut self,
        _nested: HostState<ConfWorld, ConfRules>,
        _card: i32,
        _entry: i32,
        _op: i32,
        _player_id: i32,
        _want_msg: bool,
    ) -> (
        Result<CallOut, HostErr>,
        HostState<ConfWorld, ConfRules>,
        u64,
    ) {
        // Nested `play_card` is out of scope for the thin adapter.
        (
            Err(HostErr::trap("conf_model: nested call_entry not supported")),
            HostState::new(ConfRules, ConfWorld::new(), vec![], 0),
            self.fuel,
        )
    }
}

// ---------------------------------------------------------------------------
// InlineHost: answer prompts / apply host requests immediately (no chain
// window) -- matches the model's "immediate concrete effects" stance.
// ---------------------------------------------------------------------------

pub struct ConfInline;

impl InlineHost for ConfInline {
    fn answer(&mut self, _run: &mut dyn std::any::Any, _p: EnginePrompt) -> Option<i32> {
        // First option / "no" / index 0. Cards under test are selected so a
        // fixed 0 is the interesting branch, or the branch is side-effect-free.
        Some(0)
    }
    fn apply(&mut self, run: &mut dyn std::any::Any, req: HostRequest) -> Option<i32> {
        let w = run.downcast_mut::<ConfWorld>()?;
        match req {
            HostRequest::Pay {
                from,
                to,
                amount,
                total_stage: _,
                ..
            } => {
                if amount <= 0 {
                    return Some(0);
                }
                // Base Pay semantics with no hooks: print / delete / pay-player.
                if from >= 0 {
                    let f = from as usize % 4;
                    w.money[f] = (w.money[f] - amount).max(0);
                }
                if to >= 0 {
                    let t = to as usize % 4;
                    w.money[t] = w.money[t].saturating_add(amount);
                }
                w.events.push(format!("pay_pipeline {from}->{to} {amount}"));
                Some(amount)
            }
            HostRequest::PayTotal { amount, .. } => Some(amount),
            HostRequest::Teleport { player_id, tile } => {
                if player_id >= 0 {
                    w.pos[player_id as usize % 4] = tile;
                    w.events.push(format!("teleport_to {player_id} {tile}"));
                }
                Some(1)
            }
            HostRequest::Gate { .. } => Some(1),
            HostRequest::Marker { .. } => Some(1),
            HostRequest::Draw { player_id: _, n } => Some(n),
            HostRequest::Target { player_id, tile, .. } => {
                if tile >= 0 {
                    Some(tile)
                } else {
                    Some(player_id)
                }
            }
            // Other engine-protocol requests: allow / no-op.
            _ => Some(1),
        }
    }
    fn take_halt(&mut self) -> Option<game_core::engine::Halt> {
        None
    }
}

// ---------------------------------------------------------------------------
// Comparable snapshot
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snap {
    pub n_players: i32,
    pub money: [i32; 4],
    pub pos: [i32; 4],
    pub out: [bool; 4],
    pub fire: [i32; 4],
    pub stay: [i32; 4],
    pub tiles_owner: Vec<i32>,
    pub tiles_houses: Vec<i32>,
    pub tiles_mortgaged: Vec<bool>,
    pub dest: i32,
    pub trig_value: i32,
    pub trig_target: i32,
    /// Normalized effect lines: `gain p n`, `pay p n`, `teleport p t`, ...
    pub effects: Vec<String>,
}

fn snap_real(w: &ConfWorld) -> Snap {
    Snap {
        n_players: w.n_players,
        money: w.money,
        pos: w.pos,
        out: w.out,
        fire: w.fire,
        stay: w.stay,
        tiles_owner: w.tiles_owner.to_vec(),
        tiles_houses: w.tiles_houses.to_vec(),
        tiles_mortgaged: w.tiles_mortgaged.to_vec(),
        dest: w.dest,
        trig_value: w.trig_value,
        trig_target: w.trig_target,
        effects: w.events.clone(),
    }
}

fn snap_model() -> Snap {
    kworld::with_world(|w| {
        let mut effects = Vec::new();
        for e in w.effect_slice() {
            effects.push(format!("{e:?}"));
        }
        Snap {
            n_players: w.n_players,
            money: w.money,
            pos: w.pos,
            out: w.out,
            fire: w.fire,
            stay: w.stay,
            tiles_owner: w.tiles.iter().map(|t| t.owner).collect(),
            tiles_houses: w.tiles.iter().map(|t| t.houses).collect(),
            tiles_mortgaged: w.tiles.iter().map(|t| t.mortgaged).collect(),
            dest: w.dest,
            trig_value: w.pay_amount,
            trig_target: w.trig_target,
            effects,
        }
    })
}

fn assert_state_eq(label: &str, real: &Snap, model: &Snap) {
    let mut diffs = Vec::new();
    if real.n_players != model.n_players {
        diffs.push(format!("n_players real={} model={}", real.n_players, model.n_players));
    }
    if real.money != model.money {
        diffs.push(format!("money real={:?} model={:?}", real.money, model.money));
    }
    if real.pos != model.pos {
        diffs.push(format!("pos real={:?} model={:?}", real.pos, model.pos));
    }
    if real.out != model.out {
        diffs.push(format!("out real={:?} model={:?}", real.out, model.out));
    }
    if real.fire != model.fire {
        diffs.push(format!("fire real={:?} model={:?}", real.fire, model.fire));
    }
    if real.stay != model.stay {
        diffs.push(format!("stay real={:?} model={:?}", real.stay, model.stay));
    }
    if real.tiles_owner != model.tiles_owner {
        diffs.push("tiles_owner diverged".into());
    }
    if real.tiles_houses != model.tiles_houses {
        diffs.push(format!(
            "tiles_houses diverged (real sum={}, model sum={})",
            real.tiles_houses.iter().sum::<i32>(),
            model.tiles_houses.iter().sum::<i32>()
        ));
    }
    if real.tiles_mortgaged != model.tiles_mortgaged {
        diffs.push("tiles_mortgaged diverged".into());
    }
    if real.dest != model.dest {
        diffs.push(format!("dest real={} model={}", real.dest, model.dest));
    }
    if real.trig_value != model.trig_value {
        diffs.push(format!(
            "trig_value real={} model={}",
            real.trig_value, model.trig_value
        ));
    }
    if real.trig_target != model.trig_target {
        diffs.push(format!(
            "trig_target real={} model={}",
            real.trig_target, model.trig_target
        ));
    }
    // Effect-log comparison is structural on the model's typed Effect vs the
    // real side's string events. We compare the *counts of money-moving
    // effects* so a missing gain/pay/teleport fails closed.
    let count = |s: &[String], pats: &[&str]| {
        s.iter()
            .filter(|l| pats.iter().any(|p| l.contains(p)))
            .count()
    };
    let money_pats = ["gain ", "pay ", "pay_pipeline", "GainMoney", "PayMoney", "PayTo"];
    let tele_pats = ["teleport", "Teleport"];
    let stay_pats = ["give_stay", "GiveStay", "give_stun", "GiveStun"];
    if count(&real.effects, &money_pats) != count(&model.effects, &money_pats) {
        diffs.push(format!(
            "money-effect count real={} model={}\n  real={:?}\n  model={:?}",
            count(&real.effects, &money_pats),
            count(&model.effects, &money_pats),
            real.effects,
            model.effects
        ));
    }
    if count(&real.effects, &tele_pats) != count(&model.effects, &tele_pats) {
        diffs.push(format!(
            "teleport-effect count real={} model={}",
            count(&real.effects, &tele_pats),
            count(&model.effects, &tele_pats)
        ));
    }
    if count(&real.effects, &stay_pats) != count(&model.effects, &stay_pats) {
        diffs.push(format!(
            "stay-effect count real={} model={}",
            count(&real.effects, &stay_pats),
            count(&model.effects, &stay_pats)
        ));
    }
    if !diffs.is_empty() {
        panic!(
            "MODEL-vs-ENGINE DIVERGENCE [{label}]:\n  {}\n  (a divergence is a BUG IN THE MODEL unless the message says engine bug)",
            diffs.join("\n  ")
        );
    }
}

// ---------------------------------------------------------------------------
// Shared pre-state
// ---------------------------------------------------------------------------

fn base_conf() -> ConfWorld {
    let mut w = ConfWorld::new();
    w.n_players = 3;
    w.money = [1500, 2000, 3000, 0];
    w.pos = [2, 10, 25, 0];
    w.out = [false, false, false, true];
    w.fire = [1, 0, 2, 0];
    w.fire_max = [3, 3, 3, 3];
    w.stay = [0, 1, 0, 0];
    w.tiles_owner[1] = 0; // board index 2
    w.tiles_houses[1] = 2;
    w.tiles_owner[5] = 1; // board index 6
    w.tiles_mortgaged[5] = true;
    w.trig_value = 7;
    w.trig_target = 1;
    w.turn_player = 0;
    w
}

fn base_model() -> KWorld {
    let mut w = KWorld::new();
    w.n_players = 3;
    w.money = [1500, 2000, 3000, 0];
    w.pos = [2, 10, 25, 0];
    w.out = [false, false, false, true];
    w.fire = [1, 0, 2, 0];
    w.fire_max = [3, 3, 3, 3];
    w.stay = [0, 1, 0, 0];
    w.tiles[1].owner = 0;
    w.tiles[1].houses = 2;
    w.tiles[5].owner = 1;
    w.tiles[5].mortgaged = true;
    w.pay_amount = 7;
    w.trig_target = 1;
    w.turn_player = 0;
    w
}

fn install_model_world(answers: &[i32]) {
    install_model_world_with(base_model(), answers);
}

fn install_model_world_with(mut w: KWorld, answers: &[i32]) {
    for (i, &a) in answers.iter().enumerate() {
        if i < kworld::MAX_ANSWERS {
            w.answers[i] = a;
        }
    }
    w.next_answer = 0;
    kworld::reset_world(w);
}

// ---------------------------------------------------------------------------
// Op-level differential: hostfns (real) vs dispatch (model)
// ---------------------------------------------------------------------------

/// Run one op on both sides from the shared pre-state and compare.
/// Callers intern handles *before* this; the arena is not cleared here.
fn diff_op(label: &str, answers: &[i32], real: impl FnOnce(&mut ConfHost) -> i64, model: Args, op: Op) {
    let _g = model_lock();
    install_model_world(answers);
    let model_ret = rules_kani::host::dispatch(op, model);
    let model_snap = snap_model();

    let mut conf = ConfHost::new(base_conf(), answers.to_vec());
    let mut inline = ConfInline;
    let real_ret = with_inline_host(&mut inline, || real(&mut conf));
    let real_snap = snap_real(&conf.st().wr());
    if real_ret != model_ret {
        panic!(
            "MODEL-vs-ENGINE DIVERGENCE [{label}]: return real={real_ret} model={model_ret} (BUG IN THE MODEL unless engine)"
        );
    }
    assert_state_eq(label, &real_snap, &model_snap);
}

/// Run a *sequence* of ops on both sides (model: dispatch each; real: the
/// matching `hostfns` calls). `real` returns the last hostfns value; `model`
/// returns the last dispatch value.
fn diff_op_seq(
    label: &str,
    answers: &[i32],
    real: impl FnOnce(&mut ConfHost) -> i64,
    model: impl FnOnce() -> i64,
) {
    let _g = model_lock();
    install_model_world(answers);
    let model_ret = model();
    let model_snap = snap_model();

    let mut conf = ConfHost::new(base_conf(), answers.to_vec());
    let mut inline = ConfInline;
    let real_ret = with_inline_host(&mut inline, || real(&mut conf));
    let real_snap = snap_real(&conf.st().wr());
    if real_ret != model_ret {
        panic!(
            "MODEL-vs-ENGINE DIVERGENCE [{label}]: return real={real_ret} model={model_ret} (BUG IN THE MODEL unless engine)"
        );
    }
    assert_state_eq(label, &real_snap, &model_snap);
}

#[test]
fn op_gain_matches_engine() {
    let _g = model_lock();
    // gain(player, amount, msg) -- immediate credit on the model; Pay pipeline
    // (no hooks) on the real side via ConfInline.
    let (mp, ml) = intern_msg("conf.gain");
    diff_op(
        "gain",
        &[],
        |c| game_rules::hostfns::gain(c, 0, 500, mp, ml).unwrap() as i64,
        [0, 500, mp, ml, 0, 0],
        Op::gain,
    );
}

#[test]
fn op_pay_matches_engine() {
    let _g = model_lock();
    let (mp, ml) = intern_msg("conf.pay");
    diff_op(
        "pay",
        &[],
        |c| game_rules::hostfns::pay(c, 1, 400, mp, ml).unwrap() as i64,
        [1, 400, mp, ml, 0, 0],
        Op::pay,
    );
}

#[test]
fn op_teleport_to_matches_engine() {
    let _g = model_lock();
    diff_op(
        "teleport_to",
        &[],
        |c| {
            game_rules::hostfns::teleport_to(c, 2, 40).unwrap();
            0
        },
        [2, 40, 0, 0, 0, 0],
        Op::teleport_to,
    );
}

#[test]
fn op_gain_fire_matches_engine() {
    let _g = model_lock();
    let (mp, ml) = intern_msg("conf.fire");
    diff_op(
        "gain_fire",
        &[],
        |c| game_rules::hostfns::gain_fire(c, 0, 2, mp, ml).unwrap() as i64,
        [0, 2, mp, ml, 0, 0],
        Op::gain_fire,
    );
}

#[test]
fn op_give_stay_matches_engine() {
    let _g = model_lock();
    diff_op(
        "give_stay",
        &[],
        |c| {
            game_rules::hostfns::give_stay(c, 1, 2).unwrap();
            0
        },
        [1, 2, 0, 0, 0, 0],
        Op::give_stay,
    );
}

#[test]
fn op_player_pos_money_player_count_player_out_tile_count_match() {
    let _g = model_lock();
    for (label, op, args, real) in [
        (
            "player_pos",
            Op::player_pos,
            [1, 0, 0, 0, 0, 0],
            (|c: &mut ConfHost| game_rules::hostfns::player_pos(c, 1).unwrap() as i64)
                as fn(&mut ConfHost) -> i64,
        ),
        (
            "money",
            Op::money,
            [2, 0, 0, 0, 0, 0],
            (|c: &mut ConfHost| game_rules::hostfns::money(c, 2).unwrap() as i64)
                as fn(&mut ConfHost) -> i64,
        ),
        (
            "player_count",
            Op::player_count,
            [0, 0, 0, 0, 0, 0],
            (|c: &mut ConfHost| game_rules::hostfns::player_count(c).unwrap() as i64)
                as fn(&mut ConfHost) -> i64,
        ),
        (
            "player_out",
            Op::player_out,
            [3, 0, 0, 0, 0, 0],
            (|c: &mut ConfHost| game_rules::hostfns::player_out(c, 3).unwrap() as i64)
                as fn(&mut ConfHost) -> i64,
        ),
        (
            "tile_count",
            Op::tile_count,
            [0, 0, 0, 0, 0, 0],
            (|c: &mut ConfHost| game_rules::hostfns::tile_count(c).unwrap() as i64)
                as fn(&mut ConfHost) -> i64,
        ),
    ] {
        diff_op(label, &[], real, args, op);
    }
}

#[test]
fn op_tile_named_matches_engine() {
    let _g = model_lock();
    // Only 「江户川乐器店」 is modeled; both sides return 31.
    let (np, nl) = intern_str(TILE_SHOP_NAME);
    diff_op(
        "tile_named",
        &[],
        |c| game_rules::hostfns::tile_named(c, np, nl).unwrap() as i64,
        [np, nl, 0, 0, 0, 0],
        Op::tile_named,
    );
}

#[test]
fn op_set_houses_houses_of_match() {
    let _g = model_lock();
    diff_op_seq(
        "set_houses+houses_of",
        &[],
        |c| {
            game_rules::hostfns::set_houses(c, 3, 4).unwrap();
            game_rules::hostfns::houses_of(c, 3).unwrap() as i64
        },
        || {
            rules_kani::host::dispatch(Op::set_houses, [3, 4, 0, 0, 0, 0]);
            rules_kani::host::dispatch(Op::houses_of, [3, 0, 0, 0, 0, 0])
        },
    );
    diff_op(
        "houses_of",
        &[],
        |c| game_rules::hostfns::houses_of(c, 2).unwrap() as i64,
        [2, 0, 0, 0, 0, 0],
        Op::houses_of,
    );
}

#[test]
fn op_set_owner_tile_owner_match() {
    let _g = model_lock();
    diff_op_seq(
        "set_owner+tile_owner",
        &[],
        |c| {
            game_rules::hostfns::set_owner(c, 7, 2).unwrap();
            game_rules::hostfns::tile_owner(c, 7).unwrap() as i64
        },
        || {
            rules_kani::host::dispatch(Op::set_owner, [7, 2, 0, 0, 0, 0]);
            rules_kani::host::dispatch(Op::tile_owner, [7, 0, 0, 0, 0, 0])
        },
    );
}

#[test]
fn op_add_to_hand_take_card_cards_in_hand_size_match() {
    let _g = model_lock();
    // Use a non-resonance card name so the model's pilot-compat `hand_size`
    // (bitset + pile hybrid) is exercised on an empty-start hand.
    let (cp, cl) = intern_str("R:[衍生] 压");
    diff_op_seq(
        "add_to_hand+take_card+cards_in",
        &[],
        |c| {
            game_rules::hostfns::add_to_hand(c, 0, cp, cl).unwrap();
            let ok = game_rules::hostfns::take_card(c, 0, 0, cp, cl).unwrap();
            let (buf, cap) = card_sdk::native::reserve(64);
            let _need = game_rules::hostfns::cards_in(c, 0, 0, buf, cap).unwrap();
            ok as i64
        },
        || {
            rules_kani::host::dispatch(Op::add_to_hand, [0, cp, cl, 0, 0, 0]);
            let ok = rules_kani::host::dispatch(Op::take_card, [0, 0, cp, cl, 0, 0]);
            let (buf, cap) = card_sdk::native::reserve(64);
            rules_kani::host::dispatch(Op::cards_in, [0, 0, buf, cap, 0, 0]);
            ok
        },
    );
}

#[test]
fn op_to_discard_and_hand_size() {
    let _g = model_lock();
    let (cp, cl) = intern_str("R:[衍生] 压");
    diff_op(
        "to_discard",
        &[],
        |c| {
            game_rules::hostfns::to_discard(c, 1, cp, cl).unwrap();
            0
        },
        [1, cp, cl, 0, 0, 0],
        Op::to_discard,
    );
    diff_op(
        "hand_size",
        &[],
        |c| game_rules::hostfns::hand_size(c, 0).unwrap() as i64,
        [0, 0, 0, 0, 0, 0],
        Op::hand_size,
    );
}

#[test]
fn op_set_slot_slot_match() {
    let _g = model_lock();
    let (kp, kl) = intern_str("conf.key");
    diff_op_seq(
        "set_slot+slot",
        &[],
        |c| {
            game_rules::hostfns::set_slot(c, 1, kp, kl, 9).unwrap();
            game_rules::hostfns::slot(c, 1, kp, kl).unwrap() as i64
        },
        || {
            rules_kani::host::dispatch(Op::set_slot, [1, kp, kl, 9, 0, 0]);
            rules_kani::host::dispatch(Op::slot, [1, kp, kl, 0, 0, 0])
        },
    );
}

#[test]
fn op_count_held_set_held_name_match() {
    let _g = model_lock();
    let (np, nl) = intern_str("P✽P粉丝(正)");
    diff_op_seq(
        "set_held_name+count_held_name",
        &[],
        |c| {
            game_rules::hostfns::set_held_name(c, np, nl, 2, 5).unwrap();
            game_rules::hostfns::count_held_name(c, np, nl, 2).unwrap() as i64
        },
        || {
            rules_kani::host::dispatch(Op::set_held_name, [np, nl, 2, 5, 0, 0]);
            rules_kani::host::dispatch(Op::count_held_name, [np, nl, 2, 0, 0, 0])
        },
    );
}

#[test]
fn op_set_dest_place_card_at_match() {
    let _g = model_lock();
    let (cp, cl) = intern_str("PP:[衍生]共鸣");
    let (mp, ml) = intern_msg("conf.place");
    diff_op_seq(
        "set_dest+place_card_at",
        &[],
        |c| {
            game_rules::hostfns::set_dest(c, 12).unwrap();
            game_rules::hostfns::place_card_at(c, 0, cp, cl, mp, ml).unwrap() as i64
        },
        || {
            rules_kani::host::dispatch(Op::set_dest, [12, 0, 0, 0, 0, 0]);
            rules_kani::host::dispatch(Op::place_card_at, [0, cp, cl, mp, ml, 0])
        },
    );
}

#[test]
fn op_trig_set_pay_amount_trig_value_match() {
    let _g = model_lock();
    diff_op_seq(
        "trig_set_pay_amount+trig_value",
        &[],
        |c| {
            game_rules::hostfns::trig_set_pay_amount(c, 42).unwrap();
            game_rules::hostfns::trig_value(c).unwrap() as i64
        },
        || {
            rules_kani::host::dispatch(Op::trig_set_pay_amount, [42, 0, 0, 0, 0, 0]);
            rules_kani::host::dispatch(Op::trig_value, [0, 0, 0, 0, 0, 0])
        },
    );
    diff_op(
        "trig_value",
        &[],
        |c| game_rules::hostfns::trig_value(c).unwrap() as i64,
        [0, 0, 0, 0, 0, 0],
        Op::trig_value,
    );
}

#[test]
fn op_ask_yes_fixed_answers() {
    let _g = model_lock();
    // ask(kind=YesNo=1, player, title, text) -- fixed answers [1].
    let (tp, tl) = intern_msg("conf.t");
    let (xp, xl) = intern_msg("conf.x");
    diff_op(
        "ask",
        &[1],
        |c| {
            game_rules::hostfns::opt_int(c, 1).unwrap();
            game_rules::hostfns::ask(c, 1, 0, tp, tl, xp, xl).unwrap() as i64
        },
        [1, 0, tp, tl, xp, xl],
        Op::ask,
    );
}

// ---------------------------------------------------------------------------
// Card-level differential: one representative per band + skill + event
// ---------------------------------------------------------------------------

/// Run `f` on the model side (SymOps installed) and on the real side
/// (ConfHost installed) from the same pre-state and answers.
fn diff_card(label: &str, id: &str, answers: &[i32], run: impl Fn(&str, i32) -> ()) {
    let _g = model_lock();
    card_sdk::native::clear();
    // ---- model side ----
    install_model_world(answers);
    let prev = install_host(Box::new(SymOps));
    let model_res = catch_unwind(AssertUnwindSafe(|| run(id, 0)));
    let _ = take_host();
    if let Some(p) = prev {
        let _ = install_host(p);
    }
    if let Err(e) = model_res {
        let msg = e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "model panicked".into());
        panic!("MODEL FAILED [{label}] ({id}): {msg} -- BUG IN THE MODEL (or unmodeled op)");
    }
    let model_snap = snap_model();

    // ---- real side ----
    // ConfHost: HostCtx, so the HostOps blanket applies. Install it, run the
    // same entry under ConfInline (immediate Pay/Teleport/Gate, matching the
    // model's no-chain-window stance), then recover the world via the same
    // fat-to-thin cast rules-native uses.
    card_sdk::native::clear();
    let mut inline = ConfInline;
    let host_box: Box<ConfHost> = Box::new(ConfHost::new(base_conf(), answers.to_vec()));
    let prev = install_host(host_box);
    let real_res = with_inline_host(&mut inline, || catch_unwind(AssertUnwindSafe(|| run(id, 0))));
    let host_box = take_host().expect("conf host installed");
    if let Some(p) = prev {
        let _ = install_host(p);
    }
    let raw: *mut dyn game_rules::native_shims::HostOps = Box::into_raw(host_box);
    let conf: Box<ConfHost> = unsafe { Box::from_raw(raw as *mut ConfHost) };
    if let Err(e) = real_res {
        let msg = e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "real panicked".into());
        panic!("ENGINE FAILED [{label}] ({id}): {msg}");
    }
    let real_snap = snap_real(conf.st().wr());
    assert_state_eq(&format!("card {label}"), &real_snap, &model_snap);
}

fn run_play(id: &str, player: i32) {
    rules_kani::invoke::unwrap_done(rules_kani::invoke::play(id, player));
}

fn run_hook(id: &str, player: i32) {
    rules_kani::invoke::unwrap_done(rules_kani::invoke::hook_only(id, "", player));
}

#[test]
fn card_band_ag_conforms() {
    let _g = model_lock();
    // `AG:即使夕阳落山` -- play entry; ops modeled (gain/log style).
    // Falls back to whatever invoke finds; fail-closed if unknown.
    diff_card("ag", "AG:即使夕阳落山", &[], run_play);
}

#[test]
fn card_band_crychic_conforms() {
    let _g = model_lock();
    diff_card("crychic", "CRYCHIC:（祥子）带领着大家", &[], run_play);
}

#[test]
fn card_band_general_conforms() {
    let _g = model_lock();
    // `通用:10次招募（1回限定）` -- pay + draw.
    diff_card("general", "通用:10次招募（1回限定）", &[], run_play);
}

#[test]
fn card_band_hhw_conforms() {
    let _g = model_lock();
    diff_card("hhw", "HHW:Happy, Lucky, Smile, Yeah！", &[], run_play);
}

#[test]
fn card_band_morfonica_conforms() {
    let _g = model_lock();
    // `Mor:（Rui）正论恶魔` -- four `transfer` legs. Pre-seeded answers are
    // consumed by `pay` *instead of* moving money (see `request_or_pause`), so
    // leave answers empty and let ConfInline apply every Pay.
    diff_card("morfonica", "Mor:（Rui）正论恶魔", &[], run_play);
}

#[test]
fn card_band_mujica_conforms() {
    let _g = model_lock();
    // `Mujica:心の雨` -- player_pos / others / tile_forward / roll / give_stay /
    // give_stun / gain. Dice use the shared mid-value formula.
    diff_card("mujica", "Mujica:心の雨", &[], run_play);
}

#[test]
fn card_band_mygo_conforms() {
    let _g = model_lock();
    // `MyGO:轮符雨` -- give_stay + before_turn_end. The real engine runs
    // `give_stay` through the abnormal [反击] gate; with empty answers
    // ConfInline answers 1 (allow) so both sides grant the stay. Pre-seeding
    // 0 would block the stay on the real side only (the model has no gate).
    diff_card("mygo", "MyGO:轮符雨", &[], run_play);
}

#[test]
fn card_band_pp_conforms() {
    let _g = model_lock();
    diff_card("pp", "PP:有你与我在这里共度", &[], run_play);
}

#[test]
fn card_band_ppp_conforms() {
    let _g = model_lock();
    // `PPP:[衍生]Pipopa` -- set_dest + gain + place_card.
    diff_card("ppp", "PPP:[衍生]Pipopa", &[], run_play);
}

#[test]
fn card_band_ras_conforms() {
    let _g = model_lock();
    diff_card("ras", "RAS:（MASKING）CRUSH ON THE DRUM!!!", &[], run_play);
}

#[test]
fn card_band_roselia_conforms() {
    let _g = model_lock();
    // `R:[衍生] 压` -- a pure `gain`.
    diff_card("roselia", "R:[衍生] 压", &[], run_play);
}

#[test]
fn card_band_sumimi_conforms() {
    let _g = model_lock();
    // `Sumimi:兼顾偶像与乐队` -- state + money_of + gain.
    diff_card("sumimi", "Sumimi:兼顾偶像与乐队", &[], run_play);
}

#[test]
fn card_skill_conforms() {
    let _g = model_lock();
    // `skill:千早爱音:重新开始` -- first Hook entry (declare_cap / fire pot).
    diff_card("skill", "skill:千早爱音:重新开始", &[], run_hook);
}

#[test]
fn card_event_conforms() {
    let _g = model_lock();
    // `event:很噜的感觉` -- ask_yes + teleport + pay. `ask_yes` is `ask==0`,
    // so answer **1** means "no teleport". With all-no the body is quiet
    // (tile_named("快餐店") is -1 on the model; unused when nobody teleports).
    diff_card("event", "event:很噜的感觉", &[1, 1, 1, 1], run_play);
}

// ---------------------------------------------------------------------------
// Host-ops batch: rent_of / tile_at / set_tile_at / trigger payload
// (modeled 2026-10-11 by HOST-OPS; semantics from `hostfns` + `game-core`
// `WorldOps::rent_of` / `purchase::table_rent` / `TriggerMove` / `TriggerBuy`.)
// ---------------------------------------------------------------------------

/// Pure getter with a custom shared pre-state on both sides.
fn diff_get_with(
    label: &str,
    conf: ConfWorld,
    model: KWorld,
    real: impl FnOnce(&mut ConfHost) -> i64,
    model_fn: impl FnOnce() -> i64,
) {
    let _g = model_lock();
    install_model_world_with(model, &[]);
    let model_ret = model_fn();
    let mut conf_h = ConfHost::new(conf, Vec::new());
    let mut inline = ConfInline;
    let real_ret = with_inline_host(&mut inline, || real(&mut conf_h));
    if real_ret != model_ret {
        panic!(
            "MODEL-vs-ENGINE DIVERGENCE [{label}]: return real={real_ret} model={model_ret} (BUG IN THE MODEL unless engine)"
        );
    }
}

fn rent_conf() -> ConfWorld {
    let mut w = base_conf();
    w.tiles_owner[1] = 0;
    w.tiles_houses[1] = 2;
    w.tiles_owner[5] = 1;
    w.tiles_houses[5] = 0;
    w.tiles_owner[8] = 2; // RiNG 1 (board.json index 9)
    w
}

fn rent_model() -> KWorld {
    let mut w = base_model();
    w.tiles[1].owner = 0;
    w.tiles[1].houses = 2;
    w.tiles[5].owner = 1;
    w.tiles[5].houses = 0;
    w.tiles[8].owner = 2;
    w
}

#[test]
fn op_rent_of_matches_engine() {
    let _g = model_lock();
    // Property at 2 houses -> table[2]; 0 houses -> table[0]; RiNG -> price;
    // empty table -> 0. (tile 30 = 购物中心-like 3600 land, rent[0]=360.)
    for (tile, expect) in [(1, 3220), (5, 160), (8, 1000), (0, 0), (30, 360)] {
        diff_get_with(
            &format!("rent_of[{tile}]={expect}"),
            rent_conf(),
            rent_model(),
            |c| game_rules::hostfns::rent_of(c, tile).unwrap() as i64,
            move || rules_kani::host::dispatch(Op::rent_of, [tile, 0, 0, 0, 0, 0]),
        );
    }
}

#[test]
fn op_buy_price_matches_engine() {
    let _g = model_lock();
    // quote_native = land + houses * house_cost.
    for (tile, expect) in [(1, 7000), (5, 1600), (8, 1000), (0, 0)] {
        diff_get_with(
            &format!("buy_price[{tile}]={expect}"),
            rent_conf(),
            rent_model(),
            |c| game_rules::hostfns::buy_price(c, tile).unwrap() as i64,
            move || rules_kani::host::dispatch(Op::buy_price, [tile, 0, 0, 0, 0, 0]),
        );
    }
}

#[test]
fn op_tile_at_set_tile_at_match_engine() {
    let _g = model_lock();
    let mut conf = base_conf();
    conf.field_uid.push(7);
    conf.field_tile.push(3);
    let mut model = base_model();
    model.field[0] = kworld::FieldCard {
        uid: 7,
        card: 0,
        player: 0,
        tile: 3,
        face_down: false,
        immune: false,
        crystals: 0,
        props: kworld::NameMap::new(),
    };
    model.field_n = 1;

    let mut conf_h = ConfHost::new(conf, Vec::new());
    let mut inline = ConfInline;
    let real = with_inline_host(&mut inline, || {
        let a = game_rules::hostfns::tile_at(&mut conf_h, 7).unwrap() as i64;
        let s = game_rules::hostfns::set_tile_at(&mut conf_h, 7, 20).unwrap() as i64;
        let b = game_rules::hostfns::tile_at(&mut conf_h, 7).unwrap() as i64;
        let missing = game_rules::hostfns::set_tile_at(&mut conf_h, 99, 1).unwrap() as i64;
        (a, s, b, missing)
    });
    install_model_world_with(model, &[]);
    let model = {
        let a = rules_kani::host::dispatch(Op::tile_at, [7, 0, 0, 0, 0, 0]);
        let s = rules_kani::host::dispatch(Op::set_tile_at, [7, 20, 0, 0, 0, 0]);
        let b = rules_kani::host::dispatch(Op::tile_at, [7, 0, 0, 0, 0, 0]);
        let missing = rules_kani::host::dispatch(Op::set_tile_at, [99, 1, 0, 0, 0, 0]);
        (a, s, b, missing)
    };
    if real != model {
        panic!(
            "MODEL-vs-ENGINE DIVERGENCE [tile_at/set_tile_at]: real={real:?} model={model:?}"
        );
    }
}

fn trig_conf() -> ConfWorld {
    let mut w = base_conf();
    w.trig_main = true;
    w.trig_roll_source = 2;
    w.trig_pay_is_rent = true;
    w.move_present = true;
    w.move_kind = 0;
    w.move_resolve = true;
    w.move_dir = 1;
    w.move_from = 2;
    w.move_remaining = 3;
    w.move_total = 7;
    w.move_roll = 7;
    w.move_tags.insert("firePot".to_string(), 4);
    w.buy_present = true;
    w.buy_kind = 0;
    w.buy_seller = -1;
    w.buy_price = 2500;
    w.buy_deal_owner = 2;
    w.buy_deal_houses = 1;
    w.buy_deal_mortgaged = true;
    w
}

fn trig_model() -> KWorld {
    use rules_kani::world::{BuySnap, MoveSnap};
    let mut w = base_model();
    w.trig_main = true;
    w.trig_roll_source = 2;
    w.trig_pay_is_rent = true;
    let mut ms = MoveSnap::NONE;
    ms.kind = 0;
    ms.resolve = true;
    ms.dir = 1;
    ms.from = 2;
    ms.remaining = 3;
    ms.total = 7;
    ms.roll = 7;
    ms.tags = kworld::NameMap::new();
    ms.tags.set(rules_kani::intern::intern("firePot"), 4);
    w.move_snap = ms;
    w.buy_snap = BuySnap {
        kind: 0,
        seller: -1,
        price: 2500,
        deal_owner: 2,
        deal_houses: 1,
        deal_mortgaged: true,
    };
    w
}

fn trig_get(label: &str, real: impl FnOnce(&mut ConfHost) -> i64, op: Op, args: Args) {
    diff_get_with(label, trig_conf(), trig_model(), real, move || {
        rules_kani::host::dispatch(op, args)
    });
}

#[test]
fn op_trig_move_payload_matches_engine() {
    let _g = model_lock();
    trig_get(
        "trig_move_kind",
        |c| game_rules::hostfns::trig_move_kind(c).unwrap() as i64,
        Op::trig_move_kind,
        [0; 6],
    );
    trig_get(
        "trig_move_resolve",
        |c| game_rules::hostfns::trig_move_resolve(c).unwrap() as i64,
        Op::trig_move_resolve,
        [0; 6],
    );
    trig_get(
        "trig_move_main",
        |c| game_rules::hostfns::trig_move_main(c).unwrap() as i64,
        Op::trig_move_main,
        [0; 6],
    );
    trig_get(
        "trig_move_dir",
        |c| game_rules::hostfns::trig_move_dir(c).unwrap() as i64,
        Op::trig_move_dir,
        [0; 6],
    );
    trig_get(
        "trig_move_remaining",
        |c| game_rules::hostfns::trig_move_remaining(c).unwrap() as i64,
        Op::trig_move_remaining,
        [0; 6],
    );
    trig_get(
        "trig_move_from",
        |c| game_rules::hostfns::trig_move_from(c).unwrap() as i64,
        Op::trig_move_from,
        [0; 6],
    );
    trig_get(
        "trig_move_total",
        |c| game_rules::hostfns::trig_move_total(c).unwrap() as i64,
        Op::trig_move_total,
        [0; 6],
    );
    trig_get(
        "trig_move_roll",
        |c| game_rules::hostfns::trig_move_roll(c).unwrap() as i64,
        Op::trig_move_roll,
        [0; 6],
    );
    let (tp, tl) = intern_str("firePot");
    trig_get(
        "trig_move_tag",
        move |c| game_rules::hostfns::trig_move_tag(c, tp, tl).unwrap() as i64,
        Op::trig_move_tag,
        [tp, tl, 0, 0, 0, 0],
    );
}

#[test]
fn op_trig_move_sentinels_match_engine() {
    let _g = model_lock();
    // No-move sentinels: kind=-1, roll=-1, dir=1, from=-1, remaining=0,
    // total=0, resolve=0; main is Trigger::main (independent of mv).
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with("no-move kind", conf, model,
        |c| game_rules::hostfns::trig_move_kind(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_kind, [0; 6]));
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with("no-move dir", conf, model,
        |c| game_rules::hostfns::trig_move_dir(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_dir, [0; 6]));
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with("no-move from", conf, model,
        |c| game_rules::hostfns::trig_move_from(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_from, [0; 6]));
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with("no-move roll", conf, model,
        |c| game_rules::hostfns::trig_move_roll(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_roll, [0; 6]));
    let mut conf = base_conf();
    conf.trig_main = true;
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with("no-move main", conf, model,
        |c| game_rules::hostfns::trig_move_main(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_main, [0; 6]));
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with("no-move remaining", conf, model,
        |c| game_rules::hostfns::trig_move_remaining(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_remaining, [0; 6]));
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with("no-move total", conf, model,
        |c| game_rules::hostfns::trig_move_total(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_total, [0; 6]));
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with("no-move resolve", conf, model,
        |c| game_rules::hostfns::trig_move_resolve(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_resolve, [0; 6]));
}

#[test]
fn op_trig_buy_deal_payload_matches_engine() {
    let _g = model_lock();
    trig_get(
        "trig_buy_kind",
        |c| game_rules::hostfns::trig_buy_kind(c).unwrap() as i64,
        Op::trig_buy_kind,
        [0; 6],
    );
    trig_get(
        "trig_seller",
        |c| game_rules::hostfns::trig_seller(c).unwrap() as i64,
        Op::trig_seller,
        [0; 6],
    );
    trig_get(
        "trig_price",
        |c| game_rules::hostfns::trig_price(c).unwrap() as i64,
        Op::trig_price,
        [0; 6],
    );
    trig_get(
        "trig_deal_owner",
        |c| game_rules::hostfns::trig_deal_owner(c).unwrap() as i64,
        Op::trig_deal_owner,
        [0; 6],
    );
    trig_get(
        "trig_deal_houses",
        |c| game_rules::hostfns::trig_deal_houses(c).unwrap() as i64,
        Op::trig_deal_houses,
        [0; 6],
    );
    trig_get(
        "trig_deal_mortgaged",
        |c| game_rules::hostfns::trig_deal_mortgaged(c).unwrap() as i64,
        Op::trig_deal_mortgaged,
        [0; 6],
    );
    trig_get(
        "trig_roll_source",
        |c| game_rules::hostfns::trig_roll_source(c).unwrap() as i64,
        Op::trig_roll_source,
        [0; 6],
    );
    trig_get(
        "trig_pay_is_rent",
        |c| game_rules::hostfns::trig_pay_is_rent(c).unwrap() as i64,
        Op::trig_pay_is_rent,
        [0; 6],
    );
}

#[test]
fn op_trig_buy_setters_match_engine() {
    let _g = model_lock();
    let mut conf = trig_conf();
    conf.buy_price = 100;
    conf.buy_deal_owner = -1;
    conf.buy_deal_houses = 0;
    conf.buy_deal_mortgaged = false;
    conf.move_roll = -1;
    let mut model = trig_model();
    model.buy_snap.price = 100;
    model.buy_snap.deal_owner = -1;
    model.buy_snap.deal_houses = 0;
    model.buy_snap.deal_mortgaged = false;
    model.move_snap.roll = -1;

    let mut conf_h = ConfHost::new(conf, Vec::new());
    let mut inline = ConfInline;
    let real = with_inline_host(&mut inline, || {
        game_rules::hostfns::trig_set_price(&mut conf_h, 400).unwrap();
        game_rules::hostfns::trig_set_deal_owner(&mut conf_h, 1).unwrap();
        game_rules::hostfns::trig_set_deal_houses(&mut conf_h, 2).unwrap();
        game_rules::hostfns::trig_set_deal_mortgaged(&mut conf_h, 1).unwrap();
        game_rules::hostfns::trig_set_move_roll(&mut conf_h, 11).unwrap();
        (
            game_rules::hostfns::trig_price(&mut conf_h).unwrap() as i64,
            game_rules::hostfns::trig_deal_owner(&mut conf_h).unwrap() as i64,
            game_rules::hostfns::trig_deal_houses(&mut conf_h).unwrap() as i64,
            game_rules::hostfns::trig_deal_mortgaged(&mut conf_h).unwrap() as i64,
            game_rules::hostfns::trig_move_roll(&mut conf_h).unwrap() as i64,
        )
    });
    install_model_world_with(model, &[]);
    let model = {
        rules_kani::host::dispatch(Op::trig_set_price, [400, 0, 0, 0, 0, 0]);
        rules_kani::host::dispatch(Op::trig_set_deal_owner, [1, 0, 0, 0, 0, 0]);
        rules_kani::host::dispatch(Op::trig_set_deal_houses, [2, 0, 0, 0, 0, 0]);
        rules_kani::host::dispatch(Op::trig_set_deal_mortgaged, [1, 0, 0, 0, 0, 0]);
        rules_kani::host::dispatch(Op::trig_set_move_roll, [11, 0, 0, 0, 0, 0]);
        (
            rules_kani::host::dispatch(Op::trig_price, [0; 6]),
            rules_kani::host::dispatch(Op::trig_deal_owner, [0; 6]),
            rules_kani::host::dispatch(Op::trig_deal_houses, [0; 6]),
            rules_kani::host::dispatch(Op::trig_deal_mortgaged, [0; 6]),
            rules_kani::host::dispatch(Op::trig_move_roll, [0; 6]),
        )
    };
    if real != model {
        panic!(
            "MODEL-vs-ENGINE DIVERGENCE [trig setters]: real={real:?} model={model:?}"
        );
    }
}
