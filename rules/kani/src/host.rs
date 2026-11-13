//! Symbolic host backend, installed into `card_sdk::kani_host`.
//!
//! Under `cfg(kani)` the card-sdk `sys` surface is ordinary Rust that funnels
//! every `ruleset_*` import through [`card_sdk::kani_host::call`]. This module
//! is that backend: a general model of the world ([`crate::world`]) plus an
//! effect log. Game vocabulary lives here, not in card-sdk.
//!
//! Name lookups go through [`crate::intern`] (`u16`). Unmodeled ops **fail
//! loudly**: `kani::assert(false, "unmodeled op …")` / a panic, never a silent
//! zero. Native (non-Kani) builds panic on unmodeled ops too so conformance
//! tests catch gaps.
//!
//! Faithfulness: each op mirrors the real host function in
//! `crates/game-rules/src/hostfns.rs` (read that file before inventing
//! semantics). Ops that pause for engine scheduling (`pay`, `gain`, `draw`, …)
//! are modelled as immediate concrete effects on the symbolic world; the
//! effect log records what moved.

use card_sdk::kani_host::{self, Args, Op};

use crate::board;
use crate::intern::{self, NameId};
use crate::spec::g12_status;
use crate::world::{
    self, Effect, World, CARD_RESONANCE, MAX_PLAYERS, TILE_SHOP, TILE_SHOP_NAME, TOK_PP_NEG,
    TOK_PP_POS,
};

/// Every name [`known_init`] interns. Pinned unique under `(FNV-1a, len)`
/// in `model_bounds::pilot_intern_hashes_are_unique` — Kani intern identity
/// is hash+len (bytes_eq dropped from the probe hit).
pub const PILOT_INTERN_NAMES: &[&str] = &[
    TOK_PP_POS,
    TOK_PP_NEG,
    world::COUNTER_CRYSTALS,
    world::COUNTER_CP,
    world::RENT_HOUSES,
    world::TILE_SHOP_NAME,
    world::CARD_AYA,
    CARD_RESONANCE,
    world::CARD_OTHER,
    world::CARD_EXTRA,
];

/// Pre-interned identities for every name the host compares against a
/// constant. Built once in [`install`]; comparing two `NameId`s is a `u16`
/// equality — no hash, no byte walk, no heap.
pub struct Known {
    pub tok_pp_pos: NameId,
    pub tok_pp_neg: NameId,
    pub tok_charge: NameId,
    pub crystals: NameId,
    pub cp: NameId,
    pub rent_houses: NameId,
    pub tile_shop: NameId,
    pub card_aya: NameId,
    pub card_resonance: NameId,
    pub card_other: NameId,
    pub card_extra: NameId,
    /// The Sumimi persona pair and the skills they bind (Two-in-One pilot).
    pub face_uika: NameId,
    pub face_mana: NameId,
    pub skill_uika: NameId,
    pub skill_mana: NameId,
    /// The two fire-bound prop keys (`fire::FIRE_INITIAL` / `FIRE_CAP`).
    pub fire_initial: NameId,
    pub fire_cap: NameId,
}

static mut KNOWN: Option<Known> = None;

fn known_init() -> Known {
    Known {
        tok_pp_pos: intern::intern(TOK_PP_POS),
        tok_pp_neg: intern::intern(TOK_PP_NEG),
        // Fire is a held token of the persona-skill instance (MARKERS.md §7).
        tok_charge: intern::intern("charge"),
        crystals: intern::intern(world::COUNTER_CRYSTALS),
        cp: intern::intern(world::COUNTER_CP),
        rent_houses: intern::intern(world::RENT_HOUSES),
        tile_shop: intern::intern(TILE_SHOP_NAME),
        card_aya: intern::intern(world::CARD_AYA),
        card_resonance: intern::intern(CARD_RESONANCE),
        card_other: intern::intern(world::CARD_OTHER),
        card_extra: intern::intern(world::CARD_EXTRA),
        face_uika: intern::intern("三角初华（Sumimi）"),
        face_mana: intern::intern("纯田真奈"),
        skill_uika: intern::intern("skill:三角初华（Sumimi）:成为偶像"),
        skill_mana: intern::intern("skill:纯田真奈:甜甜圈爱好者"),
        fire_initial: intern::intern(bandori_std::fire::FIRE_INITIAL),
        fire_cap: intern::intern(bandori_std::fire::FIRE_CAP),
    }
}

fn known() -> &'static Known {
    // Lazy: conformance tests drive the host without `install()`. Proofs
    // still call `install()` first so the table is warm before the body.
    unsafe {
        if KNOWN.is_none() {
            KNOWN = Some(known_init());
        }
        KNOWN.as_ref().unwrap()
    }
}

/// Install the **full** host dispatch (every modeled op). Prefer
/// [`install_backend`] with a per-harness op-subset dispatcher in proofs —
/// the full 300-op match is an SSA cliff under CBMC (see TRIAGE-LOG
/// `op-subset`). Conformance tests keep using this.
pub fn install() {
    install_backend(dispatch);
}

/// Install an arbitrary backend (typically a **narrow** per-harness
/// dispatcher built by [`crate::harness::host_subset!`]). Always warms
/// [`Known`] so name compares stay `u16` equality.
pub fn install_backend(f: kani_host::Backend) {
    kani_host::install(f);
    let k = known_init();
    // Loud: every constant name must intern (table has room for the pilot set).
    #[cfg(kani)]
    {
        kani::assert(k.tok_pp_pos.is_valid(), "known.tok_pp_pos");
        kani::assert(k.tok_pp_neg.is_valid(), "known.tok_pp_neg");
        kani::assert(k.crystals.is_valid(), "known.crystals");
        kani::assert(k.card_aya.is_valid(), "known.card_aya");
        kani::assert(k.card_resonance.is_valid(), "known.card_resonance");
        kani::assert(k.card_other.is_valid(), "known.card_other");
        kani::assert(k.card_extra.is_valid(), "known.card_extra");
        kani::assert(k.face_uika.is_valid(), "known.face_uika");
        kani::assert(k.face_mana.is_valid(), "known.face_mana");
        kani::assert(k.skill_uika.is_valid(), "known.skill_uika");
        kani::assert(k.skill_mana.is_valid(), "known.skill_mana");
        kani::assert(k.fire_initial.is_valid(), "known.fire_initial");
        kani::assert(k.fire_cap.is_valid(), "known.fire_cap");
    }
    unsafe {
        KNOWN = Some(k);
    }
}

/// Decode a guest name handle. Under the string facade the handle *is* the
/// `NameId` (`intern::INTERN_TAG | id`); otherwise fall back to reading the
/// arena bytes and interning (legacy / non-`s()` producers).
///
/// Under `cfg(kani)` an **untagged** name handle is a loud FAIL (same rule as
/// [`outside_subset`]): the facade (`ctx::s` / `key!`) always tags, so an
/// untagged handle means the body bypassed it. We do **not** `assume` the
/// path away, and we do **not** `native::read`/`to_vec` (the alloc is an SSA
/// cliff — TRIAGE-LOG `op-subset`).
#[inline(always)]
fn name_id(handle: i32, len: i32) -> NameId {
    if let Some(id) = intern::decode_handle(handle) {
        #[cfg(kani)]
        kani::assert(id.is_valid(), "name handle carries INVALID NameId");
        return id;
    }
    #[cfg(kani)]
    {
        let _ = len;
        kani::assert(false, "untagged name handle (expected ctx::s facade)");
        return intern::NameId::INVALID;
    }
    #[cfg(not(kani))]
    {
        let b = card_sdk::native::read(handle, len.max(0) as usize);
        intern::intern_bytes(&b)
    }
}

/// Bytes of a guest name, from the intern table (static, no heap).
fn name_bytes(handle: i32, len: i32) -> &'static [u8] {
    intern::resolve(name_id(handle, len))
}

/// Byte-equal to `expected`. Prefer [`name_eq_id`] when `expected` is a
/// pre-interned constant (a `u16` compare).
fn name_eq(handle: i32, len: i32, expected: &str) -> bool {
    intern::id_eq_str(name_id(handle, len), expected)
}

fn name_eq_id(handle: i32, len: i32, expected: NameId) -> bool {
    name_id(handle, len) == expected
}

fn card_slot(handle: i32, len: i32) -> u8 {
    let id = name_id(handle, len);
    let k = known();
    // Pre-interned full names: one `u16` compare each.
    if id == k.card_aya {
        return 0;
    }
    if id == k.card_resonance {
        return 1;
    }
    if id == k.card_other {
        return 2;
    }
    if id == k.card_extra {
        return 3;
    }
    let b = intern::resolve(id);
    // Kani interned-str facade from `ctx::cards_in`: a single byte whose
    // value IS the slot (0..3). See `card-sdk/src/ctx.rs` `cards_in`.
    if b.len() == 1 && b[0] < 4 {
        return b[0];
    }
    // Length + one CJK discriminator byte (legacy fallback for un-interned
    // producers). The four pilot names are distinguished by len (29 = AYA)
    // or by `b[4]` (first CJK byte after "PP:["): 衍=0xE8, 标=0xE6, 强=0xE5.
    match b.len() {
        29 => 0,
        17 => match b.get(4).copied().unwrap_or(0) {
            0xE8 => 1,
            0xE6 => 2,
            0xE5 => 3,
            _ => 2,
        },
        _ => 2,
    }
}

fn card_u16(handle: i32, len: i32) -> u16 {
    let id = name_id(handle, len);
    let k = known();
    // Full pilot names pre-interned at install: the NameId *is* the world id.
    if id == k.card_aya || id == k.card_resonance || id == k.card_other || id == k.card_extra {
        return id.0;
    }
    let b = intern::resolve(id);
    // 1-byte interned-str facade from `ctx::cards_in`.
    if b.len() == 1 && b[0] < 4 {
        return intern::intern(world::card_id_str(b[0])).0;
    }
    // Legacy length/CJK heuristic for un-interned producers.
    if b.len() == 29 {
        return k.card_aya.0;
    }
    if b.len() == 17 {
        let slot = match b.get(4).copied().unwrap_or(0) {
            0xE8 => 1u8,
            0xE6 => 2,
            0xE5 => 3,
            _ => 2,
        };
        return intern::intern(world::card_id_str(slot)).0;
    }
    id.0
}

/// Slot 0..=3 of a pile card id. Pre-interned pilot names are a `u16`
/// compare; the 1-byte facade and legacy length heuristic cover the rest.
fn card_slot_of_id(id: NameId) -> u8 {
    let k = known();
    if id == k.card_aya {
        return 0;
    }
    if id == k.card_resonance {
        return 1;
    }
    if id == k.card_other {
        return 2;
    }
    if id == k.card_extra {
        return 3;
    }
    let b = intern::resolve(id);
    if b.len() == 1 && b[0] < 4 {
        return b[0];
    }
    world::card_slot_of_bytes(b)
}

fn player_idx(p: i32) -> usize {
    if p >= 0 && (p as usize) < MAX_PLAYERS {
        p as usize
    } else {
        0
    }
}

fn tile_idx(t: i32) -> usize {
    if t >= 0 && t < 60 {
        t as usize
    } else {
        0
    }
}

/// Die roll within `count` d `sides` — symbolic under Kani, mid-value native.
/// `World::forced_roll != 0` pins the face (harness control for dice clauses).
/// Count range matches the real engine (`hostfns::roll` accepts `1..=100`);
/// a tighter clamp silently rewrites 12d4 into 8d4 (class C, eve-bushido-01).
fn roll_dice(count: i32, sides: i32) -> i32 {
    let c = count.max(0).min(100);
    let s = sides.max(1).min(1000);
    #[cfg(kani)]
    {
        let forced = world::with_world(|w| w.forced_roll);
        if forced != 0 {
            return forced;
        }
        // Explore every face of a single die; for multi-dice take a symbolic
        // sum bounded by the true range (CBMC stays small).
        if c == 1 {
            let x: i32 = kani::any();
            kani::assume(x >= 1 && x <= s);
            x
        } else if c == 0 {
            0
        } else {
            let x: i32 = kani::any();
            kani::assume(x >= c && x <= c * s);
            x
        }
    }
    #[cfg(not(kani))]
    {
        c * (s + 1) / 2
    }
}

fn unmodeled(op: Op) -> i64 {
    let n = op as i32;
    world::with_world_mut(|w| w.unmodeled_op = n);
    #[cfg(kani)]
    kani::assert(false, "unmodeled host op");
    #[cfg(not(kani))]
    panic!("unmodeled host op: {:?} ({})", op, n);
    #[allow(unreachable_code)]
    0
}

/// A harness-declared op-subset was violated: the card body reached a host
/// op outside its declared set.
///
/// **Never** `kani::assume(false)` — that would silently drop the path and
/// let a proof pass while the body used an undeclared op. This is a **loud
/// FAIL** (`kani::assert(false)`), so the proof cannot be vacuous about an
/// undeclared op. Native builds panic so conformance tests catch the gap.
pub fn outside_subset(op: Op) -> i64 {
    let n = op as i32;
    world::with_world_mut(|w| w.unmodeled_op = n);
    #[cfg(kani)]
    {
        // The numeric op is bucketed so a failed run names the missing op's
        // range in the report instead of only the bare message.
        let b = (n.max(0) as u32) / 16;
        match b {
            0 => kani::assert(false, "subset miss 0..16"),
            1 => kani::assert(false, "subset miss 16..32"),
            2 => kani::assert(false, "subset miss 32..48"),
            3 => kani::assert(false, "subset miss 48..64"),
            4 => kani::assert(false, "subset miss 64..80"),
            5 => kani::assert(false, "subset miss 80..96"),
            6 => kani::assert(false, "subset miss 96..112"),
            7 => kani::assert(false, "subset miss 112..128"),
            8 => kani::assert(false, "subset miss 128..144"),
            9 => kani::assert(false, "subset miss 144..160"),
            10 => kani::assert(false, "subset miss 160..176"),
            11 => kani::assert(false, "subset miss 176..192"),
            12 => kani::assert(false, "subset miss 192..208"),
            13 => kani::assert(false, "subset miss 208..224"),
            14 => kani::assert(false, "subset miss 224..240"),
            15 => kani::assert(false, "subset miss 240..256"),
            16 => kani::assert(false, "subset miss 256..272"),
            17 => kani::assert(false, "subset miss 272..288"),
            18 => kani::assert(false, "subset miss 288..304"),
            19 => kani::assert(false, "subset miss 304..320"),
            20 => kani::assert(false, "subset miss 320..336"),
            21 => kani::assert(false, "subset miss 336..352"),
            22 => kani::assert(false, "subset miss 352..368"),
            23 => kani::assert(false, "subset miss 368..384"),
            _ => kani::assert(false, "subset miss 384+"),
        }
    }
    #[cfg(not(kani))]
    panic!("host op outside declared subset: {:?} ({})", op, n);
    #[allow(unreachable_code)]
    0
}

