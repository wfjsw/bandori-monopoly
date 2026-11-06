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
//! Fail-closed: no allowlists. Unmodeled ops panic on the model side and
//! surface as test failures. The suite is `#[ignore]`d (opt-in) so the default
//! `cargo test` pass skips it; run it with
//! `cargo test -p rules-kani --test conf_model -- --ignored`.
//!
//! `tests/conf_model.rs` is the entry point. The generated `SymOps` bridge
//! (HostOps -> dispatch) comes from `build.rs` via OUT_DIR.

use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, MutexGuard, OnceLock};

use card_sdk::kani_host::{Args, Op};
use game_rules::native_shims::{install_host, take_host};
use game_rules::{
    with_inline_host, CallOut, CardInfo, CardPile, CardWorld, HostCtx, HostError, HostRequest,
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

/// Run a test body on a wide stack. A `World` is ~1.2 MB, and the harness
/// builds/installs several by value, which exceeds a default test thread's
/// stack.
fn big(f: impl FnOnce() + Send + 'static) {
    if std::env::var("CONF_NO_BIG").is_ok() {
        return f();
    }
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("test thread");
}

// ---------------------------------------------------------------------------
// Real-side world: a thin `CardWorld` that stores exactly the state we diff
// ---------------------------------------------------------------------------

/// One field row of the real-side world (the fields the zone family moves).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfRow {
    pub uid: i32,
    pub card: String,
    /// The field holder (whose field vector the row would sit in).
    pub player: i32,
    pub owner: i32,
    pub tile: i32,
    pub on: i32,
    pub created_by: i32,
    pub effective: bool,
    pub face_down: bool,
    /// `prop::LEAVE_DEST`, `-1` = unset.
    pub leave_dest: i32,
}

#[derive(Clone, Debug)]
pub struct ConfWorld {
    pub n_players: i32,
    pub money: [i32; 4],
    pub pos: [i32; 4],
    pub out: [bool; 4],
    pub charge: [i32; 4],
    pub charge_max: [i32; 4],
    pub stay: [i32; 4],
    pub targeted: [i32; 4],
    pub tiles_owner: [i32; 60],
    pub tiles_houses: [i32; 60],
    pub tiles_mortgaged: [bool; 60],
    /// Per-player hand card names (multiset).
    pub hands: Vec<Vec<String>>,
    /// Per-player draw pile; index 0 is the FRONT (top), matching the model.
    pub decks: Vec<Vec<String>>,
    pub discards: Vec<Vec<String>>,
    /// G11 inspection frames: the held cards per deck owner, front-first.
    pub inspection: Vec<Vec<String>>,
    pub inspection_public: [bool; 4],
    /// Keyed state (slots). Separate from held, matching the real engine.
    pub state: BTreeMap<(i32, String), i32>,
    /// `state_flag` bits per key (`state_set_flags`). Bit 0 = NEVER_NEGATIVE.
    pub state_flags: BTreeMap<(i32, String), u8>,
    /// Held / tokens.
    pub held: BTreeMap<(i32, String), i32>,
    pub dest: i32,
    pub trig_value: i32,
    pub trig_target: i32,
    /// The trigger's actor seat (the calling player of host requests).
    pub trig_player: i32,
    pub trig_cancelled: bool,
    pub turn_player: i32,
    pub events: Vec<String>,
    pub rng: u64,
    pub next_uid: i32,
    /// `Run::current_uid` -- what `self_uid` answers (the model's
    /// `bound_uid`).
    pub self_uid: i32,
    // ---- trigger payload (mirrors model `MoveSnap` / `BuySnap`) ----
    pub trig_main: bool,
    pub trig_roll_source: i32,
    pub trig_pay_is_rent: bool,
    /// `TriggerPay.kind` wire code, `-1` = no payment.
    pub trig_pay_kind: i32,
    pub trig_pay_is_fixed: bool,
    pub trig_pay_total_stage: bool,
    pub trig_charge_skill: bool,
    /// `settle_tile` / `outcome` stamps (`-1` = none).
    pub trig_settle_tile: i32,
    pub trig_settle_outcome: i32,
    /// Scoped pay factors, milli (1000 = x1).
    pub next_move_pay_factor: i32,
    pub plan_pay_factor: i32,
    pub next_settle_pay_factor: i32,
    pub settle_pay_factor: i32,
    pub move_open: bool,
    pub settle_open: u32,
    /// `paid_in_settle_by` per seat and a stored quote figure.
    pub paid_in_settle_by: [i32; 4],
    pub rent_quote: i32,
    /// Last `set_pay_amount` write (`spendBefore` fold).
    pub pay_amount: i32,
    pub pay_amount_set: bool,
    pub move_present: bool,
    pub move_kind: i32,
    pub move_settles: bool,
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
    /// Ownership after the deal. `None` = the deal keeps its default (the buyer).
    pub buy_deal_owner: Option<game_core::engine::rules::Payee>,
    /// Houses after the deal. `None` = as standing.
    pub buy_deal_houses: Option<i32>,
    pub buy_deal_mortgaged: bool,
    // ---- field instances (`tile_at` / `set_tile_at` / the zone family) ----
    pub field: Vec<ConfRow>,
    /// Acquisition sequence per tile (`-1` = nobody) and the next to hand
    /// out, plus the `firstTile` cache per seat (G08 I13).
    pub owned_seq: [i32; 60],
    pub next_owned_seq: i32,
    pub first_tile: [i32; 4],
    /// The last zone transition (`Trigger.zone` would carry it).
    pub trig_zone: Option<(i32, i32, i32, i32, bool, bool)>,
    /// The move-plan staging (G04) -- the model keeps its own copy.
    pub plan: game_core::engine::MoveCtx,
}

impl ConfWorld {
    pub fn new() -> Self {
        Self {
            n_players: 3,
            money: [0; 4],
            pos: [0; 4],
            out: [false; 4],
            charge: [0; 4],
            charge_max: [3; 4],
            stay: [0; 4],
            targeted: [0; 4],
            tiles_owner: [-1; 60],
            tiles_houses: [0; 60],
            tiles_mortgaged: [false; 60],
            hands: vec![Vec::new(); 4],
            decks: vec![Vec::new(); 4],
            discards: vec![Vec::new(); 4],
            inspection: vec![Vec::new(); 4],
            inspection_public: [false; 4],
            state: BTreeMap::new(),
            state_flags: BTreeMap::new(),
            held: BTreeMap::new(),
            dest: 0,
            trig_value: 0,
            trig_player: 0,
            trig_target: -1,
            trig_cancelled: false,
            turn_player: 0,
            events: Vec::new(),
            rng: 0x1234_5678_9abc_def0,
            next_uid: 1,
            self_uid: 1,
            trig_main: false,
            trig_roll_source: 0,
            trig_pay_is_rent: false,
            trig_pay_kind: -1,
            trig_pay_is_fixed: false,
            trig_pay_total_stage: true,
            trig_charge_skill: false,
            trig_settle_tile: -1,
            trig_settle_outcome: -1,
            next_move_pay_factor: 1000,
            plan_pay_factor: 1000,
            next_settle_pay_factor: 1000,
            settle_pay_factor: 1000,
            move_open: false,
            settle_open: 0,
            paid_in_settle_by: [0; 4],
            rent_quote: -1,
            pay_amount: 0,
            pay_amount_set: false,
            move_present: false,
            move_kind: -1,
            move_settles: false,
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
            buy_deal_owner: None,
            buy_deal_houses: None,
            buy_deal_mortgaged: false,
            field: Vec::new(),
            owned_seq: [-1; 60],
            next_owned_seq: 0,
            first_tile: [-1; 4],
            trig_zone: None,
            plan: game_core::engine::MoveCtx::default(),
        }
    }

    fn d(&mut self, sides: i32) -> i32 {
        // Same mid-value formula as the model's native `roll_dice`.
        (1 * (sides.max(1) + 1)) / 2
    }

    fn pile_of(&mut self, pile: CardPile) -> &mut Vec<Vec<String>> {
        match pile {
            CardPile::Hand => &mut self.hands,
            CardPile::Graveyard => &mut self.discards,
            CardPile::Deck => &mut self.decks,
            CardPile::Inspection => &mut self.inspection,
            _ => &mut self.hands,
        }
    }

