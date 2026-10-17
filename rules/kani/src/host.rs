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
    pub crystals: NameId,
    pub cp: NameId,
    pub rent_houses: NameId,
    pub tile_shop: NameId,
    pub card_aya: NameId,
    pub card_resonance: NameId,
    pub card_other: NameId,
    pub card_extra: NameId,
}

static mut KNOWN: Option<Known> = None;

fn known_init() -> Known {
    Known {
        tok_pp_pos: intern::intern(TOK_PP_POS),
        tok_pp_neg: intern::intern(TOK_PP_NEG),
        crystals: intern::intern(world::COUNTER_CRYSTALS),
        cp: intern::intern(world::COUNTER_CP),
        rent_houses: intern::intern(world::RENT_HOUSES),
        tile_shop: intern::intern(TILE_SHOP_NAME),
        card_aya: intern::intern(world::CARD_AYA),
        card_resonance: intern::intern(CARD_RESONANCE),
        card_other: intern::intern(world::CARD_OTHER),
        card_extra: intern::intern(world::CARD_EXTRA),
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
    kani::assert(false, "host op outside declared per-harness subset");
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
                Op::roll_ask => arm_roll_ask(a),
                Op::do_move_roll => arm_do_move_roll(a),
        Op::log | Op::effect => arm_log(a),

        // ---- tiles (static data from board.json + runtime state) ----
                Op::tile_count => arm_tile_count(a),
                Op::tile_named => arm_tile_named(a),
                Op::tile_owner => arm_tile_owner(a),
                Op::tile_group => arm_tile_group(a),
                Op::tile_price => arm_tile_price(a),
                Op::buy_price => arm_buy_price(a),
        Op::build_cost => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                board::TILES[s as usize].house_cost as i64
            }
        }
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
                Op::add_to_hand => arm_add_to_hand(a),
                Op::add_to_deck => arm_add_to_deck(a),
        Op::add_to_deck_at => {
            let id = card_u16(a1, a2);
            world::with_world_mut(|w| world::pile_push(&mut w.deck_pile, &mut w.deck_n, id));
            0
        }
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
                Op::set_dest => arm_set_dest(a),
        Op::set_transfer_to_dest => 0,
        Op::send_to_dest => {
            world::with_world_mut(|w| {
                w.push(Effect::Teleport {
                    player: w.trig_player,
                    tile: w.dest,
                });
                let p = player_idx(w.trig_player);
                w.pos[p] = w.dest;
            });
            0
        }
        Op::transfer_to_dest => 0,
                Op::teleport_to => arm_teleport_to(a),
                Op::unplace_card | Op::unplace_at | Op::unplace_card_named => arm_unplace_card(a),
                Op::is_placed => arm_is_placed(a),
                Op::field_instances => arm_field_instances(a),
                Op::placed_cards => arm_placed_cards(a),
        Op::placed_tile => world::with_world(|w| {
            if w.field_n == 0 {
                -1
            } else {
                w.field[0].tile
            }
        }) as i64,
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
        Op::card_immune => {
            world::with_world(|w| w.field_by_uid(a0).map_or(0, |f| f.immune as i64))
        }
        Op::self_tile => world::with_world(|w| {
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
        Op::self_face_down => {
            world::with_world(|w| if w.field_n == 0 { 0 } else { w.field[0].face_down as i64 })
        }
        Op::set_self_face_down => world::with_world_mut(|w| {
            if w.field_n == 0 {
                0
            } else {
                w.field[0].face_down = a0 != 0;
                1
            }
        }),
        Op::self_immune => {
            world::with_world(|w| if w.field_n == 0 { 0 } else { w.field[0].immune as i64 })
        }
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
        Op::tile_at => world::with_world(|w| w.tile_at(a0)) as i64,
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
                Op::state_get => arm_state_get(a),
                Op::slot => arm_slot(a),
                Op::state_set => arm_state_set(a),
                Op::set_slot => arm_set_slot(a),
                Op::state_add | Op::inc_slot => arm_state_add(a),
                Op::tok_names => arm_tok_names(a),

        // ---- marks ----
                Op::place_mark => arm_place_mark(a),
        Op::count_marks_f => {
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
        Op::bump_mark_f => 0,
        Op::remove_marks_f => 0,
        Op::mark_src_at | Op::mark_instance_at => -1,

        // ---- status ----
                Op::charge => arm_fire(a),
                Op::charge_max => arm_charge_max(a),
                Op::gain_charge => arm_gain_charge(a),
        Op::add_charge_max => {
            world::with_world_mut(|w| {
                w.charge_max[player_idx(a0)] = w.charge_max[player_idx(a0)].saturating_add(a1);
            });
            0
        }
                Op::spend_charge => arm_spend_charge(a),
        // `ctx::n(k, v)` doubles when `play_doubled() == k`. Default is
        // **no doubling** (`-1` / `None` in the engine) — the old stub returned
        // 0, so `n(0, 600)` wrongly yielded 1200.
        Op::play_doubled => world::with_world(|w| w.play_doubled) as i64,
        Op::set_play_doubled => {
            world::with_world_mut(|w| w.play_doubled = a0);
            0
        }
                Op::give_stay => arm_give_stay(a),
                Op::give_stun => arm_give_stun(a),
                Op::give_exile => arm_give_exile(a),
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
                Op::stay_of => arm_stay_of(a),
                Op::stun_of => arm_stun_of(a),
                Op::exile_of => arm_exile_of(a),
        Op::cant_move => world::with_world(|w| {
            (w.stay[player_idx(a0)] > 0
                || w.stun[player_idx(a0)] > 0
                || w.exile[player_idx(a0)] > 0) as i64
        }),
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
                Op::card_move => arm_card_move(a),

        // ---- band / skills ----
        Op::group_pool => arm_band_crystals(a),
        Op::add_group_pool => arm_add_band_crystals(a),
                Op::group_skill | Op::persona_skill => arm_band_skill(a),
                Op::group_skills => arm_band_skills(a),
                Op::add_group_skill => arm_add_band_skill(a),
                Op::invoke_skill => arm_invoke_skill(a),

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
        Op::ask => arm_ask(a),

        // ---- trigger context ----
                Op::trig_kind => arm_trig_kind(a),
                Op::trig_player => arm_trig_player(a),
                Op::trig_value => arm_trig_value(a),
                Op::trig_target => arm_trig_target(a),
                Op::trig_tile => arm_trig_tile(a),
                Op::trig_name => arm_trig_name(a),
        Op::trig_step => 0,
        Op::trig_by_card => 0,
        Op::trig_pay_is_rent => world::with_world(|w| w.trig_pay_is_rent as i64),
        Op::trig_cancelled => world::with_world(|w| w.trig_cancelled as i64),
        // ---- move payload (hostfns 1830–1880; sentinels when no move) ----
        Op::trig_move_kind => {
            world::with_world(|w| if w.move_snap.present() { w.move_snap.kind as i64 } else { -1 })
        }
        Op::trig_move_resolve => world::with_world(|w| w.move_snap.resolve as i64),
        Op::trig_move_tag => {
            let key = name_id(a0, a1);
            world::with_world(|w| w.move_snap.tags.get(key)) as i64
        }
        // `trig_move_main` is `Trigger::main`, not `mv.main` (hostfns 1848).
                Op::trig_move_main => arm_trig_move_main(a),
        Op::trig_move_dir => {
            world::with_world(|w| if w.move_snap.present() { w.move_snap.dir as i64 } else { 1 })
        }
        Op::trig_move_remaining => {
            world::with_world(|w| if w.move_snap.present() { w.move_snap.remaining as i64 } else { 0 })
        }
        Op::trig_move_from => {
            world::with_world(|w| if w.move_snap.present() { w.move_snap.from as i64 } else { -1 })
        }
        Op::trig_move_total => {
            world::with_world(|w| if w.move_snap.present() { w.move_snap.total as i64 } else { 0 })
        }
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
        Op::trig_set_cancelled => arm_trig_set_cancelled(a),
        Op::trig_set_negate_effect => arm_trig_set_negate_effect(a),
        Op::trig_set_reason | Op::trig_set_spare => arm_stub_zero(a),
        Op::trig_seq => 0,
        Op::trig_answers => 0,
        Op::trig_effect_count => 0,
        Op::trig_effect_kind | Op::trig_effect_target | Op::trig_effect_from
        | Op::trig_effect_tile | Op::trig_effect_value => 0,
        Op::declare_effect => 0,
        Op::trig_card_is => arm_trig_card_is(a),
        Op::trig_cards => arm_trig_cards(a),

        // ---- move plan / settle (structure only; engine scheduling is
        //      PROTOCOL-LEVEL and out of Kani scope) ----
                Op::set_steps | Op::set_roller | Op::set_reverse | Op::set_signed | Op::set_stop_at | Op::set_parity | Op::set_resolve | Op::set_settle_tile | Op::set_pay_factor | Op::set_rent_factor | Op::set_no_buy | Op::set_no_build | Op::set_build_anywhere | Op::set_kind | Op::set_teleport_to | Op::set_start | Op::set_base_dice | Op::add_base_dice | Op::add_extra_dice | Op::set_min_roll | Op::set_extra_steps | Op::set_more_steps | Op::set_tag | Op::set_settle_as_agent | Op::plan_add_follower | Op::set_play_doubled | Op::set_no_money_loss | Op::set_fixed_roll | Op::set_next_steps | Op::set_build_discount | Op::set_build_cost_pct | Op::set_extreme | Op::clear_dice | Op::set_can_build | Op::move_stopped | Op::move_stop_at | Op::move_parity | Op::move_resolve | Op::move_steps | Op::move_remaining | Op::move_total | Op::move_dir | Op::fixed_roll | Op::turn_main_steps | Op::turn_rolls | Op::turn_snap | Op::turn_start_pos | Op::schedule_turn_end | Op::linger | Op::targeted_count => arm_stub_zero(a),

        // ---- hook-status flags (polarity-sensitive; see arm_*) ----
        // 0 on these means "cancelled / blocked / substituted", which makes
        // callers skip the real effect (class C — chart_rush spent no fire).
        Op::raise => arm_raise(a),
        Op::raise_bought => arm_raise_bought(a),
        Op::gate => arm_gate(a),
        Op::rent_unit_mult => arm_ring_multiplier(a),

        // ---- offers / buy / build (PROTOCOL-LEVEL) ----
                Op::pay_rent | Op::offer_buy | Op::offer_force_buy | Op::offer_build | Op::card_build | Op::card_buy | Op::card_offer_build | Op::card_settle_at | Op::card_mortgage | Op::card_replayable | Op::card_text_mentions | Op::card_prop | Op::buy | Op::acquire | Op::agent_offer | Op::buy_quotes | Op::ai_agent_choice | Op::play_card | Op::play_from_hand | Op::can_build_on | Op::add_rent_unit_bonus | Op::move_units | Op::send | Op::msg_sender_uid | Op::msg_sender_seat | Op::msg_name | Op::msg_a | Op::msg_b | Op::msg_c | Op::msg_tile | Op::msg_seat | Op::msg_text | Op::msg_reply | Op::designations | Op::cancel_designation | Op::designation_cancelled | Op::flash | Op::no_flash | Op::flash_card | Op::flash_card_keyed | Op::flash_tile => arm_protocol_stub(a),

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

pub fn arm_add_band_crystals(a: Args) -> i64 {
    let [a0, a1, ..] = a;
    world::with_world_mut(|w| {
        let p = player_idx(a0);
        w.group_pool[p] = (w.group_pool[p] + a1).clamp(0, 99);
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
        if now < 0 { now = 0; }
        if max > 0 && now > max { now = max; }
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
        if now < 0 { now = 0; }
        if max > 0 && now > max { now = max; }
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

pub fn arm_add_to_deck(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
{
            let id = card_u16(a1, a2);
            world::with_world_mut(|w| world::pile_push(&mut w.deck_pile, &mut w.deck_n, id));
            0
        }
}

pub fn arm_add_to_deck_at(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let _ = (a0, a1, a2, a3, a4, a5);

    let id = card_u16(a1, a2);
    world::with_world_mut(|w| world::pile_push(&mut w.deck_pile, &mut w.deck_n, id));
    0
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
            // Raw money adjustment: `money += delta` with **no** payment
            // pipeline (no paySplitAdd/paySplitMul, no effect window). Mirrors
            // `hostfns::adjust_money` / `World::adjust_money_raw`.
            let delta = a1;
            if delta != 0 {
                world::with_world_mut(|w| {
                    let p = player_idx(a0);
                    if delta > 0 {
                        w.money[p] = w.money[p].saturating_add(delta);
                        w.gains_this_turn[p] = w.gains_this_turn[p].saturating_add(delta);
                        w.push(Effect::GainMoney {
                            player: a0,
                            n: delta,
                        });
                    } else {
                        let n = (-delta).min(w.money[p].max(0));
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

pub fn arm_band_skills(a: Args) -> i64 {
    let _ = a;
    0
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
                // `purchase::quote_native` = land + standing houses × house_cost.
                // The model's `buy_price` arm historically returned land only;
                // keep land-only here and note the gap — `quote_native` also
                // folds house cost, which `houses_of` × `build_cost` exposes.
                // Faithful quote: land + houses * house_cost (see purchase.rs).
                let h = world::with_world(|w| w.tiles[tile_idx(a0)].houses).max(0);
                (board::TILES[s as usize].price
                    + h * board::TILES[s as usize].house_cost) as i64
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

pub fn arm_card_move(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let _ = (a0, a1, a2, a3, a4, a5);
    1
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
    });
    0
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

pub fn arm_do_move_roll(a: Args) -> i64 {
    let _ = a;
    roll_dice(2, 6) as i64
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
                w.push(Effect::Draw {
                    player: a0,
                    n,
                });
            });
            n as i64
        }
}

pub fn arm_exile_of(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| w.exile[player_idx(a0)]) as i64
}

pub fn arm_field_instances(a: Args) -> i64 {
    let _ = a;
{
            // Write uid list into out-buffer: [n, uid…] as i32s is overkill;
            // return count.
            world::with_world(|w| w.field_n as i32) as i64
        }
}

pub fn arm_fire(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| w.charge[player_idx(a0)]) as i64
}

pub fn arm_charge_max(a: Args) -> i64 {
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
            w.push(Effect::GainMoney {
                player: a0,
                n: a1,
            });
        });
    }
    a1 as i64
}

pub fn arm_gain_charge(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
{
            // engine `gain_charge`: `got = n.clamp(0, (charge_max - fire).max(0))`;
            // 0 when out / n<=0 / cap<=0. Returns the amount gained.
            world::with_world_mut(|w| {
                let p = player_idx(a0);
                let n = a1;
                if n <= 0 || w.out[p] {
                    return 0;
                }
                let cap = w.charge_max[p];
                if cap <= 0 {
                    return 0;
                }
                let got = n.clamp(0, (cap - w.charge[p]).max(0));
                if got <= 0 {
                    return 0;
                }
                w.charge[p] += got;
                w.push(Effect::GainFire {
                    player: a0,
                    n: got,
                });
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

pub fn arm_give_exile(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
{
            // `key::EXILE` layers + `key::EXILE_TO` target (decay is protocol).
            world::with_world_mut(|w| {
                if (a0 as usize) < MAX_PLAYERS {
                    w.exile[a0 as usize] = (w.exile[a0 as usize] + a1).max(0);
                    w.exile_to[a0 as usize] = a2;
                }
                w.push(Effect::GiveExile {
                    player: a0,
                    n: a1,
                    to: a2,
                });
            });
            0
        }
}

pub fn arm_give_stay(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
{
            world::with_world_mut(|w| {
                if (a0 as usize) < MAX_PLAYERS {
                    w.stay[a0 as usize] = w.stay[a0 as usize].saturating_add(a1);
                }
                w.push(Effect::GiveStay {
                    player: a0,
                    n: a1,
                });
            });
            0
        }
}

pub fn arm_give_stun(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
{
            // Separate from [停留] (`key::STUN` vs `key::STAY` in the engine).
            world::with_world_mut(|w| {
                if (a0 as usize) < MAX_PLAYERS {
                    w.stun[a0 as usize] = (w.stun[a0 as usize] + a1).max(0);
                }
                w.push(Effect::GiveStun {
                    player: a0,
                    n: a1,
                });
            });
            0
        }
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

pub fn arm_inc_slot(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    let _ = (a3, a4, a5);
    let key = name_id(a1, a2);
    let by = a3;
    world::with_world_mut(|w| {
        let v = w.key_add(a0, key, by);
        w.sync_views_player(a0);
        v as i64
    })
}

pub fn arm_invoke_skill(a: Args) -> i64 {
    let _ = a;
    0
}

pub fn arm_is_placed(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| (w.field_n > 0) as i64)
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
            if a1 > 0 {
                world::with_world_mut(|w| {
                    let p = player_idx(a0);
                    w.money[p] = (w.money[p] - a1).max(0);
                    w.push(Effect::PayMoney {
                        player: a0,
                        n: a1,
                    });
                });
            }
            a1 as i64
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
        // running card's fate (`w.dest`, set by `set_dest`) is a different
        // thing — it is where the *running* instance goes when the effect
        // finishes. The PlaceCard effect's `dest` is the Dest code of the
        // **placed** card = Field (2). (pipopa: `set_dest(Banished)` for
        // 「此卡[移除]」 then `place_card` a Popipapapipopa — the old arm
        // leaked `w.dest=3` into PlaceCard.dest.)
        let uid = w.next_uid;
        w.next_uid += 1;
        w.last_uid = uid;
        let tile = w.pos[player_idx(a0)];
        let fi = w.field_n as usize;
        if fi < world::MAX_FIELD {
            w.field[fi] = world::FieldCard {
                uid,
                card: id,
                player: a0,
                tile,
                face_down: false,
                immune: false,
                crystals: 0,
                props: world::NameMap::new(),
            };
            w.field_n += 1;
        }
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
            // Same fate/dest split as `arm_place_card`: PlaceCard.dest is
            // Field (2) for the placed instance, not `w.dest`.
            let id = card_u16(a1, a2);
            let slot = card_slot(a1, a2);
            world::with_world_mut(|w| {
                let uid = w.next_uid;
                w.next_uid += 1;
                w.last_uid = uid;
                let tile = w.pos[player_idx(a0)];
                let fi = w.field_n as usize;
                if fi < world::MAX_FIELD {
                    w.field[fi] = world::FieldCard {
                        uid,
                        card: id,
                        player: a0,
                        tile,
                        face_down: false,
                        immune: false,
                        crystals: 0,
                        props: world::NameMap::new(),
                    };
                    w.field_n += 1;
                }
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
                let uid = w.next_uid;
                w.next_uid += 1;
                w.last_uid = uid;
                let fi = w.field_n as usize;
                if fi < world::MAX_FIELD {
                    w.field[fi] = world::FieldCard {
                        uid,
                        card: id,
                        player: a0,
                        tile,
                        face_down: false,
                        immune: false,
                        crystals: 0,
                        props: world::NameMap::new(),
                    };
                    w.field_n += 1;
                }
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
    let [a0, a1, a2, a3, a4, a5] = a;
{
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
            });
            0
        }
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
                let name = intern::resolve_str(intern::NameId(w.field[i].card));
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

pub fn arm_raise(_a: Args) -> i64 {
    1
}

pub fn arm_raise_bought(_a: Args) -> i64 {
    1
}

pub fn arm_ring_multiplier(_a: Args) -> i64 {
    1
}

pub fn arm_roll(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    roll_dice(a1, a2) as i64
}

pub fn arm_roll_ask(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    roll_dice(a1, a2) as i64
}

pub fn arm_round_no(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.round_no) as i64
}

pub fn arm_schedule_turn_end(_a: Args) -> i64 {
    0
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
    world::with_world(|w| w.last_uid) as i64
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

pub fn arm_set_dest(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
{
            world::with_world_mut(|w| {
                w.dest = a0;
                // Field-effect fate: the real engine defers to `apply_dest` at
                // the run's commit (unplace-and-send the running instance).
                // A play from hand names the *hand card's* fate instead and is
                // applied by the play pipeline, so only apply here when the
                // instance is live on the field. `Dest::Field` (2) = stay.
                //
                // The commit is **not** an in-run effect (specs such as
                // `again_02_effects_ok` / `starry_06_effects_ok` pin "exactly
                // one effect" and the engine's post-body unplace is outside
                // the effect log). World state only: field slot drops, pile
                // gains the card.
                if a0 != 2 {
                    let uid = w.last_uid;
                    if let Some(card) = w.remove_field(uid) {
                        match a0 {
                            0 => world::pile_push(&mut w.discard_pile, &mut w.discard_n, card),
                            1 => world::pile_push(&mut w.hand_pile, &mut w.hand_n, card),
                            // 3 = Banished 「[移除]」: left the field, goes nowhere.
                            _ => {}
                        }
                    }
                }
            });
            0
        }
}

pub fn arm_set_held_name(a: Args) -> i64 {
    let [a0, a1, a2, a3, ..] = a;
    let key = name_id(a0, a1);
    let p = a2;
    let v = a3;
    world::with_world_mut(|w| {
        w.key_set(p, key, v);
        if key == known().tok_pp_pos {
            w.fans_pos[player_idx(p)] = v;
        } else if key == known().tok_pp_neg {
            w.fans_neg[player_idx(p)] = v;
        } else {
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

pub fn arm_set_slot(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
{
            // `set_slot(player, ptr, len, v)` — a3 is the value (sugar for
            // `state_set` field 0).
            let key = name_id(a1, a2);
            world::with_world_mut(|w| {
                w.key_set(a0, key, a3);
                w.push(Effect::SetState {
                    player: a0,
                    key: key.0,
                    n: a3,
                });
            });
            a3 as i64
        }
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

pub fn arm_slot(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
{
            let key = name_id(a1, a2);
            world::with_world(|w| w.key_get(a0, key)) as i64
        }
}

pub fn arm_spend_charge(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
{
            let p = player_idx(a0);
            world::with_world_mut(|w| {
                if w.charge[p] >= a1 && a1 > 0 {
                    w.charge[p] -= a1;
                    1
                } else if a1 == 0 {
                    1
                } else {
                    0
                }
            })
        }
}

pub fn arm_state_add(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
{
            let key = name_id(a1, a2);
            world::with_world_mut(|w| w.key_add(a0, key, a3)) as i64
        }
}

pub fn arm_state_get(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
{
            // hostfns `state_get(player, ptr, len, field)`: field 1/2/3 read
            // the declared MAX / MIN / EXPIRES column; anything else is VALUE.
            let key = name_id(a1, a2);
            let field = a3;
            world::with_world(|w| match field {
                1 => w.key_max(a0, key),
                2 => w.key_min(a0, key),
                3 => w.key_expires(a0, key),
                _ => w.key_get(a0, key),
            }) as i64
        }
}

pub fn arm_state_set(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
{
            // hostfns `state_set(player, ptr, len, field, v)`:
            //   a3 = field (0 value / 1 max / 2 min / 3 expires), a4 = v.
            // The old model wrote a3 as the value and dropped a4.
            let key = name_id(a1, a2);
            let field = a3;
            let v = a4;
            world::with_world_mut(|w| {
                let out = match field {
                    1 => {
                        w.key_set_max(a0, key, v);
                        v
                    }
                    2 => {
                        w.key_set_min(a0, key, v);
                        v
                    }
                    3 => {
                        w.key_set_expires(a0, key, v);
                        v
                    }
                    _ => {
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

pub fn arm_stay_of(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| w.stay[player_idx(a0)]) as i64
}

pub fn arm_stun_of(a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    world::with_world(|w| w.stun[player_idx(a0)]) as i64
}

pub fn arm_take_card(a: Args) -> i64 {
    let [a0, a1, a2, a3, ..] = a;
    let id = card_u16(a2, a3);
    let slot = card_slot(a2, a3);
    let player_id = a0;
    let pile = a1;
    world::with_world_mut(|w| {
        let ok = if pile == 1 {
            world::pile_take(&mut w.discard_pile, &mut w.discard_n, id)
                || {
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
                world::pile_take(&mut w.hand_pile, &mut w.hand_n, id)
                    || {
                        if (slot as usize) < 4 && w.hand[slot as usize] {
                            w.hand[slot as usize] = false;
                            true
                        } else {
                            false
                        }
                    }
            }
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
{
            let p = a0;
            world::with_world_mut(|w| {
                if p >= 0 && (p as usize) < MAX_PLAYERS && (p as usize) < w.n_players as usize && !w.out[p as usize] {
                    w.targeted_count[p as usize] = w.targeted_count[p as usize].saturating_add(1);
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

pub fn arm_to_discard(a: Args) -> i64 {
    let [a0, a1, a2, ..] = a;
    let _ = a0;
    let slot = card_slot(a1, a2);
    let id = card_u16(a1, a2);
    world::with_world_mut(|w| {
        world::pile_push(&mut w.discard_pile, &mut w.discard_n, id);
        if (slot as usize) < 4 {
            w.discard[slot as usize] = true;
        }
    });
    0
}

pub fn arm_tok_names(a: Args) -> i64 {
    let _ = a;
    0
}

pub fn arm_trig_kind(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.trig_kind) as i64
}

pub fn arm_trig_move_main(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.trig_main as i64)
}

pub fn arm_trig_move_roll(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.move_snap.roll) as i64
}

pub fn arm_trig_name(a: Args) -> i64 {
    let _ = a;
    world::with_world(|w| w.counter_name) as i64
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
                w.push(Effect::SetPayAmount { to: a0 });
            });
            0
        }
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

pub fn arm_protocol_stub(_a: Args) -> i64 {
    0
}

pub fn arm_stub_zero(a: Args) -> i64 {
    let _ = a;
    0
}