/// The one dispatch entry. Args are the ABI's six i32 slots, in declaration order.
pub fn dispatch(op: Op, a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    match op {
        // ---- dice / log ----
        Op::roll => arm_roll(a),
        Op::roll_direct => arm_roll(a),
        Op::roll_recast => arm_roll_recast(a),
        Op::roll_move => arm_roll_move(a),
        Op::roll_pick => arm_roll_pick(a),
        Op::roll_current => arm_roll_current(a),
        Op::roll_face => arm_roll_face(a),
        Op::roll_unused_face => arm_roll_unused_face(a),
        Op::roll_rerolled => arm_roll_rerolled(a),
        Op::rolls_by_player => arm_rolls_by_player(a),
        Op::trigger_roll => arm_trigger_roll(a),
        Op::set_move_settle_answer => arm_set_move_settle_answer(a),
        Op::move_settle_answer => arm_move_settle_answer(a),
        Op::set_turn_end_settle_answer => arm_set_turn_end_settle_answer(a),
        Op::turn_end_settle_answer => arm_turn_end_settle_answer(a),
        Op::log | Op::effect => arm_log(a),

        // ---- tiles (static data from board.json + runtime state) ----
        Op::tile_count => arm_tile_count(a),
        Op::tile_named => arm_tile_named(a),
        Op::tile_owner => arm_tile_owner(a),
        Op::tile_group => arm_tile_group(a),
        Op::tile_price => arm_tile_price(a),
        Op::buy_price => arm_buy_price(a),
        // G09 §5.7: no percent / cut in the model, so the price is the base.
        Op::build_price | Op::build_cost => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                board::TILES[s as usize].house_cost as i64
            }
        }
        Op::build_cost_percent => 100,
        Op::build_discount => 0,
        Op::tile_buildable => 1,
        Op::mortgage_value => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                (board::TILES[s as usize].price / 2) as i64
            }
        }
        // `WorldOps::rent_of` — RiNG quotes `price`; otherwise `table_rent`
        // at `rent_houses` (RENT_HOUSES override → standing houses).
        Op::rent_of => world::with_world(|w| w.rent_of(a0)) as i64,
        Op::houses_of => arm_houses_of(a),
        Op::rent_houses_of => world::with_world(|w| w.tiles[tile_idx(a0)].houses) as i64,
        Op::set_houses => {
            world::with_world_mut(|w| {
                w.tiles[tile_idx(a0)].houses = a1;
                w.push(Effect::SetHouses { tile: a0, n: a1 });
            });
            0
        }
        Op::add_house => arm_add_house(a),
        Op::mortgaged_of => arm_mortgaged_of(a),
        Op::set_mortgaged => arm_set_mortgaged(a),
        Op::commit_redeem => arm_apply_redeem(a),
        Op::set_owner => arm_set_owner(a),
        Op::owned_count => arm_owned_count(a),
        Op::owned_at => arm_owned_at(a),
        Op::tile_is => arm_tile_is(a),
        Op::tile_attr => {
            // ABI: tile_attr(tile, attr_ptr, attr_len)
            let s = board::tile_slot(a0);
            if s < 0 {
                // Missing-tile defaults (mirror `tile_class::attr_of`).
                let attr = name_id(a1, a2);
                if intern::id_eq_str(attr, "group")
                    || intern::id_eq_str(attr, "tier")
                    || intern::id_eq_str(attr, "area")
                    || intern::id_eq_str(attr, "index")
                {
                    -1
                } else {
                    0
                }
            } else {
                let attr = name_id(a1, a2);
                let t = &board::TILES[s as usize];
                if intern::id_eq_str(attr, "group") {
                    t.group as i64
                } else if intern::id_eq_str(attr, "tier") {
                    -1
                } else if intern::id_eq_str(attr, "price") {
                    t.price as i64
                } else if intern::id_eq_str(attr, "house") {
                    t.house_cost as i64
                } else if intern::id_eq_str(attr, "area") {
                    -1
                } else if intern::id_eq_str(attr, "rent") {
                    t.rent.len() as i64
                } else if intern::id_eq_str(attr, "index") {
                    t.index as i64
                } else {
                    0
                }
            }
        }
        Op::tile_in => {
            // ABI: tile_in(player_id, tile, class_ptr, class_len)
            // This model keeps the historical quirks so proofs stay put:
            // buyable = unowned property; livehouse = tiles 31/50.
            let class = name_id(a2, a3);
            if intern::id_eq_str(class, "buyable") {
                let s = board::tile_slot(a1);
                if s < 0 {
                    0
                } else {
                    let t = &board::TILES[s as usize];
                    let owned = world::with_world(|w| w.tiles[tile_idx(a1)].owner);
                    (board::is_property(t) && owned < 0) as i64
                }
            } else if intern::id_eq_str(class, "shop") {
                let s = board::tile_slot(a1);
                if s < 0 {
                    0
                } else {
                    board::is_shop(&board::TILES[s as usize]) as i64
                }
            } else if intern::id_eq_str(class, "livehouse") {
                // Live House is board.json index 32 (agent) / 51 (property
                // Live House Galaxy) → engine positions 31 / 50.
                let _ = a0;
                (a1 == 31 || a1 == 50) as i64
            } else if intern::id_eq_str(class, "color:6") {
                // Same live-house set for the colour-6 class (this model's quirk).
                (a1 == 31 || a1 == 50) as i64
            } else {
                0
            }
        }
        Op::dist | Op::tile_forward => arm_dist(a),
        Op::neighbor => {
            // hostfns `neighbor`: the **next** present player in turn order,
            // wrapping. `dir < 0` walks backwards. Not a board tile.
            //
            // Must return the *first* present seat at distance 1,2,… — the
            // old loop kept overwriting `found` and returned the *last*
            // present seat (for n=4 all-present that is the far side, not
            // the neighbour). Class C; pinned by
            // `conf_hostops2::neighbor_is_the_next_present_seat`.
            let n = MAX_PLAYERS as i32;
            let dir = if a1 < 0 { -1 } else { 1 };
            let mut found = -1i32;
            let mut j = 1i32;
            while j < n {
                let k = ((a0 + dir * j).rem_euclid(n) + n).rem_euclid(n);
                let present = world::with_world(|w| {
                    (k as usize) < w.n_players as usize && !w.out[k as usize]
                });
                if present {
                    found = k;
                    break; // next, not last
                }
                j += 1;
            }
            found as i64
        }
        Op::tile_steps_ahead => arm_tile_steps_ahead(a),
        Op::players_on_count => world::with_world(|w| {
            let tile = a0;
            let except = a1;
            let mut n = 0;
            let mut p = 0;
            while p < MAX_PLAYERS {
                if (p as i32) != except && !w.out[p] && w.pos[p] == tile {
                    n += 1;
                }
                p += 1;
            }
            n
        }) as i64,
        Op::players_on_at => world::with_world(|w| {
            let tile = a0;
            let except = a1;
            let idx = a2;
            let mut n = 0;
            let mut p = 0;
            let mut found = -1;
            while p < MAX_PLAYERS {
                if (p as i32) != except && !w.out[p] && w.pos[p] == tile {
                    if n == idx {
                        found = p as i32;
                    }
                    n += 1;
                }
                p += 1;
            }
            found
        }) as i64,

        // ---- players ----
        Op::player_pos => arm_player_pos(a),
        Op::player_count => arm_player_count(a),
        Op::player_out => arm_player_out(a),
        Op::others_count => arm_others_count(a),
        Op::others_at => arm_others_at(a),
        Op::turn_player => arm_turn_player(a),
        Op::round_no => arm_round_no(a),
        Op::turn_key => arm_turn_key(a),
        // `target(player, tile, single)` is engine scheduling (hooks + the
        // `target` [反击] window). Model the **admission** half only: a living
        // seat in range is targeted and returned; out / OOB answers -1.
        // The counteract window is PROTOCOL-LEVEL (KANI.md §6) — no cancel
        // path is explored. `targeted_count` is bumped on success.
        Op::target => arm_target(a),
        Op::persona_is => arm_character_is(a),
        Op::set_persona => arm_set_persona(a),
        Op::in_group => arm_in_band(a),
        Op::self_uid => arm_self_uid(a),

        // ---- money ----
        Op::money => arm_money(a),
        Op::gain | Op::gain_typed | Op::gain_fixed => arm_gain(a),
        Op::adjust_money => arm_adjust_money(a),
        Op::pay => arm_pay(a),
        Op::pay_to => arm_pay_to(a),
        Op::pay_total => arm_pay_total(a),
        Op::pay_leg => arm_pay_leg(a),
        Op::can_pay => arm_can_pay(a),
        Op::gains_this_turn => arm_gains_this_turn(a),
        Op::paid_in_settle => 0,

        // ---- cards / piles ----
        Op::draw => arm_draw(a),
        Op::draw_event => 0,
        Op::add_to_dest => arm_add_to_dest(a),
        Op::take_card => arm_take_card(a),
        Op::cards_in => arm_cards_in(a),
        Op::to_discard => arm_to_discard(a),
        Op::hand_count => arm_hand_count(a),
        Op::hand_size => arm_hand_size(a),
        Op::discard_count => {
            // `discard_count(player, ptr, len)` → [player, ptr, len]: name (a1,a2).
            let slot = card_slot(a1, a2);
            world::with_world(|w| {
                let id = card_u16(a1, a2);
                let in_pile = world::pile_count(&w.discard_pile, w.discard_n, id);
                let in_bits = if (slot as usize) < 4 {
                    w.discard[slot as usize] as i32
                } else {
                    0
                };
                in_pile.max(in_bits)
            }) as i64
        }
        Op::discard_size => arm_discard_size(a),
        Op::deck_count => arm_deck_count(a),
        Op::discard_from_hand => arm_discard_from_hand(a),
        Op::shuffle_into_deck => arm_shuffle_into_deck(a),
        Op::place_card_at => arm_place_card_at(a),
        Op::place_card_on => arm_place_card_on(a),
        Op::set_dest => arm_set_fate(a),
        Op::set_dest_unwatched => arm_set_fate(a),
        Op::set_transfer_to_dest => world::with_world_mut(|w| {
            // Aims the *commit* at `to`; recorded until the dest is applied.
            w.dest = a1;
            w.dest_to = a0;
            0
        }),
        Op::send_to_dest => arm_send_to_dest(a),
        Op::transfer_to_dest => arm_transfer_to_dest(a),
        Op::teleport_to => arm_teleport_to(a),
        Op::unplace_card => arm_unplace_card_self(a),
        Op::unplace_at => arm_zone_leave(a),
        Op::unplace_card_named => arm_unplace_card_named(a),
        Op::is_placed => arm_is_placed(a),
        Op::is_own_placement => {
            world::with_world(|w| w.field.iter().any(|f| f.uid == a[0] && f.mine) as i64)
        }
        Op::field_instances => arm_field_instances(a),
        Op::placed_cards => arm_placed_cards(a),
        Op::placed_tile => {
            world::with_world(|w| if w.field_n == 0 { -1 } else { w.field[0].tile }) as i64
        }
        Op::set_card_tile => world::with_world_mut(|w| {
            let ok = w.set_tile_at(a0, a1);
            if ok {
                w.push(Effect::SetCardTile { uid: a0, tile: a1 });
            }
            ok as i64
        }),
        Op::set_card_face_down => world::with_world_mut(|w| {
            if let Some(f) = w.field_by_uid_mut(a0) {
                f.face_down = a1 != 0;
                1
            } else {
                0
            }
        }),
        Op::card_face_down => {
            world::with_world(|w| w.field_by_uid(a0).map_or(0, |f| f.face_down as i64))
        }
        Op::set_card_immune => world::with_world_mut(|w| {
            if let Some(f) = w.field_by_uid_mut(a0) {
                f.immune = a1 != 0;
                1
            } else {
                0
            }
        }),
        Op::card_immune => world::with_world(|w| w.field_by_uid(a0).map_or(0, |f| f.immune as i64)),
        Op::self_tile => world::with_world(|w| {
            if w.field_n == 0 {
                w.pos[player_idx(w.trig_player)]
            } else {
                w.field[0].tile
            }
        }) as i64,
        // The model keeps no slot apart from the tile (§5.7).
        Op::self_position => world::with_world(|w| {
            if w.field_n == 0 {
                w.pos[player_idx(w.trig_player)]
            } else {
                w.field[0].tile
            }
        }) as i64,
        Op::set_self_tile => world::with_world_mut(|w| {
            if w.field_n == 0 {
                0
            } else {
                let uid = w.field[0].uid;
                let ok = w.set_tile_at(uid, a0);
                if ok {
                    w.push(Effect::SetCardTile { uid, tile: a0 });
                }
                ok as i64
            }
        }),
        Op::self_face_down => world::with_world(|w| {
            if w.field_n == 0 {
                0
            } else {
                w.field[0].face_down as i64
            }
        }),
        Op::set_self_face_down => world::with_world_mut(|w| {
            // `set_self_face_down(on, cause)` -- the flip addresses the run's
            // own instance (`self_uid()`), never `field[0]`.
            match w.field_by_uid_mut(w.bound_uid) {
                Some(f) => {
                    f.face_down = a0 != 0;
                    1
                }
                None => 0,
            }
        }),
        Op::self_immune => world::with_world(|w| {
            if w.field_n == 0 {
                0
            } else {
                w.field[0].immune as i64
            }
        }),
        Op::set_self_immune => world::with_world_mut(|w| {
            if w.field_n == 0 {
                0
            } else {
                w.field[0].immune = a0 != 0;
                1
            }
        }),
        Op::is_face_down_at => {
            world::with_world(|w| w.field_by_uid(a0).map_or(0, |f| f.face_down as i64))
        }
        Op::set_face_down_at => world::with_world_mut(|w| {
            if let Some(f) = w.field_by_uid_mut(a0) {
                f.face_down = a1 != 0;
                1
            } else {
                0
            }
        }),
        // ---- zone family (G08) ----
        Op::zone_of => world::with_world(|w| w.zone_of(a0)) as i64,
        Op::attachment_host => world::with_world(|w| w.attachment_host(a0)) as i64,
        Op::attached_cards => arm_attached_cards(a),
        Op::created_cards => arm_created_cards(a),
        Op::earliest_owned_tile => world::with_world(|w| w.earliest_owned_tile(a0)) as i64,
        Op::instance_owner => {
            world::with_world(|w| w.field_by_uid(a0).map_or(-1, |f| f.owner)) as i64
        }
        Op::attach_card => arm_attach_card(a),
        Op::detach_card => arm_detach_card(a),
        Op::set_instance_owner => arm_set_instance_owner(a),
        Op::set_created_by_at => arm_set_created_by_at(a),
        Op::set_ineffective_at => arm_set_ineffective_at(a),
        Op::is_ineffective_at => {
            if a0 < 0 {
                world::with_world(|w| w.play_ineffective as i64)
            } else {
                world::with_world(|w| w.field_by_uid(a0).is_some_and(|f| !f.effective) as i64)
            }
        }
        Op::rehome_card => arm_rehome_card(a),
        Op::play_hand_card => arm_play_hand_card(a),
        Op::trigger_zone => arm_trigger_zone(a),
        Op::trig_play_outcome => world::with_world(|w| w.trig_play_outcome) as i64,
        Op::trig_out_cause => world::with_world(|w| w.trig_out_cause) as i64,
        Op::is_immune_at => {
            world::with_world(|w| w.field_by_uid(a0).map_or(0, |f| f.immune as i64))
        }
        Op::set_immune_at => world::with_world_mut(|w| {
            if let Some(f) = w.field_by_uid_mut(a0) {
                f.immune = a1 != 0;
                1
            } else {
                0
            }
        }),
        // `tile_at` / `set_tile_at` — field-instance placement (hostfns 535–543).
        // The ABI `tile_at(p)` is position -> standing tile (identity here; the
        // swap itself is proved in `spec::abi_g09`). `tile_of(uid)` is the
        // instance reader.
        Op::tile_of => world::with_world(|w| w.tile_at(a0)) as i64,
        Op::tile_at => a0 as i64,
        Op::position_of_tile => a0 as i64,
        Op::tile_extra_count => 0,
        Op::set_tile_at => world::with_world_mut(|w| {
            let ok = w.set_tile_at(a0, a1);
            if ok {
                w.push(Effect::SetCardTile { uid: a0, tile: a1 });
            }
            ok as i64
        }),

        // ---- props / counters / tokens / state ----
        Op::self_prop | Op::prop_at | Op::tile_prop => {
            // Packings differ (name_id pair must match, never the other one —
            // untagged handles loud-FAIL):
            //   self_prop(kp, kl)           -> [kp, kl, 0, …]      name (a0,a1)
            //   prop_at(uid, kp, kl)        -> [uid, kp, kl, …]    name (a1,a2)
            //   tile_prop(tile, kp, kl)     -> [tile, kp, kl, …]   name (a1,a2)
            let key = if op == Op::self_prop {
                name_id(a0, a1)
            } else {
                name_id(a1, a2)
            };
            if op == Op::tile_prop {
                world::with_world(|w| w.tiles[tile_idx(a0)].props.get(key)) as i64
            } else {
                0
            }
        }
        Op::set_self_prop | Op::set_prop_at | Op::set_tile_prop => {
            // hostfns: `set_self_prop(kp, kl, v)` writes the running instance's
            // prop; `set_tile_prop(tile, kp, kl, v)` / `set_prop_at(uid, kp, kl, v)`
            // write the tile / instance's. Packings differ — decode per op.
            // Never call `name_id` on the wrong pair: untagged handles now
            // loud-FAIL (ppcost / name_id cut).
            //
            // sys_kani:
            //   set_self_prop(kp, kl, v) -> [kp, kl, v, 0, 0, 0]
            //   set_tile_prop(tile, kp, kl, v) -> [tile, kp, kl, v, 0, 0]
            //   set_prop_at(uid, kp, kl, v) -> [uid, kp, kl, v, 0, 0]
            if op == Op::set_self_prop {
                let key = name_id(a0, a1);
                let val = a2;
                world::with_world_mut(|w| {
                    if w.field_n > 0 {
                        let i = (w.field_n as usize) - 1;
                        w.field[i].props.set(key, val);
                    }
                });
            } else if op == Op::set_tile_prop {
                let key = name_id(a1, a2);
                let val = a3;
                world::with_world_mut(|w| {
                    w.tiles[tile_idx(a0)].props.set(key, val);
                });
            } else {
                // set_prop_at(uid, kp, kl, v)
                let key = name_id(a1, a2);
                let val = a3;
                let uid = a0;
                world::with_world_mut(|w| {
                    let mut i = 0usize;
                    while i < w.field_n as usize {
                        if w.field[i].uid == uid {
                            w.field[i].props.set(key, val);
                        }
                        i += 1;
                    }
                });
            }
            0
        }
        Op::counter_self => arm_counter_self(a),
        Op::card_counter => arm_card_counter(a),
        Op::counter_at => arm_counter_at(a),
        Op::add_counter_self => arm_add_counter_self(a),
        Op::add_card_counter => arm_add_card_counter(a),
        Op::add_counter_at => arm_add_counter_at(a),
        Op::set_counter_self => arm_set_counter_self(a),
        Op::count_held | Op::count_held_name => arm_count_held(a),
        Op::add_held => arm_add_held(a),
        Op::set_held_name => arm_set_held_name(a),
        Op::held_max => arm_tok_max(a),
        Op::state_get => arm_state_get(a),
        Op::state_set => arm_state_set(a),
        Op::state_add => arm_state_add(a),
        Op::raise_after => arm_raise_after(a),
        Op::held_names => arm_tok_names(a),

        // ---- marks ----
        Op::place_mark => arm_place_mark(a),
        // Paired mark: model as two single marks (the kani world has no
        // `partner` field; `mark_partner` is stubbed −1 like `mark_src_at`).
        Op::place_mark_pair => {
            let _ = arm_place_mark([a0, a1, a2, a3, a4, a5]);
            let _ = arm_place_mark([a1, a0, a2, a3, a4, a5]);
            1
        }
        Op::count_marks_f => world::with_world(|w| {
            let mut n = 0;
            let mut i = 0;
            while i < w.marks_n as usize {
                if w.marks[i].tile == a0 {
                    n += w.marks[i].n;
                }
                i += 1;
            }
            n
        }) as i64,
        Op::bump_mark_f => 0,
        Op::remove_marks_f => 0,
        // `mark_partner` is BLOCKED-HOST (no partner field on the kani world
        // `Mark`) -- same stub shape as `mark_src_at`. `mark_tile` / `remove_mark`
        // are likewise stubbed (no uid on the kani world `Mark`).
        Op::mark_src_at | Op::mark_instance_at | Op::mark_partner | Op::mark_tile => -1,
        Op::remove_mark => 0,

        // ---- status ----
        Op::gain_marker => arm_gain_marker(a),
        Op::spend_marker => arm_spend_marker(a),
        Op::add_marker_max => {
            // Storage-only cap move (`Counter.max`); returns the new ceiling.
            // The pull-under is the rules-layer kernel's job.
            world::with_world_mut(|w| {
                let p = player_idx(a0);
                w.charge_max[p] = w.charge_max[p].saturating_add(a3).max(0);
                w.charge_max[p] as i64
            })
        }
        // `ctx::n(k, v)` doubles when `play_doubled() == k`. Default is
        // **no doubling** (`-1` / `None` in the engine) — the old stub returned
        // 0, so `n(0, 600)` wrongly yielded 1200.
        Op::play_doubled => world::with_world(|w| w.play_doubled) as i64,
        Op::set_play_doubled => {
            world::with_world_mut(|w| w.play_doubled = a0);
            0
        }
        Op::apply_abnormal => arm_apply_abnormal(a),
        // The stay move-latch is turn-flow (`World.st.skip_move`); it is the
        // `3` branch of `cant_move`. Packing: `(player, locked)`.
        Op::skip_main_move => arm_skip_main_move(a),
        Op::clear_move_skip => arm_clear_move_skip(a),
        Op::give_extra_turn => {
            // Recorded so the protocol can schedule it. Scheduling is
            // PROTOCOL-LEVEL (the turn engine owns the extra-turn queue).
            world::with_world_mut(|w| {
                if a0 >= 0 && (a0 as usize) < MAX_PLAYERS {
                    w.extra_turns[a0 as usize] = true;
                }
                w.push(Effect::GiveExtraTurn { player: a0 });
            });
            0
        }
        // Generic turn-flow / logging verbs: protocol-level, like
        // `give_extra_turn`. The model records them; the engine owns the
        // real pacing / move tail.
        Op::end_turn_now => {
            world::with_world_mut(|w| w.push(Effect::GiveExtraTurn { player: a0 }));
            0
        }
        Op::teleport_as => {
            let [a0, a1, _, a3, ..] = a;
            world::with_world_mut(|w| {
                if a0 >= 0 && (a0 as usize) < MAX_PLAYERS {
                    w.pos[a0 as usize] = a1;
                }
                if a3 != 0 {
                    w.main_moved = true;
                }
            });
            0
        }
        Op::log_as => arm_log(a),
        // Same code space as the engine (§5.4), composed from the running
        // activation: turn → skip → spent.
        Op::cant_move => arm_cant_move(a),
        Op::abnormal_count => world::with_world(|w| {
            let mut n = 0;
            let mut p = 0;
            while p < MAX_PLAYERS {
                if w.stay[p] > 0 {
                    n += 1;
                }
                p += 1;
            }
            n
        }) as i64,
        Op::run_planned_move => arm_card_move(a),

        // ---- band / skills ----
        Op::group_pool => arm_band_crystals(a),
        Op::add_group_pool => arm_add_band_crystals(a),
        Op::group_skill => arm_band_skill(a),
        Op::persona_skill => arm_persona_skill(a),
        Op::persona => arm_persona(a),
        Op::group_skills => arm_band_skills(a),
        Op::add_group_skill => arm_add_band_skill(a),
        Op::invoke_entry => arm_invoke_entry(a),
        Op::invoke_frame => arm_invoke_frame(a),
        Op::invoke_skill => arm_invoke_skill(a),

        // ---- nearest selection (G07) ----
        Op::nearest => arm_nearest(a),
        Op::nearest_with_prop => arm_nearest_with_prop(a),

        // ---- v114 (G12) status / blocked-ness / immunity ----
        // The trigger readers expose the stamp the harness / engine put on
        // the raise; the ordinal form runs the engine's exact `cant_move_from`
        // predicate (`spec::g12_status`, shared with the plain read above).
        Op::trigger_activation_ordinal => arm_trigger_activation_ordinal(a),
        Op::trigger_abnormal_blocked => arm_trigger_abnormal_blocked(a),
        Op::trigger_move_abnormal => arm_trigger_move_abnormal(a),
        Op::cant_move_code_for_activation => arm_cant_move_code_for_activation(a),
        Op::is_immune_from_at => arm_is_immune_from_at(a),
        Op::set_self_immune_except_owner => arm_set_self_immune_except_owner(a),
        Op::dice_immune => arm_dice_immune(a),
        Op::set_dice_immune => arm_set_dice_immune(a),

        // ---- inspection / reveal (G11) ----
        Op::inspection_begin => arm_inspection_begin(a),
        Op::inspection_end => arm_inspection_end(a),
        Op::reveal_pile => arm_reveal_pile(a),
        Op::reveal_card => arm_reveal_card(a),

        // ---- outcome / cause / world surface (G10) ----
        Op::trig_by_card_instance => world::with_world(|w| w.trig_by_card_instance as i64),
        Op::trig_redirected_from => world::with_world(|w| w.trig_redirected_from as i64),
        Op::trig_settle_facts => world::with_world(|w| w.trig_settle_facts as i64),
        Op::trig_settle_paid_value => world::with_world(|w| w.trig_settle_paid_value as i64),
        Op::draw_event_as => arm_draw_event_as(a),
        Op::event_defer => arm_event_defer(a),
        Op::world_snapshot => arm_world_snapshot(a),
        Op::world_restore => arm_world_restore(a),
        Op::world_snapshot_drop => arm_world_snapshot_drop(a),
        Op::discard_next_fx => arm_discard_next_fx(a),
        Op::auction => arm_auction(a),
        Op::persona_skins => arm_persona_skins(a),

        // ---- events ----
        Op::event_is_active => {
            let id = name_id(a0, a1);
            world::with_world(|w| {
                let mut i = 0;
                let mut on = 0;
                while i < w.events_n as usize {
                    if w.events[i] == id.0 {
                        on = 1;
                    }
                    i += 1;
                }
                on
            }) as i64
        }
        Op::event_expire => {
            let id = name_id(a0, a1);
            world::with_world_mut(|w| {
                w.push(Effect::EventExpire { key: id });
            });
            0
        }
        Op::event_deck_push | Op::event_banish => 0,

        // ---- prompts ----
        Op::opt_int | Op::opt_str | Op::opt_tile => arm_opt(a),
        Op::opt_price | Op::opt_ai => arm_opt_price(a),
        // v97: multi-select -- stores the count on the prompt; not an option,
        // so it must not bump `opt_n` (that would widen the symbolic answer
        // range past the real option list).
        Op::opt_count => 0,
        Op::ask => arm_ask(a),

        // ---- trigger context ----
        Op::trig_kind => arm_trig_kind(a),
        Op::trig_player => arm_trig_player(a),
        Op::trig_value => arm_trig_value(a),
        Op::trig_target => arm_trig_target(a),
        Op::trig_tile => arm_trig_tile(a),
        // No slot in the model: the position reads as the tile (§5.7).
        Op::trig_position => arm_trig_tile(a),
        Op::trig_name => arm_trig_name(a),
        Op::trig_step => 0,
        Op::trig_by_card => 0,
        // Raise identity + resolution frame + per-instance cap are stubbed
        // until the model grows them (`trig_uid` 0 = "no identity",
        // `trig_instance` -1 = "none", `body_enter` 1 = "run the body").
        Op::trig_uid => 0,
        Op::trig_instance => -1,
        Op::body_enter => 1,
        Op::body_leave => 0,
        Op::counter_max_at => 0,
        Op::set_counter_max_at => 0,
        // ---- effective card + offer claim (G05 §5, §6.2) ----
        Op::set_fixed_roll => arm_set_fixed_roll(a),
        Op::fixed_roll => arm_fixed_roll(a),
        Op::effective_card_at => arm_effective_card_at(a),
        Op::set_effective_card_at => arm_set_effective_card_at(a),
        Op::plan_claimed => arm_plan_claimed(a),
        Op::plan_claim => arm_plan_claim(a),
        Op::trig_pay_is_rent => world::with_world(|w| w.trig_pay_is_rent as i64),
        Op::trig_cancelled => world::with_world(|w| w.trig_cancelled as i64),
        // ---- move payload (hostfns 1830–1880; sentinels when no move) ----
        Op::trig_move_kind => world::with_world(|w| {
            if w.move_snap.present() {
                w.move_snap.kind as i64
            } else {
                -1
            }
        }),
        Op::trig_move_settles => world::with_world(|w| w.move_snap.settles as i64),
        Op::trig_move_tag => {
            let key = name_id(a0, a1);
            world::with_world(|w| w.move_snap.tags.get(key)) as i64
        }
        // `trig_move_main` is `Trigger::main`, not `mv.main` (hostfns 1848).
        Op::trig_move_main => arm_trig_move_main(a),
        // `trig_settle_redirect` is `TriggerMove::settle_redirect` (hostfns).
        Op::trig_settle_redirect => world::with_world(|w| w.move_snap.settle_redirect as i64),
        Op::trig_move_dir => world::with_world(|w| {
            if w.move_snap.present() {
                w.move_snap.dir as i64
            } else {
                1
            }
        }),
        // No path in the model: length 0, every index 0 (§5.7).
        Op::trig_move_path_length => 0,
        Op::trig_move_path => 0,
        Op::trig_move_remaining => world::with_world(|w| {
            if w.move_snap.present() {
                w.move_snap.remaining as i64
            } else {
                0
            }
        }),
        Op::trig_move_from => world::with_world(|w| {
            if w.move_snap.present() {
                w.move_snap.from as i64
            } else {
                -1
            }
        }),
        Op::trig_move_total => world::with_world(|w| {
            if w.move_snap.present() {
                w.move_snap.total as i64
            } else {
                0
            }
        }),
        Op::trig_move_rolled => {
            world::with_world(|w| (w.move_snap.present() && w.move_snap.rolled) as i64)
        }
        Op::trig_move_effective_steps => arm_trig_move_effective_steps(a),
        // ---- v108 plan reads ----
        Op::move_effective_steps => arm_move_effective_steps(a),
        Op::move_forced => world::with_world(|w| w.forced as i64),
        Op::move_planned_teleport => world::with_world(|w| w.teleport as i64),
        Op::move_pay_factor => world::with_world(|w| w.plan_pay_factor) as i64,
        // The model keeps no rent factor; 1000 (x1) is the neutral answer.
        Op::move_rent_factor => 1000,
        // ---- v108 plan setters / walk control / board geometry ----
        Op::set_forced => arm_set_forced(a),
        Op::set_main_override => arm_set_main_override(a),
        Op::void_walk => arm_void_walk(a),
        Op::clear_steps => arm_clear_steps(a),
        Op::set_steps => arm_set_steps(a),
        Op::set_teleport_to => arm_set_teleport_to(a),
        Op::set_parity => arm_set_parity(a),
        Op::add_extra_steps => arm_add_extra_steps(a),
        // The engine resolves the axis readers against the board data; the
        // model board is not enabled here, so they answer "no tile".
        Op::nearest_tile_on_axis | Op::farthest_tile_on_axis => -1,
        // `mv.and_then(|m| m.roll).unwrap_or(-1)` — no-move and no-face are -1.
        Op::trig_move_roll => arm_trig_move_roll(a),
        Op::trig_roll_source => world::with_world(|w| w.trig_roll_source) as i64,
        // ---- buy payload (hostfns 1886–1936) ----
        Op::trig_buy_kind => world::with_world(|w| w.buy_snap.kind) as i64,
        Op::trig_seller => world::with_world(|w| w.buy_snap.seller) as i64,
        Op::trig_price => world::with_world(|w| w.buy_snap.price) as i64,
        Op::trig_deal_owner => world::with_world(|w| w.buy_snap.deal_owner) as i64,
        Op::trig_deal_houses => world::with_world(|w| w.buy_snap.deal_houses) as i64,
        Op::trig_deal_mortgaged => world::with_world(|w| w.buy_snap.deal_mortgaged as i64),
        Op::trig_set_pay_amount => arm_trig_set_pay_amount(a),
        Op::trig_set_pay_target => arm_trig_set_pay_target(a),
        Op::pay_command => arm_pay_command(a),
        Op::set_pay_factor => arm_set_pay_factor(a),
        Op::trig_set_pay_kind => arm_trig_set_pay_kind(a),
        Op::trig_set_pay_shape => arm_trig_set_pay_shape(a),
        Op::multiply_pay_factor => arm_multiply_pay_factor(a),
        Op::pay_factor => {
            world::with_world(|w| w.plan_pay_factor.saturating_mul(w.settle_pay_factor)) as i64
        }
        Op::settle_tile => world::with_world(|w| w.trig_settle_tile) as i64,
        Op::settle_outcome => world::with_world(|w| w.trig_settle_outcome) as i64,
        Op::paid_in_settle_by => world::with_world(|w| w.paid_in_settle_by[player_idx(a0)]) as i64,
        Op::rent_quote => world::with_world(|w| w.rent_quote) as i64,
        Op::trig_set_price => {
            world::with_world_mut(|w| w.buy_snap.price = a0);
            0
        }
        Op::trig_set_move_roll => arm_trig_set_move_roll(a),
        Op::trig_set_deal_owner => {
            world::with_world_mut(|w| w.buy_snap.deal_owner = a0);
            0
        }
        Op::trig_set_deal_houses => {
            world::with_world_mut(|w| w.buy_snap.deal_houses = a0);
            0
        }
        Op::trig_set_deal_mortgaged => {
            world::with_world_mut(|w| w.buy_snap.deal_mortgaged = a0 != 0);
            0
        }
        // 真奈 join (CH-05), faithful to hostfns: the redirect seat is `-1`
        // outside a join; `set_join_used` only records the flag. Who sets the
        // redirect (the join opener's counter window) is PROTOCOL-LEVEL, so a
        // harness that needs a join arranges `w.trig_join_redirect` itself.
        Op::join_redirect_target => world::with_world(|w| w.trig_join_redirect) as i64,
        Op::set_join_used => {
            world::with_world_mut(|w| w.trig_join_used = true);
            0
        }
        // ---- v112 join window / counter facts (shape only) ----
        // No join round is modelled: `open` is an always-applied pause that
        // answers 1 and marks the pass, `shield` sets the same flag from the
        // joiner's side; the report arms are empty and the openers `-1`.
        Op::join_opener | Op::join_joiner | Op::join_designation => -1,
        Op::join_seats
        | Op::trigger_subject_card
        | Op::trigger_link_copied
        | Op::trigger_answered_kind
        | Op::countered_by_this_turn => 0,
        Op::join_shield => {
            world::with_world_mut(|w| w.trig_join_used = true);
            1
        }
        Op::join_open => {
            world::with_world_mut(|w| w.trig_join_opened = true);
            1
        }
        Op::join_opened => world::with_world(|w| w.trig_join_opened as i64),
        Op::join_joiner_count
        | Op::join_joiner_shielded
        | Op::trigger_pending_negation
        | Op::trigger_extreme_armed
        | Op::trigger_chain_depth => 0,
        Op::trig_set_cancelled => arm_trig_set_cancelled(a),
        Op::trig_set_abnormal_allowed => arm_trig_set_abnormal_allowed(a),
        Op::trig_set_negate_effect => arm_trig_set_negate_effect(a),
        Op::trig_set_reason | Op::trig_set_spare => arm_stub_zero(a),
        Op::trig_seq => 0,
        Op::trig_answers => 0,
        Op::trig_effect_count => 0,
        Op::trig_effect_kind
        | Op::trig_effect_target
        | Op::trig_effect_from
        | Op::trig_effect_tile
        | Op::trig_effect_value => 0,
        Op::declare_effect => 0,
        Op::trig_card_is => arm_trig_card_is(a),
        Op::trig_cards => arm_trig_cards(a),

        // ---- move plan / settle (structure only; engine scheduling is
        //      PROTOCOL-LEVEL and out of Kani scope) ----
        Op::set_roller
        | Op::set_reverse
        | Op::set_signed
        | Op::set_stop_at
        | Op::set_settles
        | Op::set_settle_tile
        | Op::set_rent_factor
        | Op::set_no_buy
        | Op::set_no_build
        | Op::set_build_anywhere
        | Op::set_kind
        | Op::set_start
        | Op::set_base_dice
        | Op::add_base_dice
        | Op::add_extra_dice
        | Op::set_min_roll
        | Op::set_extra_steps
        | Op::set_more_steps
        | Op::set_tag
        | Op::set_settle_as_agent
        | Op::plan_add_follower
        | Op::set_play_doubled
        | Op::set_no_money_loss
        | Op::set_next_steps
        | Op::set_build_discount
        | Op::set_build_cost_pct
        | Op::set_extreme
        | Op::clear_dice
        | Op::set_can_build
        | Op::move_stopped
        | Op::move_stop_at
        | Op::move_parity
        | Op::move_settles
        | Op::move_steps
        | Op::move_remaining
        | Op::move_total
        | Op::move_dir
        | Op::turn_main_steps
        | Op::turn_rolls
        | Op::turn_snapshot
        | Op::turn_start_pos
        | Op::schedule_turn_end
        | Op::linger => arm_stub_zero(a),
        Op::targeted_count => world::with_world(|w| {
            let p = a0;
            if p >= 0 && (p as usize) < MAX_PLAYERS {
                w.targeted_count[p as usize] as i64
            } else {
                0
            }
        }),

        // ---- hook-status flags (polarity-sensitive; see arm_*) ----
        // 0 on these means "cancelled / blocked / substituted", which makes
        // callers skip the real effect (class C — chart_rush spent no fire).
        Op::raise => arm_raise(a),
        Op::raise_bought => arm_raise_bought(a),
        Op::gate => arm_gate(a),
        Op::rent_unit_mult => arm_ring_multiplier(a),

        // ---- designation cancel (modelled: one bit, WINDOW-03) ----
        Op::cancel_designation => arm_cancel_designation(a),
        Op::designation_cancelled => arm_designation_cancelled(a),

        // ---- offers / buy / build (PROTOCOL-LEVEL) ----
        Op::pay_rent
        | Op::charge_rent
        | Op::offer_buy
        | Op::offer_force_buy
        | Op::offer_build
        | Op::card_build
        | Op::card_build_as
        | Op::card_buy
        | Op::card_offer_build
        | Op::card_mortgage
        | Op::query_guard
        | Op::declared_arg
        | Op::card_text_mentions
        | Op::card_prop
        | Op::buy
        | Op::acquire
        | Op::agent_offer
        | Op::buy_quotes
        | Op::ai_agent_choice
        | Op::play_card
        | Op::play_from_hand
        | Op::can_build_on
        | Op::add_rent_unit_bonus
        | Op::move_units
        | Op::designations
        | Op::flash
        | Op::no_flash
        | Op::flash_card
        | Op::flash_card_keyed
        | Op::flash_tile => arm_protocol_stub(a),

        // A protocol stub, but recorded: a clause can still be held to "it
        // settles here, and nowhere when it is on no square".
        Op::settle_tile_at => arm_settle_tile_at(a),

        // ---- cross-card messages (`ctx::send` / `On::Message`) ----
        Op::send => arm_send(a),
        Op::msg_sender_uid => arm_msg_sender_uid(a),
        Op::msg_sender_seat => arm_msg_sender_seat(a),
        Op::msg_name => arm_msg_name(a),
        Op::msg_arg0 => arm_msg_a(a),
        Op::msg_arg1 => arm_msg_b(a),
        Op::msg_arg2 => arm_msg_c(a),
        Op::msg_tile => arm_msg_tile(a),
        Op::msg_seat => arm_msg_seat(a),
        Op::msg_text => arm_msg_text(a),
        Op::msg_reply => arm_msg_reply(a),

        other => unmodeled(other),
    }
}

