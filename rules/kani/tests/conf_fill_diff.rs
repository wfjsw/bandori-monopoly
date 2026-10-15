//! Guard→world fill differential (trusted-base control).
//!
//! One concrete [`Fixture`] feeds **two fillers** and **two evaluators**:
//!
//! | path | fill | eval |
//! |---|---|---|
//! | (A) production | `game_rules::cond_pre::{fill_window, fill_candidate}` over [`SnapSrc`] | `CompiledPre::compile` + `WindowScope::eval` (error ⇒ false) |
//! | (B) Kani IR | `rules_kani::ir_fill::fill_from_world` (+ geometry bins) | `rules_kani::generated::cond_irs::eval_id` |
//!
//! Asserts (A) == (B) for every one of the 139 generated guards on every
//! fixture, plus `guard_id!` / `BY_ID` id stability.
//!
//! ## What this pins
//!
//! `fill_window`/`fill_candidate` **field semantics** against `ir_fill` on a
//! shared fixture — not the live engine host (that is `conf_model.rs`). Both
//! fillers read the same fixture numbers; any disagreement is a binding bug
//! in `ir_fill` or in the production snapshot fill.
//!
//! ## Known IR gaps the fixture keeps neutral (both sides agree on 0 / false)
//!
//! * `gains_this_turn` / `targeted_count` — `World` has no trackers; fixtures
//!   leave them 0 (see `ir_fill` module docs).
//! * bin geometry — `ir_fill` leaves `IrCtx::bin` empty (answers 0). This
//!   control supplies the same geometry production's `CondView` computes from
//!   the shared fixture (positions / tile owners / move span) before eval, so
//!   `players_on` / `next_dist` / `owned_within` / `on_path` see equal values.
//!   Fixtures also keep those clauses mostly on the 0 side so a missing bin
//!   would fail closed rather than silently pass.
//! * `between` is not used by any inventory guard source.

use game_core::engine::rules::{Dir, Effect, RollSource};
use game_rules::cond_pre::{
    fill_candidate, fill_window, window_scope, CompiledPre, SnapSrc, WindowCtx,
};
use game_rules::{MoveKind, Trigger, TriggerKind, TriggerMove, TriggerPay};

use rules_cond_ir::ctx::{FnId, IrCtx};
use rules_cond_ir::eval::eval_node;
use rules_cond_ir::nodes::Kind;
use rules_cond_ir::Val;
use rules_kani::generated::cond_irs::{self, eval_id, BY_ID, GUARD_COUNT};
use rules_kani::ir_fill::{self, TriggerOverlay, TILE_NAMES};
use rules_kani::intern;
use rules_kani::world::{self as kworld, World as KWorld};