    fn pile_ref(&self, pile: CardPile) -> &Vec<Vec<String>> {
        match pile {
            CardPile::Hand => &self.hands,
            CardPile::Graveyard => &self.discards,
            CardPile::Deck => &self.decks,
            CardPile::Inspection => &self.inspection,
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
        self.events
            .push(format!("dice {player_id} {count}d{sides}={t}"));
        t
    }
    fn log(&mut self, player_id: i32, msg: EngineMsg) {
        self.events.push(format!("log {player_id} {msg}"));
    }
    fn plan_mut(&mut self) -> &mut game_core::engine::MoveCtx {
        &mut self.plan
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
        if rules_kani::board::is_deed(t) {
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
    /// ABI v56: kind by wire spelling. Replaces `is_start`/`is_deed`/`is_agent`.
    fn tile_is(&self, tile: i32, kind: &str) -> bool {
        let s = rules_kani::board::tile_slot(tile);
        if s < 0 {
            return false;
        }
        let t = &rules_kani::board::TILES[s as usize];
        match kind {
            "circle" => rules_kani::board::is_start(t),
            "ring" => rules_kani::board::is_deed(t),
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
        // The engine's ownership write is `World::set_tile_owner`, which also
        // writes the acquisition ledger (G08 I13).
        self.set_tile_owner(tile, player_id);
        self.events.push(format!("set_owner {tile} {player_id}"));
    }
    fn dist(&self, a: i32, b: i32) -> i32 {
        // The shorter way around the board loop.
        let d = (b - a).rem_euclid(60);
        d.min(60 - d)
    }
    fn tile_forward(&self, a: i32, b: i32) -> i32 {
        // Steps forward from a to b around the board loop.
        (b - a).rem_euclid(60)
    }
    fn nearest(&self, by: i32, kind: i32) -> Vec<i32> {
        self.nearest_with_prop(by, kind, None)
    }
    fn nearest_with_prop(&self, by: i32, kind: i32, _key: Option<&str>) -> Vec<i32> {
        // The model holds no field instances, so the property union is empty.
        let shortest = kind == card_sdk::abi::DistanceKind::Shortest as i32;
        let at = if by >= 0 && by < self.n_players {
            self.pos[by as usize]
        } else {
            return Vec::new();
        };
        let mut best = i32::MAX;
        let mut out: Vec<i32> = Vec::new();
        for p in 0..self.n_players {
            if p == by || self.out[p as usize] {
                continue;
            }
            let d = if shortest {
                self.dist(at, self.pos[p as usize])
            } else {
                self.tile_forward(at, self.pos[p as usize])
            };
            if !shortest && d == 0 {
                continue;
            }
            if d < best {
                best = d;
                out.clear();
                out.push(p);
            } else if d == best {
                out.push(p);
            }
        }
        out
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
    fn add_to_dest(&mut self, player_id: i32, card: &str, dest: i32, _counted: bool) {
        let p = player_id.max(0) as usize % 4;
        match dest {
            0 => {
                self.discards[p].push(card.to_string());
                self.events.push(format!("to_discard {player_id} {card}"));
                // Same pile-write refill as [`Self::to_discard`].
                if self.inspection[p].is_empty()
                    && self.decks[p].is_empty()
                    && !self.discards[p].is_empty()
                {
                    self.decks[p] = std::mem::take(&mut self.discards[p]);
                }
            }
            1 => {
                self.hands[p].push(card.to_string());
                self.events.push(format!("add_to_dest {player_id} {card}"));
            }
            4 | 5 | 6 => {
                self.decks[p].push(card.to_string());
            }
            // Field / Banished are not piles.
            _ => {}
        }
    }
    fn take_card(&mut self, player_id: i32, card: &str, src: CardPile) -> bool {
        let p = player_id.max(0) as usize % 4;
        let v = self.pile_of(src)[p].clone();
        if let Some(i) = v.iter().position(|c| c == card) {
            self.pile_of(src)[p].remove(i);
            self.events.push(format!("take_card {player_id} {card}"));
            true
        } else {
            false
        }
    }
    /// The engine applies these three directly on the world (no inline
    /// request), so the conf adapter owns their `CardWorld` halves.
    fn inspection_end(&mut self, player: i32, shuffle: bool) -> i32 {
        let p = player.max(0) as usize % 4;
        let held = std::mem::take(&mut self.inspection[p]);
        let n = held.len();
        if shuffle {
            self.decks[p].extend(held);
        } else {
            let mut rest = std::mem::take(&mut self.decks[p]);
            let mut back = held;
            back.append(&mut rest);
            self.decks[p] = back;
        }
        self.events.push(format!("inspection_return {player} {n}"));
        n as i32
    }
    fn reveal_pile(&mut self, player: i32, pile: CardPile, _reason: EngineMsg) {
        self.events.push(format!("reveal {player} {}", pile as i32));
    }
    fn reveal_card(&mut self, player: i32, card: &str, _reason: EngineMsg) {
        self.events.push(format!("reveal_card {player} {card}"));
    }
    fn cards_in(&self, player_id: i32, pile: CardPile) -> Vec<String> {
        let p = player_id.max(0) as usize % 4;
        self.pile_ref(pile)[p].clone()
    }
    fn to_discard(&mut self, player_id: i32, card: &str) {
        let p = player_id.max(0) as usize % 4;
        self.discards[p].push(card.to_string());
        self.events.push(format!("to_discard {player_id} {card}"));
        // The pile write's refill (`World::refill_after_pile_write` -> the
        // graveyard back into an emptied deck); the model does the same.
        if self.inspection[p].is_empty() && self.decks[p].is_empty() && !self.discards[p].is_empty()
        {
            self.decks[p] = std::mem::take(&mut self.discards[p]);
        }
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
        self.discards[p]
            .iter()
            .filter(|c| c.as_str() == card)
            .count() as i32
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
        if self.take_card(player_id, card, CardPile::Hand) {
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
        let tile = self.pos[player_id.max(0) as usize % 4];
        self.place_card_on(player_id, tile, card, _note)
    }
    fn place_card_on(&mut self, player_id: i32, tile: i32, card: &str, _note: EngineMsg) -> i32 {
        self.events
            .push(format!("place_card_on {player_id} {tile} {card}"));
        let uid = self.next_uid;
        self.next_uid += 1;
        self.field.push(ConfRow {
            uid,
            card: card.to_string(),
            player: player_id,
            owner: player_id,
            tile,
            on: -1,
            created_by: -1,
            effective: true,
            face_down: false,
            leave_dest: -1,
        });
        uid
    }
    fn self_uid(&self) -> i32 {
        self.self_uid
    }
    fn set_self_uid(&mut self, uid: i32) {
        self.self_uid = uid;
    }
    fn set_dest(&mut self, dest: i32, _cause: i32) {
        self.dest = dest;
    }
    fn set_transfer_to_dest(&mut self, _to: i32, dest: i32, _cause: i32) {
        self.dest = dest;
    }
    fn send_to_dest(&mut self, dest: i32, cause: i32) -> Option<i32> {
        let got = self.zone_leave(self.self_uid, dest, -1, cause);
        (got >= 0).then_some(got)
    }
    fn transfer_to_dest(&mut self, to: i32, dest: i32, cause: i32) -> Option<i32> {
        let got = self.zone_leave(self.self_uid, dest, to, cause);
        (got >= 0).then_some(got)
    }
    fn rent_unit_mult(&self) -> i32 {
        10
    }
    fn add_rent_unit_bonus(&mut self, _n: i32) -> i32 {
        0
    }
    fn teleport_to(&mut self, player_id: i32, tile: i32) {
        let p = player_id.max(0) as usize % 4;
        self.pos[p] = tile;
        self.events.push(format!("teleport_to {player_id} {tile}"));
    }
    fn is_placed(&self) -> i32 {
        0
    }
    // keyed state (slots)
    fn state_var(&self, _player_id: i32, _key: &str) -> game_core::state::StateVar {
        game_core::state::StateVar::default()
    }
    fn state_get(&self, player_id: i32, key: &str) -> i32 {
        // The `stay` column is seeded by `base_conf` without a map entry, so
        // it stays authoritative for that key (the model's `key_get` mirrors
        // `w.stay` the same way).
        if key == "stay" {
            return self.stay[player_id.max(0) as usize % 4];
        }
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
        let value = if self
            .state_flags
            .get(&(player_id, key.to_string()))
            .copied()
            .unwrap_or(0)
            & 1
            != 0
        {
            value.max(0)
        } else {
            value
        };
        self.state.insert((player_id, key.to_string()), value);
        // [停留] lives in the generic keyed state, but the snapshot compares
        // the `stay` column; the model mirrors the same key into `w.stay`
        // (`arm_state_set`), so the map alone would desync the two sides.
        if key == "stay" {
            self.stay[player_id.max(0) as usize % 4] = value;
        }
        self.events
            .push(format!("set_slot {player_id} {key} {value}"));
        let p = player_id.max(0) as usize % 4;
        match key {
            "stay" => {
                self.stay[p] = value.max(0);
                self.events.push(format!("give_stay {player_id} {value}"));
            }
            "stun" => self.events.push(format!("give_stun {player_id} {value}")),
            "exile" => self.events.push(format!("give_exile {player_id} {value}")),
            _ => {}
        }
        value
    }
    fn state_add(&mut self, player_id: i32, key: &str, delta: i32) -> i32 {
        if key == "stay" {
            // The column is the base value (see `state_get`); the model's
            // `arm_state_add` raises a `GiveStay` effect on every add, so the
            // event line keeps the effect-count comparison honest.
            let p = player_id.max(0) as usize % 4;
            let value = self.stay[p].saturating_add(delta).max(0);
            self.stay[p] = value;
            self.state.insert((player_id, key.to_string()), value);
            self.events.push(format!("give_stay {player_id} {delta}"));
            return value;
        }
        let floor = self
            .state_flags
            .get(&(player_id, key.to_string()))
            .copied()
            .unwrap_or(0)
            & 1
            != 0;
        let e = self.state.entry((player_id, key.to_string())).or_insert(0);
        *e = e.saturating_add(delta);
        if floor {
            *e = (*e).max(0);
        }
        let out = *e;
        // Mirror the status columns the snapshot compares (the model's
        // `arm_state_add` writes its columns and logs the same effect).
        let p = player_id.max(0) as usize % 4;
        match key {
            "stay" => {
                self.stay[p] = (self.stay[p].saturating_add(delta)).max(0);
                self.events.push(format!("give_stay {player_id} {delta}"));
            }
            "stun" => self.events.push(format!("give_stun {player_id} {delta}")),
            "exile" => self.events.push(format!("give_exile {player_id} {delta}")),
            _ => {}
        }
        out
    }
    fn state_set_bounds(&mut self, _: i32, _: &str, _: i32, _: i32) {}
    fn state_set_expires(&mut self, _: i32, _: &str, _: Option<game_core::state::Tick>) {}
    fn state_set_flags(&mut self, player_id: i32, key: &str, flags: u8) {
        self.state_flags.insert((player_id, key.to_string()), flags);
    }
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
    fn set_held_name(&mut self, name: &str, player_id: i32, v: i32, _max: i32, _instance: i32) {
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
    fn gain_marker(&mut self, player_id: i32, _name: &str, n: i32, _reason: EngineMsg) -> i32 {
        // Matches `game_core::engine::ops::gain_marker`: return the amount
        // actually gained (clamped under cap), not the new total.
        let p = player_id.max(0) as usize % 4;
        if n <= 0 {
            return 0;
        }
        let got = n.clamp(0, (self.charge_max[p] - self.charge[p]).max(0));
        if got <= 0 {
            return 0;
        }
        self.charge[p] += got;
        self.events.push(format!("gain_marker {player_id} {got}"));
        got
    }
    fn spend_marker(&mut self, player_id: i32, _name: &str, n: i32, _reason: EngineMsg) -> i32 {
        let p = player_id.max(0) as usize % 4;
        if n <= 0 {
            return 1;
        }
        if self.charge[p] >= n {
            self.charge[p] -= n;
            1
        } else {
            0
        }
    }
    fn add_marker_max(&mut self, player_id: i32, _name: &str, n: i32) -> i32 {
        let p = player_id.max(0) as usize % 4;
        self.charge_max[p] = self.charge_max[p].saturating_add(n);
        self.charge_max[p]
    }
    fn apply_abnormal(&mut self, player_id: i32, kind: i32, n: i32, _to: i32) {
        let p = player_id.max(0) as usize % 4;
        match kind {
            1 => {
                self.stay[p] = (self.stay[p] + n).max(0);
                self.events.push(format!("give_stay {player_id} {n}"));
            }
            2 => {
                self.events.push(format!("give_stun {player_id} {n}"));
            }
            3 => {
                self.events.push(format!("give_exile {player_id} {n}"));
            }
            _ => {}
        }
    }
    fn skip_main_move(&mut self, _player_id: i32, _locked: i32) {}
    fn give_extra_turn(&mut self, _player_id: i32) {}
    fn end_turn_now(&mut self, _player_id: i32) {}
    fn teleport_as(&mut self, _player_id: i32, _tile: i32, _resolve: i32, _as_main: i32) {}
    fn log_as(&mut self, _player_id: i32, _kind: &str, _msg: EngineMsg) {}
    fn can_pay(&self, player_id: i32) -> i32 {
        (self.money(player_id) > 0) as i32
    }
    fn cant_move_from(&self, player_id: i32, _activation_ordinal: i32) -> i32 {
        // Same 0/1/3 shape as the bridge; `cant_move` inherits it through the
        // trait default.
        if self.turn_player != player_id {
            1
        } else if self.stay[player_id.max(0) as usize % 4] > 0 {
            3
        } else {
            0
        }
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
    fn persona_is(&self, _player_id: i32, _name: &str) -> i32 {
        0
    }
    fn set_persona(&mut self, _player_id: i32, _name: &str) {}
    fn in_group(&self, _player_id: i32, _name: &str) -> i32 {
        0
    }
    fn trigger(&self) -> game_rules::Trigger {
        use card_sdk::abi::BuyKind;
        let mut t = game_rules::Trigger::default();
        t.value = self.trig_value;
        t.target = self.trig_target;
        t.is_main_move = self.trig_main;
        t.roll_source = game_core::engine::rules::RollSource::from_i32(self.trig_roll_source);
        if self.move_present {
            t.mv = Some(game_rules::TriggerMove {
                kind: game_rules::MoveKind::from_i32(self.move_kind)
                    .unwrap_or(game_rules::MoveKind::Walk),
                settles: self.move_settles,
                tags: self
                    .move_tags
                    .iter()
                    .map(|(k, v)| (k.clone(), *v))
                    .collect(),
                is_main_move: self.trig_main,
                dir: game_core::engine::rules::Dir::from_i32(self.move_dir),
                from: self.move_from,
                remaining: self.move_remaining,
                total: self.move_total,
                roll: if self.move_roll >= 0 {
                    Some(self.move_roll)
                } else {
                    None
                },
                by_skill: false,
                settle_redirect: false,
                rolled: self.move_roll >= 0,
                effective: self.move_remaining,
                path: Vec::new(),
                abnormal: false,
            });
        }
        if self.trig_pay_kind >= 0 || self.trig_pay_is_rent {
            t.pay = Some(game_rules::TriggerPay {
                kind: game_core::engine::PayKind::from_i32(self.trig_pay_kind.max(0)),
                shape: game_core::engine::PayShape::from_i32(self.trig_pay_is_fixed as i32),
                is_rent: self.trig_pay_is_rent,
                total_stage: self.trig_pay_total_stage,
            });
        }
        t.charge_skill = self.trig_charge_skill;
        t.settle_tile = self.trig_settle_tile;
        t.outcome = (self.trig_settle_outcome >= 0)
            .then(|| game_core::engine::SettleOutcome::from_i32(self.trig_settle_outcome));
        if self.buy_present {
            t.buy = Some(game_rules::TriggerBuy {
                kind: BuyKind::from_i32(self.buy_kind).unwrap_or(BuyKind::Land),
                seller: game_core::engine::rules::Payee::from_i32(self.buy_seller),
                price: self.buy_price,
                deal_owner: self.buy_deal_owner,
                deal_houses: self.buy_deal_houses,
                deal_mortgaged: self.buy_deal_mortgaged,
            });
        }
        if self.trig_cancelled {
            t.negate_activation();
        }
        t.zone = self
            .trig_zone
            .map(
                |(from, to, cause, holder, wasted, counted)| game_kinds::ZoneChange {
                    from: game_kinds::Zone::from_i32(from),
                    to: game_kinds::Zone::from_i32(to),
                    cause: game_kinds::ZoneChangeCause::from_i32(cause),
                    holder,
                    wasted,
                    counted,
                },
            );
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
        _kind: &str,
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
    fn set_trigger_deal_owner(&mut self, v: Option<game_core::engine::rules::Payee>) {
        self.buy_deal_owner = v;
        self.buy_present = true;
    }
    fn set_trigger_deal_houses(&mut self, v: Option<i32>) {
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
    fn paid_in_settle_by(&self, seat: i32) -> i32 {
        *self
            .paid_in_settle_by
            .get(seat.max(0) as usize % 4)
            .unwrap_or(&0)
    }
    fn rent_quote(&mut self, _payer: i32, _tile: i32) -> i32 {
        self.rent_quote
    }
    fn set_trigger_pay_kind(&mut self, kind: i32) {
        self.trig_pay_kind = kind;
    }
    fn set_trigger_pay_shape(&mut self, shape: i32) -> bool {
        self.trig_pay_is_fixed = shape == 1;
        true
    }
    fn settle_tile(&self) -> i32 {
        self.trig_settle_tile
    }
    fn settle_outcome(&self) -> i32 {
        self.trig_settle_outcome
    }
    fn pay_factor(&self) -> i32 {
        self.plan_pay_factor.saturating_mul(self.settle_pay_factor)
    }
    fn set_pay_factor(&mut self, milli: i32) {
        // The `Auto` scope composes where a factor is live (MONEY-01).
        self.multiply_pay_factor(milli, 4);
    }
    fn multiply_pay_factor(&mut self, milli: i32, scope: i32) {
        let milli = milli.max(0);
        let scope = match game_core::engine::PayFactorScope::from_i32(scope) {
            game_core::engine::PayFactorScope::ThisMove if !self.move_open => {
                game_core::engine::PayFactorScope::NextMove
            }
            game_core::engine::PayFactorScope::ThisSettle if self.settle_open == 0 => {
                game_core::engine::PayFactorScope::NextSettle
            }
            game_core::engine::PayFactorScope::Auto if self.settle_open > 0 => {
                game_core::engine::PayFactorScope::ThisSettle
            }
            game_core::engine::PayFactorScope::Auto if self.move_open => {
                game_core::engine::PayFactorScope::ThisMove
            }
            game_core::engine::PayFactorScope::Auto => game_core::engine::PayFactorScope::NextMove,
            s => s,
        };
        match scope {
            game_core::engine::PayFactorScope::ThisMove => {
                self.plan_pay_factor = self.plan_pay_factor.saturating_mul(milli)
            }
            game_core::engine::PayFactorScope::ThisSettle => {
                self.settle_pay_factor = self.settle_pay_factor.saturating_mul(milli)
            }
            game_core::engine::PayFactorScope::NextMove => {
                self.next_move_pay_factor = self.next_move_pay_factor.saturating_mul(milli)
            }
            game_core::engine::PayFactorScope::NextSettle => {
                self.next_settle_pay_factor = self.next_settle_pay_factor.saturating_mul(milli)
            }
            game_core::engine::PayFactorScope::Auto => unreachable!(),
        }
    }
    fn gains_this_turn(&self, _player: i32) -> i32 {
        0
    }
    fn targeted_count(&self, player_id: i32) -> i32 {
        if player_id >= 0 && (player_id as usize) < self.targeted.len() {
            self.targeted[player_id as usize]
        } else {
            0
        }
    }
    fn field_instances(&self, player_id: i32) -> Vec<(i32, String)> {
        // An attachment is reachable through its host (G08 I6).
        self.field
            .iter()
            .filter(|r| r.player == player_id && r.on < 0)
            .map(|r| (r.uid, r.card.clone()))
            .collect()
    }
    fn tile_at(&self, uid: i32) -> i32 {
        self.row(uid).map_or(-1, |i| self.field[i].tile)
    }
    fn set_tile_at(&mut self, uid: i32, tile: i32) -> bool {
        match self.row(uid) {
            Some(i) => {
                self.field[i].tile = tile;
                self.events.push(format!("set_tile_at {uid} {tile}"));
                true
            }
            None => false,
        }
    }

    // ---- G08 zone family ---------------------------------------------------

    fn zone_of(&self, uid: i32) -> i32 {
        let Some(i) = self.row(uid) else { return 0 };
        if self.field[i].on >= 0 {
            9
        } else {
            3
        }
    }
    fn attachment_host(&self, uid: i32) -> i32 {
        self.row(uid).map_or(-1, |i| self.field[i].on)
    }
    fn attached_cards(&self, host: i32) -> Vec<i32> {
        self.field
            .iter()
            .filter(|r| r.on == host)
            .map(|r| r.uid)
            .collect()
    }
    fn created_cards(&self, creator: i32) -> Vec<i32> {
        self.field
            .iter()
            .filter(|r| r.created_by == creator)
            .map(|r| r.uid)
            .collect()
    }
    fn earliest_owned_tile(&self, player_id: i32) -> i32 {
        let mut best_seq = i32::MAX;
        let mut best_tile = -1;
        for t in 0..60 {
            if self.tiles_owner[t] == player_id && self.owned_seq[t] >= 0 {
                if self.owned_seq[t] < best_seq {
                    best_seq = self.owned_seq[t];
                    best_tile = t as i32;
                }
            }
        }
        best_tile
    }
    fn instance_owner(&self, uid: i32) -> i32 {
        self.row(uid).map_or(-1, |i| self.field[i].owner)
    }
    fn set_instance_owner(&mut self, uid: i32, owner: i32) -> bool {
        match self.row(uid) {
            Some(i) => {
                self.field[i].owner = owner;
                true
            }
            None => false,
        }
    }
    fn set_created_by_at(&mut self, uid: i32, creator: i32) -> bool {
        match self.row(uid) {
            Some(i) => {
                self.field[i].created_by = creator;
                true
            }
            None => false,
        }
    }
    fn set_ineffective_at(&mut self, uid: i32, on: bool) -> bool {
        match self.row(uid) {
            Some(i) => {
                self.field[i].effective = !on;
                true
            }
            None => false,
        }
    }
    fn is_ineffective_at(&self, uid: i32) -> bool {
        self.row(uid).is_some_and(|i| !self.field[i].effective)
    }
    fn attach_card(&mut self, host: i32, card: &str, cause: i32, face_down: bool) -> i32 {
        let Some(hi) = self.row(host) else { return -1 };
        let caller = self.trig_player;
        let tile = self.field[hi].tile;
        let host_holder = self.field[hi].player;
        // A hand copy is taken first; with none, the caller's first live
        // instance is re-targeted (cycle-guarded) -- `zone_attach`'s order.
        let p = caller.max(0) as usize % 4;
        if let Some(k) = self.hands[p].iter().position(|c| c == card) {
            self.hands[p].remove(k);
        } else {
            let live = self
                .field
                .iter()
                .find(|r| r.player == caller && r.card == card && r.uid != host)
                .map(|r| r.uid);
            let Some(uid) = live else { return -1 };
            let from = self.zone_of(uid);
            if from != 3 && from != 9 {
                return -1;
            }
            let mut cursor = host;
            let mut guard = 0;
            while cursor >= 0 && guard < 4096 {
                if cursor == uid {
                    return -1;
                }
                cursor = self.attachment_host(cursor);
                guard += 1;
            }
            let ri = self.row(uid).unwrap();
            self.field[ri].on = host;
            self.field[ri].face_down = face_down;
            self.field[ri].tile = tile;
            self.events.push(format!("attach_card {host} {card}"));
            self.trig_zone = Some((from, 9, cause, host_holder, false, false));
            return uid;
        }
        let uid = self.next_uid;
        self.next_uid += 1;
        self.events.push(format!("attach_card {host} {card}"));
        self.trig_zone = Some((0, 9, cause, host_holder, false, false));
        self.field.push(ConfRow {
            uid,
            card: card.to_string(),
            player: caller,
            owner: caller,
            tile,
            on: host,
            created_by: -1,
            effective: true,
            face_down,
            leave_dest: -1,
        });
        uid
    }
    fn detach_card(&mut self, uid: i32, destination: i32, cause: i32) -> i32 {
        if self.attachment_host(uid) < 0 {
            return -1;
        }
        self.zone_leave(uid, destination, -1, cause)
    }
    fn rehome_card(&mut self, tile: i32, owner: i32) -> bool {
        self.set_tile_owner(tile, owner);
        self.events.push(format!("set_owner {tile} {owner}"));
        true
    }
    fn play_hand_card(&mut self, player_id: i32, card: &str) -> bool {
        let p = player_id.max(0) as usize % 4;
        let Some(k) = self.hands[p].iter().position(|c| c == card) else {
            return false;
        };
        self.hands[p].remove(k);
        let uid = self.next_uid;
        self.next_uid += 1;
        let tile = self.pos[p];
        self.events
            .push(format!("play_hand_card {player_id} {card}"));
        self.field.push(ConfRow {
            uid,
            card: card.to_string(),
            player: player_id,
            owner: player_id,
            tile,
            on: -1,
            created_by: -1,
            effective: true,
            face_down: false,
            leave_dest: -1,
        });
        true
    }
    fn unplace_at(&mut self, uid: i32, destination: i32, cause: i32) -> i32 {
        self.zone_leave(uid, destination, -1, cause)
    }
    fn set_face_down_at(&mut self, uid: i32, on: bool, _cause: i32) -> bool {
        match self.row(uid) {
            Some(i) => {
                self.field[i].face_down = on;
                true
            }
            None => false,
        }
    }
    fn set_self_face_down(&mut self, on: bool, _cause: i32) -> bool {
        let uid = self.self_uid;
        self.set_face_down_at(uid, on, 0)
    }
    fn unplace_card(&mut self) -> i32 {
        let uid = self.self_uid;
        self.zone_leave(uid, -1, -1, 3)
    }
}

impl ConfWorld {
    /// Row index of `uid`.
    fn row(&self, uid: i32) -> Option<usize> {
        self.field.iter().position(|r| r.uid == uid)
    }

    /// `set_tile_owner` -- write the deed and its sequence, rebuild the
    /// previous owner's `firstTile` (G08 I13).
    fn set_tile_owner(&mut self, tile: i32, owner: i32) {
        let Ok(t) = usize::try_from(tile) else { return };
        if t >= 60 {
            return;
        }
        let cur = self.tiles_owner[t];
        if cur == owner {
            return;
        }
        self.tiles_owner[t] = owner;
        self.owned_seq[t] = if owner >= 0 {
            let s = self.next_owned_seq;
            self.next_owned_seq = s.saturating_add(1);
            s
        } else {
            -1
        };
        if cur >= 0 {
            let first = self.earliest_owned_tile(cur);
            self.first_tile[cur as usize % 4] = first;
        }
    }

    /// The zone writer minus hooks (`PlayCtx::zone_leave` / `zone_commit`).
    fn zone_leave(&mut self, uid: i32, destination: i32, to_seat: i32, cause: i32) -> i32 {
        let Some(ri) = self.row(uid) else { return -1 };
        let from = self.zone_of(uid);
        if from != 3 && from != 9 {
            return -1;
        }
        let mut to = if destination < 0 {
            4
        } else {
            match destination {
                1 => 2,
                2 => 3,
                3 => 5,
                4..=6 => 1,
                _ => 4,
            }
        };
        let rewrite = match self.field[ri].leave_dest {
            1 => 2,
            2 => 3,
            3 => 5,
            4..=6 => 1,
            _ => 4,
        };
        if rewrite != to && to != 9 {
            to = rewrite;
        }
        let row = self.field[ri].clone();
        let holder = if to == 3 || to_seat < 0 {
            row.owner
        } else {
            to_seat
        };
        let wasted = !row.effective;
        self.events
            .push(format!("zone_leave {uid} {from} {to} {cause}"));

        // Attachments, then children, then the leaver.
        let atts: Vec<i32> = self.attached_cards(uid);
        for a in atts {
            if let Some(i) = self.row(a) {
                self.field[i].on = -1;
            }
            let d = self.field[self.row(a).unwrap()].leave_dest;
            self.zone_leave(a, d, -1, 8);
        }
        let kids: Vec<i32> = self.created_cards(uid);
        for c in kids {
            if let Some(i) = self.row(c) {
                self.field[i].created_by = -1;
            }
            self.zone_leave(c, 0, -1, 9);
        }

        self.trig_zone = Some((from, to, cause, holder, wasted, false));
        let holder_now = self.field[ri].player;
        self.field.remove(ri);
        match to {
            3 => {
                self.field.push(ConfRow { on: -1, ..row });
                holder_now
            }
            4 => {
                let p = holder.max(0) as usize % 4;
                self.discards[p].push(row.card.clone());
                self.events
                    .push(format!("to_discard {} {}", holder, row.card));
                holder.max(0)
            }
            2 => {
                let p = holder.max(0) as usize % 4;
                self.hands[p].push(row.card.clone());
                self.events
                    .push(format!("add_to_dest {} {}", holder, row.card));
                holder.max(0)
            }
            _ => holder.max(0),
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
            state: HostState::new(
                ConfRules,
                world,
                game_core::engine::GuestLog {
                    answers,
                    ..Default::default()
                },
                0,
            ),
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
    fn read_guest(&mut self, ptr: i32, len: i32) -> Result<Vec<u8>, HostError> {
        Ok(card_sdk::native::read(ptr, len.max(0) as usize))
    }
    fn write_guest(&mut self, ptr: i32, bytes: &[u8]) -> Result<(), HostError> {
        card_sdk::native::write(ptr, bytes);
        Ok(())
    }
    fn fuel(&mut self) -> Result<u64, HostError> {
        Ok(self.fuel)
    }
    fn set_fuel(&mut self, v: u64) -> Result<(), HostError> {
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
        _arg: &[u8],
        _want_msg: bool,
    ) -> (
        Result<CallOut, HostError>,
        HostState<ConfWorld, ConfRules>,
        u64,
    ) {
        // Nested `play_card` is out of scope for the thin adapter.
        (
            Err(HostError::trap(
                "conf_model: nested call_entry not supported",
            )),
            HostState::new(ConfRules, ConfWorld::new(), Default::default(), 0),
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
    fn apply(
        &mut self,
        run: &mut dyn std::any::Any,
        _log: &mut game_core::engine::GuestLog,
        _start: game_core::engine::JournalStart,
        req: HostRequest,
    ) -> Option<i32> {
        let w = run.downcast_mut::<ConfWorld>()?;
        match req {
            HostRequest::Pay {
                from,
                to,
                amount,
                runs_total_stage: _,
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
            HostRequest::AdjustMoney {
                player_id, delta, ..
            } => {
                // Raw money write (`ctx::adjust_money`): no pipeline, no
                // window. Matches the model's `arm_adjust_money`.
                let p = player_id.max(0) as usize % 4;
                if delta > 0 {
                    w.money[p] = w.money[p].saturating_add(delta);
                    w.events
                        .push(format!("GainMoney {{ player: {player_id}, n: {delta} }}"));
                } else if delta < 0 {
                    let n = (-delta).min(w.money[p].max(0));
                    w.money[p] -= n;
                }
                Some(w.money[p])
            }
            HostRequest::PayCommand {
                from,
                to,
                amount,
                spec,
                ..
            } => {
                let shape = spec & 0xF;
                let kind = (spec >> 4) & 0xF;
                let flags = (spec >> 8) & 0xF;
                let raw_tile = (spec >> 12) & 0xF_FFFF;
                w.trig_pay_kind = kind;
                w.trig_pay_is_fixed = shape == 1;
                w.trig_pay_total_stage = flags & 4 != 0;
                w.trig_settle_tile = if raw_tile > 0 { raw_tile - 1 } else { -1 };
                if amount > 0 {
                    if from >= 0 {
                        let f = from as usize % 4;
                        w.money[f] = (w.money[f] - amount).max(0);
                    }
                    if to >= 0 {
                        let t = to as usize % 4;
                        w.money[t] = w.money[t].saturating_add(amount);
                    }
                    w.events.push(format!("pay_pipeline {from}->{to} {amount}"));
                }
                Some(amount)
            }
            HostRequest::ApplyRedeem {
                player_id,
                tile,
                amount,
            } => {
                // G02 `spendBefore`: the write is the cost, else the asked one.
                let cost = if w.pay_amount_set {
                    w.pay_amount
                } else {
                    amount.max(0)
                };
                w.pay_amount_set = false;
                if cost > 0 {
                    let p = player_id.max(0) as usize % 4;
                    let n = cost.min(w.money[p].max(0));
                    w.money[p] -= n;
                }
                w.tiles_mortgaged[tile.max(0) as usize % 60] = false;
                w.events.push(format!("redeem {player_id} {tile} {cost}"));
                Some(cost)
            }
            // Dice answer the model's mid-value face (`roll_dice`, native
            // branch). Same clamps, so both sides roll the same number.
            HostRequest::Roll { count, sides, .. } => {
                let c = count.max(0).min(100);
                let s = sides.max(1).min(1000);
                Some(c * (s + 1) / 2)
            }
            HostRequest::Teleport { player_id, tile } => {
                if player_id >= 0 {
                    w.pos[player_id as usize % 4] = tile;
                    w.events.push(format!("teleport_to {player_id} {tile}"));
                }
                Some(1)
            }
            HostRequest::Gate { .. } => Some(1),
            // Raw money rides no pipeline; let the hostfn's own
            // `adjust_money_raw` fallback (`gain`/`pay` on the world) run.
            HostRequest::AdjustMoney { .. } => None,
            // The shared mid-value dice formula (`d` / the model's
            // `roll_dice`): an effect roll is `count` mid faces, a move roll
            // its 2d6 plan sum.
            HostRequest::Roll {
                count,
                sides,
                purpose,
                ..
            } => Some(if purpose == 1 {
                7
            } else {
                (0..count.max(0)).map(|_| (sides.max(1) + 1) / 2).sum()
            }),
            HostRequest::Marker { .. } => Some(1),
            HostRequest::Draw {
                player_id: _, n, ..
            } => Some(n),
            // G11: the frame move is the answer, front-first, clamped to the
            // deck. The `inspected` raises are hook-side and not modelled.
            HostRequest::InspectionBegin {
                looker,
                owner,
                count,
                public,
            } => {
                let o = owner.max(0) as usize % 4;
                let n = count.max(0) as usize;
                let n = n.min(w.decks[o].len());
                let held: Vec<String> = w.decks[o].drain(..n).collect();
                w.inspection[o].extend(held);
                w.inspection_public[o] = public;
                w.events
                    .push(format!("inspection_hold {looker} {owner} {n}"));
                Some(n as i32)
            }
            // `inspection_end` / `reveal_*` are applied by the engine directly
            // on the world (`CardWorld`), never as a request.
            HostRequest::InspectionEnd { .. }
            | HostRequest::RevealPile { .. }
            | HostRequest::RevealCard { .. } => Some(1),
            HostRequest::Target {
                player_id, tile, ..
            } => {
                if tile >= 0 {
                    Some(tile)
                } else {
                    Some(player_id)
                }
            }
            // ---- G08 zone family: the writer, immediately ----
            HostRequest::Leave {
                uid,
                destination,
                to,
                cause,
            } => Some(w.zone_leave(uid, destination, to, cause)),
            HostRequest::Flip {
                uid,
                face_down,
                cause,
            } => Some(w.set_face_down_at(uid, face_down != 0, cause) as i32),
            HostRequest::Attach {
                host,
                card,
                cause,
                face_down,
            } => Some(w.attach_card(host, &card, cause, face_down != 0) as i32),
            HostRequest::Detach {
                uid,
                destination,
                cause,
            } => Some(w.detach_card(uid, destination, cause)),
            HostRequest::Rehome { tile, owner } => {
                w.set_tile_owner(tile, owner);
                w.events.push(format!("set_owner {tile} {owner}"));
                Some(1)
            }
            HostRequest::PlayHand { player_id, card } => {
                Some(w.play_hand_card(player_id, &card) as i32)
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
    pub charge: [i32; 4],
    pub stay: [i32; 4],
    pub tiles_owner: Vec<i32>,
    pub tiles_houses: Vec<i32>,
    pub tiles_mortgaged: Vec<bool>,
    pub dest: i32,
    pub trig_value: i32,
    pub trig_target: i32,
    /// Field rows (uid / owner / on / effective / face / tile / creator).
    pub field: Vec<ConfRow>,
    pub owned_seq: [i32; 60],
    pub first_tile: [i32; 4],
    pub trig_zone: Option<(i32, i32, i32, i32, bool, bool)>,
    pub hands: Vec<Vec<String>>,
    pub discards: Vec<Vec<String>>,
    pub decks: Vec<Vec<String>>,
    /// Normalized effect lines: `gain p n`, `pay p n`, `teleport p t`, ...
    pub effects: Vec<String>,
}

fn snap_real(w: &ConfWorld) -> Snap {
    Snap {
        n_players: w.n_players,
        money: w.money,
        pos: w.pos,
        out: w.out,
        charge: w.charge,
        stay: w.stay,
        tiles_owner: w.tiles_owner.to_vec(),
        tiles_houses: w.tiles_houses.to_vec(),
        tiles_mortgaged: w.tiles_mortgaged.to_vec(),
        dest: w.dest,
        trig_value: w.trig_value,
        trig_target: w.trig_target,
        field: w.field.clone(),
        owned_seq: w.owned_seq,
        first_tile: w.first_tile,
        trig_zone: w.trig_zone,
        // The model keeps one global pile; flatten the real seats the same
        // way so the two sides are comparable.
        hands: vec![w.hands.iter().flatten().cloned().collect()],
        discards: vec![w.discards.iter().flatten().cloned().collect()],
        decks: vec![w.decks.iter().flatten().cloned().collect()],
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
            charge: w.charge,
            stay: w.stay,
            tiles_owner: w.tiles.iter().map(|t| t.owner).collect(),
            tiles_houses: w.tiles.iter().map(|t| t.houses).collect(),
            tiles_mortgaged: w.tiles.iter().map(|t| t.mortgaged).collect(),
            dest: w.dest,
            trig_value: w.pay_amount,
            trig_target: w.trig_target,
            field: {
                let leave_key = rules_kani::intern::intern(rules_kani::world::PROP_LEAVE_DEST);
                let mut rows = Vec::new();
                for f in w.field.iter() {
                    if f.card == u16::MAX {
                        continue;
                    }
                    rows.push(ConfRow {
                        uid: f.uid,
                        card: rules_kani::intern::resolve_str(rules_kani::intern::NameId(f.card))
                            .to_string(),
                        player: f.player,
                        owner: f.owner,
                        tile: f.tile,
                        on: f.on,
                        created_by: f.created_by,
                        effective: f.effective,
                        face_down: f.face_down,
                        leave_dest: f.props.get_opt(leave_key).unwrap_or(-1),
                    });
                }
                rows
            },
            owned_seq: w.owned_seq,
            first_tile: w.first_tile,
            trig_zone: w.trig_zone_present.then_some((
                w.trig_zone_from,
                w.trig_zone_to,
                w.trig_zone_cause,
                w.trig_zone_holder,
                w.trig_zone_wasted,
                w.trig_zone_counted,
            )),
            // The model's piles are global; the diffs read seat 0.
            hands: vec![resolve_pile(&w.hand_pile, w.hand_n as usize)],
            discards: vec![resolve_pile(&w.discard_pile, w.discard_n as usize)],
            decks: vec![resolve_pile(&w.deck_pile, w.deck_n as usize)],
            effects,
        }
    })
}

fn resolve_pile(pile: &[u16; rules_kani::world::MAX_CARDS], n: usize) -> Vec<String> {
    pile[..n.min(pile.len())]
        .iter()
        .map(|&id| {
            if id == u16::MAX {
                // The model's generic drawn card; the engine names it the
                // same way in this test double.
                "<draw>".to_string()
            } else {
                rules_kani::intern::resolve_str(rules_kani::intern::NameId(id)).to_string()
            }
        })
        .collect()
}

fn assert_state_eq(label: &str, real: &Snap, model: &Snap) {
    let mut diffs = Vec::new();
    if real.n_players != model.n_players {
        diffs.push(format!(
            "n_players real={} model={}",
            real.n_players, model.n_players
        ));
    }
    if real.money != model.money {
        diffs.push(format!(
            "money real={:?} model={:?}",
            real.money, model.money
        ));
    }
    if real.pos != model.pos {
        diffs.push(format!("pos real={:?} model={:?}", real.pos, model.pos));
    }
    if real.out != model.out {
        diffs.push(format!("out real={:?} model={:?}", real.out, model.out));
    }
    if real.charge != model.charge {
        diffs.push(format!(
            "fire real={:?} model={:?}",
            real.charge, model.charge
        ));
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
    if real.field != model.field {
        diffs.push(format!(
            "field rows real={:?} model={:?}",
            real.field, model.field
        ));
    }
    if real.owned_seq != model.owned_seq {
        diffs.push("owned_seq diverged".into());
    }
    if real.first_tile != model.first_tile {
        diffs.push(format!(
            "first_tile real={:?} model={:?}",
            real.first_tile, model.first_tile
        ));
    }
    if real.hands != model.hands {
        diffs.push(format!(
            "hands real={:?} model={:?}",
            real.hands, model.hands
        ));
    }
    if real.discards != model.discards {
        diffs.push(format!(
            "discards real={:?} model={:?}",
            real.discards, model.discards
        ));
    }
    if real.decks != model.decks {
        diffs.push(format!(
            "decks real={:?} model={:?}",
            real.decks, model.decks
        ));
    }
    if real.trig_zone != model.trig_zone {
        diffs.push(format!(
            "trig_zone real={:?} model={:?}",
            real.trig_zone, model.trig_zone
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
    let money_pats = [
        "gain ",
        "pay ",
        "pay_pipeline",
        "GainMoney",
        "PayMoney",
        "PayTo",
    ];
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

fn base_conf_with_zone() -> ConfWorld {
    let mut w = base_conf();
    // 1 host (player 0, tile 3), 2 attached to 1, 3 created by 1.
    w.field = vec![
        ConfRow {
            uid: 1,
            card: zone_card(0).to_string(),
            player: 0,
            owner: 0,
            tile: 3,
            on: -1,
            created_by: -1,
            effective: true,
            face_down: false,
            leave_dest: -1,
        },
        ConfRow {
            uid: 2,
            card: zone_card(1).to_string(),
            player: 0,
            owner: 0,
            tile: 3,
            on: 1,
            created_by: -1,
            effective: true,
            face_down: false,
            leave_dest: -1,
        },
        ConfRow {
            uid: 3,
            card: zone_card(2).to_string(),
            player: 0,
            owner: 0,
            tile: 3,
            on: -1,
            created_by: 1,
            effective: true,
            face_down: false,
            leave_dest: -1,
        },
    ];
    w.next_uid = 4;
    w.hands[0].push(zone_card(1).to_string());
    w
}

fn base_model_with_zone() -> KWorld {
    let mut w = base_model();
    w.field[0] = zone_row(1, zone_card(0), -1, -1, -1, 3);
    w.field[1] = zone_row(2, zone_card(1), 1, -1, -1, 3);
    w.field[2] = zone_row(3, zone_card(2), -1, 1, -1, 3);
    w.field_n = 3;
    w.next_uid = 4;
    w.bound_uid = 1;
    w.hand_pile[0] = rules_kani::intern::intern(zone_card(1)).0;
    w.hand_n = 1;
    w
}

fn zone_card(i: usize) -> &'static str {
    ["PP:[G08]host", "PP:[G08]att", "PP:[G08]child"][i]
}

fn zone_row(
    uid: i32,
    card: &str,
    on: i32,
    created_by: i32,
    owner: i32,
    tile: i32,
) -> kworld::FieldCard {
    let mut f = kworld::FieldCard::EMPTY;
    f.uid = uid;
    f.card = rules_kani::intern::intern(card).0;
    f.player = 0;
    f.owner = if owner < 0 { 0 } else { owner };
    f.tile = tile;
    f.on = on;
    f.created_by = created_by;
    f
}

/// Run one op on both sides from an explicit pre-state and compare returns +
/// state.
fn diff_with(
    label: &str,
    conf: ConfWorld,
    model: KWorld,
    real: impl FnOnce(&mut ConfHost) -> i64,
    model_fn: impl FnOnce() -> i64,
) {
    let _g = model_lock();
    install_model_world_with(model, &[]);
    let model_ret = model_fn();
    let model_snap = snap_model();
    let mut conf_h = ConfHost::new(conf, Vec::new());
    let mut inline = ConfInline;
    let real_ret = with_inline_host(&mut inline, || real(&mut conf_h));
    let real_snap = snap_real(&conf_h.st().wr());
    if real_ret != model_ret {
        panic!(
            "MODEL-vs-ENGINE DIVERGENCE [{label}]: return real={real_ret} model={model_ret} (BUG IN THE MODEL unless engine)"
        );
    }
    assert_state_eq(label, &real_snap, &model_snap);
}

fn base_conf() -> ConfWorld {
    let mut w = ConfWorld::new();
    w.n_players = 3;
    w.money = [1500, 2000, 3000, 0];
    w.pos = [2, 10, 25, 0];
    w.out = [false, false, false, true];
    w.charge = [1, 0, 2, 0];
    w.charge_max = [3, 3, 3, 3];
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
    w.charge = [1, 0, 2, 0];
    w.charge_max = [3, 3, 3, 3];
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
fn diff_op(
    label: &str,
    answers: &[i32],
    real: impl FnOnce(&mut ConfHost) -> i64,
    model: Args,
    op: Op,
) {
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
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_gain_matches_engine() {
    big(|| {
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
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_pay_matches_engine() {
    big(|| {
        let _g = model_lock();
        let (mp, ml) = intern_msg("conf.pay");
        diff_op(
            "pay",
            &[],
            |c| game_rules::hostfns::pay(c, 1, 400, mp, ml).unwrap() as i64,
            [1, 400, mp, ml, 0, 0],
            Op::pay,
        );
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_teleport_to_matches_engine() {
    big(|| {
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
    });
}
/// `gain_charge` was ABI-88 genericized to `gain_marker` (name-as-data).
/// Same contract: clamped at `charge_max`, returns the amount gained.
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_gain_marker_matches_engine() {
    big(|| {
        let _g = model_lock();
        let (np, nl) = intern_str("charge");
        let (mp, ml) = intern_msg("conf.charge");
        diff_op(
            "gain_marker",
            &[],
            |c| game_rules::hostfns::gain_marker(c, 0, "charge", 2, mp, ml).unwrap() as i64,
            [0, np, nl, 2, mp, ml],
            Op::gain_marker,
        );
    });
}
/// `give_stay` was ABI-88 genericized to `apply_abnormal(player, kind=STAY, n, to)`.
/// Same contract: a stay layer appears and the grant is logged.
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_apply_abnormal_stay_matches_engine() {
    big(|| {
        let _g = model_lock();
        // kind 1 = stay (bandori_std::kind::abnormal::Stay wire).
        diff_op(
            "apply_abnormal_stay",
            &[],
            |c| {
                game_rules::hostfns::apply_abnormal(c, 1, 1, 2, -1).unwrap();
                0
            },
            [1, 1, 2, -1, 0, 0],
            Op::apply_abnormal,
        );
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_player_pos_money_player_count_player_out_tile_count_match() {
    big(|| {
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
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_tile_named_matches_engine() {
    big(|| {
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
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_set_houses_houses_of_match() {
    big(|| {
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
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_set_owner_tile_owner_match() {
    big(|| {
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
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_add_to_dest_take_card_cards_in_hand_size_match() {
    big(|| {
        let _g = model_lock();
        // Use a non-resonance card name so the model's pilot-compat `hand_size`
        // (bitset + pile hybrid) is exercised on an empty-start hand.
        let (cp, cl) = intern_str("R:[衍生] 压");
        diff_op_seq(
            "add_to_dest+take_card+cards_in",
            &[],
            |c| {
                game_rules::hostfns::add_to_dest(c, 0, cp, cl, 1, 0).unwrap();
                let ok = game_rules::hostfns::take_card(c, 0, cp, cl, 0).unwrap();
                let (buf, cap) = card_sdk::native::reserve(64);
                let _need = game_rules::hostfns::cards_in(c, 0, 0, buf, cap).unwrap();
                ok as i64
            },
            || {
                rules_kani::host::dispatch(Op::add_to_dest, [0, cp, cl, 1, 0, 0]);
                let ok = rules_kani::host::dispatch(Op::take_card, [0, cp, cl, 0, 0, 0]);
                let (buf, cap) = card_sdk::native::reserve(64);
                rules_kani::host::dispatch(Op::cards_in, [0, 0, buf, cap, 0, 0]);
                ok
            },
        );
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_to_discard_and_hand_size() {
    big(|| {
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
    });
}
// ---------------------------------------------------------------------------
// G11 inspection / reveal (design `spec/abi/G11-information-and-deck.md` §9)
// ---------------------------------------------------------------------------

/// Seed `owner`'s deck front-first on both sides, run the ops, and compare the
/// answers plus the deck and frame contents. `exact = false` compares the
/// multisets instead (a shuffle).
fn diff_inspection(
    label: &str,
    seed_front: &[&str],
    exact: bool,
    real: impl FnOnce(&mut ConfHost) -> i64,
    model: impl FnOnce() -> i64,
) {
    let _g = model_lock();
    install_model_world(&[]);
    let ids: Vec<u16> = seed_front
        .iter()
        .map(|c| card_sdk::intern::intern(c).0)
        .collect();
    kworld::with_world_mut(|w| {
        w.deck_n = 0;
        for (i, id) in ids.iter().enumerate() {
            w.deck_pile[i] = *id;
        }
        w.deck_n = ids.len() as u8;
    });
    let model_ret = model();
    let (mut model_deck, mut model_held) = model_piles();

    let mut conf = ConfHost::new(base_conf(), Vec::new());
    conf.st_mut().w().decks[1] = seed_front.iter().map(|c| c.to_string()).collect();
    let mut inline = ConfInline;
    let real_ret = with_inline_host(&mut inline, || real(&mut conf));
    let (mut real_deck, mut real_held) = real_piles(&conf);

    if exact {
        if real_deck != model_deck || real_held != model_held {
            panic!(
                "MODEL-vs-ENGINE DIVERGENCE [{label}]: deck real={real_deck:?} model={model_deck:?}; held real={real_held:?} model={model_held:?}"
            );
        }
    } else {
        real_deck.sort();
        model_deck.sort();
        real_held.sort();
        model_held.sort();
        if real_deck != model_deck || real_held != model_held {
            panic!(
                "MODEL-vs-ENGINE DIVERGENCE [{label}]: deck multiset real={real_deck:?} model={model_deck:?}; held real={real_held:?} model={model_held:?}"
            );
        }
    }
    if real_ret != model_ret {
        panic!(
            "MODEL-vs-ENGINE DIVERGENCE [{label}]: return real={real_ret} model={model_ret} (BUG IN THE MODEL unless engine)"
        );
    }
}

/// `(deck[1], frame[1])` from the real world.
fn real_piles(c: &ConfHost) -> (Vec<String>, Vec<String>) {
    let w = c.st().wr();
    (w.decks[1].clone(), w.inspection[1].clone())
}

/// `(deck[1], frame[1])` from the model world, as names.
fn model_piles() -> (Vec<String>, Vec<String>) {
    kworld::with_world(|w| {
        let name =
            |id: u16| card_sdk::intern::resolve_str(card_sdk::intern::NameId(id)).to_string();
        let deck = (0..w.deck_n as usize)
            .map(|i| name(w.deck_pile[i]))
            .collect();
        let held = (0..w.inspection_n as usize)
            .map(|i| name(w.inspection_pile[i]))
            .collect();
        (deck, held)
    })
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_inspection_begin_takes_the_top_first() {
    big(|| {
        let _g = model_lock();
        diff_inspection(
            "inspection_begin",
            &["A", "B", "C"],
            true,
            |c| game_rules::hostfns::inspection_begin(c, 0, 1, 2, 0).unwrap() as i64,
            || rules_kani::host::dispatch(Op::inspection_begin, [0, 1, 2, 0, 0, 0]),
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_inspection_begin_clamps_at_the_deck() {
    big(|| {
        let _g = model_lock();
        diff_inspection(
            "inspection_begin clamp",
            &["A", "B"],
            true,
            |c| game_rules::hostfns::inspection_begin(c, 0, 1, 9, 1).unwrap() as i64,
            || rules_kani::host::dispatch(Op::inspection_begin, [0, 1, 9, 1, 0, 0]),
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_inspection_end_returns_in_order() {
    big(|| {
        let _g = model_lock();
        diff_inspection(
            "inspection_end",
            &["A", "B", "C"],
            true,
            |c| {
                game_rules::hostfns::inspection_begin(c, 0, 1, 2, 0).unwrap();
                game_rules::hostfns::inspection_end(c, 1, 0).unwrap() as i64
            },
            || {
                rules_kani::host::dispatch(Op::inspection_begin, [0, 1, 2, 0, 0, 0]);
                rules_kani::host::dispatch(Op::inspection_end, [1, 0, 0, 0, 0, 0])
            },
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_inspection_end_shuffle_keeps_the_multiset() {
    big(|| {
        let _g = model_lock();
        diff_inspection(
            "inspection_end shuffle",
            &["A", "B", "C"],
            false,
            |c| {
                game_rules::hostfns::inspection_begin(c, 0, 1, 2, 0).unwrap();
                game_rules::hostfns::inspection_end(c, 1, 1).unwrap() as i64
            },
            || {
                rules_kani::host::dispatch(Op::inspection_begin, [0, 1, 2, 0, 0, 0]);
                rules_kani::host::dispatch(Op::inspection_end, [1, 1, 0, 0, 0, 0])
            },
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_to_discard_does_not_refill_while_held() {
    big(|| {
        let _g = model_lock();
        let (cp, cl) = intern_str("held.card");
        diff_inspection(
            "held to_discard",
            &["A"],
            true,
            |c| {
                game_rules::hostfns::inspection_begin(c, 0, 1, 1, 0).unwrap();
                game_rules::hostfns::to_discard(c, 1, cp, cl).unwrap();
                0
            },
            || {
                rules_kani::host::dispatch(Op::inspection_begin, [0, 1, 1, 0, 0, 0]);
                rules_kani::host::dispatch(Op::to_discard, [1, cp, cl, 0, 0, 0]);
                0
            },
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_reveal_touches_no_pile() {
    big(|| {
        let _g = model_lock();
        let (mp, ml) = intern_msg("conf.reveal");
        let (cp, cl) = intern_str("A");
        diff_inspection(
            "reveal",
            &["A", "B"],
            true,
            |c| {
                game_rules::hostfns::reveal_pile(c, 0, 2, mp, ml).unwrap();
                game_rules::hostfns::reveal_card(c, 0, cp, cl, mp, ml).unwrap();
                0
            },
            || {
                rules_kani::host::dispatch(Op::reveal_pile, [0, 2, mp, ml, 0, 0]);
                rules_kani::host::dispatch(Op::reveal_card, [0, cp, cl, mp, ml, 0]);
                0
            },
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn state_set_get_value_match() {
    big(|| {
        let _g = model_lock();
        let (kp, kl) = intern_str("conf.key");
        diff_op_seq(
            "state_set+state_get",
            &[],
            |c| {
                game_rules::hostfns::state_set(c, 1, kp, kl, 0, 9).unwrap();
                game_rules::hostfns::state_get(c, 1, kp, kl, 0).unwrap() as i64
            },
            || {
                rules_kani::host::dispatch(Op::state_set, [1, kp, kl, 0, 9, 0]);
                rules_kani::host::dispatch(Op::state_get, [1, kp, kl, 0, 0, 0])
            },
        );
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_count_held_set_held_name_match() {
    big(|| {
        let _g = model_lock();
        let (np, nl) = intern_str("P✽P粉丝(正)");
        diff_op_seq(
            "set_held_name+count_held_name",
            &[],
            |c| {
                game_rules::hostfns::set_held_name(c, np, nl, 2, 5, 0, -1).unwrap();
                game_rules::hostfns::count_held_name(c, np, nl, 2).unwrap() as i64
            },
            || {
                rules_kani::host::dispatch(Op::set_held_name, [np, nl, 2, 5, 0, 0]);
                rules_kani::host::dispatch(Op::count_held_name, [np, nl, 2, 0, 0, 0])
            },
        );
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_set_dest_place_card_at_match() {
    big(|| {
        let _g = model_lock();
        let (cp, cl) = intern_str("PP:[衍生]共鸣");
        let (mp, ml) = intern_msg("conf.place");
        diff_op_seq(
            "set_dest+place_card_at",
            &[],
            |c| {
                game_rules::hostfns::set_dest(c, 12, 3).unwrap();
                game_rules::hostfns::place_card_at(c, 0, cp, cl, mp, ml).unwrap() as i64
            },
            || {
                rules_kani::host::dispatch(Op::set_dest, [12, 0, 0, 0, 0, 0]);
                rules_kani::host::dispatch(Op::place_card_at, [0, cp, cl, mp, ml, 0])
            },
        );
        // The `counted` bit rides after the dest (position a4, not a3).
        diff_op(
            "add_to_dest counted",
            &[],
            |c| {
                game_rules::hostfns::add_to_dest(c, 0, cp, cl, 0, 1).unwrap();
                0
            },
            [0, cp, cl, 0, 1, 0],
            Op::add_to_dest,
        );
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_trig_set_pay_amount_trig_value_match() {
    big(|| {
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
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_ask_yes_fixed_answers() {
    big(|| {
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
    });
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
    let real_res = with_inline_host(&mut inline, || {
        catch_unwind(AssertUnwindSafe(|| run(id, 0)))
    });
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
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_band_ag_conforms() {
    big(|| {
        let _g = model_lock();
        // `AG:即使夕阳落山` -- play entry; ops modeled (gain/log style).
        // Falls back to whatever invoke finds; fail-closed if unknown.
        diff_card("ag", "AG:即使夕阳落山", &[], run_play);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_band_crychic_conforms() {
    big(|| {
        let _g = model_lock();
        diff_card("crychic", "CRYCHIC:（祥子）带领着大家", &[], run_play);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_band_general_conforms() {
    big(|| {
        let _g = model_lock();
        // `通用:10次招募（1回限定）` -- pay + draw.
        diff_card("general", "通用:10次招募（1回限定）", &[], run_play);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_band_hhw_conforms() {
    big(|| {
        let _g = model_lock();
        diff_card("hhw", "HHW:Happy, Lucky, Smile, Yeah！", &[], run_play);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_band_morfonica_conforms() {
    big(|| {
        let _g = model_lock();
        // `Mor:（Rui）正论恶魔` -- four `transfer` legs. Pre-seeded answers are
        // consumed by `pay` *instead of* moving money (see `request_or_pause`), so
        // leave answers empty and let ConfInline apply every Pay.
        diff_card("morfonica", "Mor:（Rui）正论恶魔", &[], run_play);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_band_mujica_conforms() {
    big(|| {
        let _g = model_lock();
        // `Mujica:心の雨` -- player_pos / others / tile_forward / roll /
        // apply_abnormal(stay/stun) / gain. Dice use the shared mid-value formula.
        diff_card("mujica", "Mujica:心の雨", &[], run_play);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_band_mygo_conforms() {
    big(|| {
        let _g = model_lock();
        // `MyGO:轮符雨` -- stay grant + before_turn_end. The real engine runs
        // the stay write through the abnormal [反击] gate; with empty answers
        // ConfInline answers 1 (allow) so both sides grant the stay. Pre-seeding
        // 0 would block the stay on the real side only (the model has no gate).
        diff_card("mygo", "MyGO:轮符雨", &[], run_play);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_band_pp_conforms() {
    big(|| {
        let _g = model_lock();
        diff_card("pp", "PP:有你与我在这里共度", &[], run_play);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_band_ppp_conforms() {
    big(|| {
        let _g = model_lock();
        // `PPP:[衍生]Pipopa` -- set_dest + gain + place_card.
        diff_card("ppp", "PPP:[衍生]Pipopa", &[], run_play);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_band_ras_conforms() {
    big(|| {
        let _g = model_lock();
        diff_card("ras", "RAS:（MASKING）CRUSH ON THE DRUM!!!", &[], run_play);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_band_roselia_conforms() {
    big(|| {
        let _g = model_lock();
        // `R:[衍生] 压` -- a pure `gain`.
        diff_card("roselia", "R:[衍生] 压", &[], run_play);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_band_sumimi_conforms() {
    big(|| {
        let _g = model_lock();
        // `Sumimi:兼顾偶像与乐队` -- state + money_of + gain.
        diff_card("sumimi", "Sumimi:兼顾偶像与乐队", &[], run_play);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_skill_conforms() {
    big(|| {
        let _g = model_lock();
        // `skill:千早爱音:重新开始` -- first Hook entry (declare_cap / fire pot).
        diff_card("skill", "skill:千早爱音:重新开始", &[], run_hook);
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn card_event_conforms() {
    big(|| {
        let _g = model_lock();
        // `event:很噜的感觉` -- ask_yes + teleport + pay. `ask_yes` is `ask==0`,
        // so answer **1** means "no teleport". With all-no the body is quiet
        // (tile_named("快餐店") is -1 on the model; unused when nobody teleports).
        diff_card("event", "event:很噜的感觉", &[1, 1, 1, 1], run_play);
    });
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
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_rent_of_matches_engine() {
    big(|| {
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
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_buy_price_matches_engine() {
    big(|| {
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
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_tile_at_set_tile_at_match_engine() {
    big(|| {
        let _g = model_lock();
        let mut conf = base_conf();
        conf.field.push(ConfRow {
            uid: 7,
            card: "PP:[标题偶像]".into(),
            player: 0,
            owner: 0,
            tile: 3,
            on: -1,
            created_by: -1,
            effective: true,
            face_down: false,
            leave_dest: -1,
        });
        let mut model = base_model();
        model.field[0] = kworld::FieldCard {
            uid: 7,
            card: 2,
            player: 0,
            owner: 0,
            tile: 3,
            face_down: false,
            immune: false,
            immune_except_owner: false,
            crystals: 0,
            props: kworld::NameMap::new(),
            effective_card: u16::MAX,
            on: -1,
            created_by: -1,
            effective: true,
            mine: false,
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
    });
}
fn trig_conf() -> ConfWorld {
    let mut w = base_conf();
    w.trig_main = true;
    w.trig_roll_source = 2;
    w.trig_pay_is_rent = true;
    w.move_present = true;
    w.move_kind = 0;
    w.move_settles = true;
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
    w.buy_deal_owner = Some(game_core::engine::rules::Payee::from_i32(2));
    w.buy_deal_houses = Some(1);
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
    ms.settles = true;
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
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_trig_move_payload_matches_engine() {
    let _g = model_lock();
    trig_get(
        "trig_move_kind",
        |c| game_rules::hostfns::trig_move_kind(c).unwrap() as i64,
        Op::trig_move_kind,
        [0; 6],
    );
    trig_get(
        "trig_move_settles",
        |c| game_rules::hostfns::trig_move_settles(c).unwrap() as i64,
        Op::trig_move_settles,
        [0; 6],
    );
    trig_get(
        "trig_move_main",
        |c| game_rules::hostfns::trig_move_main(c).unwrap() as i64,
        Op::trig_move_main,
        [0; 6],
    );
    trig_get(
        "trig_settle_redirect",
        |c| game_rules::hostfns::trig_settle_redirect(c).unwrap() as i64,
        Op::trig_settle_redirect,
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
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_trig_move_sentinels_match_engine() {
    let _g = model_lock();
    // No-move sentinels: kind=-1, roll=-1, dir=1, from=-1, remaining=0,
    // total=0, resolve=0; main is Trigger::main (independent of mv).
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with(
        "no-move kind",
        conf,
        model,
        |c| game_rules::hostfns::trig_move_kind(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_kind, [0; 6]),
    );
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with(
        "no-move dir",
        conf,
        model,
        |c| game_rules::hostfns::trig_move_dir(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_dir, [0; 6]),
    );
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with(
        "no-move from",
        conf,
        model,
        |c| game_rules::hostfns::trig_move_from(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_from, [0; 6]),
    );
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with(
        "no-move roll",
        conf,
        model,
        |c| game_rules::hostfns::trig_move_roll(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_roll, [0; 6]),
    );
    let mut conf = base_conf();
    conf.trig_main = true;
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with(
        "no-move is_main_move",
        conf,
        model,
        |c| game_rules::hostfns::trig_move_main(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_main, [0; 6]),
    );
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with(
        "no-move remaining",
        conf,
        model,
        |c| game_rules::hostfns::trig_move_remaining(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_remaining, [0; 6]),
    );
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with(
        "no-move total",
        conf,
        model,
        |c| game_rules::hostfns::trig_move_total(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_total, [0; 6]),
    );
    let conf = base_conf();
    let mut model = base_model();
    model.trig_main = true;
    diff_get_with(
        "no-move resolve",
        conf,
        model,
        |c| game_rules::hostfns::trig_move_settles(c).unwrap() as i64,
        || rules_kani::host::dispatch(Op::trig_move_settles, [0; 6]),
    );
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_trig_buy_deal_payload_matches_engine() {
    big(|| {
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
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_trig_buy_setters_match_engine() {
    big(|| {
        let _g = model_lock();
        let mut conf = trig_conf();
        conf.buy_price = 100;
        conf.buy_deal_owner = None;
        conf.buy_deal_houses = None;
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
            panic!("MODEL-vs-ENGINE DIVERGENCE [trig setters]: real={real:?} model={model:?}");
        }
    });
}
// ---------------------------------------------------------------------------
// G02 pay pipeline ops
// ---------------------------------------------------------------------------

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_trig_set_pay_kind_and_shape() {
    big(|| {
        diff_op_seq(
            "trig_set_pay_kind+trig_set_pay_shape",
            &[],
            |c| {
                game_rules::hostfns::trig_set_pay_kind(c, 3).unwrap();
                game_rules::hostfns::trig_set_pay_shape(c, 1).unwrap() as i64
            },
            || {
                rules_kani::host::dispatch(Op::trig_set_pay_kind, [3, 0, 0, 0, 0, 0]);
                rules_kani::host::dispatch(Op::trig_set_pay_shape, [1, 0, 0, 0, 0, 0])
            },
        );
        diff_op_seq(
            "trig_set_pay_shape normal",
            &[],
            |c| game_rules::hostfns::trig_set_pay_shape(c, 0).unwrap() as i64,
            || rules_kani::host::dispatch(Op::trig_set_pay_shape, [0, 0, 0, 0, 0, 0]),
        );
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_multiply_pay_factor_scopes() {
    big(|| {
        let conf = base_conf();
        let mut model = base_model();
        // `thisMove` open: a `ThisMove` multiply must land on the plan, and the
        // getter composes plan x settle.
        let mut conf = conf;
        conf.move_open = true;
        model.move_open = true;
        diff_get_with(
            "multiply_pay_factor thisMove",
            conf,
            model,
            |c| {
                game_rules::hostfns::multiply_pay_factor(c, 500, 1).unwrap();
                game_rules::hostfns::pay_factor(c).unwrap() as i64
            },
            || {
                rules_kani::host::dispatch(Op::multiply_pay_factor, [500, 1, 0, 0, 0, 0]);
                rules_kani::host::dispatch(Op::pay_factor, [0; 6])
            },
        );
        // `thisMove` without a move falls forward to `nextMove` (getter ignores it).
        diff_get_with(
            "multiply_pay_factor nextMove fallback",
            base_conf(),
            base_model(),
            |c| {
                game_rules::hostfns::multiply_pay_factor(c, 500, 1).unwrap();
                game_rules::hostfns::pay_factor(c).unwrap() as i64
            },
            || {
                rules_kani::host::dispatch(Op::multiply_pay_factor, [500, 1, 0, 0, 0, 0]);
                rules_kani::host::dispatch(Op::pay_factor, [0; 6])
            },
        );
        // `thisSettle` without a settle falls forward to `nextSettle`.
        diff_get_with(
            "multiply_pay_factor nextSettle fallback",
            base_conf(),
            base_model(),
            |c| {
                game_rules::hostfns::multiply_pay_factor(c, 500, 3).unwrap();
                game_rules::hostfns::pay_factor(c).unwrap() as i64
            },
            || {
                rules_kani::host::dispatch(Op::multiply_pay_factor, [500, 3, 0, 0, 0, 0]);
                rules_kani::host::dispatch(Op::pay_factor, [0; 6])
            },
        );
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_set_pay_factor_auto() {
    big(|| {
        let mut conf = base_conf();
        let mut model = base_model();
        conf.settle_open = 1;
        conf.settle_pay_factor = 800;
        model.settle_open = 1;
        model.settle_pay_factor = 800;
        diff_get_with(
            "set_pay_factor in settle",
            conf,
            model,
            |c| {
                game_rules::hostfns::set_pay_factor(c, 400).unwrap();
                game_rules::hostfns::pay_factor(c).unwrap() as i64
            },
            || {
                rules_kani::host::dispatch(Op::set_pay_factor, [400, 0, 0, 0, 0, 0]);
                rules_kani::host::dispatch(Op::pay_factor, [0; 6])
            },
        );
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_settle_readers() {
    big(|| {
        let mut conf = base_conf();
        let mut model = base_model();
        conf.trig_settle_tile = 2;
        conf.trig_settle_outcome = 1;
        model.trig_settle_tile = 2;
        model.trig_settle_outcome = 1;
        diff_get_with(
            "settle_tile",
            conf,
            model,
            |c| game_rules::hostfns::settle_tile(c).unwrap() as i64,
            || rules_kani::host::dispatch(Op::settle_tile, [0; 6]),
        );
        let mut conf = base_conf();
        let mut model = base_model();
        conf.trig_settle_outcome = 1;
        model.trig_settle_outcome = 1;
        diff_get_with(
            "settle_outcome",
            conf,
            model,
            |c| game_rules::hostfns::settle_outcome(c).unwrap() as i64,
            || rules_kani::host::dispatch(Op::settle_outcome, [0; 6]),
        );
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_paid_in_settle_by_and_rent_quote() {
    big(|| {
        let mut conf = base_conf();
        let mut model = base_model();
        conf.paid_in_settle_by = [10, 20, 30, 40];
        model.paid_in_settle_by = [10, 20, 30, 40];
        diff_get_with(
            "paid_in_settle_by",
            conf,
            model,
            |c| game_rules::hostfns::paid_in_settle_by(c, 2).unwrap() as i64,
            || rules_kani::host::dispatch(Op::paid_in_settle_by, [2, 0, 0, 0, 0, 0]),
        );
        let mut conf = base_conf();
        let mut model = base_model();
        conf.rent_quote = 640;
        model.rent_quote = 640;
        diff_get_with(
            "rent_quote",
            conf,
            model,
            |c| game_rules::hostfns::rent_quote(c, 0, 3).unwrap() as i64,
            || rules_kani::host::dispatch(Op::rent_quote, [0, 3, 0, 0, 0, 0]),
        );
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_pay_command_moves_money() {
    big(|| {
        let (mp, ml) = intern_msg("conf.pay_command");
        // spec: shape 0 (normal), kind 1 (rent), no flags, no tile.
        let spec = 0 | (1 << 4);
        diff_op(
            "pay_command",
            &[],
            |c| game_rules::hostfns::pay_command(c, 0, 1, 300, spec, mp, ml).unwrap() as i64,
            [0, 1, 300, spec, mp, ml],
            Op::pay_command,
        );
        // A fixed leg with a card kind to seat 2, tile index 5 (stored +1).
        let spec = 1 | (0 << 4) | (4 << 2) | ((5 + 1) << 12);
        diff_op(
            "pay_command fixed",
            &[],
            |c| game_rules::hostfns::pay_command(c, 0, 2, 120, spec, mp, ml).unwrap() as i64,
            [0, 2, 120, spec, mp, ml],
            Op::pay_command,
        );
    });
}
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_commit_redeem_folds_spend_before() {
    big(|| {
        // No write: the asked amount is the cost.
        diff_op(
            "commit_redeem plain",
            &[],
            |c| game_rules::hostfns::commit_redeem(c, 0, 1, 400).unwrap() as i64,
            [0, 1, 400, 0, 0, 0],
            Op::commit_redeem,
        );
        // A written cost (a `spendBefore` reduction) wins.
        let mut conf = base_conf();
        conf.pay_amount = 90;
        conf.pay_amount_set = true;
        let mut model = base_model();
        model.pay_amount = 90;
        model.pay_amount_set = true;
        diff_get_with(
            "commit_redeem folded",
            conf,
            model,
            |c| game_rules::hostfns::commit_redeem(c, 0, 1, 400).unwrap() as i64,
            || rules_kani::host::dispatch(Op::commit_redeem, [0, 1, 400, 0, 0, 0]),
        );
    });
}
/// Not an assertion, a signpost: both worlds are large (the model's 60-tile
/// `NameMap`s dominate), so every test body runs on [`big`]'s wide stack.
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn world_size_needs_the_big_stack() {
    big(|| {
        println!(
            "KWorld={} ConfWorld={}",
            std::mem::size_of::<KWorld>(),
            std::mem::size_of::<ConfWorld>()
        );
    });
}

// ---------------------------------------------------------------------------
// G08 zone family (v114)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_zone_family_reads() {
    big(|| {
        let _g = model_lock();
        diff_with(
            "zone_of attached",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::zone_of(c, 2).unwrap() as i64,
            || rules_kani::host::dispatch(Op::zone_of, [2, 0, 0, 0, 0, 0]),
        );
        diff_with(
            "zone_of dead",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::zone_of(c, 99).unwrap() as i64,
            || rules_kani::host::dispatch(Op::zone_of, [99, 0, 0, 0, 0, 0]),
        );
        diff_with(
            "attachment_host",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::attachment_host(c, 2).unwrap() as i64,
            || rules_kani::host::dispatch(Op::attachment_host, [2, 0, 0, 0, 0, 0]),
        );
        diff_with(
            "attached_cards",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::attached_cards(c, 1, 0, 0).unwrap() as i64,
            || rules_kani::host::dispatch(Op::attached_cards, [1, 0, 0, 0, 0, 0]),
        );
        diff_with(
            "created_cards",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::created_cards(c, 1, 0, 0).unwrap() as i64,
            || rules_kani::host::dispatch(Op::created_cards, [1, 0, 0, 0, 0, 0]),
        );
        diff_with(
            "instance_owner",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::instance_owner(c, 2).unwrap() as i64,
            || rules_kani::host::dispatch(Op::instance_owner, [2, 0, 0, 0, 0, 0]),
        );
    });
}

/// `field_instances` is the postcard list on the real side and the compact
/// `[uid, name]` pair list under Kani, so compare the *content*, not the
/// return value (a byte length vs a count).
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_field_instances_skips_attached() {
    big(|| {
        let _g = model_lock();
        let mut conf_h = ConfHost::new(base_conf_with_zone(), Vec::new());
        let mut inline = ConfInline;
        let real = with_inline_host(&mut inline, || conf_h.st().wr().field_instances(0));
        let real_uids: Vec<i32> = real.iter().map(|(uid, _)| *uid).collect();
        assert_eq!(real_uids, vec![1, 3], "attached row 2 is not listed");

        install_model_world_with(base_model_with_zone(), &[]);
        let mut out = [0i32; 8];
        let n = kworld::with_world(|w| w.field_instances_into(0, &mut out));
        assert_eq!(&out[..n], &[1, 3], "the model lists the same uids");
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_zone_family_writes() {
    big(|| {
        let _g = model_lock();
        diff_with(
            "set_instance_owner",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::set_instance_owner(c, 2, 1).unwrap() as i64,
            || rules_kani::host::dispatch(Op::set_instance_owner, [2, 1, 0, 0, 0, 0]),
        );
        diff_with(
            "set_created_by_at",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::set_created_by_at(c, 2, 3).unwrap() as i64,
            || rules_kani::host::dispatch(Op::set_created_by_at, [2, 3, 0, 0, 0, 0]),
        );
        diff_with(
            "set_ineffective_at + is_ineffective_at",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| {
                game_rules::hostfns::set_ineffective_at(c, 2, 1).unwrap();
                game_rules::hostfns::is_ineffective_at(c, 2).unwrap() as i64
            },
            || {
                rules_kani::host::dispatch(Op::set_ineffective_at, [2, 1, 0, 0, 0, 0]);
                rules_kani::host::dispatch(Op::is_ineffective_at, [2, 0, 0, 0, 0, 0])
            },
        );
        diff_with(
            "set_ineffective_at dead",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::set_ineffective_at(c, 99, 1).unwrap() as i64,
            || rules_kani::host::dispatch(Op::set_ineffective_at, [99, 1, 0, 0, 0, 0]),
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_set_face_down_at_and_self() {
    big(|| {
        let _g = model_lock();
        diff_with(
            "set_face_down_at live",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::set_face_down_at(c, 2, 1, 14).unwrap() as i64,
            || rules_kani::host::dispatch(Op::set_face_down_at, [2, 1, 14, 0, 0, 0]),
        );
        diff_with(
            "set_face_down_at dead",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::set_face_down_at(c, 99, 1, 14).unwrap() as i64,
            || rules_kani::host::dispatch(Op::set_face_down_at, [99, 1, 14, 0, 0, 0]),
        );
        diff_with(
            "set_self_face_down",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::set_self_face_down(c, 1, 14).unwrap() as i64,
            || rules_kani::host::dispatch(Op::set_self_face_down, [1, 14, 0, 0, 0, 0]),
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_attach_and_detach_card() {
    big(|| {
        let _g = model_lock();
        let (cp, cl) = intern_str(zone_card(1));
        let (mp, ml) = intern_str("PP:[G08]missing");
        diff_with(
            "attach_card",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::attach_card(c, 1, cp, cl, 14, 1).unwrap() as i64,
            || rules_kani::host::dispatch(Op::attach_card, [1, cp, cl, 14, 1, 0]),
        );
        diff_with(
            "attach_card dead host",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::attach_card(c, 99, cp, cl, 14, 0).unwrap() as i64,
            || rules_kani::host::dispatch(Op::attach_card, [99, cp, cl, 14, 0, 0]),
        );
        diff_with(
            "attach_card missing copy",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::attach_card(c, 1, mp, ml, 14, 0).unwrap() as i64,
            || rules_kani::host::dispatch(Op::attach_card, [1, mp, ml, 14, 0, 0]),
        );
        // No hand copy: the caller's live instance is re-targeted.
        {
            let (hp, hl) = intern_str(zone_card(0));
            let mut conf = base_conf_with_zone();
            conf.hands[0].clear();
            conf.next_uid = 4;
            conf.field.push(ConfRow {
                uid: 4,
                card: zone_card(0).to_string(),
                player: 0,
                owner: 0,
                tile: 3,
                on: -1,
                created_by: -1,
                effective: true,
                face_down: false,
                leave_dest: -1,
            });
            let mut model = base_model_with_zone();
            model.hand_n = 0;
            model.next_uid = 4;
            model.field[3] = zone_row(4, zone_card(0), -1, -1, 0, 3);
            model.field_n = 4;
            diff_with(
                "attach_card live fallback",
                conf,
                model,
                |c| game_rules::hostfns::attach_card(c, 1, hp, hl, 14, 1).unwrap() as i64,
                || rules_kani::host::dispatch(Op::attach_card, [1, hp, hl, 14, 1, 0]),
            );
        }
        // Live fallback onto a descendant is a cycle.
        {
            let (hp, hl) = intern_str(zone_card(0));
            diff_with(
                "attach_card cycle refused",
                base_conf_with_zone(),
                base_model_with_zone(),
                |c| game_rules::hostfns::attach_card(c, 2, hp, hl, 14, 0).unwrap() as i64,
                || rules_kani::host::dispatch(Op::attach_card, [2, hp, hl, 14, 0, 0]),
            );
        }
        diff_with(
            "detach_card to hand",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::detach_card(c, 2, 1, 15).unwrap() as i64,
            || rules_kani::host::dispatch(Op::detach_card, [2, 1, 15, 0, 0, 0]),
        );
        diff_with(
            "detach_card graveyard",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::detach_card(c, 2, 0, 15).unwrap() as i64,
            || rules_kani::host::dispatch(Op::detach_card, [2, 0, 15, 0, 0, 0]),
        );
        diff_with(
            "detach_card not attached",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::detach_card(c, 1, 1, 15).unwrap() as i64,
            || rules_kani::host::dispatch(Op::detach_card, [1, 1, 15, 0, 0, 0]),
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_unplace_at_cascades() {
    big(|| {
        let _g = model_lock();
        diff_with(
            "unplace_at cascade",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::unplace_at(c, 1, 0, 3).unwrap() as i64,
            || rules_kani::host::dispatch(Op::unplace_at, [1, 0, 3, 0, 0, 0]),
        );
        diff_with(
            "unplace_at dead",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::unplace_at(c, 99, 0, 3).unwrap() as i64,
            || rules_kani::host::dispatch(Op::unplace_at, [99, 0, 3, 0, 0, 0]),
        );
        diff_with(
            "unplace_card self",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::unplace_card(c).unwrap() as i64,
            || rules_kani::host::dispatch(Op::unplace_card, [0, 0, 0, 0, 0, 0]),
        );
        diff_with(
            "send_to_dest",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::send_to_dest(c, 0, 3).unwrap() as i64,
            || rules_kani::host::dispatch(Op::send_to_dest, [0, 3, 0, 0, 0, 0]),
        );
        diff_with(
            "transfer_to_dest",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::transfer_to_dest(c, 1, 0, 3).unwrap() as i64,
            || rules_kani::host::dispatch(Op::transfer_to_dest, [1, 0, 3, 0, 0, 0]),
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_earliest_owned_and_rehome() {
    big(|| {
        let _g = model_lock();
        let mut conf = base_conf_with_zone();
        conf.owned_seq[1] = 0;
        conf.owned_seq[5] = 1;
        conf.next_owned_seq = 2;
        conf.first_tile[0] = 1;
        conf.first_tile[1] = 5;
        let mut model = base_model_with_zone();
        model.owned_seq[1] = 0;
        model.owned_seq[5] = 1;
        model.next_owned_seq = 2;
        model.first_tile[0] = 1;
        model.first_tile[1] = 5;
        diff_with(
            "earliest_owned_tile",
            conf.clone(),
            model.clone(),
            |c| game_rules::hostfns::earliest_owned_tile(c, 0).unwrap() as i64,
            || rules_kani::host::dispatch(Op::earliest_owned_tile, [0, 0, 0, 0, 0, 0]),
        );
        diff_with(
            "earliest_owned_tile none",
            conf.clone(),
            model.clone(),
            |c| game_rules::hostfns::earliest_owned_tile(c, 2).unwrap() as i64,
            || rules_kani::host::dispatch(Op::earliest_owned_tile, [2, 0, 0, 0, 0, 0]),
        );
        diff_with(
            "rehome_card",
            conf,
            model,
            |c| game_rules::hostfns::rehome_card(c, 1, 1).unwrap() as i64,
            || rules_kani::host::dispatch(Op::rehome_card, [1, 1, 0, 0, 0, 0]),
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_play_hand_card() {
    big(|| {
        let _g = model_lock();
        let (cp, cl) = intern_str(zone_card(1));
        let (mp, ml) = intern_str("PP:[G08]missing");
        diff_with(
            "play_hand_card",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::play_hand_card(c, 0, cp, cl).unwrap() as i64,
            || rules_kani::host::dispatch(Op::play_hand_card, [0, cp, cl, 0, 0, 0]),
        );
        diff_with(
            "play_hand_card missing",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| game_rules::hostfns::play_hand_card(c, 0, mp, ml).unwrap() as i64,
            || rules_kani::host::dispatch(Op::play_hand_card, [0, mp, ml, 0, 0, 0]),
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_set_dest_unwatched() {
    big(|| {
        let _g = model_lock();
        diff_with(
            "set_dest_unwatched",
            base_conf_with_zone(),
            base_model_with_zone(),
            |c| {
                game_rules::hostfns::set_dest_unwatched(c, 2).unwrap();
                0
            },
            || rules_kani::host::dispatch(Op::set_dest_unwatched, [2, 0, 0, 0, 0, 0]),
        );
    });
}

#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn op_trigger_zone_payload() {
    big(|| {
        let _g = model_lock();
        let mut conf = base_conf_with_zone();
        conf.trig_zone = Some((9, 4, 8, 0, true, false));
        let mut model = base_model_with_zone();
        model.trig_zone_present = true;
        model.trig_zone_from = 9;
        model.trig_zone_to = 4;
        model.trig_zone_cause = 8;
        model.trig_zone_holder = 0;
        model.trig_zone_wasted = true;
        model.trig_zone_counted = false;
        let model_bytes = std::cell::RefCell::new(Vec::new());
        diff_with(
            "trigger_zone",
            conf,
            model,
            |c| {
                let (buf, cap) = card_sdk::native::reserve(32);
                let n = game_rules::hostfns::trigger_zone(c, buf, cap).unwrap();
                let bytes = card_sdk::native::read(buf, n.max(0) as usize);
                assert_eq!(bytes, *model_bytes.borrow(), "payload bytes");
                n as i64
            },
            || {
                let (buf, cap) = card_sdk::native::reserve(32);
                let n = rules_kani::host::dispatch(Op::trigger_zone, [buf, cap, 0, 0, 0, 0]);
                *model_bytes.borrow_mut() = card_sdk::native::read(buf, n.max(0) as usize);
                n
            },
        );
    });
}