#[allow(dead_code)]
fn _w(_w: World) {}

// ===========================================================================
// Per-harness op-subset handlers
// ===========================================================================
//
// Each `arm_*` is the single implementation of one (group of) host op(s).
// `dispatch` calls them; a **narrow** per-harness dispatcher built by
// [`crate::harness::host_subset!`] calls only the ones the card body can
// reach and sends everything else to [`outside_subset`] (loud FAIL — never
// `assume(false)`). That keeps the CBMC match at a handful of arms instead of
// the full ~300-op `dispatch` (TRIAGE-LOG `op-subset`).

/// `Op::log` / `Op::effect` — narration counter only (not the effect frame).

// ---- merged bands-a + bands-b subset arms (deduped) ----

/// `Op::add_group_pool(player, np, nl, n, max)` — band-skill pool.
/// Args are `(player, name_ptr, name_len, n, max)`; `n` is the delta (a3),
/// not the name handle (a1). Model bound keeps the pool in `0..=99`.
pub fn arm_add_band_crystals(a: Args) -> i64 {
    let [a0, _np, _nl, n, max, ..] = a;
    world::with_world_mut(|w| {
        let p = player_idx(a0);
        let mut now = w.group_pool[p] + n;
        if now < 0 {
            now = 0;
        }
        if max > 0 && now > max {
            now = max;
        }
        w.group_pool[p] = now.clamp(0, 99);
        w.group_pool[p] as i64
    })
}

pub fn arm_add_band_skill(a: Args) -> i64 {
    let _ = a;
    0
}

pub fn arm_add_card_counter(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let key = name_id(a3, a4);
    let n = a5;
    let max = 0;

    world::with_world_mut(|w| {
        let p = w.trig_player;
        let was = if key == known().crystals {
            w.field_crystals(w.last_uid)
        } else {
            w.key_get(p, key)
        };
        let mut now = was + n;
        if now < 0 {
            now = 0;
        }
        if max > 0 && now > max {
            now = max;
        }
        if key == known().crystals {
            w.set_field_crystals(w.last_uid, now);
        } else {
            w.key_set(p, key, now);
        }
        w.push(Effect::AddCounter {
            owner: p,
            key: key.0,
            n: now - was,
        });
        now as i64
    })
}

pub fn arm_add_counter_at(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // `add_counter_at(uid, np, nl, n, max)` — same clamp on `uid`.
        let uid = a0;
        let key = name_id(a1, a2);
        let n = a3;
        let max = a4;
        world::with_world_mut(|w| {
            let was = if key == known().crystals {
                w.field_crystals(uid)
            } else {
                w.field_counter(uid, key)
            };
            let mut now = was + n;
            if now < 0 {
                now = 0;
            }
            if max > 0 && now > max {
                now = max;
            }
            if key == known().crystals {
                w.set_field_crystals(uid, now);
            } else {
                w.set_field_counter(uid, key, now);
            }
            w.push(Effect::AddCounter {
                owner: uid,
                key: key.0,
                n: now - was,
            });
            now as i64
        })
    }
}

pub fn arm_add_counter_self(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let key = name_id(a0, a1);
    let n = a2;
    let max = a3;

    world::with_world_mut(|w| {
        let p = w.trig_player;
        let was = if key == known().crystals {
            w.field_crystals(w.last_uid)
        } else {
            w.key_get(p, key)
        };
        let mut now = was + n;
        if now < 0 {
            now = 0;
        }
        if max > 0 && now > max {
            now = max;
        }
        if key == known().crystals {
            w.set_field_crystals(w.last_uid, now);
        } else {
            w.key_set(p, key, now);
        }
        w.push(Effect::AddCounter {
            owner: p,
            key: key.0,
            n: now - was,
        });
        now as i64
    })
}

pub fn arm_add_held(a: Args) -> i64 {
    let [a0, a1, a2, a3, ..] = a;
    let key = name_id(a0, a1);
    world::with_world_mut(|w| {
        let v = w.key_add(a2, key, a3);
        w.sync_views_player(a2);
        w.push(Effect::AddToken {
            player: a2,
            key: key.0,
            n: a3,
        });
        v as i64
    })
}

pub fn arm_add_house(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        world::with_world_mut(|w| {
            let t = tile_idx(a0);
            w.tiles[t].houses = w.tiles[t].houses.saturating_add(a1);
            w.push(Effect::SetHouses {
                tile: a0,
                n: w.tiles[t].houses,
            });
            w.tiles[t].houses as i64
        })
    }
}

/// `add_to_dest(player, ptr, len, dest)` — pile by the wire `Dest` code.
/// Field / Banished are not piles (`place_card` / "gone"); no-op here.
pub fn arm_add_to_dest(a: Args) -> i64 {
    let [a0, a1, a2, a3, ..] = a;
    match a3 {
        0 => arm_to_discard([a0, a1, a2, 0, 0, 0]),
        1 => arm_add_to_hand([a0, a1, a2, 0, 0, 0]),
        4 | 5 | 6 => {
            let id = card_u16(a1, a2);
            world::with_world_mut(|w| world::pile_push(&mut w.deck_pile, &mut w.deck_n, id));
            0
        }
        _ => 0,
    }
}

pub fn arm_add_to_hand(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        let id = card_u16(a1, a2);
        let slot = card_slot(a1, a2);
        world::with_world_mut(|w| {
            world::pile_push(&mut w.hand_pile, &mut w.hand_n, id);
            if (slot as usize) < 4 {
                w.hand[slot as usize] = true;
                if slot == 1 {
                    w.hand_resonance[player_idx(a0)] += 1;
                }
            }
            w.push(Effect::AddToHand {
                player: a0,
                card: slot,
            });
        });
        0
    }
}

pub fn arm_adjust_money(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // Raw money adjustment: a gain is `money += delta` with **no** payment
        // pipeline. A loss is a 消耗: it first raises the `spendBefore` window
        // at the full cost (G02 §2.21, I5), so a responder can reduce it with
        // `set_pay_amount`; the fold below is the settled debit. Mirrors
        // `hostfns::adjust_money` / the `AdjustMoney` arm of
        // `apply_host_request_body`.
        let delta = a1;
        if delta != 0 {
            let mut cost = delta;
            if delta < 0 {
                let name = intern::intern("spendBefore");
                let _ = arm_raise([a0, intern::encode_handle(name), 0, -delta, 0, 0]);
                cost = -fold_spend_before(-delta);
            }
            world::with_world_mut(|w| {
                let p = player_idx(a0);
                if cost > 0 {
                    w.money[p] = w.money[p].saturating_add(cost);
                    w.gains_this_turn[p] = w.gains_this_turn[p].saturating_add(cost);
                    w.push(Effect::GainMoney {
                        player: a0,
                        n: cost,
                    });
                } else if cost < 0 {
                    let n = (-cost).min(w.money[p].max(0));
                    w.money[p] -= n;
                }
            });
        }
        world::with_world(|w| w.money[player_idx(a0)]) as i64
    }
}

pub fn arm_ask(a: Args) -> i64 {
    let _ = a;
    let n = world::with_world(|w| w.opt_n.max(1));
    world::with_world_mut(|w| w.opt_n = 0);
    #[cfg(kani)]
    {
        if world::with_world(|w| w.symbolic_answers) {
            let x: i32 = kani::any();
            kani::assume(x >= 0 && x < n);
            return x as i64;
        }
    }
    world::with_world_mut(|w| {
        let i = w.next_answer;
        if i < world::MAX_ANSWERS {
            let v = w.answers[i];
            w.next_answer = i + 1;
            v as i64
        } else {
            0
        }
    })
}

pub fn arm_band_crystals(a: Args) -> i64 {
    let [a0, ..] = a;
    world::with_world(|w| w.group_pool[player_idx(a0)]) as i64
}

pub fn arm_band_skill(a: Args) -> i64 {
    let _ = a;
    {
        // Return 0 = no skill id (string returns go through out-buffers in
        // the real ABI; the model answers "none").
        0
    }
}

/// `group_skills` -- postcard `Vec<(uid, id, extra)>` of the player's attached
/// skill instances (any field row named `skill:…`). Empty when none is planted,
/// which is what every older harness saw from the `0` stub.
pub fn arm_band_skills(a: Args) -> i64 {
    let [a0, buf, cap, ..] = a;
    let list: Vec<(i32, String, i32)> = world::with_world(|w| {
        let mut v = Vec::new();
        let mut i = 0usize;
        while i < w.field_n as usize {
            if w.field[i].player == a0 {
                let name = intern::resolve_str(intern::NameId(w.field[i].card));
                if name.starts_with("skill:") {
                    v.push((w.field[i].uid, name.to_string(), 0));
                }
            }
            i += 1;
        }
        v
    });
    let bytes = postcard::to_allocvec(&list).unwrap_or_default();
    if bytes.is_empty() {
        return 0;
    }
    if buf == 0 {
        return bytes.len() as i64;
    }
    if (bytes.len() as i32) > cap {
        return -1;
    }
    card_sdk::native::write(buf, &bytes);
    bytes.len() as i64
}

/// `persona_skill` -- the seat's character-skill id, 0 (none) while
/// [`World::persona_skill`] is 0.
///
/// Kani wire: two LE bytes of the interned [`intern::NameId`] (`buf == 0` is
/// the probe). The guest's `cfg(kani)` twin asks for exactly that, so neither
/// side runs postcard's byte-at-a-time UTF-8 validator over the id.
#[cfg(kani)]
pub fn arm_persona_skill(a: Args) -> i64 {
    let [_a0, buf, cap, ..] = a; // one bound skill per seat, the persona's own
    let id = world::with_world(|w| w.persona_skill);
    if id == 0 {
        return 0;
    }
    if buf == 0 {
        return 2;
    }
    if cap < 2 {
        return -1;
    }
    card_sdk::native::write(buf, &id.to_le_bytes());
    2
}

/// Production wire: postcard `String` (the bytes the real host writes).
#[cfg(not(kani))]
pub fn arm_persona_skill(a: Args) -> i64 {
    let [_a0, buf, cap, ..] = a; // one bound skill per seat, the persona's own
    let bytes = world::with_world(|w| {
        if w.persona_skill == 0 {
            Vec::new()
        } else {
            let s = intern::resolve_str(intern::NameId(w.persona_skill)).to_string();
            postcard::to_allocvec(&s).unwrap_or_default()
        }
    });
    if bytes.is_empty() {
        return 0;
    }
    if buf == 0 {
        return bytes.len() as i64;
    }
    if (bytes.len() as i32) > cap {
        return -1;
    }
    card_sdk::native::write(buf, &bytes);
    bytes.len() as i64
}

pub fn arm_build_cost(a: Args) -> i64 {
    let [a0, ..] = a;
    let s = board::tile_slot(a0);
    if s < 0 {
        0
    } else {
        board::TILES[s as usize].house_cost as i64
    }
}

pub fn arm_buy_price(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        let s = board::tile_slot(a0);
        if s < 0 {
            0
        } else {
            // Faithful `purchase::quote_native`: land + houses * house_cost
            // (see purchase.rs).
            let h = world::with_world(|w| w.tiles[tile_idx(a0)].houses).max(0);
            (board::TILES[s as usize].price + h * board::TILES[s as usize].house_cost) as i64
        }
    }
}

pub fn arm_can_build_on(_a: Args) -> i64 {
    0
}

pub fn arm_can_pay(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| (w.money[player_idx(a0)] > 0) as i64)
}

pub fn arm_card_counter(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let key = name_id(a3, a4);
    world::with_world(|w| {
        if key == known().crystals {
            w.field_crystals(w.last_uid)
        } else {
            w.key_get(w.trig_player, key)
        }
    }) as i64
}

// ---- v114 (G12) status / blocked-ness / immunity -------------------------

/// `trigger::activation_ordinal` — the running activation stamp.
pub fn arm_trigger_activation_ordinal(_a: Args) -> i64 {
    world::with_world(|w| w.turn_activation_ordinal) as i64
}

/// `trigger::abnormal_blocked` — the raise's blocked-ness bit.
pub fn arm_trigger_abnormal_blocked(_a: Args) -> i64 {
    world::with_world(|w| w.trig_abnormal_blocked as i64)
}

/// `trigger::move_abnormal` — the in-flight frame's span bit, OR the turn
/// latch `end_plan` copies it into (`turn.move_abnormal`).
pub fn arm_trigger_move_abnormal(_a: Args) -> i64 {
    world::with_world(|w| (w.move_snap.abnormal || w.turn_move_abnormal) as i64)
}

/// `cant_move_code_for_activation(player, activation)` — the engine's order,
/// turn → skip → spent, with "spent" suppressed for the asking activation.
pub fn arm_cant_move_code_for_activation(a: Args) -> i64 {
    let [a0, a1, ..] = a;
    world::with_world(|w| {
        g12_status::cant_move_code(
            w.turn_player,
            a0,
            w.skip_move,
            w.main_moved,
            w.move_activation_ordinal,
            a1,
        ) as i64
    })
}

/// `cant_move(player)` — the same code against the running activation.
pub fn arm_cant_move(a: Args) -> i64 {
    let [a0, ..] = a;
    world::with_world(|w| {
        g12_status::cant_move_code(
            w.turn_player,
            a0,
            w.skip_move,
            w.main_moved,
            w.move_activation_ordinal,
            w.turn_activation_ordinal,
        ) as i64
    })
}

/// `skip_main_move(player, locked)` / `clear_move_skip(player)` — the turn's
/// move latch (`World.st.skip_move`), the `3` branch of `cant_move`.
pub fn arm_skip_main_move(a: Args) -> i64 {
    world::with_world_mut(|w| w.skip_move = a[1] != 0);
    0
}

pub fn arm_clear_move_skip(_a: Args) -> i64 {
    world::with_world_mut(|w| w.skip_move = false);
    0
}

/// `is_immune_from_at(uid, by)` — false without an instance, else the
/// any-flavour flag or the except-owner flavour for a foreign causer.
pub fn arm_is_immune_from_at(a: Args) -> i64 {
    let [a0, a1, ..] = a;
    world::with_world(|w| {
        w.field_by_uid(a0).is_some_and(|f| {
            g12_status::is_immune_from(f.immune, f.immune_except_owner, f.player, a1)
        }) as i64
    })
}