// ---------------------------------------------------------------------------
// Fixture: one concrete world both fillers read
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Seat {
    money: i32,
    charge: i32,
    crystals: i32,
    hand: i32,
    pos: i32,
    out: bool,
    stay: i32,
    stun: i32,
    /// Pending `stunStart` layer (starts counting next turn).
    stun_start: i32,
    exile: i32,
    no_hand: i32,
    /// `[不可阻挡]` layers.
    unstoppable: i32,
    persona: &'static str,
    group: &'static str,
    /// Non-zero named tokens (`tok('…')`).
    tokens: Vec<(&'static str, i32)>,
    /// Non-zero latches (`slot('…')`) — a subset of `SLOT_NAMES`.
    slots: Vec<(&'static str, i32)>,
    /// `gains_this_turn` / `targeted_count` stay 0 (ir_fill gap; see module docs).
    #[allow(dead_code)]
    gains: i32,
    #[allow(dead_code)]
    targeted: i32,
}

impl Default for Seat {
    fn default() -> Self {
        Self {
            money: 1500,
            charge: 0,
            crystals: 0,
            hand: 3,
            pos: 0,
            out: false,
            stay: 0,
            stun: 0,
            stun_start: 0,
            exile: 0,
            no_hand: 0,
            unstoppable: 0,
            persona: "",
            group: "",
            tokens: Vec::new(),
            slots: Vec::new(),
            gains: 0,
            targeted: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct TileFx {
    owner: i32,
    houses: i32,
    mortgaged: bool,
}

impl Default for TileFx {
    fn default() -> Self {
        Self {
            owner: -1,
            houses: 0,
            mortgaged: false,
        }
    }
}

#[derive(Clone, Debug)]
struct MoveFx {
    /// `None` = no move payload; `Some(roll)` with `roll < 0` = pre-cast null face.
    roll: Option<i64>,
    kind: Option<i64>,
    remaining: i64,
    /// `Trigger.main` (the turn's [主要移动]) — not `TriggerMove.main`.
    main: bool,
    dir: i64,
}

#[derive(Clone, Debug)]
struct Trig {
    kind: TriggerKind,
    actor: i32,
    target: i32,
    tile: i32,
    value: i64,
    step: i64,
    /// `-1` = no by-card.
    by: i64,
    pay_is_rent: bool,
    roll_source: i64,
    trigger_card: &'static str,
    counter_name: &'static str,
    /// Negative = null.
    plan_fixed_roll: i64,
    mv: Option<MoveFx>,
    /// `(kind, from, hits)` — `kind` is the `TriggerKind` wire value.
    chain: Vec<(i64, i64, i64)>,
    turn_player: i32,
    turn_key: i32,
}

impl Default for Trig {
    fn default() -> Self {
        Self {
            kind: TriggerKind::Pay,
            actor: 0,
            target: 1,
            tile: 0,
            value: 100,
            step: 0,
            by: -1,
            pay_is_rent: false,
            roll_source: -1,
            trigger_card: "",
            counter_name: "",
            plan_fixed_roll: -1,
            mv: None,
            chain: Vec::new(),
            turn_player: 0,
            turn_key: 1,
        }
    }
}

#[derive(Clone, Debug)]
struct Fixture {
    name: &'static str,
    n_players: i32,
    seats: Vec<Seat>,
    tiles: Vec<TileFx>,
    trig: Trig,
    /// Candidate card name (`card.id` / `card.placed`).
    card: &'static str,
    card_placed: bool,
    /// `-2` = not placed, `-1` = with owner, else the field tile.
    card_tile: i64,
    card_counters: Vec<(&'static str, i64)>,
}

impl Fixture {
    fn new(name: &'static str, n_players: i32) -> Self {
        let mut seats = Vec::new();
        for i in 0..n_players {
            let mut s = Seat::default();
            // Distinct positions so `players_on` stays 0 (geometry-safe) while
            // `neighbor` still sees a full ring of seats.
            s.pos = i * 10;
            seats.push(s);
        }
        Self {
            name,
            n_players,
            seats,
            tiles: vec![TileFx::default(); 60],
            trig: Trig::default(),
            card: "PP:[丸山彩]憧憬的前方",
            card_placed: true,
            card_tile: -1,
            card_counters: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// (A) production: SnapSrc over the fixture
// ---------------------------------------------------------------------------

fn board_tile_name(tile: i32) -> String {
    TILE_NAMES
        .iter()
        .find(|(id, _)| *id == tile)
        .map(|(_, n)| n.to_string())
        .unwrap_or_default()
}

fn board_tile_named(name: &str) -> i32 {
    TILE_NAMES
        .iter()
        .find(|(_, n)| *n == name)
        .map(|(id, _)| *id)
        .unwrap_or(-1)
}

fn board_is_start(tile: i32) -> bool {
    matches!(rules_kani::board::TILES.get(tile as usize), Some(t) if t.kind == 0)
}
fn board_is_deed(tile: i32) -> bool {
    matches!(rules_kani::board::TILES.get(tile as usize), Some(t) if t.kind == 3)
}
fn board_is_venue(tile: i32) -> bool {
    matches!(rules_kani::board::TILES.get(tile as usize), Some(t) if (t.kind == 1 || t.kind == 3) && t.group == 6)
}
fn board_is_buyable(tile: i32) -> bool {
    matches!(rules_kani::board::TILES.get(tile as usize), Some(t) if t.kind == 1 || t.kind == 3)
}
fn board_price(tile: i32) -> i32 {
    rules_kani::board::TILES
        .get(tile as usize)
        .map(|t| t.price)
        .unwrap_or(0)
}

/// Chain kind wire values for the `Effect.kind` strings `fill_window` maps
/// through `TriggerKind::from_str`.
fn chain_kind_str(kind: i64) -> &'static str {
    match kind {
        x if x == TriggerKind::Pay.as_i32() as i64 => "pay",
        x if x == TriggerKind::Target.as_i32() as i64 => "target",
        x if x == TriggerKind::Abnormal.as_i32() as i64 => "abnormal",
        x if x == TriggerKind::Card.as_i32() as i64 => "card",
        x if x == TriggerKind::Settle.as_i32() as i64 => "settle",
        _ => "pay",
    }
}

impl SnapSrc for Fixture {
    fn trigger(&self) -> Trigger {
        let t = &self.trig;
        let mut tr = Trigger {
            kind: t.kind,
            player_id: t.actor,
            target: t.target,
            tile: t.tile,
            value: t.value as i32,
            step: t.step as i32,
            by_card: if t.by >= 0 { Some(t.by as i32) } else { None },
            main: t.mv.as_ref().map(|m| m.main).unwrap_or(false),
            pay: if t.pay_is_rent {
                Some(TriggerPay { is_rent: true })
            } else if matches!(t.kind, TriggerKind::Pay | TriggerKind::Paid) {
                Some(TriggerPay { is_rent: false })
            } else {
                None
            },
            roll_source: match t.roll_source {
                1 => RollSource::Charge,
                2 => RollSource::Card,
                3 => RollSource::Skill,
                _ => RollSource::Unattributed,
            },
            card: if t.trigger_card.is_empty() {
                None
            } else {
                Some(t.trigger_card.to_string())
            },
            name: if t.counter_name.is_empty() {
                None
            } else {
                Some(t.counter_name.to_string())
            },
            ..Trigger::default()
        };
        // `Trigger.main` is the turn's [主要移动], independent of the move payload.
        tr.main = t.mv.as_ref().map(|m| m.main).unwrap_or(false);
        if let Some(m) = &t.mv {
            tr.mv = Some(TriggerMove {
                kind: if m.kind == Some(MoveKind::Teleport as i64) {
                    MoveKind::Teleport
                } else {
                    MoveKind::Walk
                },
                resolve: true,
                tags: Vec::new(),
                main: m.main,
                dir: if m.dir < 0 { Dir::Backward } else { Dir::Forward },
                from: 0,
                remaining: m.remaining as i32,
                total: m.remaining as i32,
                roll: m.roll.map(|r| r as i32),
            });
        }
        tr.effects = t
            .chain
            .iter()
            .map(|&(kind, from, hits)| Effect {
                kind: chain_kind_str(kind),
                target: hits as i32,
                from: from as i32,
                tile: -1,
                value: 0,
            })
            .collect();
        // `plan.fixed_roll` lives on the live host, not `Trigger`; production
        // `fill_window` reads it via `SnapSrc::fixed_roll` (below).
        tr
    }

    fn tile_named(&self, name: &str) -> i32 {
        board_tile_named(name)
    }
    fn tile_count(&self) -> i32 {
        60
    }
    fn tile_name(&self, tile: i32) -> String {
        board_tile_name(tile)
    }
    fn tile_is(&self, tile: i32, kind: &str) -> bool {
        match kind {
            "circle" => board_is_start(tile),
            "ring" => board_is_deed(tile),
            _ => false,
        }
    }
    fn tile_in(&self, _player_id: i32, tile: i32, class: &str) -> bool {
        match class {
            "buyable" => board_is_buyable(tile),
            "livehouse" => board_is_venue(tile),
            _ => false,
        }
    }
    fn player_count(&self) -> i32 {
        self.n_players
    }
    fn money(&self, p: i32) -> i32 {
        self.seats.get(p as usize).map(|s| s.money).unwrap_or(0)
    }
    fn charge(&self, p: i32) -> i32 {
        self.seats.get(p as usize).map(|s| s.charge).unwrap_or(0)
    }
    fn group_pool(&self, p: i32, name: &str) -> i32 {
        // Fixture holds one unnamed group-pool scalar; any name reads it.
        let _ = name;
        self.seats.get(p as usize).map(|s| s.crystals).unwrap_or(0)
    }
    fn card_pool(&self, p: i32, card: &str, name: &str) -> i32 {
        self.card_counter(p, card, name)
    }
    fn hand_size(&self, p: i32) -> i32 {
        self.seats.get(p as usize).map(|s| s.hand).unwrap_or(0)
    }
    fn player_pos(&self, p: i32) -> i32 {
        self.seats.get(p as usize).map(|s| s.pos).unwrap_or(-1)
    }
    fn player_out(&self, p: i32) -> i32 {
        self.seats
            .get(p as usize)
            .map(|s| s.out as i32)
            .unwrap_or(0)
    }
    fn stay_of(&self, p: i32) -> i32 {
        self.seats.get(p as usize).map(|s| s.stay).unwrap_or(0)
    }
    fn stun_of(&self, p: i32) -> i32 {
        self.seats.get(p as usize).map(|s| s.stun).unwrap_or(0)
    }
    fn state_get(&self, p: i32, key: &str) -> i32 {
        let Some(s) = self.seats.get(p as usize) else {
            return 0;
        };
        match key {
            "stay" => s.stay,
            "stun" => s.stun,
            "stunStart" => s.stun_start,
            "exile" => s.exile,
            "noHand" => s.no_hand,
            "unstoppable" => s.unstoppable,
            other => s.slots.iter().find(|(n, _)| *n == other).map(|(_, v)| *v).unwrap_or(0),
        }
    }
    fn state_entries(&self, p: i32) -> Vec<(String, i32)> {
        let Some(s) = self.seats.get(p as usize) else {
            return Vec::new();
        };
        let mut rows = vec![
            ("stay".to_string(), s.stay),
            ("stun".to_string(), s.stun),
            ("stunStart".to_string(), s.stun_start),
            ("exile".to_string(), s.exile),
            ("noHand".to_string(), s.no_hand),
            ("unstoppable".to_string(), s.unstoppable),
        ];
        for (n, v) in &s.slots {
            rows.push((n.to_string(), *v));
        }
        rows.into_iter().filter(|(_, v)| *v != 0).collect()
    }
    fn persona_skill_id(&self, _p: i32) -> Option<String> {
        None
    }
    fn group_skill_id(&self, _p: i32) -> Option<String> {
        None
    }
    fn persona_name(&self, p: i32) -> Option<String> {
        self.seats
            .get(p as usize)
            .map(|s| s.persona.to_string())
            .filter(|s| !s.is_empty())
    }
    fn group_name(&self, p: i32) -> Option<String> {
        self.seats
            .get(p as usize)
            .map(|s| s.group.to_string())
            .filter(|s| !s.is_empty())
    }
    fn tok(&self, p: i32, name: &str) -> i32 {
        self.seats
            .get(p as usize)
            .and_then(|s| s.tokens.iter().find(|(n, _)| *n == name).map(|(_, v)| *v))
            .unwrap_or(0)
    }
    fn tok_names(&self, p: i32, prefix: &str) -> Vec<String> {
        self.seats
            .get(p as usize)
            .map(|s| {
                s.tokens
                    .iter()
                    .filter(|(n, v)| *v != 0 && n.starts_with(prefix))
                    .map(|(n, _)| n.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }
    fn owned_count(&self, p: i32) -> i32 {
        self.tiles.iter().filter(|t| t.owner == p).count() as i32
    }
    fn card_counter(&self, _p: i32, _card: &str, name: &str) -> i32 {
        self.card_counters
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| *v as i32)
            .unwrap_or(0)
    }
    fn tile_owner(&self, tile: i32) -> i32 {
        self.tiles.get(tile as usize).map(|t| t.owner).unwrap_or(-1)
    }
    fn houses_of(&self, tile: i32) -> i32 {
        self.tiles.get(tile as usize).map(|t| t.houses).unwrap_or(0)
    }
    fn mortgaged_of(&self, tile: i32) -> i32 {
        self.tiles
            .get(tile as usize)
            .map(|t| t.mortgaged as i32)
            .unwrap_or(0)
    }
    fn tile_price(&self, tile: i32) -> i32 {
        board_price(tile)
    }
    fn turn_player(&self) -> i32 {
        self.trig.turn_player
    }
    fn turn_key(&self) -> i32 {
        self.trig.turn_key
    }
    fn fixed_roll(&self) -> i32 {
        self.trig.plan_fixed_roll as i32
    }
    fn gains_this_turn(&self, p: i32) -> i32 {
        self.seats.get(p as usize).map(|s| s.gains).unwrap_or(0)
    }
    fn targeted_count(&self, p: i32) -> i32 {
        self.seats.get(p as usize).map(|s| s.targeted).unwrap_or(0)
    }
    fn card_tile(&self, _p: i32, _card: &str) -> i32 {
        // The fixture stores the raw host value directly (`-2` = none).
        if self.card_placed {
            self.card_tile as i32
        } else {
            -2
        }
    }
}

// ---------------------------------------------------------------------------
// (B) Kani IR: World + TriggerOverlay → IrCtx
// ---------------------------------------------------------------------------

fn to_world(fx: &Fixture) -> KWorld {
    let mut w = KWorld::new();
    w.n_players = fx.n_players;
    for (p, s) in fx.seats.iter().enumerate() {
        w.money[p] = s.money;
        w.charge[p] = s.charge;
        w.group_pool[p] = s.crystals;
        w.pos[p] = s.pos;
        w.out[p] = s.out;
        w.stay[p] = s.stay;
        // `hand_size_view` = resonance + non-slot-1 bits + `hand_n`.
        w.hand_n = s.hand.clamp(0, 255) as u8;
        if !s.persona.is_empty() {
            w.persona_id[p] = intern::intern(s.persona).0;
        }
        if !s.group.is_empty() {
            w.group_id[p] = intern::intern(s.group).0;
        }
        w.player_keys[p].set(intern::intern(ir_fill::KEY_STUN), s.stun);
        w.player_keys[p].set(intern::intern(ir_fill::KEY_STUN_START), s.stun_start);
        w.player_keys[p].set(intern::intern(ir_fill::KEY_EXILE), s.exile);
        w.player_keys[p].set(intern::intern(ir_fill::KEY_NO_HAND), s.no_hand);
        w.player_keys[p].set(intern::intern(ir_fill::KEY_UNSTOPPABLE), s.unstoppable);
        for &(n, v) in &s.tokens {
            let id = intern::intern(n);
            w.player_keys[p].set(id, v);
            if n == kworld::TOK_PP_POS {
                w.fans_pos[p] = v;
            } else if n == kworld::TOK_PP_NEG {
                w.fans_neg[p] = v;
            }
        }
        for &(n, v) in &s.slots {
            w.player_keys[p].set(intern::intern(n), v);
        }
    }
    for (i, t) in fx.tiles.iter().enumerate() {
        w.tiles[i].owner = t.owner;
        w.tiles[i].houses = t.houses;
        w.tiles[i].mortgaged = t.mortgaged;
    }
    w.trig_player = fx.trig.actor;
    w.trig_target = fx.trig.target;
    w.trig_tile = fx.trig.tile;
    w.trig_value = fx.trig.value as i32;
    w.trig_kind = fx.trig.kind.as_i32() as i32;
    w.turn_player = fx.trig.turn_player;
    w.turn_key = fx.trig.turn_key;
    w
}

fn to_overlay(fx: &Fixture) -> TriggerOverlay {
    let t = &fx.trig;
    let mut ov = TriggerOverlay::default();
    ov.kind = Some(t.kind.as_i32() as i32);
    ov.value = Some(t.value);
    ov.step = t.step;
    ov.by = t.by;
    ov.pay_is_rent = t.pay_is_rent;
    ov.roll_source = t.roll_source;
    // Production derives `abnormal` from `TriggerKind::Abnormal`.
    ov.abnormal = matches!(t.kind, TriggerKind::Abnormal);
    ov.trigger_card = if t.trigger_card.is_empty() {
        0
    } else {
        rules_cond_ir::ctx::id_of(t.trigger_card)
    };
    ov.counter_name = if t.counter_name.is_empty() {
        0
    } else {
        rules_cond_ir::ctx::id_of(t.counter_name)
    };
    ov.plan_fixed_roll = t.plan_fixed_roll;
    if let Some(m) = &t.mv {
        ov.move_roll = m.roll.unwrap_or(-1);
        ov.move_kind = m.kind.unwrap_or(-1);
        ov.move_remaining = m.remaining;
        ov.move_main = m.main;
        ov.move_dir = m.dir;
    }
    for &(kind, from, hits) in &t.chain {
        ov.push_chain(kind, from, hits);
    }
    for &(n, v) in &fx.card_counters {
        ov.push_card_counter(n, v);
    }
    ov
}

fn as_i64(v: Val) -> i64 {
    match v {
        Val::Int(i) => i,
        Val::Bool(b) => b as i64,
        _ => 0,
    }
}

/// `ir_fill` leaves `IrCtx::bin` empty. Production's `CondView` computes
/// geometry live from the window snapshot. This fills the same numbers into
/// the bin table the IR `call_fn` reads, for the argument pairs the guard
/// actually mentions (folded `Var`/`ConstInt` args only — the inventory's
/// geometry calls are that shape).
fn fill_geometry_bins(c: &mut IrCtx, prog: &rules_cond_ir::Program, fx: &Fixture) {
    let nodes = prog.nodes;
    for n in nodes {
        if n.kind != Kind::Call {
            continue;
        }
        let fid = n.payload;
        let nargs = n.b as usize;
        let first = n.a as usize;
        if nargs < 2 || first + nargs > nodes.len() {
            continue;
        }
        let a = as_i64(eval_node(nodes, first as u16, c));
        let b = as_i64(eval_node(nodes, (first + 1) as u16, c));
        let v = geometry_value(fx, fid, a, b);
        if v != 0 {
            c.bin_set(a, b, v);
        }
    }
}

/// Mirror of production `CondView`'s geometry accessors (see
/// `crates/game-rules/src/cond_pre.rs` `SnapView`). Only the four functions
/// the inventory uses need bins.
fn geometry_value(fx: &Fixture, fid: i64, a: i64, b: i64) -> i64 {
    let n = 60i64;
    let pos = |seat: i64| -> i64 {
        if seat < 0 {
            -1
        } else {
            fx.seats.get(seat as usize).map(|s| s.pos as i64).unwrap_or(-1)
        }
    };
    let out = |seat: i64| -> bool {
        seat < 0
            || fx
                .seats
                .get(seat as usize)
                .map(|s| s.out)
                .unwrap_or(true)
    };
    let exile = |seat: i64| -> i64 {
        fx.seats.get(seat as usize).map(|s| s.exile as i64).unwrap_or(1)
    };
    let owner_of = |tile: i64| -> i64 {
        fx.tiles
            .get(tile as usize)
            .map(|t| t.owner as i64)
            .unwrap_or(-1)
    };
    let dist = |x: i64, y: i64| -> i64 {
        if n <= 0 {
            return 0;
        }
        let d = ((y - x) % n + n) % n;
        d.min(n - d)
    };
    match fid {
        x if x == FnId::PlayersOn as i64 => {
            // (tile, except)
            (0..fx.n_players as i64)
                .filter(|&s| s != b && !out(s) && exile(s) == 0 && pos(s) == a)
                .count() as i64
        }
        x if x == FnId::NextDist as i64 => {
            // (p, dir)
            let p = a;
            let dir = b;
            let ppos = pos(p);
            if n <= 0 || ppos < 0 {
                return -1;
            }
            let mut best = i64::MAX;
            for o in 0..fx.n_players as i64 {
                if o == p || out(o) {
                    continue;
                }
                let q = pos(o);
                if q < 0 {
                    continue;
                }
                let fwd = if dir >= 0 {
                    ((q - ppos) % n + n) % n
                } else {
                    ((ppos - q) % n + n) % n
                };
                if fwd > 0 && fwd < best {
                    best = fwd;
                }
            }
            if best == i64::MAX {
                -1
            } else {
                best
            }
        }
        x if x == FnId::OwnedWithin as i64 => {
            // (p, radius)
            let p = a;
            let radius = b;
            let ppos = pos(p);
            if ppos < 0 {
                return 0;
            }
            (0..n)
                .filter(|&t| owner_of(t) == p && dist(ppos, t) <= radius)
                .count() as i64
        }
        x if x == FnId::OnPath as i64 => {
            // (me, them) — `them`'s 1..=|move.roll| forward path.
            let me = a;
            let them = b;
            let roll = fx
                .trig
                .mv
                .as_ref()
                .and_then(|m| m.roll)
                .unwrap_or(-1);
            if roll < 0 {
                return 0;
            }
            let steps = roll.abs();
            let tpos = pos(them);
            if n <= 0 || tpos < 0 {
                return 0;
            }
            (1..=steps)
                .filter(|i| owner_of((tpos + i) % n) == me)
                .count() as i64
        }
        x if x == FnId::OthersWithin as i64 => {
            let p = a;
            let radius = b;
            let ppos = pos(p);
            if ppos < 0 {
                return 0;
            }
            (0..fx.n_players as i64)
                .filter(|&o| {
                    if o == p || out(o) {
                        return false;
                    }
                    let d = dist(ppos, pos(o));
                    d > 0 && d <= radius
                })
                .count() as i64
        }
        _ => 0,
    }
}

/// IR-side fill: `fill_from_world` + geometry bins for this guard.
fn fill_ir(fx: &Fixture, owner: i32, prog: &rules_cond_ir::Program) -> IrCtx {
    let w = to_world(fx);
    let ov = to_overlay(fx);
    let card_id = rules_cond_ir::ctx::id_of(fx.card);
    let mut c = ir_fill::fill_from_world(&w, owner, card_id, fx.card_placed, fx.card_tile, &ov);
    fill_geometry_bins(&mut c, prog, fx);
    c
}

// ---------------------------------------------------------------------------
// Evaluators
// ---------------------------------------------------------------------------

fn prod_eval(src: &str, fx: &Fixture, owner: i32) -> bool {
    let win: WindowCtx = fill_window(fx);
    let cand = fill_candidate(fx, owner, fx.card, fx.card_placed);
    let pre = CompiledPre::compile(src).expect("inventory guard must compile");
    let scope = window_scope(&win);
    // `WindowScope::eval` maps any CEL error to false — same root rule as IR.
    scope.eval(&pre.cond, &cand)
}

fn ir_eval(id: usize, fx: &Fixture, owner: i32) -> bool {
    let prog = &cond_irs::BY_ID[id].program;
    let c = fill_ir(fx, owner, prog);
    eval_id(id, &c)
}

fn check_guard(id: usize, fx: &Fixture, owner: i32) -> (bool, bool) {
    let src = BY_ID[id].src;
    let a = prod_eval(src, fx, owner);
    let b = ir_eval(id, fx, owner);
    assert_eq!(
        a, b,
        "fill differential: fixture={} owner={} guard#{id} src={src:?}\n  prod={a} ir={b}",
        fx.name, owner
    );
    (a, b)
}

fn check_all(fx: &Fixture) {
    for owner in 0..fx.n_players {
        for id in 0..GUARD_COUNT {
            check_guard(id, fx, owner);
        }
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// (1) 2 players, poorest-style money split, PayAdd value > 0, card with owner,
/// circle tile, fans present. Covers aya_haruka_02 `value > 0` (true).
fn fx_aya_poorest_value() -> Fixture {
    let mut fx = Fixture::new("aya_poorest_value", 2);
    fx.seats[0].money = 200; // poorest
    fx.seats[1].money = 5000;
    fx.seats[0].pos = 0;
    fx.seats[1].pos = 10;
    fx.seats[0].tokens = vec![(ir_fill::TOK_PP_POS, 2), (ir_fill::TOK_PP_NEG, 1)];
    fx.trig = Trig {
        kind: TriggerKind::PayAdd,
        actor: 0,
        target: 1,
        tile: 0, // CiRCLE
        value: 100,
        turn_player: 0,
        ..Trig::default()
    };
    fx.card_placed = true;
    fx.card_tile = -1; // with owner
    fx
}

/// (2) Same shape but `value == 0` and owner is not poorest — aya guard false.
fn fx_aya_value_zero_richest() -> Fixture {
    let mut fx = Fixture::new("aya_value_zero_richest", 2);
    fx.seats[0].money = 9000;
    fx.seats[1].money = 100;
    fx.seats[0].pos = 0;
    fx.seats[1].pos = 10;
    fx.seats[0].tokens = vec![(ir_fill::TOK_PP_POS, 2)];
    fx.trig = Trig {
        kind: TriggerKind::PayAdd,
        actor: 0,
        target: 1,
        tile: 0,
        value: 0,
        turn_player: 0,
        ..Trig::default()
    };
    fx.card_placed = true;
    fx.card_tile = -1;
    fx
}

/// (3) Placed card on a ring tile (RiNG 1 = slot 8), not-placed counterpart
/// exercised via `card_tile = -2` / `card_placed = false` on seat 1's probe.
fn fx_tile_kinds() -> Fixture {
    let mut fx = Fixture::new("tile_kinds", 2);
    fx.seats[0].pos = 1;
    fx.seats[1].pos = 20;
    // Own a property at slot 2 (distance 1 from pos 1 → owned_within 3 is live
    // on production; IR bin fill must match).
    fx.tiles[2].owner = 0;
    fx.tiles[2].houses = 2;
    fx.tiles[2].mortgaged = true;
    fx.trig = Trig {
        kind: TriggerKind::Settle,
        actor: 0,
        target: -1,
        tile: 8, // RiNG 1 — ring ∧ live-house ∧ buyable
        value: 0,
        turn_player: 0,
        ..Trig::default()
    };
    fx.card = "PP:[标题偶像]";
    fx.card_placed = true;
    fx.card_tile = 8;
    fx.card_counters = vec![("crystals", 3), ("cp", 1)];
    fx
}

/// (4) tok('P✽P粉丝(正)') present vs the absent twin (seats differ).
fn fx_fans_tok() -> Fixture {
    let mut fx = Fixture::new("fans_tok", 4);
    for (i, s) in fx.seats.iter_mut().enumerate() {
        s.pos = i as i32 * 7;
    }
    fx.seats[0].tokens = vec![(ir_fill::TOK_PP_POS, 4), ("PAREO标记", 2), ("星星贴纸", 1)];
    fx.seats[1].tokens = vec![(ir_fill::TOK_PP_NEG, 1)];
    // seats 2/3: no fans tokens
    fx.trig = Trig {
        kind: TriggerKind::Target,
        actor: 1,
        target: 0,
        tile: 15, // CiRCLE 咖啡厅 (cafe, not buyable)
        value: 50,
        turn_player: 2,
        turn_key: 9,
        ..Trig::default()
    };
    fx.card_placed = true;
    fx.card_tile = -1;
    fx
}

/// (5) Chain with a Pay link present, plus slot latches. Also a move payload
/// with a real face (others out → geometry stays 0 on both sides).
fn fx_chain_pay_move() -> Fixture {
    let mut fx = Fixture::new("chain_pay_move", 2);
    fx.seats[0].pos = 30;
    fx.seats[1].pos = 40;
    fx.seats[1].out = true; // next_dist / on_path stay 0
    fx.seats[0].slots = vec![
        ("skillState", 2),
        ("asUsualTurn", 9),
        ("lastWalk", 1),
        ("one_of_us_partner", 2),
    ];
    fx.trig = Trig {
        kind: TriggerKind::Pay,
        actor: 0,
        target: 1,
        tile: 30,
        value: 5000,
        by: 1,
        pay_is_rent: true,
        chain: vec![
            (TriggerKind::Pay.as_i32() as i64, 0, 1),
            (TriggerKind::Target.as_i32() as i64, 1, 0),
        ],
        mv: Some(MoveFx {
            roll: Some(5),
            kind: Some(MoveKind::Walk as i64),
            remaining: 2,
            main: true,
            dir: 1,
        }),
        turn_player: 0,
        turn_key: 9,
        ..Trig::default()
    };
    fx.card_placed = true;
    fx.card_tile = 30;
    fx.card_counters = vec![("crystals", 0)];
    fx
}

/// (6) No chain, Abnormal window, group_is, persona_is, counter_is, not placed.
fn fx_no_chain_abnormal() -> Fixture {
    let mut fx = Fixture::new("no_chain_abnormal", 4);
    for (i, s) in fx.seats.iter_mut().enumerate() {
        s.pos = 5 + i as i32 * 11;
    }
    fx.seats[0].group = "Pastel✽Palettes";
    fx.seats[0].persona = "丸山彩";
    fx.seats[0].charge = 2;
    fx.seats[0].hand = 7;
    fx.seats[0].slots = vec![("skill.manaDonut.half", 1), ("detour_next_roll", 1)];
    fx.trig = Trig {
        kind: TriggerKind::Abnormal,
        actor: 1,
        target: 0,
        tile: 44, // 星之鼓动山丘
        value: -3,
        counter_name: "crystals",
        trigger_card: "PP:[丸山彩]憧憬的前方",
        plan_fixed_roll: 4,
        turn_player: 0,
        turn_key: 3,
        ..Trig::default()
    };
    fx.card = "PP:[丸山彩]憧憬的前方";
    fx.card_placed = false;
    fx.card_tile = -2;
    fx
}

/// (7) 2 players, property tile with owner ≠ actor, rent pay — `pay_is_rent`
/// and `tile.owner` paths.
fn fx_rent_property() -> Fixture {
    let mut fx = Fixture::new("rent_property", 2);
    fx.seats[0].pos = 4;
    fx.seats[1].pos = 33;
    fx.tiles[1].owner = 1; // 购物中心, far from both positions
    fx.tiles[1].houses = 1;
    fx.trig = Trig {
        kind: TriggerKind::Paid,
        actor: 0,
        target: 1,
        tile: 1,
        value: 300,
        pay_is_rent: true,
        by: 1,
        turn_player: 1,
        ..Trig::default()
    };
    fx.card_placed = true;
    fx.card_tile = 1;
    fx
}

/// (8) 4 players, richest/owner.money style, Teleport move (null face),
/// live-house tile (slot 22 = 便利店? — use slot 9 ring 2).
fn fx_four_players_teleport() -> Fixture {
    let mut fx = Fixture::new("four_players_teleport", 4);
    for (i, s) in fx.seats.iter_mut().enumerate() {
        s.pos = [3, 17, 39, 52][i];
        s.money = [100, 200, 300, 400][i];
    }
    fx.tiles[51].owner = 0; // 北泽精肉店 — dist from pos 3 is large
    fx.trig = Trig {
        kind: TriggerKind::Teleport,
        actor: 2,
        target: 2,
        tile: 9, // RiNG 2
        value: 0,
        mv: Some(MoveFx {
            roll: None, // null face → on_path / next_dist short-circuit
            kind: Some(MoveKind::Teleport as i64),
            remaining: 0,
            main: false,
            dir: -1,
        }),
        turn_player: 2,
        turn_key: 12,
        ..Trig::default()
    };
    fx.card_placed = true;
    fx.card_tile = 9;
    fx.card_counters = vec![("crystals", 1)];
    fx
}

/// Status-gate layer counts on one seat: stay / stun / stunStart / exile /
/// unstoppable all non-zero so a `stay(p)`-shaped guard cannot pass by
/// reading unfilled zeros.
fn fx_status_layers() -> Fixture {
    let mut fx = Fixture::new("status_layers", 2);
    fx.seats[0].stay = 2;
    fx.seats[0].stun = 1;
    fx.seats[0].stun_start = 3;
    fx.seats[0].exile = 0;
    fx.seats[0].unstoppable = 0;
    fx.seats[1].stay = 0;
    fx.seats[1].stun = 0;
    fx.seats[1].stun_start = 1;
    fx.seats[1].exile = 4;
    fx.seats[1].unstoppable = 2;
    fx.trig = Trig {
        kind: TriggerKind::Pay,
        actor: 1,
        target: 1,
        tile: 0,
        value: 100,
        turn_player: 0,
        ..Trig::default()
    };
    fx.card_placed = true;
    fx.card_tile = -1;
    fx
}

fn all_fixtures() -> Vec<Fixture> {
    vec![
        fx_aya_poorest_value(),
        fx_aya_value_zero_richest(),
        fx_tile_kinds(),
        fx_fans_tok(),
        fx_chain_pay_move(),
        fx_no_chain_abnormal(),
        fx_rent_property(),
        fx_four_players_teleport(),
        fx_status_layers(),
    ]
}


/// Index of a guard by its source string. Not a hardcoded id: this branch
/// adds guards, which shifts the generated table's indices.
fn guard_index(src: &str) -> usize {
    BY_ID
        .iter()
        .position(|g| g.src == src)
        .unwrap_or_else(|| panic!("guard not in the generated table: {src:?}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Id stability: `guard_id!(src)` / `guard_id_const(src)` == the `BY_ID` index
/// whose `src` field is that string, for every entry.
#[test]
fn guard_ids_are_stable() {
    assert_eq!(GUARD_COUNT, BY_ID.len(), "generated table size matches the table");
    for (i, g) in BY_ID.iter().enumerate() {
        assert_eq!(g.id, i, "GuardIr.id must equal BY_ID index for {src:?}", src = g.src);
        assert_eq!(
            cond_irs::guard_id_const(g.src),
            i,
            "guard_id_const({src:?}) must be {i}",
            src = g.src
        );
        assert_eq!(
            cond_irs::guard_id_opt(g.src),
            Some(i),
            "guard_id_opt({src:?}) must be Some({i})",
            src = g.src
        );
    }
    // Compile-time macro agrees on a sample of every shape class.
    assert_eq!(rules_kani::guard_id!("actor == owner && value > 0"), guard_index("actor == owner && value > 0"));
    assert_eq!(rules_kani::guard_id!("false"), guard_index("false"));
    assert_eq!(
        rules_kani::guard_id!(
            "actor == owner && effect.has(Pay) && target >= 0 && target != owner && value > 0"
        ),
        guard_index("actor == owner && effect.has(Pay) && target >= 0 && target != owner && value > 0")
    );
    assert_eq!(rules_kani::guard_id!("card.placed"), guard_index("card.placed"));
}

/// The differential: every fixture × every seat × all 139 guards.
#[test]
fn fill_window_vs_ir_fill_agree_on_every_guard() {
    let fixtures = all_fixtures();
    assert!(fixtures.len() >= 4, "need at least 4 fixtures");
    let mut comparisons = 0usize;
    for fx in &fixtures {
        check_all(fx);
        comparisons += fx.n_players as usize * GUARD_COUNT;
    }
    assert_eq!(
        comparisons,
        fixtures
            .iter()
            .map(|f| f.n_players as usize * GUARD_COUNT)
            .sum::<usize>()
    );
    // 2+2+2+4+2+4+2+4 = seats × guards = comparisons.
    eprintln!(
        "conf_fill_diff: {} fixtures × {} guards on {} seats = {} comparisons",
        fixtures.len(),
        GUARD_COUNT,
        fixtures.iter().map(|f| f.n_players).sum::<i32>(),
        comparisons
    );
}

/// Non-vacuity: the fixtures actually flip the guards they are named for.
/// (If both sides were stuck false the differential would still "pass".)
#[test]
fn fixtures_flip_the_guards_they_target() {
    let aya = "actor == owner && value > 0";
    let chain_pay =
        "actor == owner && effect.has(Pay) && target >= 0 && target != owner && value > 0";
    let is_start = "actor == owner && is_start(tile.id)";
    // No `actor == owner` clause — the fans fixture's actor is seat 1.
    let fans = "value > 0 && tok('P✽P粉丝(正)') >= 1";
    let placed = "card.placed";
    let band = "group_is(owner, 'Pastel✽Palettes')";

    let f1 = fx_aya_poorest_value();
    assert!(prod_eval(aya, &f1, 0), "fixture 1: aya guard should hold");
    assert!(ir_eval(guard_index(aya), &f1, 0), "fixture 1: IR aya guard should hold");

    let f2 = fx_aya_value_zero_richest();
    assert!(!prod_eval(aya, &f2, 0), "fixture 2: value==0 → aya false");

    let f3 = fx_tile_kinds();
    assert!(prod_eval(is_start, &f3, 0) == false, "tile 8 is ring, not circle");
    // Circle tile 0:
    let mut f3b = f3.clone();
    f3b.trig.tile = 0;
    assert!(prod_eval(is_start, &f3b, 0));
    assert!(ir_eval(guard_index(is_start), &f3b, 0));

    let f5 = fx_chain_pay_move();
    assert!(prod_eval(chain_pay, &f5, 0), "chain Pay present");
    assert!(ir_eval(guard_index(chain_pay), &f5, 0));
    let mut f5b = f5.clone();
    f5b.trig.chain.clear();
    assert!(!prod_eval(chain_pay, &f5b, 0), "chain Pay absent");

    let f4 = fx_fans_tok();
    assert!(prod_eval(fans, &f4, 0), "seat 0 has fans");
    assert!(ir_eval(guard_index(fans), &f4, 0));
    assert!(!prod_eval(fans, &f4, 2), "seat 2 has no fans");
    assert!(!ir_eval(guard_index(fans), &f4, 2));

    let f6 = fx_no_chain_abnormal();
    assert!(!prod_eval(placed, &f6, 0), "not placed");
    assert!(prod_eval(band, &f6, 0), "Pastel✽Palettes band");
    assert!(ir_eval(guard_index(band), &f6, 0));
}

/// Per-fixture names, so a failure names the world immediately.
#[test]
fn fill_diff_aya_poorest_value() {
    check_all(&fx_aya_poorest_value());
}
#[test]
fn fill_diff_aya_value_zero_richest() {
    check_all(&fx_aya_value_zero_richest());
}
#[test]
fn fill_diff_tile_kinds() {
    check_all(&fx_tile_kinds());
}
#[test]
fn fill_diff_fans_tok() {
    check_all(&fx_fans_tok());
}
#[test]
fn fill_diff_chain_pay_move() {
    check_all(&fx_chain_pay_move());
}
#[test]
fn fill_diff_no_chain_abnormal() {
    check_all(&fx_no_chain_abnormal());
}
#[test]
fn fill_diff_rent_property() {
    check_all(&fx_rent_property());
}
#[test]
fn fill_diff_four_players_teleport() {
    check_all(&fx_four_players_teleport());
}
#[test]
fn fill_diff_status_layers() {
    check_all(&fx_status_layers());
}

/// `state(p, 'key')` through both fillers, on a fixture whose seat carries
/// stay / stun / stunStart / exile / unstoppable. The inventory has no
/// `state(p,'key')` guard yet, so this lowers the sources directly (same
/// `fill_window` / `fill_from_world` seam `check_guard` uses).
#[test]
fn fill_diff_state_fns() {
    let fx = fx_status_layers();
    // (src, owner, expected) — agreement first, then a truth value so a
    // double-zero pass cannot hide a fill gap.
    let cases: &[(&str, i32, bool)] = &[
        (
            "state(owner,'stay') > 0 && state(owner,'unstoppable') == 0",
            0,
            true,
        ),
        (
            "state(owner,'stay') > 0 && state(owner,'unstoppable') == 0",
            1,
            false,
        ),
        ("state(owner,'stun') + state(owner,'stunStart') == 4", 0, true),
        ("state(owner,'stun') + state(owner,'stunStart') == 1", 1, true),
        (
            "state(owner,'exile') == 0 && state(owner,'unstoppable') == 0",
            0,
            true,
        ),
        (
            "state(actor,'exile') == 4 && state(actor,'unstoppable') == 2",
            0,
            true,
        ),
        (
            "state(owner,'stay') + state(owner,'stun') + state(owner,'stunStart') == 6",
            0,
            true,
        ),
    ];
    for &(src, owner, want) in cases {
        let a = prod_eval(src, &fx, owner);
        let prog = rules_cond::lower::lower_src(src).expect("lower state fn src");
        let w = to_world(&fx);
        let ov = to_overlay(&fx);
        let card_id = rules_cond_ir::ctx::id_of(fx.card);
        let c = ir_fill::fill_from_world(&w, owner, card_id, fx.card_placed, fx.card_tile, &ov);
        let b = prog.eval_bool(&c);
        assert_eq!(
            a, b,
            "state-fn fill differential: fixture={} owner={owner} src={src:?}\n  prod={a} ir={b}",
            fx.name
        );
        assert_eq!(a, want, "state-fn truth: owner={owner} src={src:?} got={a}");
    }
}