/// `set_self_immune_except_owner(on)` — mirrors `set_self_immune`: writes the
/// model's running instance (slot 0) and answers whether it exists.
pub fn arm_set_self_immune_except_owner(a: Args) -> i64 {
    world::with_world_mut(|w| {
        if w.field_n == 0 {
            0
        } else {
            w.field[0].immune_except_owner = a[0] != 0;
            1
        }
    })
}

/// `dice_immune()` — the turn's flag.
pub fn arm_dice_immune(_a: Args) -> i64 {
    world::with_world(|w| w.dice_immune as i64)
}

/// `set_dice_immune(on)` — sets the turn flag and answers its previous value.
pub fn arm_set_dice_immune(a: Args) -> i64 {
    world::with_world_mut(|w| {
        let prev = w.dice_immune;
        w.dice_immune = a[0] != 0;
        prev as i64
    })
}

/// `run_planned_move` — `1` ran, `0` refused.
///
/// The engine's decision (`G04-move-plan` §4.10): a main move of the turn
/// player whose slot is spent is refused, unless the call replaces the open
/// walk (`window && main_override && void_walk && teleport` — the inlined
/// `move_plan::replacement_taken`). An accepted main move latches the slot.
pub fn arm_card_move(a: Args) -> i64 {
    let player = a[0];
    world::with_world_mut(|w| {
        let is_main_move = player == w.turn_player && (!w.forced || w.main_override);
        if is_main_move
            && w.main_moved
            && !(w.window && w.main_override && w.void_walk && w.teleport)
        {
            0
        } else {
            if is_main_move {
                w.main_moved = true;
                // The move is latched to the activation that spent it
                // (`play.rs`, both the direct and the landing path), so a
                // replay of the same routine re-reads it as its own.
                w.move_activation_ordinal = w.turn_activation_ordinal;
            }
            1
        }
    })
}

pub fn arm_card_offer_build(_a: Args) -> i64 {
    0
}

pub fn arm_cards_in(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // Bounded layout `[n, id…]` under Kani (see ctx::cards_in). The
        // guest rebuilds names from interned ids via the same table.
        let mut ids: [u8; world::MAX_CARDS] = [0; world::MAX_CARDS];
        let mut n = 0usize;
        world::with_world(|w| {
            if a1 == 1 {
                // Discard: pilot bitset first, then pile.
                let mut s = 0u8;
                while s < 4 {
                    if w.discard[s as usize] && n < world::MAX_CARDS {
                        ids[n] = s;
                        n += 1;
                    }
                    s += 1;
                }
                let mut i = 0usize;
                while i < w.discard_n as usize && n < world::MAX_CARDS {
                    let id = intern::NameId(w.discard_pile[i]);
                    // NameId compare against the pre-interned pilot ids —
                    // no byte walk. Unknown ids fall back to the zero-loop
                    // byte heuristic.
                    let slot = card_slot_of_id(id);
                    // de-dup against bitset slots already emitted
                    let mut seen = false;
                    let mut j = 0usize;
                    while j < n {
                        if ids[j] == slot {
                            seen = true;
                        }
                        j += 1;
                    }
                    if !seen {
                        ids[n] = slot;
                        n += 1;
                    }
                    i += 1;
                }
            } else if a1 == 0 {
                let p = player_idx(a0);
                let res = w.hand_resonance[p].min(2);
                let mut k = 0;
                while k < res && n < world::MAX_CARDS {
                    ids[n] = 1;
                    n += 1;
                    k += 1;
                }
                let mut s = 0u8;
                while s < 4 {
                    if s != 1 && w.hand[s as usize] && n < world::MAX_CARDS {
                        ids[n] = s;
                        n += 1;
                    }
                    s += 1;
                }
            } else if a1 == 3 {
                // Field: cards placed by `a0`, in placement order.
                let mut i = 0usize;
                while i < w.field_n as usize && n < world::MAX_CARDS {
                    if w.field[i].player == a0 {
                        ids[n] = card_slot_of_id(intern::NameId(w.field[i].card));
                        n += 1;
                    }
                    i += 1;
                }
            } else if a1 == 2 {
                // Deck: draw pile, top card first.
                let mut i = 0usize;
                while i < w.deck_n as usize && n < world::MAX_CARDS {
                    ids[n] = card_slot_of_id(intern::NameId(w.deck_pile[i]));
                    n += 1;
                    i += 1;
                }
            } else if a1 == 4 {
                // Inspection frame: revealed entries only, front-first.
                let mut i = 0usize;
                while i < w.inspection_n as usize && n < world::MAX_CARDS {
                    if w.inspection_revealed[i] {
                        ids[n] = card_slot_of_id(intern::NameId(w.inspection_pile[i]));
                        n += 1;
                    }
                    i += 1;
                }
            }
        });
        let mut encoded = [0u8; 16];
        encoded[0] = n as u8;
        let mut i = 0usize;
        while i < n {
            encoded[1 + i] = ids[i];
            i += 1;
        }
        let need = (1 + n) as i32;
        if a2 == 0 {
            return need as i64;
        }
        if a3 < need {
            return -1;
        }
        card_sdk::native::write(a2, &encoded[..need as usize]);
        need as i64
    }
}

/// `persona` -- the seat's character card name id, 0 (none) while
/// [`World::persona_id`] is `NameId::INVALID` (`0` is a real intern slot).
///
/// Kani wire: two LE bytes of the interned name id (`buf == 0` is the probe),
/// mirroring the `cfg(kani)` guest twin of [`bandori_std::persona`].
pub fn arm_persona(a: Args) -> i64 {
    let [a0, buf, cap, ..] = a;
    let id = world::with_world(|w| w.persona_id[player_idx(a0)]);
    if id == u16::MAX {
        return 0;
    }
    if buf == 0 {
        return 2;
    }
    if cap < 2 {
        return -1;
    }
    card_sdk::native::write(buf, &id.to_le_bytes());
    2
}

pub fn arm_character_is(a: Args) -> i64 {
    let [a0, a1, a2, ..] = a;
    let want = name_id(a1, a2);
    world::with_world(|w| (w.persona_id[player_idx(a0)] == want.0) as i64)
}

pub fn arm_set_persona(a: Args) -> i64 {
    let [a0, a1, a2, ..] = a;
    let name = name_id(a1, a2);
    world::with_world_mut(|w| {
        w.persona_id[player_idx(a0)] = name.0;
        // The name brings its skill, like the real host's `bind_skills`. Only
        // the two Sumimi faces are modelled; any other name answers "none".
        w.persona_skill = persona_skill_id(name);
    });
    0
}

/// The persona skill a face binds. The pilot set holds the Sumimi pair the
/// Two-in-One proofs swap between; every other name maps to 0 (none).
fn persona_skill_id(name: intern::NameId) -> u16 {
    let k = known();
    if name == k.face_mana {
        k.skill_mana.0
    } else if name == k.face_uika {
        k.skill_uika.0
    } else {
        0
    }
}

/// `card_prop` -- one declared static prop of a card rule, read off the real
/// [`card_sdk::CardDef::props`] (the authored number, not a copy in the host).
/// Pilot set: the two Sumimi persona skills the swap reads its fire bounds
/// from (`fire::FIRE_INITIAL` / `FIRE_CAP`); any other id answers 0. Every
/// name is a pre-interned `u16` compare (no interning in the arm — the CBMC
/// cost of hashing here showed up as a timeout).
pub fn arm_card_prop(a: Args) -> i64 {
    // The `card_id!` table is feature-gated; the arm must still compile for
    // units that do not enable the band (0 never matches a real table id).
    #[cfg(feature = "band-sumimi")]
    const UIKA_SKILL: usize = crate::card_id!("skill:三角初华（Sumimi）:成为偶像");
    #[cfg(not(feature = "band-sumimi"))]
    const UIKA_SKILL: usize = 0;
    #[cfg(feature = "band-sumimi")]
    const MANA_SKILL: usize = crate::card_id!("skill:纯田真奈:甜甜圈爱好者");
    #[cfg(not(feature = "band-sumimi"))]
    const MANA_SKILL: usize = 0;

    let [ip, il, kp, kl, ..] = a;
    let id = name_id(ip, il);
    let key = name_id(kp, kl);
    let k = known();
    let idx = if id == k.skill_uika {
        UIKA_SKILL
    } else if id == k.skill_mana {
        MANA_SKILL
    } else {
        return 0;
    };
    let want = if key == k.fire_initial {
        bandori_std::fire::FIRE_INITIAL
    } else if key == k.fire_cap {
        bandori_std::fire::FIRE_CAP
    } else {
        return 0;
    };
    let mut v = 0;
    for (name, n) in crate::invoke::by_idx(idx).props {
        if *name == want {
            v = *n;
        }
    }
    v as i64
}

pub fn arm_count_held(a: Args) -> i64 {
    let [a0, a1, a2, ..] = a;
    let key = name_id(a0, a1);
    let p = a2;
    world::with_world(|w| {
        let pp = player_idx(p);
        if key == known().tok_pp_pos {
            w.fans_pos[pp]
        } else if key == known().tok_pp_neg {
            w.fans_neg[pp]
        } else if key == known().tok_charge {
            // Fire lives on the same store `gain_marker` writes.
            w.charge[pp]
        } else {
            w.key_get(p, key)
        }
    }) as i64
}

pub fn arm_count_marks_f(a: Args) -> i64 {
    let [a0, ..] = a;
    world::with_world(|w| {
        let mut n = 0;
        let mut i = 0;
        while i < w.marks_n as usize {
            if w.marks[i].tile == a0 {
                n += w.marks[i].n;
            }
            i += 1;
        }
        n
    }) as i64
}

pub fn arm_counter_at(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // `counter_at(uid, np, nl)` — the **uid** instance's counter.
        let uid = a0;
        let key = name_id(a1, a2);
        world::with_world(|w| {
            if key == known().crystals {
                w.field_crystals(uid)
            } else {
                w.field_counter(uid, key)
            }
        }) as i64
    }
}

pub fn arm_counter_self(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let key = name_id(a0, a1);
    world::with_world(|w| {
        if key == known().crystals {
            w.field_crystals(w.last_uid)
        } else {
            w.key_get(w.trig_player, key)
        }
    }) as i64
}

pub fn arm_deck_count(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.deck_n as i32) as i64
}

pub fn arm_discard_from_hand(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        let slot = card_slot(a1, a2);
        let id = card_u16(a1, a2);
        world::with_world_mut(|w| {
            let ok = world::pile_take(&mut w.hand_pile, &mut w.hand_n, id)
                || ((slot as usize) < 4 && {
                    let b = w.hand[slot as usize];
                    w.hand[slot as usize] = false;
                    if slot == 1 && w.hand_resonance[player_idx(a0)] > 0 {
                        w.hand_resonance[player_idx(a0)] -= 1;
                        true
                    } else {
                        b
                    }
                });
            if ok {
                world::pile_push(&mut w.discard_pile, &mut w.discard_n, id);
                if (slot as usize) < 4 {
                    w.discard[slot as usize] = true;
                }
                w.push(Effect::DiscardFromHand {
                    player: a0,
                    card: slot,
                });
            }
            ok as i64
        })
    }
}

pub fn arm_discard_size(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.discard_n as i32) as i64
}

pub fn arm_dist(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // Forward distance on a 60-tile ring (hostfns `dist`).
        let n = world::TILE_COUNT;
        let d = (a1 - a0).rem_euclid(n);
        d as i64
    }
}

/// Shared face producer for `Op::roll` / `Op::roll_direct`. `purpose` is the
/// ABI's `RollPurpose` wire code (0 = Effect `count`d`sides`, 1 = Move plan
/// tables). Move rolls used to be a dedicated `do_move_roll` import; they now
/// arrive as `roll_direct` with purpose 1 and the args' count/sides ignored.
pub fn arm_roll(a: Args) -> i64 {
    let [_player_id, count, sides, purpose, _source, _] = a;
    if purpose == 1 {
        arm_do_move_roll(a)
    } else {
        let v = roll_dice(count, sides);
        world::with_world_mut(|w| w.roll_calls = w.roll_calls.saturating_add(1));
        v as i64
    }
}

/// Concrete face of 1 — isolation for harnesses that pin the die so a
/// downstream `Vec` walk (rainday's agent-area build) sees a single branch.
/// Not a silent-zero: a real 1d10 face.
pub fn arm_roll_face_one(_a: Args) -> i64 {
    1
}

pub fn arm_do_move_roll(a: Args) -> i64 {
    let _ = a;
    roll_dice(2, 6) as i64
}

/// Precedence rank of a `roll_source` code (`RollSource::rank`).
fn source_rank(code: i32) -> i32 {
    match code {
        1 => 3, // Charge
        3 => 2, // Skill
        2 => 1, // Card
        _ => 0, // Unattributed
    }
}

/// Push one symbolic cast onto the transaction at `id`, returning its face.
fn roll_cast(w: &mut world::World, id: i32, source: i32) -> Option<i32> {
    let mut i = 0usize;
    while i < w.roll_depth as usize {
        if w.rolls[i].open && w.rolls[i].id == id {
            let t = &mut w.rolls[i];
            // Codes are not in rank order (charge 1 outranks skill 3 outranks
            // card 2): mirror `RollSource::rank`, as `cast` does.
            if source_rank(source) > source_rank(t.source) {
                t.source = source;
            }
            if (t.n as usize) >= 4 {
                return None;
            }
            let v = roll_dice(1, 20);
            t.casts[t.n as usize] = world::RollCastM {
                value: v,
                source: t.source,
            };
            t.n += 1;
            t.chosen = t.n - 1;
            t.unused = -1;
            let n = w.roll_hist_n as usize;
            if n < 8 {
                w.roll_hist[n] = (t.roller, v);
                w.roll_hist_n += 1;
            }
            return Some(v);
        }
        i += 1;
    }
    None
}

/// `ctx::roll_recast` -- one more cast on an open transaction.
pub fn arm_roll_recast(a: Args) -> i64 {
    let [id, source, ..] = a;
    world::with_world_mut(|w| roll_cast(w, id, source).unwrap_or(0) as i64)
}

/// `ctx::roll_move` -- a top-level victim move transaction, symbolic face.
pub fn arm_roll_move(a: Args) -> i64 {
    let [player_id, source, ..] = a;
    world::with_world_mut(|w| {
        let d = if w.roll_depth < 2 { w.roll_depth } else { 1 };
        let id = w.next_roll_id;
        w.next_roll_id += 1;
        let v = roll_dice(1, 20);
        let mut t = world::RollTx::EMPTY;
        t.id = id;
        t.roller = player_id;
        t.purpose = 1;
        t.source = source;
        t.has_result = true;
        t.open = true;
        t.casts[0] = world::RollCastM { value: v, source };
        t.n = 1;
        w.rolls[d as usize] = t;
        if w.roll_depth < 2 {
            w.roll_depth += 1;
        }
        let n = w.roll_hist_n as usize;
        if n < 8 {
            w.roll_hist[n] = (player_id, v);
            w.roll_hist_n += 1;
        }
        v as i64
    })
}

/// `ctx::roll_pick` -- clamp, record the other face of the pair as unused.
pub fn arm_roll_pick(a: Args) -> i64 {
    let [id, index, ..] = a;
    world::with_world_mut(|w| {
        let mut i = 0usize;
        while i < w.roll_depth as usize {
            if w.rolls[i].open && w.rolls[i].id == id {
                let t = &mut w.rolls[i];
                if t.n == 0 {
                    return 0;
                }
                let idx = index.max(0) as usize;
                let idx = if idx >= t.n as usize {
                    t.n as usize - 1
                } else {
                    idx
                };
                let old = t.chosen as usize;
                if idx != old {
                    t.unused = t.casts[old].value;
                    t.chosen = idx as u8;
                }
                return 0;
            }
            i += 1;
        }
        0
    })
}

/// `ctx::roll_current` -- the innermost open transaction id.
pub fn arm_roll_current(_a: Args) -> i64 {
    world::with_world(|w| {
        if w.roll_depth == 0 {
            -1
        } else {
            w.rolls[w.roll_depth as usize - 1].id as i64
        }
    })
}

pub fn arm_roll_face(a: Args) -> i64 {
    let [id, index, ..] = a;
    world::with_world(|w| {
        let mut i = 0usize;
        while i < w.roll_depth as usize {
            if w.rolls[i].open && w.rolls[i].id == id {
                let t = &w.rolls[i];
                if (index.max(0) as usize) < t.n as usize {
                    return t.casts[index.max(0) as usize].value as i64;
                }
                return -1;
            }
            i += 1;
        }
        -1
    })
}

pub fn arm_roll_unused_face(a: Args) -> i64 {
    let [id, ..] = a;
    world::with_world(|w| {
        let mut i = 0usize;
        while i < w.roll_depth as usize {
            if w.rolls[i].open && w.rolls[i].id == id {
                return w.rolls[i].unused as i64;
            }
            i += 1;
        }
        -1
    })
}

pub fn arm_roll_rerolled(a: Args) -> i64 {
    let [id, ..] = a;
    world::with_world(|w| {
        let mut i = 0usize;
        while i < w.roll_depth as usize {
            if w.rolls[i].open && w.rolls[i].id == id {
                return (w.rolls[i].n > 1) as i64;
            }
            i += 1;
        }
        0
    })
}

pub fn arm_rolls_by_player(a: Args) -> i64 {
    let [player_id, buf, cap, ..] = a;
    let mut rolls: Vec<i32> = Vec::new();
    world::with_world(|w| {
        let mut i = 0usize;
        while i < w.roll_hist_n as usize {
            if w.roll_hist[i].0 == player_id {
                rolls.push(w.roll_hist[i].1);
            }
            i += 1;
        }
    });
    let bytes = postcard::to_allocvec(&rolls).unwrap_or_default();
    if buf == 0 {
        return bytes.len() as i64;
    }
    if (bytes.len() as i32) > cap {
        return -1;
    }
    card_sdk::native::write(buf, &bytes);
    bytes.len() as i64
}

pub fn arm_trigger_roll(a: Args) -> i64 {
    let [buf, cap, ..] = a;
    let payload = world::with_world(|w| {
        w.trig_roll.map(|t| {
            let mut casts: Vec<card_sdk::abi::TriggerCast> = Vec::new();
            let mut i = 0usize;
            while i < t.n as usize {
                casts.push(card_sdk::abi::TriggerCast {
                    value: t.casts[i].value,
                    source: t.casts[i].source,
                });
                i += 1;
            }
            card_sdk::abi::RollPayload {
                id: t.id,
                purpose: t.purpose,
                source: t.source,
                rerolled: t.n > 1,
                chosen_index: t.chosen as i32,
                unused_face: t.unused,
                casts,
                final_face: (t.final_face >= 0).then_some(t.final_face),
            }
        })
    });
    let bytes = postcard::to_allocvec(&payload).unwrap_or_default();
    if buf == 0 {
        return bytes.len() as i64;
    }
    if (bytes.len() as i32) > cap {
        return -1;
    }
    card_sdk::native::write(buf, &bytes);
    bytes.len() as i64
}

pub fn arm_set_move_settle_answer(a: Args) -> i64 {
    let [player, answer, ..] = a;
    world::with_world_mut(|w| {
        if player >= 0 && (player as usize) < MAX_PLAYERS {
            w.move_answer[player as usize] = answer;
        }
    });
    0
}

pub fn arm_move_settle_answer(a: Args) -> i64 {
    let [player, ..] = a;
    world::with_world(|w| {
        w.move_answer
            .get(player.max(0) as usize)
            .copied()
            .unwrap_or(0)
    }) as i64
}

pub fn arm_set_turn_end_settle_answer(a: Args) -> i64 {
    let [player, answer, ..] = a;
    world::with_world_mut(|w| {
        if player >= 0 && (player as usize) < MAX_PLAYERS {
            w.turn_answer[player as usize] = answer;
        }
    });
    0
}

pub fn arm_turn_end_settle_answer(a: Args) -> i64 {
    let [player, ..] = a;
    world::with_world(|w| {
        w.turn_answer
            .get(player.max(0) as usize)
            .copied()
            .unwrap_or(0)
    }) as i64
}

pub fn arm_draw(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // hostfns::draw: reply is the count drawn. Model: push `n` generic
        // cards into hand (bounded) and record the effect.
        let n = a1.max(0).min(8);
        world::with_world_mut(|w| {
            let mut i = 0;
            while i < n {
                world::pile_push(&mut w.hand_pile, &mut w.hand_n, u16::MAX);
                i += 1;
            }
            if a2 & card_sdk::abi::draw_flag::OVER_HAND_LIMIT != 0 {
                w.push(Effect::DrawOver { player: a0, n });
            } else {
                w.push(Effect::Draw { player: a0, n });
            }
        });
        n as i64
    }
}

pub fn arm_exile_layers(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| w.exile[player_idx(a0)]) as i64
}

pub fn arm_field_instances(a: Args) -> i64 {
    let [a0, buf, cap, ..] = a;
    // Compact pairs under Kani (see `ctx::field_instances` cfg(kani) facade):
    // `buf == 0` → just the player's field-instance count.
    // `buf != 0` → LE `i32` pairs `[uid, name_id]` per instance, return count
    // (or -1 when `cap` is too small). Never postcard — that deserializer is
    // the `twoinone_01b_*` 1800 s cliff (unbounded `Vec<(i32, String)>`).
    const MAX_F: usize = world::MAX_FIELD;
    let mut uids = [0i32; MAX_F];
    let mut names = [0u16; MAX_F];
    let mut n = 0usize;
    world::with_world(|w| {
        let mut i = 0usize;
        while i < w.field_n as usize && n < MAX_F {
            // An attachment (`on >= 0`) is reachable through its host, not
            // listed twice (G08 I6).
            if w.field[i].player == a0 && w.field[i].on < 0 {
                uids[n] = w.field[i].uid;
                names[n] = w.field[i].card;
                n += 1;
            }
            i += 1;
        }
    });
    if buf == 0 {
        return n as i64;
    }
    let need = (n * 8) as i32;
    if cap < need {
        return -1;
    }
    let mut encoded = [0u8; MAX_F * 8];
    let mut i = 0usize;
    while i < n {
        let base = i * 8;
        let ub = uids[i].to_le_bytes();
        let nb = (names[i] as i32).to_le_bytes();
        encoded[base] = ub[0];
        encoded[base + 1] = ub[1];
        encoded[base + 2] = ub[2];
        encoded[base + 3] = ub[3];
        encoded[base + 4] = nb[0];
        encoded[base + 5] = nb[1];
        encoded[base + 6] = nb[2];
        encoded[base + 7] = nb[3];
        i += 1;
    }
    card_sdk::native::write(buf, &encoded[..need as usize]);
    n as i64
}

pub fn arm_fire(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| w.charge[player_idx(a0)]) as i64
}

pub fn arm_marker_max(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| w.charge_max[player_idx(a0)]) as i64
}

pub fn arm_gain(a: Args) -> i64 {
    let [a0, a1, ..] = a;
    if a1 > 0 {
        world::with_world_mut(|w| {
            let p = player_idx(a0);
            w.money[p] = w.money[p].saturating_add(a1);
            w.gains_this_turn[p] = w.gains_this_turn[p].saturating_add(a1);
            w.push(Effect::GainMoney { player: a0, n: a1 });
        });
    }
    a1 as i64
}

pub fn arm_gain_marker(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // engine `gain_marker`: uncapped dumb add on the held-token row.
        // Cap / partial-gain policy lives in the rules-layer kernel
        // (`bandori_std::fire::gain_clamped`); the engine does **not** clamp.
        // 0 when out / n<=0. Returns the amount gained.
        // Wire: (player, name_p, name_l, n, why_p, why_l).
        world::with_world_mut(|w| {
            let p = player_idx(a0);
            let n = a3;
            if n <= 0 || w.out[p] {
                return 0;
            }
            // Uncapped dumb add (production `World::gain_marker`).
            let got = n;
            w.charge[p] = w.charge[p].saturating_add(got);
            w.push(Effect::GainFire { player: a0, n: got });
            got as i64
        })
    }
}

pub fn arm_gains_this_turn(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        world::with_world(|w| w.gains_this_turn[player_idx(a0)]) as i64
    }
}

pub fn arm_gate(a: Args) -> i64 {
    let [a0, ..] = a;
    world::with_world(|w| {
        let out_of_range = a0 < 0 || a0 as usize >= MAX_PLAYERS || a0 >= w.n_players;
        let dead = !out_of_range && w.out[player_idx(a0)];
        if out_of_range || dead {
            0
        } else {
            1
        }
    })
}

/// One generic abnormal-layer write. Wire: (player, kind, n, to).
/// kind 1 = stay, 2 = stun, 3 = exile.
pub fn arm_apply_abnormal(a: Args) -> i64 {
    let [a0, kind, n, to, ..] = a;
    world::with_world_mut(|w| {
        if (a0 as usize) >= MAX_PLAYERS {
            return;
        }
        match kind {
            1 => {
                w.stay[a0 as usize] = (w.stay[a0 as usize] + n).max(0);
                w.push(Effect::GiveStay { player: a0, n });
            }
            2 => {
                w.stun[a0 as usize] = (w.stun[a0 as usize] + n).max(0);
                w.push(Effect::GiveStun { player: a0, n });
            }
            3 => {
                w.exile[a0 as usize] = (w.exile[a0 as usize] + n).max(0);
                w.push(Effect::GiveExile { player: a0, n, to });
            }
            _ => {}
        }
    });
    0
}

pub fn arm_hand_count(a: Args) -> i64 {
    // `hand_count(player, ptr, len)` → [player, ptr, len]: name is (a1,a2),
    // seat is a0. (Was (a0,a1) = (player, ptr) — untagged name_id loud-FAIL.)
    let [a0, a1, a2, ..] = a;
    let slot = card_slot(a1, a2);
    world::with_world(|w| {
        if slot == 1 {
            w.hand_resonance[player_idx(a0)]
        } else {
            let id = card_u16(a1, a2);
            let in_pile = world::pile_count(&w.hand_pile, w.hand_n, id);
            let in_bits = if (slot as usize) < 4 {
                w.hand[slot as usize] as i32
            } else {
                0
            };
            in_pile.max(in_bits)
        }
    }) as i64
}

pub fn arm_hand_size(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| {
        let p = player_idx(a0);
        let mut n = w.hand_resonance[p];
        let mut s = 0usize;
        while s < 4 {
            if s != 1 && w.hand[s] {
                n += 1;
            }
            s += 1;
        }
        n + w.hand_n as i32
    }) as i64
}

pub fn arm_houses_of(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| w.tiles[tile_idx(a0)].houses) as i64
}

pub fn arm_in_band(a: Args) -> i64 {
    let [a0, a1, a2, ..] = a;
    let want = name_id(a1, a2);
    world::with_world(|w| (w.group_id[player_idx(a0)] == want.0) as i64)
}

/// `state_add` -- also the model for its former `inc_slot` sugar (one host
/// op, so one arm; `key_add` already mirrors fan keys via `sync_fan_key`).
pub fn arm_inc_slot(a: Args) -> i64 {
    arm_state_add(a)
}

/// `invoke_entry` -- the nested guest does not run under Kani, so record the
/// call facts (`G13` §5.5) and answer a symbolic `Dest`. `by_instance` is the
/// model's running-instance slot (`last_uid`, the same one `arm_self_uid`
/// answers; a `place` inside the body advances it, as the model's `self_uid`
/// does); `instance` is the target's own field slot, if any.
pub fn arm_invoke_entry(a: Args) -> i64 {
    let [player, card_p, card_n, entry_p, entry_n, flags] = a;
    let card = name_id(card_p, card_n);
    let entry = name_id(entry_p, entry_n);
    let paid = flags & 1 != 0;
    world::with_world_mut(|w| {
        let by_instance = w.last_uid;
        let instance = w
            .field
            .iter()
            .find(|f| f.player == player && (f.card == card.0 || f.effective_card == card.0))
            .map_or(-1, |f| f.uid);
        let depth = w.invoke_n as i32;
        if (w.invoke_n as usize) < world::MAX_INVOKE {
            w.invoke_stack[w.invoke_n as usize] =
                (card, entry, player, by_instance, instance, paid, depth);
        }
        w.invoke_n = w.invoke_n.saturating_add(1);
        w.last_invoke = (card, entry, paid, by_instance);
    });
    #[cfg(kani)]
    {
        // A borrowed body lands on Graveyard or Field; an entry that names no
        // destination answers `DEST_UNSET` (-1).
        let d: i32 = kani::any();
        kani::assume(d == -1 || d == 0 || d == 2);
        d as i64
    }
    #[cfg(not(kani))]
    {
        0
    }
}

/// `invoke_skill` -- the `HostOps` alias of [`arm_invoke_entry`] with the
/// reserved `entry::PRESS` and no flags.
pub fn arm_invoke_skill(a: Args) -> i64 {
    let [player, p, n, ..] = a;
    let entry = intern::encode_handle(intern::intern(card_sdk::abi::entry::PRESS));
    arm_invoke_entry([player, p, n, entry, 0, 0])
}

/// `invoke_frame` -- the innermost recorded frame, LE pair record
/// `[card, entry, by, by_instance, instance, paid, depth]` (24 bytes).
pub fn arm_invoke_frame(a: Args) -> i64 {
    let [buf, cap, ..] = a;
    if world::with_world(|w| w.invoke_n) == 0 {
        return 0;
    }
    const RECORD: usize = 24;
    if buf == 0 {
        return RECORD as i64;
    }
    if cap < RECORD as i32 {
        return -1;
    }
    let frame = world::with_world(|w| {
        let i = (w.invoke_n as usize - 1).min(world::MAX_INVOKE - 1);
        w.invoke_stack[i]
    });
    let (card, entry, by, by_instance, instance, paid, depth) = frame;
    let mut encoded = [0u8; RECORD];
    let cb = card.0.to_le_bytes();
    let eb = entry.0.to_le_bytes();
    encoded[0] = cb[0];
    encoded[1] = cb[1];
    encoded[2] = eb[0];
    encoded[3] = eb[1];
    // Constant offsets (the `arm_field_instances` shape): no dynamic index for
    // CBMC to prove.
    let bb = by.to_le_bytes();
    encoded[4] = bb[0];
    encoded[5] = bb[1];
    encoded[6] = bb[2];
    encoded[7] = bb[3];
    let ib = by_instance.to_le_bytes();
    encoded[8] = ib[0];
    encoded[9] = ib[1];
    encoded[10] = ib[2];
    encoded[11] = ib[3];
    let nb = instance.to_le_bytes();
    encoded[12] = nb[0];
    encoded[13] = nb[1];
    encoded[14] = nb[2];
    encoded[15] = nb[3];
    let pb = (paid as i32).to_le_bytes();
    encoded[16] = pb[0];
    encoded[17] = pb[1];
    encoded[18] = pb[2];
    encoded[19] = pb[3];
    let db = depth.to_le_bytes();
    encoded[20] = db[0];
    encoded[21] = db[1];
    encoded[22] = db[2];
    encoded[23] = db[3];
    card_sdk::native::write(buf, &encoded);
    RECORD as i64
}

/// `ctx::nearest` -- the nearest other present seat(s), LE `i32`s. The
/// selection is §7.5 of G07: raw nearest set in seat order, then (with a key)
/// the matching seats distance-then-seat ordered, deduplicated.
pub fn arm_nearest(a: Args) -> i64 {
    arm_nearest_impl(a, false)
}

/// `ctx::nearest_with_prop` -- `arm_nearest` plus every seat holding an
/// instance whose prop `key` (a `(handle, len)` pair like every name argument)
/// equals `by`.
pub fn arm_nearest_with_prop(a: Args) -> i64 {
    arm_nearest_impl(a, true)
}

fn arm_nearest_impl(a: Args, with_prop: bool) -> i64 {
    let [by, kind, kp, kl, buf, cap] = a;
    let key = if with_prop {
        Some(name_id(kp, kl))
    } else {
        None
    };
    world::with_world(|w| {
        let n_players = w.n_players;
        let at = if by >= 0 && by < n_players {
            w.pos[by as usize]
        } else {
            -1
        };
        if at < 0 {
            return 0;
        }
        let n_tiles = w.tiles.len() as i32;
        if n_tiles <= 0 {
            return 0;
        }
        let shortest = kind == 1;
        let measure = |a: i32, b: i32| {
            let d = ((b - a) % n_tiles + n_tiles) % n_tiles;
            if shortest {
                d.min(n_tiles - d)
            } else {
                d
            }
        };
        let mut raw: [i32; MAX_PLAYERS] = [0; MAX_PLAYERS];
        let mut raw_n = 0usize;
        let mut best = i32::MAX;
        let mut p = 0i32;
        while p < n_players {
            if p != by && !w.out[p as usize] {
                let there = w.pos[p as usize];
                if there >= 0 {
                    let d = measure(at, there);
                    if shortest || d != 0 {
                        if d < best {
                            best = d;
                            raw_n = 0;
                        }
                        if d == best && raw_n < MAX_PLAYERS {
                            raw[raw_n] = p;
                            raw_n += 1;
                        }
                    }
                }
            }
            p += 1;
        }
        let mut extra: [i32; MAX_PLAYERS] = [0; MAX_PLAYERS];
        let mut extra_n = 0usize;
        if let Some(key) = key {
            let mut q = 0i32;
            while q < n_players {
                if q != by && !w.out[q as usize] && w.pos[q as usize] >= 0 {
                    let mut held = false;
                    let mut i = 0usize;
                    while i < w.field_n as usize {
                        let f = &w.field[i];
                        if f.player == q && f.props.get_opt(key) == Some(by) {
                            held = true;
                        }
                        i += 1;
                    }
                    let mut seen = false;
                    let mut r = 0usize;
                    while r < raw_n {
                        if raw[r] == q {
                            seen = true;
                        }
                        r += 1;
                    }
                    if held && !seen && extra_n < MAX_PLAYERS {
                        extra[extra_n] = q;
                        extra_n += 1;
                    }
                }
                q += 1;
            }
            // Insertion sort by (distance, seat): the extra set is tiny.
            let mut i = 1usize;
            while i < extra_n {
                let v = extra[i];
                let kv = (measure(at, w.pos[v as usize]), v);
                let mut j = i;
                while j > 0 {
                    let before = extra[j - 1];
                    if (measure(at, w.pos[before as usize]), before) <= kv {
                        break;
                    }
                    extra[j] = before;
                    j -= 1;
                }
                extra[j] = v;
                i += 1;
            }
        }
        let total = raw_n + extra_n;
        if buf == 0 {
            return total as i64;
        }
        if cap < (total as i32) * 4 {
            return -1;
        }
        let mut encoded = [0u8; MAX_PLAYERS * 4];
        let mut i = 0usize;
        while i < raw_n {
            let b = (raw[i] as i32).to_le_bytes();
            let o = i * 4;
            encoded[o] = b[0];
            encoded[o + 1] = b[1];
            encoded[o + 2] = b[2];
            encoded[o + 3] = b[3];
            i += 1;
        }
        let mut j = 0usize;
        while j < extra_n {
            let b = extra[j].to_le_bytes();
            let o = (raw_n + j) * 4;
            encoded[o] = b[0];
            encoded[o + 1] = b[1];
            encoded[o + 2] = b[2];
            encoded[o + 3] = b[3];
            j += 1;
        }
        card_sdk::native::write(buf, &encoded[..total * 4]);
        total as i64
    })
}

/// `is_own_placement` -- did the running body mint this row? The real host
/// keeps a per-drive uid log; the model marks the row at `make_row`.
pub fn arm_is_placed(a: Args) -> i64 {
    let _ = a;
    // Instance-bound: the *running* uid must have a slot. `field_n > 0` was
    // wrong with a second copy -- or any other card -- on the field.
    world::with_world(|w| (w.last_uid != 0 && w.field.iter().any(|f| f.uid == w.last_uid)) as i64)
}

/// `effective_card_at` -- LE `u16` id of the surface card (2 bytes), 0 when
/// the instance is not in play; `buf == 0` is the size probe.
pub fn arm_effective_card_at(a: Args) -> i64 {
    let [a0, buf, cap, ..] = a;
    let id = world::with_world(|w| {
        w.field.iter().find(|f| f.uid == a0).map(|f| {
            // An unmarked instance answers its own card (`u16::MAX` is the
            // model's "no marker", as in `arm_placed_cards`).
            if f.effective_card == u16::MAX {
                f.card
            } else {
                f.effective_card
            }
        })
    });
    let Some(id) = id else { return 0 };
    if buf == 0 {
        return 2;
    }
    if cap < 2 {
        return -1;
    }
    card_sdk::native::write(buf, &id.to_le_bytes());
    2
}

/// `set_effective_card_at` -- `INVALID` clears; 0 when the uid has no slot.
pub fn arm_set_effective_card_at(a: Args) -> i64 {
    let [a0, p, n, ..] = a;
    let id = name_id(p, n);
    // The engine clears on the empty surface (`""`), which interns to a valid
    // id -- `is_valid` alone would keep a marker to the empty name.
    let clear = !id.is_valid() || intern::resolve_str(id).is_empty();
    world::with_world_mut(|w| {
        if let Some(f) = w.field.iter_mut().find(|f| f.uid == a0) {
            f.effective_card = if clear { u16::MAX } else { id.0 };
            1
        } else {
            0
        }
    })
}

pub fn arm_rent_of(a: Args) -> i64 {
    let [a0, ..] = a;
    world::with_world(|w| w.rent_of(a0)) as i64
}

pub fn arm_linger(_a: Args) -> i64 {
    0
}

pub fn arm_log(a: Args) -> i64 {
    let [a0, ..] = a;
    world::with_world_mut(|w| w.push_narration(a0, 0));
    0
}

pub fn arm_money(a: Args) -> i64 {
    let [a0, ..] = a;
    world::with_world(|w| w.money[player_idx(a0)]) as i64
}

pub fn arm_mortgaged_of(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        world::with_world(|w| w.tiles[tile_idx(a0)].mortgaged) as i64
    }
}

pub fn arm_opt(a: Args) -> i64 {
    let _ = a;
    world::with_world_mut(|w| w.opt_n += 1);
    0
}

pub fn arm_opt_price(a: Args) -> i64 {
    let _ = a;
    {
        world::with_world_mut(|w| w.opt_n += 1);
        0
    }
}

/// `opt_count(want)` before a multi-select `ask`: the select count rides on the
/// prompt, so it must not bump `opt_n` (see the dispatch entry).
pub fn arm_opt_count(a: Args) -> i64 {
    let _ = a;
    0
}

pub fn arm_others_at(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| {
        let me = a0;
        let idx = a1;
        let mut n = 0;
        let mut p = 0;
        let mut found = -1;
        while p < MAX_PLAYERS {
            if (p as i32) != me && !w.out[p] && p < w.n_players as usize {
                if n == idx {
                    found = p as i32;
                }
                n += 1;
            }
            p += 1;
        }
        found
    }) as i64
}

pub fn arm_others_count(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| {
        let me = a0;
        let mut n = 0;
        let mut p = 0;
        while p < MAX_PLAYERS {
            if (p as i32) != me && !w.out[p] && p < w.n_players as usize {
                n += 1;
            }
            p += 1;
        }
        n
    }) as i64
}

pub fn arm_owned_at(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| {
        let p = a0;
        let idx = a1;
        let mut n = 0;
        let mut found = -1;
        let mut outer = 0usize;
        while outer < 8 {
            let mut k = 0usize;
            while k < 8 {
                let i = outer * 8 + k;
                if i < 60 && w.tiles[i].owner == p {
                    if n == idx {
                        // 0-based engine index (was wrongly `i + 1`).
                        found = i as i32;
                    }
                    n += 1;
                }
                k += 1;
            }
            outer += 1;
        }
        found
    }) as i64
}

pub fn arm_owned_count(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| {
        let p = a0;
        // 8×8 chunk walk over the 60-tile board (unwind bound 8, not 60).
        let mut n = 0;
        let mut outer = 0usize;
        while outer < 8 {
            let mut k = 0usize;
            while k < 8 {
                let i = outer * 8 + k;
                if i < 60 && w.tiles[i].owner == p {
                    n += 1;
                }
                k += 1;
            }
            outer += 1;
        }
        n
    }) as i64
}

pub fn arm_pay(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // `pay_shortfall` = the payer cannot cover the amount: move what is
        // there and answer it (the real side's shortfall path).
        let (short, avail) = world::with_world(|w| (w.pay_shortfall, w.money[player_idx(a0)]));
        let moved = if short { a1.min(avail) } else { a1 };
        if moved > 0 {
            world::with_world_mut(|w| {
                let p = player_idx(a0);
                w.money[p] = (w.money[p] - moved).max(0);
                w.push(Effect::PayMoney {
                    player: a0,
                    n: moved,
                });
            });
        }
        moved as i64
    }
}

pub fn arm_pay_leg(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        if a2 > 0 {
            world::with_world_mut(|w| {
                let f = player_idx(a0);
                let t = player_idx(a1);
                w.money[f] = (w.money[f] - a2).max(0);
                w.money[t] = w.money[t].saturating_add(a2);
                w.push(Effect::PayTo {
                    from: a0,
                    to: a1,
                    n: a2,
                });
            });
        }
        a2 as i64
    }
}

pub fn arm_pay_to(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        if a1 > 0 {
            world::with_world_mut(|w| {
                let from = player_idx(a0);
                let to = player_idx(a1); // wrong slot — see note below
                let _ = to;
                let _ = from;
            });
        }
        // ABI: pay_to(from, to, amount, …). Re-dispatch with packed args.
        // In the wire form a0=from, a1=to, a2=amount.
        let from = a0;
        let to = a1;
        let amount = a2;
        if amount > 0 {
            world::with_world_mut(|w| {
                let f = player_idx(from);
                let t = player_idx(to);
                w.money[f] = (w.money[f] - amount).max(0);
                w.money[t] = w.money[t].saturating_add(amount);
                w.push(Effect::PayTo {
                    from,
                    to,
                    n: amount,
                });
            });
        }
        amount as i64
    }
}

pub fn arm_pay_total(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // (from, to, total) — same as pay_to for the model.
        let from = a0;
        let to = a1;
        let amount = a2;
        if amount > 0 {
            world::with_world_mut(|w| {
                let f = player_idx(from);
                let t = player_idx(to);
                w.money[f] = (w.money[f] - amount).max(0);
                w.money[t] = w.money[t].saturating_add(amount);
                w.push(Effect::PayTo {
                    from,
                    to,
                    n: amount,
                });
            });
        }
        amount as i64
    }
}

pub fn arm_place_card(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let _ = (a0, a1, a2, a3, a4, a5);

    // ABI `[player, cp, cl, ptr, len, 0]` — card name at (a1, a2).
    let id = card_u16(a1, a2);
    let slot = card_slot(a1, a2);
    world::with_world_mut(|w| {
        // `place_card` puts the **new** instance on the owner's field. The
        // running card's dest (`w.dest`, set by `set_dest`) is a different
        // thing — it is where the *running* instance goes when the effect
        // finishes. The PlaceCard effect's `dest` is the Dest code of the
        // **placed** card = Field (2). (pipopa: `set_dest(Banished)` for
        // 「此卡[移除]」 then `place_card` a Popipapapipopa — the old arm
        // leaked `w.dest=3` into PlaceCard.dest.)
        let tile = w.pos[player_idx(a0)];
        let uid = w.make_row(id, a0, a0, tile);
        w.push(Effect::PlaceCard {
            player: a0,
            dest: 2,
            card: slot,
        });
        uid as i64
    })
}

pub fn arm_place_card_at(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // ABI `[player, cp, cl, ptr, len, 0]` — card name at (a1, a2).
        // Same dest/dest split as `arm_place_card`: PlaceCard.dest is
        // Field (2) for the placed instance, not `w.dest`.
        let id = card_u16(a1, a2);
        let slot = card_slot(a1, a2);
        world::with_world_mut(|w| {
            let tile = w.pos[player_idx(a0)];
            let uid = w.make_row(id, a0, a0, tile);
            w.push(Effect::PlaceCard {
                player: a0,
                dest: 2,
                card: slot,
            });
            uid as i64
        })
    }
}

pub fn arm_place_card_on(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // ABI `[player, tile, cp, cl, ptr, len]` — card name at (a2, a3),
        // landing tile at a1 (HHW kkr-02 「放置在CiRCLE」). The old arm
        // read the name from (a1, a2) = (tile, cp) and used `dest`.
        let tile = a1;
        let id = card_u16(a2, a3);
        let slot = card_slot(a2, a3);
        world::with_world_mut(|w| {
            let uid = w.make_row(id, a0, a0, tile);
            w.push(Effect::PlaceCard {
                player: a0,
                dest: tile,
                card: slot,
            });
            uid as i64
        })
    }
}

pub fn arm_place_mark(a: Args) -> i64 {
    let [a0, ..] = a;
    // ABI is filter-based; record a mark on tile a0.
    world::with_world_mut(|w| {
        let i = w.marks_n as usize;
        if i < world::MAX_MARKS {
            w.marks[i] = world::Mark {
                tile: a0,
                key: NameId::INVALID,
                src_card: u16::MAX,
                instance: 0,
                n: 1,
            };
            w.marks_n += 1;
        }
        w.push(Effect::PlaceMark {
            tile: a0,
            key: NameId::INVALID,
        });
        // Return a plausible never-zero mark uid (the real allocator is
        // monotonic from 1; the kani world has no uid field).
        (w.marks_n.max(1)) as i64
    })
}

pub fn arm_placed_cards(a: Args) -> i64 {
    let [a0, buf, cap, ..] = a;
    // Postcard-encode `Vec<String>` of the player's field card names,
    // matching `hostfns::placed_cards`. The guest `ctx::placed_cards`
    // decodes with `postcard::from_bytes`.
    let names: Vec<String> = world::with_world(|w| {
        let mut v = Vec::new();
        let mut i = 0usize;
        while i < w.field_n as usize {
            if w.field[i].player == a0 {
                // Dispatch resolution: a marked instance answers with the
                // stand-in's surface, never the row's own card.
                let card = if w.field[i].effective_card == u16::MAX {
                    w.field[i].card
                } else {
                    w.field[i].effective_card
                };
                let name = intern::resolve_str(intern::NameId(card));
                v.push(name.to_string());
            }
            i += 1;
        }
        v
    });
    let bytes = postcard::to_allocvec(&names).unwrap_or_default();
    if buf == 0 {
        return bytes.len() as i64;
    }
    if (bytes.len() as i32) > cap {
        return -1;
    }
    card_sdk::native::write(buf, &bytes);
    bytes.len() as i64
}

pub fn arm_player_count(a: Args) -> i64 {
    let _ = a;
    let n = world::with_world(|w| w.n_players) as i64;
    #[cfg(kani)]
    kani::assume(n >= 2 && n <= 4);
    n
}

pub fn arm_player_out(a: Args) -> i64 {
    let [a0, ..] = a;
    world::with_world(|w| w.out[player_idx(a0)]) as i64
}

pub fn arm_player_pos(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| w.pos[player_idx(a0)]) as i64
}

pub fn arm_players_on_at(a: Args) -> i64 {
    let [a0, a1, a2, ..] = a;
    world::with_world(|w| {
        let tile = a0;
        let except = a1;
        let idx = a2;
        let mut n = 0;
        let mut p = 0;
        let mut found = -1;
        while p < MAX_PLAYERS {
            if (p as i32) != except && !w.out[p] && w.pos[p] == tile {
                if n == idx {
                    found = p as i32;
                }
                n += 1;
            }
            p += 1;
        }
        found
    }) as i64
}

pub fn arm_players_on_count(a: Args) -> i64 {
    let [a0, a1, ..] = a;
    world::with_world(|w| {
        let tile = a0;
        let except = a1;
        let mut n = 0;
        let mut p = 0;
        while p < MAX_PLAYERS {
            if (p as i32) != except && !w.out[p] && w.pos[p] == tile {
                n += 1;
            }
            p += 1;
        }
        n
    }) as i64
}

/// `ctx::raise(player_id, kind, value)` — record the trigger payload and run
/// any in-host listener for the kind. Returns 1 = settled (nobody cancelled).
///
/// `FansFlip` (107) is the first real client: `fans::flip_up` raises it
/// **before** the dict writes with `value` = the effect's intended X. Pastel
/// listens `On::Action(&[FansFlip], "actor == owner")`, so only band-skill
/// instances whose owner == `player_id` run `on_fans_flip_kani`. Cancelled /
/// aborted (`0`) leaves the caller's counters untouched.
pub fn arm_raise(a: Args) -> i64 {
    let [player_id, kp, kl, value, ..] = a;
    // Nested trigger (matches `wasm_rules::raise_core`): save the outer
    // payload, install the raise's, run listeners, restore. `arm_trig_value`
    // answers `pay_amount` (the hostfns `trigger().value` twin); `trig_value`
    // is the ir_fill / fixture view of the same number.
    let kind_sp: &'static str = crate::intern::resolve_str(name_id(kp, kl));
    let saved = world::with_world(|w| {
        (
            w.trig_kind,
            w.trig_player,
            w.trig_value,
            w.pay_amount,
            w.trig_cancelled,
        )
    });
    world::with_world_mut(|w| {
        w.trig_kind = kind_sp;
        w.trig_player = player_id;
        w.trig_value = value;
        w.pay_amount = value;
        w.trig_cancelled = false;
    });
    let mut settled = 1i64;
    #[cfg(all(kani, feature = "skills-groups"))]
    {
        if kind_sp == "r:bandori:107" {
            settled = arm_raise_fans_flip(player_id, value);
        }
    }
    #[cfg(not(all(kani, feature = "skills-groups")))]
    let _ = (player_id, value);
    // Restore the outer trigger. The raise's cancelled flag is the *reply*
    // (0 = cancelled), not a mutation of the outer payload.
    world::with_world_mut(|w| {
        w.trig_kind = saved.0;
        w.trig_player = saved.1;
        w.trig_value = saved.2;
        w.pay_amount = saved.3;
        w.trig_cancelled = saved.4;
    });
    settled
}

/// FansFlip listener dispatch: every `skill:Pastel✽Palettes:与偶像一起` field
/// instance whose owner is the raise seat (`actor == owner`), newest first
/// is irrelevant — one instance per seat in the proofs.
#[cfg(all(kani, feature = "skills-groups"))]
fn arm_raise_fans_flip(player_id: i32, value: i32) -> i64 {
    let _ = value;
    let band = intern::intern(world::TOK_PP_BAND).0;
    let saved_uid = world::with_world(|w| w.last_uid);
    // Collect matching uids first (the body may mutate the field).
    let mut hits = [0i32; 4];
    let mut n = 0usize;
    world::with_world(|w| {
        let mut j = 0usize;
        while j < w.field_n as usize {
            if w.field[j].player == player_id && w.field[j].card == band && n < hits.len() {
                hits[n] = w.field[j].uid;
                n += 1;
            }
            j += 1;
        }
    });
    let mut i = 0usize;
    while i < n {
        let uid = hits[i];
        world::with_world_mut(|w| w.last_uid = uid);
        let _ = skill_groups::pastel::on_fans_flip_kani(player_id);
        i += 1;
    }
    world::with_world_mut(|w| w.last_uid = saved_uid);
    // A listener may have cancelled the raise.
    if world::with_world(|w| w.trig_cancelled) {
        0
    } else {
        1
    }
}

pub fn arm_raise_bought(_a: Args) -> i64 {
    1
}

pub fn arm_ring_multiplier(_a: Args) -> i64 {
    1
}

pub fn arm_round_no(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.round_no) as i64
}

pub fn arm_schedule_turn_end(a: Args) -> i64 {
    let [a0, a1, ..] = a;
    world::with_world_mut(|w| {
        w.last_schedule = (a0, a1);
        w.schedule_turn_end_n = w.schedule_turn_end_n.saturating_add(1);
    });
    0
}

/// `settle_tile_at` — a protocol stub (the settle's real scheduling is
/// PROTOCOL-LEVEL), but record the call so a clause can still be held to "it
/// settles here, and nowhere when placed nowhere".
pub fn arm_settle_tile_at(a: Args) -> i64 {
    let [a0, a1, ..] = a;
    world::with_world_mut(|w| {
        w.last_settle = (a0, a1);
        w.settle_calls = w.settle_calls.saturating_add(1);
    });
    0
}

/// `trigger::is_settle_redirect()` — the pending settle was card-forced
/// (`TriggerMove::settle_redirect`).
pub fn arm_trig_settle_redirect(_a: Args) -> i64 {
    world::with_world(|w| w.move_snap.settle_redirect as i64)
}

pub fn arm_self_tile(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let _ = (a0, a1, a2, a3, a4, a5);
    world::with_world(|w| {
        if w.field_n == 0 {
            w.pos[player_idx(w.trig_player)]
        } else {
            w.field[0].tile
        }
    }) as i64
}

pub fn arm_self_uid(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.bound_uid) as i64
}

pub fn arm_set_counter_self(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        let key = name_id(a0, a1);
        let v = a2;
        world::with_world_mut(|w| {
            if key == known().crystals {
                w.set_field_crystals(w.last_uid, v);
            } else {
                w.key_set(w.trig_player, key, v);
            }
            w.push(Effect::AddCounter {
                owner: w.trig_player,
                key: key.0,
                n: v,
            });
        });
        v as i64
    }
}

pub fn arm_set_fate(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        world::with_world_mut(|w| {
            w.dest = a0;
            // Field-effect dest: the real engine defers to `apply_dest` at
            // the run's commit (unplace-and-send the running instance).
            // A play from hand names the *hand card's* dest instead and is
            // applied by the play pipeline, so only apply here when the
            // instance is live on the field. `Dest::Field` (2) = stay.
            //
            // The commit is **not** an in-run effect (specs such as
            // `again_02_effects_ok` / `starry_06_effects_ok` pin "exactly
            // one effect" and the engine's post-body unplace is outside
            // the effect log). World state only: field slot drops, pile
            // gains the card.
            if a0 != 2 {
                // The run's instance, **not** `last_uid`: an inner `place_card`
                // advances the self slot, but `apply_dest` files
                // `Run::current_uid`.
                let uid = w.bound_uid;
                let to = w.dest_to;
                if let Some(card) = w.remove_field(uid) {
                    match a0 {
                        0 => world::pile_push(&mut w.discard_pile, &mut w.discard_n, card),
                        1 => world::pile_push(&mut w.hand_pile, &mut w.hand_n, card),
                        // 3 = Banished 「[移除]」: left the field, goes nowhere.
                        _ => {}
                    }
                }
                let _ = to;
            }
        });
        0
    }
}

/// `Op::send_to_dest` — the run's own instance leaves into `dest` **now**
/// (`ctx::send_to_dest`), `-1` = its own leave destination. Mirrors
/// `hostfns::send_to_dest` → `HostRequest::Leave { uid: self_uid(), to: -1 }`.
pub fn arm_send_to_dest(a: Args) -> i64 {
    let [a0, a1, ..] = a;
    world::with_world_mut(|w| {
        w.dest = a0;
        let uid = w.bound_uid;
        w.zone_leave(uid, a0, -1, a1)
    }) as i64
}

/// `Op::transfer_to_dest` — `send_to_dest` into another seat's area
/// (`to` is whose; mirrors `hostfns::transfer_to_dest`).
pub fn arm_transfer_to_dest(a: Args) -> i64 {
    let [a0, a1, a2, ..] = a;
    world::with_world_mut(|w| {
        w.dest = a1;
        let uid = w.bound_uid;
        w.zone_leave(uid, a1, a0, a2)
    }) as i64
}

/// `Op::unplace_card` — the run's own instance leaves into its own leave
/// destination with cause `Effect` (mirrors `hostfns::unplace_card`).
pub fn arm_unplace_card_self(_a: Args) -> i64 {
    world::with_world_mut(|w| {
        let uid = w.bound_uid;
        w.zone_leave(uid, -1, -1, world::CAUSE_EFFECT)
    }) as i64
}

/// `Op::unplace_at` — the leave writer (`unplace_at(uid, destination,
/// cause)`; `-1` = the instance's own leave destination).
pub fn arm_zone_leave(a: Args) -> i64 {
    let [uid, destination, cause, ..] = a;
    world::with_world_mut(|w| w.zone_leave(uid, destination, -1, cause)) as i64
}

/// `Op::unplace_card_named` — a name-addressed take-off that files nothing
/// (mirrors `CardWorld::unplace_card_named` over `World::unplace_card`).
pub fn arm_unplace_card_named(a: Args) -> i64 {
    let [a0, cp, cl, ..] = a;
    let id = card_u16(cp, cl);
    world::with_world_mut(|w| {
        let mut i = 0usize;
        let mut idx = None;
        while i < w.field_n as usize {
            if w.field[i].player == a0 && w.field[i].card == id {
                idx = Some(i);
                break;
            }
            i += 1;
        }
        match idx {
            Some(i) => {
                let uid = w.field[i].uid;
                w.remove_field(uid);
                w.push(Effect::UnplaceCard { uid });
                1
            }
            None => 0,
        }
    })
}

/// Write a `Vec<i32>` as LE `i32`s; answers the byte length (mirrors
/// `hostfns::write_uid_list`, including "too small a buffer still answers
/// the length").
fn write_uid_list(buf: i32, cap: i32, list: &[i32]) -> i64 {
    let need = list.len() as i32 * 4;
    if buf != 0 && need <= cap {
        let mut bytes = [0u8; world::MAX_FIELD * 4];
        let mut i = 0usize;
        while i < list.len() {
            let b = list[i].to_le_bytes();
            let base = i * 4;
            bytes[base] = b[0];
            bytes[base + 1] = b[1];
            bytes[base + 2] = b[2];
            bytes[base + 3] = b[3];
            i += 1;
        }
        card_sdk::native::write(buf, &bytes[..need as usize]);
    }
    need as i64
}

/// `Op::attached_cards` — the host's direct attachments in filing order.
pub fn arm_attached_cards(a: Args) -> i64 {
    let [host, buf, cap, ..] = a;
    let list = world::with_world(|w| {
        let mut out = [0i32; world::MAX_FIELD];
        let n = w.attached_into(host, &mut out);
        out[..n].to_vec()
    });
    write_uid_list(buf, cap, &list)
}

/// `Op::created_cards` — the creator's direct children in creation order.
pub fn arm_created_cards(a: Args) -> i64 {
    let [creator, buf, cap, ..] = a;
    let list = world::with_world(|w| {
        let mut out = [0i32; world::MAX_FIELD];
        let n = w.created_into(creator, &mut out);
        out[..n].to_vec()
    });
    write_uid_list(buf, cap, &list)
}

/// `Op::attach_card(host, ptr, len, cause, face_down)` — file a fresh
/// instance of the named card from the player's hand onto `host`. Answers the
/// new uid, `-1` when refused (dead host / card not in hand).
pub fn arm_attach_card(a: Args) -> i64 {
    let [host, cp, cl, cause, face_down, ..] = a;
    let id = card_u16(cp, cl);
    world::with_world_mut(|w| {
        let Some(host_row) = w.field_by_uid(host).copied() else {
            return -1;
        };
        let caller = w.trig_player;
        // A copy in the caller's hand is taken first; a name with no hand copy
        // falls back to re-targeting the caller's first live instance
        // (cycle-guarded), exactly as `zone_attach` does.
        let mut slot = None;
        let mut i = 0usize;
        while i < w.hand_n as usize {
            if w.hand_pile[i] == id {
                slot = Some(i);
                break;
            }
            i += 1;
        }
        if slot.is_none() {
            let mut live = None;
            let mut li = 0usize;
            while li < w.field_n as usize {
                let f = w.field[li];
                if f.card != u16::MAX && f.player == caller && f.card == id && f.uid != host {
                    live = Some(f.uid);
                    break;
                }
                li += 1;
            }
            if let Some(uid) = live {
                let Some((from, holder)) = w.zone_prepare_attach(uid, host) else {
                    return -1;
                };
                if let Some(f) = w.field_by_uid_mut(uid) {
                    f.on = host;
                    f.face_down = face_down != 0;
                    f.tile = host_row.tile;
                }
                w.trig_zone_present = true;
                w.trig_zone_from = from;
                w.trig_zone_to = world::ZONE_ATTACHED;
                w.trig_zone_cause = cause;
                w.trig_zone_holder = holder;
                w.trig_zone_wasted = false;
                w.trig_zone_counted = false;
                return uid;
            }
            return -1;
        }
        let Some(k) = slot else {
            return -1;
        };
        // One copy leaves the hand.
        let mut j = k;
        while j + 1 < w.hand_n as usize {
            w.hand_pile[j] = w.hand_pile[j + 1];
            j += 1;
        }
        w.hand_n = w.hand_n.saturating_sub(1);
        let uid = w.make_row(id, caller, caller, host_row.tile);
        if let Some(f) = w.field_by_uid_mut(uid) {
            f.on = host;
            f.face_down = face_down != 0;
        }
        w.trig_zone_present = true;
        w.trig_zone_from = world::ZONE_NOWHERE;
        w.trig_zone_to = world::ZONE_ATTACHED;
        w.trig_zone_cause = cause;
        w.trig_zone_holder = w.holder_of(host);
        w.trig_zone_wasted = false;
        w.trig_zone_counted = false;
        uid
    }) as i64
}

/// `Op::detach_card(uid, dest, cause)` — take the instance off its host.
pub fn arm_detach_card(a: Args) -> i64 {
    let [uid, dest, cause, ..] = a;
    world::with_world_mut(|w| {
        if w.attachment_host(uid) < 0 {
            return -1;
        }
        w.zone_leave(uid, dest, -1, cause)
    }) as i64
}

pub fn arm_set_instance_owner(a: Args) -> i64 {
    let [uid, owner, ..] = a;
    world::with_world_mut(|w| match w.field_by_uid_mut(uid) {
        Some(f) => {
            f.owner = owner;
            1
        }
        None => 0,
    })
}

pub fn arm_set_created_by_at(a: Args) -> i64 {
    let [uid, creator, ..] = a;
    world::with_world_mut(|w| match w.field_by_uid_mut(uid) {
        Some(f) => {
            f.created_by = creator;
            1
        }
        None => 0,
    })
}

pub fn arm_set_ineffective_at(a: Args) -> i64 {
    let [uid, on, ..] = a;
    // `-1` is the running hand play, not a row (G08 §2.6 / CH-07).
    if uid < 0 {
        world::with_world_mut(|w| w.play_ineffective = on != 0);
        return 1;
    }
    world::with_world_mut(|w| match w.field_by_uid_mut(uid) {
        Some(f) => {
            f.effective = on == 0;
            1
        }
        None => 0,
    })
}

/// `Op::rehome_card(tile, owner)` — the deed row moves seat unchanged; the
/// acquisition ledger follows (`set_tile_owner`). Always answers 1.
pub fn arm_rehome_card(a: Args) -> i64 {
    let [tile, owner, ..] = a;
    world::with_world_mut(|w| {
        w.set_tile_owner(tile, owner);
        w.push(Effect::SetOwner {
            tile,
            player: owner,
        });
    });
    1
}

/// `Op::play_hand_card(player, ptr, len)` — remove one copy from the hand
/// and file its instance as a play on the player's tile. Answers `1`, `0`
/// when the copy is not in hand.
pub fn arm_play_hand_card(a: Args) -> i64 {
    let [player, cp, cl, ..] = a;
    let id = card_u16(cp, cl);
    world::with_world_mut(|w| {
        let mut slot = None;
        let mut i = 0usize;
        while i < w.hand_n as usize {
            if w.hand_pile[i] == id {
                slot = Some(i);
                break;
            }
            i += 1;
        }
        let Some(k) = slot else {
            return 0;
        };
        let mut j = k;
        while j + 1 < w.hand_n as usize {
            w.hand_pile[j] = w.hand_pile[j + 1];
            j += 1;
        }
        w.hand_n = w.hand_n.saturating_sub(1);
        let tile = w.pos[player_idx(player)];
        let uid = w.make_row(id, player, player, tile);
        w.bound_uid = uid;
        w.trig_play_outcome = 0;
        1
    })
}

/// `Op::trigger_zone` — the trigger's postcard `Option<ZoneChange>` payload,
/// `&[]` outside the zone family.
pub fn arm_trigger_zone(a: Args) -> i64 {
    let [buf, cap, ..] = a;
    let payload = world::with_world(|w| {
        w.trig_zone_present.then(|| game_kinds::ZoneChange {
            from: game_kinds::Zone::from_i32(w.trig_zone_from),
            to: game_kinds::Zone::from_i32(w.trig_zone_to),
            cause: game_kinds::ZoneChangeCause::from_i32(w.trig_zone_cause),
            holder: w.trig_zone_holder,
            wasted: w.trig_zone_wasted,
            counted: w.trig_zone_counted,
        })
    });
    let bytes = postcard::to_allocvec(&payload).unwrap_or_default();
    if buf != 0 && bytes.len() as i32 <= cap {
        card_sdk::native::write(buf, &bytes);
    }
    bytes.len() as i64
}

/// Live ceiling on the held row (`Counter.max`; 0 = uncapped / undeclared).
/// The Kani world models fire's ceiling as `charge_max`.
pub fn arm_tok_max(a: Args) -> i64 {
    let [a0, a1, a2, ..] = a;
    let key = name_id(a0, a1);
    let p = player_idx(a2);
    world::with_world(|w| {
        if key == known().tok_charge {
            w.charge_max[p] as i64
        } else {
            0
        }
    })
}

pub fn arm_set_held_name(a: Args) -> i64 {
    // Wire: (name_p, name_l, player_id, v, max, instance). `max` is the
    // production tri-state (`>= 0` writes that ceiling, `< 0` leaves the stored
    // one alone) -- see `game_core::engine::ops::set_named`.
    let [a0, a1, a2, a3, a4, ..] = a;
    let key = name_id(a0, a1);
    let p = a2;
    let v = a3;
    let max = a4;
    world::with_world_mut(|w| {
        if key == known().tok_pp_pos {
            w.fans_pos[player_idx(p)] = v;
        } else if key == known().tok_pp_neg {
            w.fans_neg[player_idx(p)] = v;
        } else if key == known().tok_charge {
            // Fire lives on the same store `gain_marker` writes. `set_row`
            // advertises the ceiling here too -- without this, `fire::declare`
            // left `charge_max` at 0 under the proof host and every gain was
            // refused while production accepted it.
            w.charge[player_idx(p)] = v;
            if max >= 0 {
                w.charge_max[player_idx(p)] = max;
            }
        } else {
            w.key_set(p, key, v);
            w.sync_views_player(p);
        }
        w.push(Effect::SetToken {
            player: p,
            key: key.0,
            n: v,
        });
    });
    0
}

pub fn arm_set_mortgaged(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        world::with_world_mut(|w| {
            w.tiles[tile_idx(a0)].mortgaged = a1 != 0;
            w.push(Effect::SetMortgaged {
                tile: a0,
                on: a1 != 0,
            });
        });
        0
    }
}

/// `commit_redeem(player, tile, amount)` -- the mechanism half of a redeem:
/// clear the mortgage flag. Modelled as the unmortgage write (the `redeemed`
/// state key and the fee have no model field).
pub fn arm_apply_redeem(a: Args) -> i64 {
    let [a0, a1, a2, ..] = a;
    // G02 `spendBefore`: the write is the cost, else the asked amount.
    let cost = fold_spend_before(a2.max(0));
    if cost > 0 {
        world::with_world_mut(|w| {
            let p = player_idx(a0);
            let n = cost.min(w.money[p].max(0));
            w.money[p] -= n;
        });
    }
    world::with_world_mut(|w| {
        w.tiles[tile_idx(a1)].mortgaged = false;
        w.push(Effect::SetMortgaged {
            tile: a1,
            on: false,
        });
    });
    cost as i64
}

pub fn arm_set_owner(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        world::with_world_mut(|w| {
            w.tiles[tile_idx(a0)].owner = a1;
            w.push(Effect::SetOwner {
                tile: a0,
                player: a1,
            });
        });
        0
    }
}

pub fn arm_set_prop_at(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let _ = (a4, a5);
    let key = name_id(a1, a2);
    let val = a3;
    let uid = a0;
    world::with_world_mut(|w| {
        let mut i = 0usize;
        while i < w.field_n as usize {
            if w.field[i].uid == uid {
                w.field[i].props.set(key, val);
            }
            i += 1;
        }
    });
    0
}

pub fn arm_set_self_immune(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let _ = (a0, a1, a2, a3, a4, a5);
    world::with_world_mut(|w| {
        if w.field_n == 0 {
            0
        } else {
            w.field[0].immune = a0 != 0;
            1
        }
    })
}

/// `self_prop(kp, kl)` -- read the running instance's prop (the reader half of
/// [`arm_set_self_prop`]).
pub fn arm_self_prop(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let _ = (a2, a3, a4, a5);
    let key = name_id(a0, a1);
    world::with_world(|w| {
        if w.field_n > 0 {
            let i = (w.field_n as usize) - 1;
            w.field[i].props.get(key)
        } else {
            0
        }
    }) as i64
}

pub fn arm_set_self_prop(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let _ = (a3, a4, a5);
    let key = name_id(a0, a1);
    let val = a2;
    world::with_world_mut(|w| {
        if w.field_n > 0 {
            let i = (w.field_n as usize) - 1;
            w.field[i].props.set(key, val);
        }
    });
    0
}

pub fn arm_set_self_tile(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let _ = (a0, a1, a2, a3, a4, a5);
    world::with_world_mut(|w| {
        if w.field_n == 0 {
            0
        } else {
            let uid = w.field[0].uid;
            let ok = w.set_tile_at(uid, a0);
            if ok {
                w.push(Effect::SetCardTile { uid, tile: a0 });
            }
            ok as i64
        }
    })
}

pub fn arm_set_tile_prop(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let _ = (a4, a5);
    let key = name_id(a1, a2);
    let val = a3;
    world::with_world_mut(|w| {
        w.tiles[tile_idx(a0)].props.set(key, val);
    });
    0
}

pub fn arm_shuffle_into_deck(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // Engine: move `hand` / `discard` into `draw` and shuffle. Order is
        // symbolic/irrelevant — model as a multiset move (task B8).
        let do_hand = a1 != 0;
        let do_discard = a2 != 0;
        world::with_world_mut(|w| {
            let mut moved = 0i32;
            if do_hand {
                let mut i = 0usize;
                while i < w.hand_n as usize {
                    let id = w.hand_pile[i];
                    if id != u16::MAX {
                        let di = w.deck_n as usize;
                        if di < world::MAX_CARDS {
                            w.deck_pile[di] = id;
                            w.deck_n += 1;
                            moved += 1;
                        }
                        w.hand_pile[i] = u16::MAX;
                    }
                    i += 1;
                }
                w.hand_n = 0;
            }
            if do_discard {
                let mut i = 0usize;
                while i < w.discard_n as usize {
                    let id = w.discard_pile[i];
                    if id != u16::MAX {
                        let di = w.deck_n as usize;
                        if di < world::MAX_CARDS {
                            w.deck_pile[di] = id;
                            w.deck_n += 1;
                            moved += 1;
                        }
                        w.discard_pile[i] = u16::MAX;
                    }
                    i += 1;
                }
                w.discard_n = 0;
            }
            moved as i64
        })
    }
}

pub fn arm_spend_marker(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // All-or-nothing; returns 1 when the player had enough. No cap
        // consultation -- spending is not the capped direction.
        let p = player_idx(a0);
        let n = a3;
        world::with_world_mut(|w| {
            if n <= 0 {
                1
            } else if w.charge[p] >= n {
                w.charge[p] -= n;
                1
            } else {
                0
            }
        })
    }
}

/// The world abnormal column a well-known `state` key aliases (1 stay / 2 stun
/// / 3 exile / 0 none). The shared status verbs write `state` keys, while
/// `snapshot`, `cant_move` and `abnormal_count` read the dedicated columns; the
/// generic `state_*` arms decode the keys so both views stay one model (the
/// retired `give_stay` / `stay_of` ops used to do this).
fn status_kind(key: NameId) -> i32 {
    if intern::id_eq_str(key, "stay") {
        1
    } else if intern::id_eq_str(key, "stun") {
        2
    } else if intern::id_eq_str(key, "exile") {
        3
    } else {
        0
    }
}

/// The `ppFansUp` / `ppFansDown` state keys alias the world's fan columns:
/// ABI v88 moved the fan counters onto the generic `state::*` surface
/// (`pp_fans::up/down/grant`), while `snapshot` and the specs still read the
/// columns the retired `count_held` / `set_held_name` ops mapped. 1 = up,
/// 2 = down, 0 = not a fan key.
fn fan_kind(key: NameId) -> i32 {
    if intern::id_eq_str(key, "ppFansUp") {
        1
    } else if intern::id_eq_str(key, "ppFansDown") {
        2
    } else {
        0
    }
}

pub fn arm_state_add(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        let key = name_id(a1, a2);
        let fan = fan_kind(key);
        if fan != 0 {
            return world::with_world_mut(|w| {
                if a0 < 0 || (a0 as usize) >= MAX_PLAYERS {
                    return 0;
                }
                let p = a0 as usize;
                let out = if fan == 1 {
                    w.fans_pos[p] = (w.fans_pos[p] + a3).max(0);
                    w.fans_pos[p]
                } else {
                    w.fans_neg[p] = (w.fans_neg[p] + a3).max(0);
                    w.fans_neg[p]
                };
                w.key_set(a0, key, out);
                out
            }) as i64;
        }
        let kind = status_kind(key);
        if kind == 0 {
            return world::with_world_mut(|w| w.key_add(a0, key, a3)) as i64;
        }
        world::with_world_mut(|w| {
            if a0 < 0 || (a0 as usize) >= MAX_PLAYERS {
                return 0;
            }
            let p = a0 as usize;
            let out = match kind {
                1 => {
                    w.stay[p] = (w.stay[p] + a3).max(0);
                    w.push(Effect::GiveStay { player: a0, n: a3 });
                    w.stay[p]
                }
                2 => {
                    w.stun[p] = (w.stun[p] + a3).max(0);
                    w.push(Effect::GiveStun { player: a0, n: a3 });
                    w.stun[p]
                }
                _ => {
                    w.exile[p] = (w.exile[p] + a3).max(0);
                    w.push(Effect::GiveExile {
                        player: a0,
                        n: a3,
                        to: w.exile_to[p],
                    });
                    w.exile[p]
                }
            };
            // Keep the generic map in step for readers that bypass the columns.
            w.key_set(a0, key, out);
            out
        }) as i64
    }
}

/// `raise_after(player, name_p, name_l, value)` — queue a post-commit raise.
/// The body-only model does not run deferred listeners (same as
/// `CardWorld::raise_after`'s default no-op); the call is accepted so guest
/// bodies that queue one complete.
pub fn arm_raise_after(a: Args) -> i64 {
    let _ = a;
    0
}

pub fn arm_state_get(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // hostfns `state_get(player, ptr, len, field)`: field 1/2/3/4 read
        // the declared MIN / MAX / EXPIRES / FLAGS column; anything else is
        // VALUE. Column ids are the SDK's `state::{MIN,MAX,EXPIRES,FLAGS}`.
        let key = name_id(a1, a2);
        let field = a3;
        let kind = if field == 0 { status_kind(key) } else { 0 };
        let fan = if field == 0 { fan_kind(key) } else { 0 };
        world::with_world(|w| match fan {
            1 => w.fans_pos[player_idx(a0)],
            2 => w.fans_neg[player_idx(a0)],
            _ => match kind {
                1 => w.stay[player_idx(a0)],
                2 => w.stun[player_idx(a0)],
                3 => w.exile[player_idx(a0)],
                _ => match field {
                    1 => w.key_min(a0, key),
                    2 => w.key_max(a0, key),
                    3 => w.key_expires(a0, key),
                    4 => w.key_flags(a0, key) as i32,
                    _ => w.key_get(a0, key),
                },
            },
        }) as i64
    }
}

pub fn arm_state_set(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // hostfns `state_set(player, ptr, len, field, v)`:
        //   a3 = field (0 value / 1 min / 2 max / 3 expires / 4 flags),
        //   a4 = v. Column ids are the SDK's `state::{VALUE,MIN,MAX,EXPIRES,FLAGS}`.
        // The old model wrote a3 as the value and dropped a4.
        let key = name_id(a1, a2);
        let field = a3;
        let v = a4;
        let kind = if field == 0 { status_kind(key) } else { 0 };
        let fan = if field == 0 { fan_kind(key) } else { 0 };
        if fan != 0 {
            return world::with_world_mut(|w| {
                // Fan rows floor at 0 (counter semantics; `arm_state_add`
                // clamps the same way), and the map shadow stores the same
                // figure the column gets.
                let stored = v.max(0);
                if a0 >= 0 && (a0 as usize) < MAX_PLAYERS {
                    let p = a0 as usize;
                    if fan == 1 {
                        w.fans_pos[p] = stored;
                    } else {
                        w.fans_neg[p] = stored;
                    }
                }
                w.key_set(a0, key, stored);
                stored
            }) as i64;
        }
        world::with_world_mut(|w| {
            let out = match field {
                1 => {
                    w.key_set_min(a0, key, v);
                    v
                }
                2 => {
                    w.key_set_max(a0, key, v);
                    v
                }
                3 => {
                    w.key_set_expires(a0, key, v);
                    v
                }
                4 => {
                    w.key_set_flags(a0, key, v as u8);
                    v
                }
                _ => {
                    let v = if kind != 0 && w.key_never_negative(a0, key) {
                        v.max(0)
                    } else {
                        v
                    };
                    if kind != 0 && a0 >= 0 && (a0 as usize) < MAX_PLAYERS {
                        match kind {
                            1 => w.stay[a0 as usize] = v,
                            2 => w.stun[a0 as usize] = v,
                            _ => w.exile[a0 as usize] = v,
                        }
                    }
                    w.key_set(a0, key, v);
                    v
                }
            };
            if field == 0 {
                w.push(Effect::SetState {
                    player: a0,
                    key: key.0,
                    n: v,
                });
            }
            out
        }) as i64
    }
}

pub fn arm_stay_layers(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| w.stay[player_idx(a0)]) as i64
}

pub fn arm_stun_layers(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| w.stun[player_idx(a0)]) as i64
}

pub fn arm_take_card(a: Args) -> i64 {
    let [a0, a1, a2, a3, ..] = a;
    let id = card_u16(a1, a2);
    let slot = card_slot(a1, a2);
    let player_id = a0;
    let pile = a3;
    world::with_world_mut(|w| {
        let ok = if pile == 1 {
            world::pile_take(&mut w.discard_pile, &mut w.discard_n, id) || {
                if (slot as usize) < 4 && w.discard[slot as usize] {
                    w.discard[slot as usize] = false;
                    true
                } else {
                    false
                }
            }
        } else if pile == 0 {
            if slot == 1 && w.hand_resonance[player_idx(player_id)] > 0 {
                w.hand_resonance[player_idx(player_id)] -= 1;
                true
            } else {
                world::pile_take(&mut w.hand_pile, &mut w.hand_n, id) || {
                    if (slot as usize) < 4 && w.hand[slot as usize] {
                        w.hand[slot as usize] = false;
                        true
                    } else {
                        false
                    }
                }
            }
        } else if pile == 2 {
            let ok = world::pile_take(&mut w.deck_pile, &mut w.deck_n, id);
            if ok {
                refill_if_empty(w, player_id);
            }
            ok
        } else if pile == 4 {
            world::pile_take(&mut w.inspection_pile, &mut w.inspection_n, id)
        } else {
            false
        };
        if ok {
            w.push(Effect::TakeFromPile {
                player: player_id,
                pile,
                card: slot,
            });
            1
        } else {
            0
        }
    })
}

pub fn arm_target(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    // A refused designation is the test-facing shape of 网络链接异常: answer -1.
    if world::with_world(|w| w.target_refused) {
        return -1;
    }
    // Tile targeting (`ctx::target_tile` sends player = -1): the real side's
    // inline Target answers the tile itself when one is named, so a -1 here
    // made every tile designation fail closed on the model side.
    if a0 < 0 && a1 >= 0 {
        let tile = a1;
        return world::with_world_mut(|w| {
            if tile as usize >= 60 {
                return -1;
            }
            let owner = w.tiles[tile as usize].owner;
            // An own tile still runs the window; it is not «other source» for
            // the count, and a designation during the seat's own turn is
            // before its between-turns window.
            if owner >= 0
                && (owner as usize) < w.n_players as usize
                && owner != w.trig_player
                && owner != w.turn_player
            {
                w.targeted_count[owner as usize] =
                    w.targeted_count[owner as usize].saturating_add(1);
            }
            tile as i64
        });
    }
    {
        // Tile targeting (`target(-1, tile, 0)`, ctx::target_tile): the tile
        // itself is the answer. Mirrors `InlineHost::apply`'s `Target` arm.
        if a1 >= 0 {
            return a1 as i64;
        }
        let p = a0;
        world::with_world_mut(|w| {
            if p >= 0
                && (p as usize) < MAX_PLAYERS
                && (p as usize) < w.n_players as usize
                && !w.out[p as usize]
            {
                // A self-designation runs the window but never counts; the
                // count only accumulates between the seat's turns.
                if p != w.trig_player && p != w.turn_player {
                    w.targeted_count[p as usize] = w.targeted_count[p as usize].saturating_add(1);
                }
                p as i64
            } else {
                -1
            }
        })
    }
}

pub fn arm_teleport_to(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        world::with_world_mut(|w| {
            if (a0 as usize) < MAX_PLAYERS {
                w.pos[a0 as usize] = a1;
            }
            w.push(Effect::Teleport {
                player: a0,
                tile: a1,
            });
        });
        0
    }
}

pub fn arm_tile_count(a: Args) -> i64 {
    let _ = a;
    world::TILE_COUNT as i64
}

pub fn arm_tile_group(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        let s = board::tile_slot(a0);
        if s < 0 {
            0
        } else {
            board::TILES[s as usize].group as i64
        }
    }
}

pub fn arm_tile_in(a: Args) -> i64 {
    let [a0, a1, a2, a3, ..] = a;
    let _ = a0;
    let class = name_id(a2, a3);
    if intern::id_eq_str(class, "buyable") {
        let s = board::tile_slot(a1);
        if s < 0 {
            0
        } else {
            let t = &board::TILES[s as usize];
            let owned = world::with_world(|w| w.tiles[tile_idx(a1)].owner);
            (board::is_property(t) && owned < 0) as i64
        }
    } else if intern::id_eq_str(class, "shop") {
        let s = board::tile_slot(a1);
        if s < 0 {
            0
        } else {
            board::is_shop(&board::TILES[s as usize]) as i64
        }
    } else if intern::id_eq_str(class, "livehouse") {
        (a1 == 31 || a1 == 50) as i64
    } else if intern::id_eq_str(class, "color:6") {
        (a1 == 31 || a1 == 50) as i64
    } else if let Some(g) = intern::resolve_str(class)
        .strip_prefix("color:")
        .and_then(|g| g.parse::<i32>().ok())
    {
        // Generic colour class: the tile's own group (`is_color_lazy`'s first
        // arm; the `anyColor` / `colorFor` props are not modelled).
        let s = board::tile_slot(a1);
        (s >= 0 && board::TILES[s as usize].group as i32 == g) as i64
    } else {
        0
    }
}

pub fn arm_tile_is(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // ABI: tile_is(tile, kind_ptr, kind_len)
        let s = board::tile_slot(a0);
        if s < 0 {
            0
        } else {
            let kind = name_id(a1, a2);
            let t = &board::TILES[s as usize];
            // Compare via interned wire spellings.
            let hit = [
                ("circle", board::is_start(t)),
                ("ring", board::is_deed(t)),
                ("agent", board::is_agent(t)),
                ("property", board::is_property(t)),
                ("cafe", t.kind == 4),
                ("edogawa", t.kind == 5),
                ("ryuseido", t.kind == 6),
            ]
            .iter()
            .any(|(k, hit)| *hit && intern::id_eq_str(kind, k));
            hit as i64
        }
    }
}

pub fn arm_tile_named(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        // Real board: `data.tiles.iter().position(|t| t.name == name)`
        // (hostfns `tile_named`). Every `board.json` name resolves; unknown
        // names are -1.
        let id = name_id(a0, a1);
        let mut found = board::tile_named_bytes(intern::resolve(id));
        // Pilot boards (all `pos < 8`) keep the stub-shop index so the
        // money/teleport pilots against the 8-tile fixture still work.
        if id == known().tile_shop {
            let pilot = world::with_world(|w| w.pos.iter().all(|&p| p < 8));
            if pilot {
                found = TILE_SHOP;
            }
        }
        found as i64
    }
}

pub fn arm_tile_owner(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| w.tiles[tile_idx(a0)].owner) as i64
}

pub fn arm_tile_price(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        let s = board::tile_slot(a0);
        if s < 0 {
            0
        } else {
            board::TILES[s as usize].price as i64
        }
    }
}

pub fn arm_tile_steps_ahead(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        let pos = world::with_world(|w| w.pos[player_idx(a0)]);
        ((pos + a1).rem_euclid(60)) as i64
    }
}

/// The minimal refill the model lacked (v114): an emptied deck takes the
/// discard front back, unless an open inspection frame holds that seat's deck.
fn refill_if_empty(w: &mut World, owner: i32) {
    if w.inspection_deck_owner == player_idx(owner) as i32 || w.deck_n > 0 {
        return;
    }
    while w.deck_n == 0 && w.discard_n > 0 {
        let card = w.discard_pile[0];
        let mut i = 1usize;
        while i < w.discard_n as usize {
            w.discard_pile[i - 1] = w.discard_pile[i];
            i += 1;
        }
        w.discard_n -= 1;
        world::pile_unshift(&mut w.deck_pile, &mut w.deck_n, card);
    }
}

pub fn arm_to_discard(a: Args) -> i64 {
    let [a0, a1, a2, ..] = a;
    let slot = card_slot(a1, a2);
    let id = card_u16(a1, a2);
    world::with_world_mut(|w| {
        world::pile_push(&mut w.discard_pile, &mut w.discard_n, id);
        if (slot as usize) < 4 {
            w.discard[slot as usize] = true;
        }
        refill_if_empty(w, a0);
    });
    0
}

/// `inspection_begin` -- turn up to `count` cards off `owner`'s deck front into
/// the frame. The `inspected` raises are not modelled (no hook dispatch under
/// Kani); a proof asserts on the move.
pub fn arm_inspection_begin(a: Args) -> i64 {
    let [a0, a1, a2, a3, ..] = a;
    let want = a2.max(0) as usize;
    world::with_world_mut(|w| {
        let moved = world::pile_shift_front(
            &mut w.deck_pile,
            &mut w.deck_n,
            want,
            &mut w.inspection_pile,
            &mut w.inspection_n,
        );
        w.inspection_deck_owner = player_idx(a1) as i32;
        w.inspection_public = a3 != 0;
        w.inspection_frame_n = 1;
        let mut i = 0usize;
        while i < moved {
            w.inspection_revealed[i] = true;
            i += 1;
        }
        w.push(Effect::InspectionHold {
            looker: a0,
            owner: a1,
            n: moved as i32,
        });
        moved as i64
    })
}

/// `inspection_end` -- return the frame to the deck front in order.
///
/// The `shuffle` flag is deliberately unmodelled: the proof model's stub keeps
/// the order for it too (a real shuffle is a permutation the proofs compare by
/// multiset, and no deterministic order is provable without the RNG). The
/// engine's shuffle is exercised by the conformance suite and the card tests.
pub fn arm_inspection_end(a: Args) -> i64 {
    let [a0, _a1, ..] = a;
    world::with_world_mut(|w| {
        let owner = w.inspection_deck_owner;
        let mut n = 0i32;
        let mut j = w.inspection_n;
        while j > 0 {
            j -= 1;
            let card = w.inspection_pile[j as usize];
            world::pile_unshift(&mut w.deck_pile, &mut w.deck_n, card);
            n += 1;
        }
        w.inspection_n = 0;
        w.inspection_frame_n = 0;
        w.inspection_deck_owner = -1;
        w.push(Effect::InspectionReturn { owner: a0, n });
        n as i64
    })
}

/// `reveal_pile` -- a publication; no pile moves.
pub fn arm_reveal_pile(a: Args) -> i64 {
    let [a0, a1, ..] = a;
    world::with_world_mut(|w| {
        w.push(Effect::Reveal {
            player: a0,
            pile: a1,
        });
    });
    0
}

/// `reveal_card` -- one card published (`pile = -1`, a single card).
pub fn arm_reveal_card(a: Args) -> i64 {
    let [a0, ..] = a;
    world::with_world_mut(|w| {
        w.push(Effect::Reveal {
            player: a0,
            pile: -1,
        });
    });
    0
}

/// `held_names` -- LE `u16` [`NameId`]s of the seat's **non-zero** counter
/// names with the prefix (`角色标记:*` for nanami). `buf == 0` is the count
/// probe (the `field_instances` convention: no postcard, no UTF-8 scan).
pub fn arm_tok_names(a: Args) -> i64 {
    let [a0, a1, a2, buf, cap, ..] = a;
    let prefix = intern::resolve(name_id(a1, a2));
    let mut ids = [0u16; intern::SLOTS];
    let n = world::with_world(|w| {
        let p = player_idx(a0);
        // Only the already-interned prefix space is scanned: the table is
        // append-only and `n` is concrete at every call site.
        let bound = intern::len() as usize;
        let mut n = 0usize;
        let mut i = 0usize;
        while i < bound {
            let key = intern::NameId(i as u16);
            if let Some(v) = w.player_keys[p].get_opt(key) {
                if v != 0 && intern::resolve(key).starts_with(prefix) {
                    ids[n] = i as u16;
                    n += 1;
                }
            }
            i += 1;
        }
        n
    });
    if buf == 0 {
        return n as i64;
    }
    if cap < (n * 2) as i32 {
        return -1;
    }
    let mut encoded = [0u8; intern::SLOTS * 2];
    let mut i = 0usize;
    while i < n {
        let b = ids[i].to_le_bytes();
        encoded[i * 2] = b[0];
        encoded[i * 2 + 1] = b[1];
        i += 1;
    }
    card_sdk::native::write(buf, &encoded[..n * 2]);
    n as i64
}

/// `ctx::cancel_designation(seat)` -- the modelled cancel bit; the last write
/// wins (the Kani world has no per-pair list).
pub fn arm_cancel_designation(a: Args) -> i64 {
    let seat = a[0] as i32;
    world::with_world_mut(|w| w.designated_cancelled = seat);
    1
}

/// `ctx::designation_cancelled(seat)`.
pub fn arm_designation_cancelled(a: Args) -> i64 {
    let seat = a[0] as i32;
    world::with_world(|w| (w.designated_cancelled == seat) as i64)
}

/// `trigger::redirected_from()` -- the raw seat (`-1` = not redirected).
pub fn arm_trig_redirected_from(_a: Args) -> i64 {
    world::with_world(|w| w.trig_redirected_from as i64)
}

/// `trigger::play_outcome()` -- the raw bit (`-1` = none).
pub fn arm_trig_play_outcome(_a: Args) -> i64 {
    world::with_world(|w| w.trig_play_outcome) as i64
}

/// `trigger::kind()` — two-call out-buffer: write the spelling bytes.
pub fn arm_trig_kind(a: Args) -> i64 {
    let [p, n, ..] = a;
    let sp = world::with_world(|w| w.trig_kind);
    let bytes = sp.as_bytes();
    if p == 0 {
        return bytes.len() as i64;
    }
    if n < bytes.len() as i32 {
        return bytes.len() as i64;
    }
    card_sdk::native::write(p, bytes);
    bytes.len() as i64
}

/// `ctx::draw_event_as` -- attribute an event draw to `player` at `tile`.
/// The draw itself is not modelled; the attribution is recorded.
pub fn arm_draw_event_as(a: Args) -> i64 {
    let [player, tile, ..] = a;
    world::with_world_mut(|w| w.last_event_draw = (player, tile));
    1
}

/// `ctx::event_defer` -- claim the `(event, player, draw)` triple for the running
/// instance (design G10 §6.2). Only on an `event` trigger with a card; an
/// existing record answers 1 for its own claimer and 0 for anyone else, so a
/// re-run of the same instance never pushes a second record.
pub fn arm_event_defer(_a: Args) -> i64 {
    world::with_world_mut(|w| {
        if w.trig_kind != "event" || w.trig_card == 0 {
            return 0;
        }
        let event = w.trig_card;
        let player = w.trig_player;
        let mut i = 0;
        while i < w.deferred_n as usize {
            if w.deferred[i].event == event
                && w.deferred[i].player == player
                && w.deferred[i].ordinal == w.trig_ordinal
            {
                return (w.deferred[i].deferrer_uid == w.bound_uid) as i64;
            }
            i += 1;
        }
        if (w.deferred_n as usize) >= crate::world::MAX_DEFERRED {
            return 0;
        }
        let n = w.deferred_n as usize;
        w.deferred[n] = crate::world::DeferredEvent {
            event,
            player,
            tile: w.trig_tile,
            ordinal: w.trig_ordinal,
            due_owner: w.turn_player,
            deferrer_uid: w.bound_uid,
            after_turn_key: w.turn_key,
        };
        w.deferred_n += 1;
        1
    })
}

/// `ctx::world_snapshot` -- allocate a key (1-based) and copy the six
/// families. `0` when the store is full.
pub fn arm_world_snapshot(_a: Args) -> i64 {
    world::with_world_mut(|w| {
        let mut slot = usize::MAX;
        let mut i = 0;
        while i < crate::world::MAX_SNAPSHOTS {
            if !w.snapshots[i].used {
                slot = i;
                break;
            }
            i += 1;
        }
        if slot == usize::MAX {
            return 0;
        }
        let key = w.next_snapshot_key.max(1);
        w.next_snapshot_key = key + 1;
        let mut data = crate::world::SnapData::empty();
        data.pos = w.pos;
        data.money = w.money;
        data.state = w.player_keys;
        let mut t = 0;
        while t < 60 {
            data.owners[t] = w.tiles[t].owner;
            data.houses[t] = w.tiles[t].houses;
            data.mortgaged[t] = w.tiles[t].mortgaged;
            t += 1;
        }
        let e = &mut w.snapshots[slot];
        e.used = true;
        e.key = key;
        e.owners[0] = w.bound_uid;
        e.owners_n = 1;
        e.data = data;
        key as i64
    })
}

/// `ctx::world_restore` -- absolute, idempotent write-back; `1` on a key the
/// running instance owns, `0` otherwise.
pub fn arm_world_restore(a: Args) -> i64 {
    let [key, ..] = a;
    world::with_world_mut(|w| {
        let mut slot = usize::MAX;
        let mut i = 0;
        while i < crate::world::MAX_SNAPSHOTS {
            if w.snapshots[i].used && w.snapshots[i].key == key {
                slot = i;
                break;
            }
            i += 1;
        }
        if slot == usize::MAX {
            return 0;
        }
        let own = w.bound_uid;
        let mut owned = false;
        let mut o = 0;
        while o < w.snapshots[slot].owners_n as usize {
            if w.snapshots[slot].owners[o] == own {
                owned = true;
            }
            o += 1;
        }
        if !owned {
            return 0;
        }
        let data = w.snapshots[slot].data;
        w.pos = data.pos;
        w.money = data.money;
        w.player_keys = data.state;
        let mut t = 0;
        while t < 60 {
            w.tiles[t].owner = data.owners[t];
            w.tiles[t].houses = data.houses[t];
            w.tiles[t].mortgaged = data.mortgaged[t];
            t += 1;
        }
        1
    })
}

/// `ctx::discard_next_fx` -- arm the seat's one-shot. The draw loop is
/// engine-side and not modelled under Kani, so the arm only acknowledges.
pub fn arm_discard_next_fx(_a: Args) -> i64 {
    1
}

/// `ctx::auction` -- the bid loop is engine-side and not modelled under Kani.
/// `0` is the pause sentinel (`EXIT_NEED_INPUT`), so the body returns through
/// `Err(Prompt)` and the caller's continuation is untouched.
pub fn arm_auction(_a: Args) -> i64 {
    0
}

/// `ctx::persona_skins` -- an empty skin list under Kani (the roster is host
/// data). The two-call shape sees length 0 and stops.
pub fn arm_persona_skins(_a: Args) -> i64 {
    0
}

/// `ctx::world_snapshot_drop` -- forget the instance's ownership; the last
/// owner frees the key.
pub fn arm_world_snapshot_drop(a: Args) -> i64 {
    let [key, ..] = a;
    world::with_world_mut(|w| {
        let mut i = 0;
        while i < crate::world::MAX_SNAPSHOTS {
            if w.snapshots[i].used && w.snapshots[i].key == key {
                let own = w.bound_uid;
                let e = &mut w.snapshots[i];
                let mut kept = 0u8;
                let mut o = 0;
                while o < e.owners_n as usize {
                    if e.owners[o] != own {
                        e.owners[kept as usize] = e.owners[o];
                        kept += 1;
                    }
                    o += 1;
                }
                e.owners_n = kept;
                if kept == 0 {
                    e.used = false;
                }
                break;
            }
            i += 1;
        }
        1
    })
}

pub fn arm_trig_move_main(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.trig_main as i64)
}

pub fn arm_trig_move_roll(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.move_snap.roll) as i64
}

/// `trigger::move_tag(k)` — a named tag on the move payload (`0` when absent,
/// which is also the answer with no move at all).
pub fn arm_trig_move_tag(a: Args) -> i64 {
    let [a0, a1, ..] = a;
    let key = name_id(a0, a1);
    world::with_world(|w| w.move_snap.tags.get(key)) as i64
}

/// `trigger::name()` — two-call out-buffer writing the interned name bytes.
pub fn arm_trig_name(a: Args) -> i64 {
    let [p, n, ..] = a;
    let id = world::with_world(|w| w.counter_name);
    let bytes: &[u8] = if id == 0 {
        &b""[..]
    } else {
        crate::intern::resolve(crate::intern::NameId(id as u16))
    };
    if p == 0 {
        return bytes.len() as i64;
    }
    if n < bytes.len() as i32 {
        return bytes.len() as i64;
    }
    card_sdk::native::write(p, bytes);
    bytes.len() as i64
}

pub fn arm_trig_player(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.trig_player) as i64
}

/// `trigger::card_is(id)` — is the trigger about that card? Reads
/// `World::trig_card` (the trigger card's name id, 0 = none).
pub fn arm_trig_card_is(a: Args) -> i64 {
    let [a0, a1, ..] = a;
    let want = name_id(a0, a1);
    world::with_world(|w| (w.trig_card == want.0 as i32 && w.trig_card != 0) as i64)
}

/// `trigger::cards(buf, cap)` — the trigger card's name id as a 1-element
/// list. The guest decodes it as a name-id pair.
pub fn arm_trig_cards(a: Args) -> i64 {
    let [buf, cap, ..] = a;
    let id = world::with_world(|w| w.trig_card);
    if buf == 0 {
        return if id != 0 { 1 } else { 0 };
    }
    if cap < 1 {
        return -1;
    }
    // 1-element list: just the name id as i32.
    let bytes = (id as i32).to_le_bytes();
    card_sdk::native::write(buf, &bytes);
    1
}

pub fn arm_trig_set_cancelled(a: Args) -> i64 {
    let _ = a;
    world::with_world_mut(|w| {
        w.trig_cancelled = true;
    });
    0
}

pub fn arm_trig_set_abnormal_allowed(a: Args) -> i64 {
    let _ = a;
    world::with_world_mut(|w| {
        w.trig_abnormal_allowed = true;
    });
    0
}

pub fn arm_trig_set_move_roll(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        world::with_world_mut(|w| w.move_snap.roll = a0);
        0
    }
}

pub fn arm_trig_set_negate_effect(a: Args) -> i64 {
    let _ = a;
    world::with_world_mut(|w| {
        w.trig_negate = true;
    });
    0
}

pub fn arm_trig_set_pay_amount(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        world::with_world_mut(|w| {
            w.pay_amount = a0;
            w.pay_amount_set = true;
            w.push(Effect::SetPayAmount { to: a0 });
        });
        0
    }
}

/// G02 `spendBefore` fold: the last `set_pay_amount` write is the cost (a
/// written `0` is a free spend); with no write the caller's `cost` stands. A
/// cancel is not signalled and does not zero the cost.
pub fn fold_spend_before(cost: i32) -> i32 {
    let (set, v) = world::with_world(|w| (w.pay_amount_set, w.pay_amount));
    world::with_world_mut(|w| w.pay_amount_set = false);
    if set {
        v
    } else {
        cost
    }
}

pub fn arm_pay_command(a: Args) -> i64 {
    let [from, to, amount, spec, _, _] = a;
    let shape = spec & 0xF;
    let kind = (spec >> 4) & 0xF;
    let flags = (spec >> 8) & 0xF;
    let raw_tile = (spec >> 12) & 0xF_FFFF;
    let tile = if raw_tile > 0 { raw_tile - 1 } else { -1 };
    world::with_world_mut(|w| {
        w.trig_pay_kind = kind;
        w.trig_pay_is_fixed = shape == 1;
        w.trig_pay_total_stage = flags & 4 != 0;
        w.trig_pay_split_total = flags & 8 != 0;
        w.trig_settle_tile = tile;
    });
    // A `Fixed` leg moves the amount verbatim; the symbolic model does not
    // apply factors, so `Normal` moves it verbatim too. A `pay_shortfall`
    // world moves what the payer has and packs `SHORTFALL` (no `SETTLED`); a
    // full move packs `SETTLED`.
    let (short, avail) = world::with_world(|w| (w.pay_shortfall, w.money[player_idx(from)]));
    let moved = if short { amount.min(avail) } else { amount };
    if moved > 0 {
        world::with_world_mut(|w| {
            let f = player_idx(from);
            w.money[f] = (w.money[f] - moved).max(0);
            if to >= 0 {
                let t = player_idx(to);
                w.money[t] = w.money[t].saturating_add(moved);
            }
            // A bank payment (`to < 0`) is the `PayMoney` shape the specs read,
            // the same one `arm_pay` pushes; a seat-to-seat leg is `PayTo`.
            if to < 0 {
                w.push(Effect::PayMoney {
                    player: from,
                    n: moved,
                });
            } else {
                w.push(Effect::PayTo { from, to, n: moved });
            }
        });
    }
    let mut receipt = moved & card_sdk::abi::pay_receipt::AMOUNT_MASK;
    if shape == 1 {
        receipt |= card_sdk::abi::pay_receipt::FIXED;
    }
    if short {
        receipt |= card_sdk::abi::pay_receipt::SHORTFALL;
    } else {
        receipt |= card_sdk::abi::pay_receipt::SETTLED;
    }
    receipt as i64
}

pub fn arm_trig_set_pay_kind(a: Args) -> i64 {
    world::with_world_mut(|w| w.trig_pay_kind = a[0]);
    0
}

pub fn arm_trig_set_pay_shape(a: Args) -> i64 {
    // The model records no stage cursor, so a shape write always takes.
    world::with_world_mut(|w| w.trig_pay_is_fixed = a[0] == 1);
    1
}

/// Resolve a wire factor scope: `Auto` (0) is the innermost live one, and an
/// unopened `this` scope falls forward to its `next` instance.
fn resolve_scope(w: &crate::world::World, scope: i32) -> i32 {
    let auto = if w.settle_open > 0 {
        3
    } else if w.move_open {
        1
    } else {
        0
    };
    match if scope == 0 { auto } else { scope } {
        1 if !w.move_open => 0,
        3 if w.settle_open == 0 => 2,
        s => s,
    }
}

pub fn arm_multiply_pay_factor(a: Args) -> i64 {
    let milli = a[0].max(0);
    world::with_world_mut(|w| match resolve_scope(w, a[1]) {
        1 => w.plan_pay_factor = w.plan_pay_factor.saturating_mul(milli),
        2 => w.next_settle_pay_factor = w.next_settle_pay_factor.saturating_mul(milli),
        3 => w.settle_pay_factor = w.settle_pay_factor.saturating_mul(milli),
        _ => w.next_move_pay_factor = w.next_move_pay_factor.saturating_mul(milli),
    });
    0
}

pub fn arm_set_pay_factor(a: Args) -> i64 {
    let milli = a[0].max(0);
    // The `Auto` scope composes where a factor is live (MONEY-01).
    world::with_world_mut(|w| match resolve_scope(w, 0) {
        1 => w.plan_pay_factor = w.plan_pay_factor.saturating_mul(milli),
        2 => w.next_settle_pay_factor = w.next_settle_pay_factor.saturating_mul(milli),
        3 => w.settle_pay_factor = w.settle_pay_factor.saturating_mul(milli),
        _ => w.next_move_pay_factor = w.next_move_pay_factor.saturating_mul(milli),
    });
    0
}

pub fn arm_trig_set_pay_target(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        world::with_world_mut(|w| w.trig_target = a0);
        0
    }
}

pub fn arm_trig_target(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.trig_target) as i64
}

pub fn arm_trig_tile(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.trig_tile) as i64
}

pub fn arm_trig_value(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.pay_amount) as i64
}

pub fn arm_turn_key(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.turn_key) as i64
}

pub fn arm_turn_player(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.turn_player) as i64
}

pub fn arm_unplace(a: Args) -> i64 {
    let [a0, ..] = a;
    world::with_world_mut(|w| {
        if let Some(card) = w.remove_field(a0) {
            world::pile_push(&mut w.discard_pile, &mut w.discard_n, card);
        }
        w.push(Effect::UnplaceCard { uid: a0 });
    });
    0
}

pub fn arm_unplace_card(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    {
        world::with_world_mut(|w| {
            // `unplace_card` = running instance (`last_uid`); `unplace_at`
            // addresses `a0`; `unplace_card_named` is name-addressed and
            // still effect-only. All three share this arm.
            let uid = if (0..w.field_n as usize).any(|i| w.field[i].uid == a0) {
                a0
            } else {
                w.last_uid
            };
            if let Some(card) = w.remove_field(uid) {
                world::pile_push(&mut w.discard_pile, &mut w.discard_n, card);
            }
            w.push(Effect::UnplaceCard { uid });
        });
        0
    }
}

pub fn arm_plan_stub(_a: Args) -> i64 {
    0
}

/// `plan::claim` — an accepted offer takes this move for itself, which stands
/// every later offer's guard down (`G05` §5).
pub fn arm_plan_claim(_a: Args) -> i64 {
    world::with_world_mut(|w| w.plan_claimed = true);
    0
}

pub fn arm_plan_claimed(_a: Args) -> i64 {
    world::with_world(|w| w.plan_claimed as i64)
}

/// `plan::set_fixed_roll` — pins the face (`opts.plan_fixed_roll`).
pub fn arm_set_fixed_roll(a: Args) -> i64 {
    world::with_world_mut(|w| w.plan_fixed_roll = a[0]);
    0
}

/// `ctx::fixed_roll` — the pinned face, `-1` when unpinned.
pub fn arm_fixed_roll(_a: Args) -> i64 {
    world::with_world(|w| w.plan_fixed_roll as i64)
}

pub fn arm_protocol_stub(_a: Args) -> i64 {
    0
}

pub fn arm_stub_zero(a: Args) -> i64 {
    let _ = a;
    0
}

pub fn arm_trig_move_effective_steps(_a: Args) -> i64 {
    world::with_world(|w| {
        if w.move_snap.present() {
            w.move_snap.effective as i64
        } else {
            0
        }
    })
}

/// `plan::effective_steps()` — the move's length as the plan stands.
pub fn arm_move_effective_steps(_a: Args) -> i64 {
    world::with_world(|w| {
        game_kinds::move_plan::effective_steps(w.steps, None, w.move_snap.roll.max(0)) as i64
    })
}

pub fn arm_set_forced(a: Args) -> i64 {
    world::with_world_mut(|w| w.forced = a[0] != 0);
    0
}

pub fn arm_set_main_override(a: Args) -> i64 {
    world::with_world_mut(|w| w.main_override = a[0] != 0);
    0
}

pub fn arm_void_walk(_a: Args) -> i64 {
    world::with_world_mut(|w| w.void_walk = true);
    0
}

pub fn arm_clear_steps(_a: Args) -> i64 {
    world::with_world_mut(|w| w.steps = None);
    0
}

pub fn arm_set_steps(a: Args) -> i64 {
    world::with_world_mut(|w| w.steps = Some(a[0]));
    0
}

pub fn arm_set_teleport_to(a: Args) -> i64 {
    world::with_world_mut(|w| w.teleport = a[0] >= 0);
    0
}

/// `2` canonicalizes to `0` (even), mirroring the engine.
pub fn arm_set_parity(a: Args) -> i64 {
    world::with_world_mut(|w| w.parity = if a[0] == 2 { 0 } else { a[0] });
    0
}

pub fn arm_add_extra_steps(a: Args) -> i64 {
    world::with_world_mut(|w| w.steps = Some(w.steps.unwrap_or(0) + a[0]));
    0
}

/// Reader stubs whose "nothing here" answer is `-1` (geometry off the model
/// board, missing payload).
pub fn arm_stub_neg(a: Args) -> i64 {
    let _ = a;
    -1
}

// ===========================================================================
// Cross-card messages (`ctx::send` / `On::Message`)
// ===========================================================================
//
// The kani facade packs Target / Message as fixed records (see
// `card-sdk/src/ctx.rs` `target_bytes` / `msg_bytes` / `send_card_msg`).
// `arm_send` unpacks them, stashes the payload on `World`, and dispatches to
// the receiver's `On::Message` body. Generic support stays for `mark:cp`'s
// `"place"` channel and test fixtures; the PP fans channel is gone (state
// dict + `FansFlip` raise, see [`arm_raise`]).

/// Read a little-endian i32 at `off` from a packed arena record.
fn le_i32(b: &[u8], off: usize) -> i32 {
    let mut t = [0u8; 4];
    let mut i = 0usize;
    while i < 4 {
        if let Some(&x) = b.get(off + i) {
            t[i] = x;
        }
        i += 1;
    }
    i32::from_le_bytes(t)
}

fn le_u16(b: &[u8], off: usize) -> u16 {
    let lo = b.get(off).copied().unwrap_or(0) as u16;
    let hi = b.get(off + 1).copied().unwrap_or(0) as u16;
    lo | (hi << 8)
}

/// Unpack a packed arena record into a small stack buffer.
fn unpack_arena(handle: i32, len: i32, out: &mut [u8; 24]) -> usize {
    let n = (len.max(0) as usize).min(out.len());
    let b = card_sdk::native::read(handle, n);
    let mut i = 0usize;
    while i < n {
        if let Some(&x) = b.get(i) {
            out[i] = x;
        }
        i += 1;
    }
    n
}

/// `ctx::send` — unpack the packed Target/Message, run the receiver's
/// `On::Message` body, return its reply (0 when nobody answers).
pub fn arm_send(a: Args) -> i64 {
    let [tp, tl, np, nl, pp, pl] = a;
    // Message name rides the interned `s(name)` handle on `np`.
    let name = name_id(np, nl);
    // Target pack: kind (0=Uid, 1=Card, 2=Board), who, card NameId.
    let mut tbuf = [0u8; 24];
    unpack_arena(tp, tl, &mut tbuf);
    let kind = le_i32(&tbuf, 0);
    let who = le_i32(&tbuf, 4);
    let card = le_u16(&tbuf, 8);
    // Payload pack: seat, a, b, c, tile.
    let mut pbuf = [0u8; 24];
    unpack_arena(pp, pl, &mut pbuf);
    let seat = le_i32(&pbuf, 0);
    let av = le_i32(&pbuf, 4);
    let bv = le_i32(&pbuf, 8);
    let cv = le_i32(&pbuf, 12);
    let tile = le_i32(&pbuf, 16);

    // Sender context for the handler.
    let (sender_uid, sender_seat) = world::with_world(|w| (w.last_uid, w.trig_player));

    // Stash the payload — `msg_*` reads it. No named-channel special-case:
    // the only previous client (PP fans) moved to the state dict +
    // `FansFlip` raise. Generic `ctx::send` / `msg_*` stay for `mark:cp`
    // (`"place"`) and tests; those proofs call the body (`*_kani`) directly.
    world::with_world_mut(|w| {
        w.msg_name = name.0;
        w.msg_seat = seat;
        w.msg_arg0 = av;
        w.msg_arg1 = bv;
        w.msg_arg2 = cv;
        w.msg_tile = tile;
        w.msg_reply = 0;
        w.msg_sender_uid = sender_uid;
        w.msg_sender_seat = sender_seat;
    });
    let _ = (kind, who, card);
    0
}

pub fn arm_msg_sender_uid(_a: Args) -> i64 {
    world::with_world(|w| w.msg_sender_uid) as i64
}

pub fn arm_msg_sender_seat(_a: Args) -> i64 {
    world::with_world(|w| w.msg_sender_seat) as i64
}

/// `msg_name(0,0)` under kani returns the interned NameId handle (see
/// `ctx::message::name_is`). A fill call is unused (`read_str` returns empty).
pub fn arm_msg_name(a: Args) -> i64 {
    let [p, n, ..] = a;
    if p == 0 && n == 0 {
        let id = world::with_world(|w| w.msg_name);
        return intern::encode_handle(intern::NameId(id)) as i64;
    }
    0
}

pub fn arm_msg_a(_a: Args) -> i64 {
    world::with_world(|w| w.msg_arg0) as i64
}

pub fn arm_msg_b(_a: Args) -> i64 {
    world::with_world(|w| w.msg_arg1) as i64
}

pub fn arm_msg_c(_a: Args) -> i64 {
    world::with_world(|w| w.msg_arg2) as i64
}

pub fn arm_msg_tile(_a: Args) -> i64 {
    world::with_world(|w| w.msg_tile) as i64
}

pub fn arm_msg_seat(_a: Args) -> i64 {
    world::with_world(|w| w.msg_seat) as i64
}

pub fn arm_msg_text(_a: Args) -> i64 {
    0
}

pub fn arm_msg_reply(a: Args) -> i64 {
    let [v, ..] = a;
    world::with_world_mut(|w| w.msg_reply = v);
    0
}
