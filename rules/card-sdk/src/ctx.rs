//! Host functions available to card effects (the Rust port of the C# `H.*` surface).
//!
//! Every piece of text a card shows (log lines, mark notes, prompts, options) is a
//! [`Msg`]: a key in the card crate's `locales/*.json` plus typed arguments. The
//! host never sees display text, and each player reads it in their own language.
//!
//! Ops that would need a player decision do not exist here on purpose: `ask_*`
//! blocks, the host publishes the question, and the run is replayed with the
//! answer -- so a card reads like straight-line code while staying deterministic.

#[cfg(target_arch = "wasm32")]
use alloc::{string::String, vec::Vec};

pub use crate::abi::{AbKind, CardPile};
use crate::abi::{MoveKind, PromptKind, TriggerKind};
use crate::msg::Msg;
use crate::Prompt;

mod sys {
    // The link attribute is wasm-only; on other targets the imports stay
    // declared so the analyzer resolves `ctx::*` in card sources.
    #[cfg_attr(target_arch = "wasm32", link(wasm_import_module = "bandori"))]
    extern "C" {
        // dice & log
        pub fn roll(player_id: i32, count: i32, sides: i32) -> i32;
        pub fn log(player_id: i32, ptr: i32, len: i32);
        pub fn effect(player_id: i32, ptr: i32, len: i32);
        // board
        pub fn tile_count() -> i32;
        pub fn tile_named(ptr: i32, len: i32) -> i32;
        pub fn tile_owner(tile: i32) -> i32;
        pub fn player_pos(player_id: i32) -> i32;
        pub fn tile_steps_ahead(player_id: i32, steps: i32) -> i32;
        pub fn rent_of(tile: i32) -> i32;
        pub fn buy_price(tile: i32) -> i32;
        pub fn build_cost(tile: i32) -> i32;
        pub fn mortgage_value(tile: i32) -> i32;
        pub fn owned_count(player_id: i32) -> i32;
        pub fn owned_at(player_id: i32, index: i32) -> i32;
        pub fn is_buyable(tile: i32) -> i32;
        pub fn is_shop(tile: i32) -> i32;
        pub fn is_ring(tile: i32) -> i32;
        pub fn is_circle(tile: i32) -> i32;
        pub fn is_live_house(tile: i32) -> i32;
        pub fn tile_group(tile: i32) -> i32;
        pub fn tile_price(tile: i32) -> i32;
        pub fn houses_of(tile: i32) -> i32;
        pub fn set_houses(tile: i32, n: i32);
        pub fn add_house(tile: i32, n: i32) -> i32;
        pub fn mortgaged_of(tile: i32) -> i32;
        pub fn set_mortgaged(tile: i32, v: i32);
        pub fn set_owner(tile: i32, player_id: i32);
        pub fn dist(a: i32, b: i32) -> i32;
        pub fn tile_forward(a: i32, b: i32) -> i32;
        pub fn neighbor(player_id: i32, dir: i32) -> i32;
        pub fn players_on_count(tile: i32, except: i32) -> i32;
        pub fn players_on_at(tile: i32, except: i32, index: i32) -> i32;
        // players & money
        pub fn player_count() -> i32;
        pub fn player_out(player_id: i32) -> i32;
        pub fn others_count(player_id: i32) -> i32;
        pub fn others_at(player_id: i32, index: i32) -> i32;
        pub fn money(player_id: i32) -> i32;
        pub fn gain(player_id: i32, amount: i32, ptr: i32, len: i32) -> i32;
        pub fn pay(player_id: i32, amount: i32, ptr: i32, len: i32) -> i32;
        // hand & deck
        pub fn draw(player_id: i32, n: i32) -> i32;
        pub fn add_to_hand(player_id: i32, ptr: i32, len: i32);
        pub fn add_to_deck(player_id: i32, ptr: i32, len: i32, shuffle: i32);
        pub fn add_to_deck_at(player_id: i32, ptr: i32, len: i32, pos: i32);
        pub fn take_card(player_id: i32, pile: i32, ptr: i32, len: i32) -> i32;
        pub fn cards_in(player_id: i32, pile: i32, buf: i32, cap: i32) -> i32;
        pub fn to_discard(player_id: i32, ptr: i32, len: i32);
        pub fn hand_count(player_id: i32, ptr: i32, len: i32) -> i32;
        pub fn hand_size(player_id: i32) -> i32;
        pub fn discard_count(player_id: i32, ptr: i32, len: i32) -> i32;
        pub fn deck_count(player_id: i32) -> i32;
        pub fn discard_size(player_id: i32) -> i32;
        pub fn discard_from_hand(player_id: i32, ptr: i32, len: i32) -> i32;
        pub fn shuffle_into_deck(player_id: i32, hand: i32, discard: i32) -> i32;
        // field cards
        pub fn place_card_at(player_id: i32, cp: i32, cl: i32, ptr: i32, len: i32) -> i32;
        pub fn set_dest(dest: i32);
        pub fn set_transfer_to_dest(to: i32, dest: i32);
        pub fn send_to_dest(dest: i32) -> i32;
        pub fn transfer_to_dest(to: i32, dest: i32) -> i32;
        pub fn ring_multiplier() -> i32;
        pub fn add_ring_bonus(n: i32) -> i32;
        pub fn teleport_to(player_id: i32, tile: i32);
        pub fn unplace_card() -> i32;
        pub fn self_tile() -> i32;
        pub fn set_self_tile(tile: i32) -> i32;
        pub fn self_face_down() -> i32;
        pub fn set_self_face_down(on: i32) -> i32;
        pub fn self_immune() -> i32;
        pub fn set_self_immune(on: i32) -> i32;
        pub fn is_placed() -> i32;
        pub fn crystals() -> i32;
        pub fn set_crystals(n: i32) -> i32;
        pub fn add_crystals(n: i32, max: i32) -> i32;
        // marks & tokens
        pub fn add_mark(tile: i32, player_id: i32, kp: i32, kl: i32, np: i32, nl: i32);
        pub fn count_marks(tile: i32, kp: i32, kl: i32, owner: i32) -> i32;
        pub fn remove_marks(tile: i32, kp: i32, kl: i32, owner: i32) -> i32;
        pub fn tok(player_id: i32, ptr: i32, len: i32) -> i32;
        pub fn set_tok(player_id: i32, ptr: i32, len: i32, v: i32);
        pub fn add_tok(player_id: i32, ptr: i32, len: i32, by: i32, max: i32) -> i32;
        // per-player slots
        pub fn state_get(player_id: i32, ptr: i32, len: i32, field: i32) -> i32;
        pub fn state_set(player_id: i32, ptr: i32, len: i32, field: i32, v: i32) -> i32;
        pub fn state_add(player_id: i32, ptr: i32, len: i32, delta: i32) -> i32;

        pub fn slot(player_id: i32, ptr: i32, len: i32) -> i32;
        pub fn set_slot(player_id: i32, ptr: i32, len: i32, v: i32);
        pub fn inc_slot(player_id: i32, ptr: i32, len: i32, by: i32) -> i32;
        // pots & status
        pub fn band_crystals(player_id: i32) -> i32;
        pub fn add_band_crystals(player_id: i32, n: i32, max: i32) -> i32;
        pub fn fire(player_id: i32) -> i32;
        pub fn fire_max(player_id: i32) -> i32;
        pub fn gain_fire(player_id: i32, n: i32, ptr: i32, len: i32) -> i32;
        pub fn give_stay(player_id: i32, n: i32);
        pub fn give_stun(player_id: i32, n: i32);
        pub fn give_exile(player_id: i32, n: i32, to: i32);
        pub fn give_extra_turn(player_id: i32);
        pub fn can_pay(player_id: i32) -> i32;
        pub fn cant_move(player_id: i32) -> i32;
        pub fn spend_fire(player_id: i32, n: i32, ptr: i32, len: i32) -> i32;
        pub fn stay_of(player_id: i32) -> i32;
        pub fn stun_of(player_id: i32) -> i32;
        pub fn turn_player() -> i32;
        pub fn round_no() -> i32;
        pub fn turn_key() -> i32;
        pub fn character_is(player_id: i32, ptr: i32, len: i32) -> i32;
        pub fn in_band(player_id: i32, ptr: i32, len: i32) -> i32;
        // prompts & trigger
        pub fn opt_int(v: i32);
        pub fn opt_str(ptr: i32, len: i32);
        pub fn ask(
            kind: i32,
            player_id: i32,
            title_ptr: i32,
            title_len: i32,
            text_ptr: i32,
            text_len: i32,
        ) -> i32;
        pub fn trig_kind() -> i32;
        pub fn trig_player() -> i32;
        pub fn trig_target() -> i32;
        pub fn trig_tile() -> i32;
        pub fn trig_value() -> i32;
        pub fn trig_step() -> i32;
        pub fn trig_by_card() -> i32;
        pub fn trig_pay_is_rent() -> i32;
        pub fn trig_move_kind() -> i32;
        pub fn trig_move_resolve() -> i32;
        pub fn trig_move_tag(kp: i32, kl: i32) -> i32;
        pub fn trig_move_main() -> i32;
        pub fn trig_move_dir() -> i32;
        pub fn trig_move_remaining() -> i32;
        pub fn trig_move_total() -> i32;
        pub fn trig_cards(buf: i32, cap: i32) -> i32;
        pub fn trig_move_roll() -> i32;
        pub fn trig_set_move_roll(v: i32);
        pub fn trig_set_pay_amount(v: i32);
        pub fn trig_set_pay_target(to: i32);
        pub fn trig_set_cancelled();
        pub fn trig_set_negate_effect();
        pub fn trig_set_spare(seat: i32);
        pub fn trig_cancelled() -> i32;
        pub fn trig_seq() -> i32;
        pub fn trig_answers() -> i32;
        pub fn trig_effect_count() -> i32;
        pub fn trig_effect_kind(i: i32) -> i32;
        pub fn trig_effect_target(i: i32) -> i32;
        pub fn trig_effect_from(i: i32) -> i32;
        pub fn trig_effect_tile(i: i32) -> i32;
        pub fn trig_effect_value(i: i32) -> i32;
        pub fn declare_effect(kind: i32, target: i32, from: i32, tile: i32, value: i32);
        pub fn trig_card_is(ptr: i32, len: i32) -> i32;
        pub fn play_card(id_ptr: i32, id_len: i32, player_id: i32) -> i32;
        pub fn card_move(player_id: i32) -> i32;
        pub fn agent_landing(player_id: i32, agent: i32) -> i32;
        pub fn card_replayable(player_id: i32, ptr: i32, len: i32) -> i32;
        // movement shaping: the move being planned (C# `TurnCtx.Plan`)
        pub fn set_steps(n: i32);
        pub fn set_reverse(on: i32);
        pub fn set_signed(on: i32);
        pub fn set_stop_at(n: i32);
        pub fn set_parity(n: i32);
        pub fn set_resolve(on: i32);
        pub fn set_settle_tile(n: i32);
        pub fn set_pay_factor(n: i32);
        pub fn set_rent_factor(n: i32);
        pub fn set_no_buy(on: i32);
        pub fn set_no_build(on: i32);
        pub fn set_build_anywhere(on: i32);
        pub fn set_kind(kind: i32);
        pub fn set_teleport_to(tile: i32);
        pub fn set_start(tile: i32, ptr: i32, len: i32);
        pub fn set_base_dice(count: i32, sides: i32, ptr: i32, len: i32);
        pub fn add_base_dice(count: i32, sides: i32, ptr: i32, len: i32);
        pub fn add_extra_dice(count: i32, sides: i32, ptr: i32, len: i32);
        pub fn move_stopped() -> i32;
        pub fn set_min_roll(n: i32);
        pub fn set_extra_steps(n: i32);
        pub fn set_more_steps(n: i32);
        pub fn set_tag(kp: i32, kl: i32, v: i32);
        pub fn set_no_circle_reward(on: i32);
        pub fn set_settle_as_agent(on: i32);
        pub fn set_bonus(n: i32, ptr: i32, len: i32);
        pub fn move_stop_at() -> i32;
        pub fn move_parity() -> i32;
        pub fn move_resolve() -> i32;
        pub fn move_steps() -> i32;
        pub fn move_remaining() -> i32;
        pub fn move_total() -> i32;
        pub fn move_dir() -> i32;
        pub fn abnormal_count(player_id: i32) -> i32;
        pub fn target(player_id: i32, tile: i32, single: i32) -> i32;
        pub fn targeted_count(player_id: i32) -> i32;
        pub fn placed_tile(player_id: i32, ptr: i32, len: i32) -> i32;
        pub fn play_doubled() -> i32;
        pub fn set_play_doubled(n: i32);
        // turn plan & scheduling
        pub fn schedule_turn_end(player_id: i32, mode: i32);
        pub fn set_no_money_loss(player_id: i32);
        pub fn set_fixed_roll(n: i32);
        pub fn fixed_roll() -> i32;
        pub fn set_next_steps(player_id: i32, n: i32);
        pub fn turn_main_steps() -> i32;
        pub fn add_fire_max(player_id: i32, n: i32) -> i32;
        pub fn card_crystals(player_id: i32, cp: i32, cl: i32) -> i32;
        pub fn card_build(player_id: i32, tile: i32) -> i32;
        pub fn set_can_build(on: i32);
        pub fn clear_dice();
        pub fn can_build_on(player_id: i32, tile: i32) -> i32;
        pub fn card_buy(player_id: i32, tile: i32) -> i32;
        pub fn card_immune(player_id: i32, cp: i32, cl: i32) -> i32;
        pub fn card_mortgage(player_id: i32, tile: i32) -> i32;
        pub fn card_text_mentions(cp: i32, cl: i32, np: i32, nl: i32) -> i32;
        pub fn gain_fixed(player_id: i32, amount: i32, ptr: i32, len: i32) -> i32;
        pub fn is_agent(tile: i32) -> i32;
        pub fn paid_in_settle() -> i32;
        pub fn place_card_on(
            player_id: i32,
            tile: i32,
            cp: i32,
            cl: i32,
            ptr: i32,
            len: i32,
        ) -> i32;
        pub fn play_from_hand() -> i32;
        pub fn set_build_discount(n: i32, layers: i32);
        pub fn set_buy_discount(n: i32);
        pub fn set_card_immune(player_id: i32, cp: i32, cl: i32, on: i32) -> i32;
        pub fn set_card_tile(player_id: i32, cp: i32, cl: i32, tile: i32) -> i32;
        pub fn set_extreme(v: i32);
        pub fn set_free_buy(on: i32);
        pub fn set_raze_on_buy(on: i32);
        pub fn set_tile_color(tile: i32, group: i32);
        pub fn turn_rolls(buf: i32, cap: i32) -> i32;
        pub fn turn_snap(player_id: i32, buf: i32) -> i32;
        pub fn turn_start_pos(player_id: i32) -> i32;
        pub fn is_color(player_id: i32, tile: i32, group: i32) -> i32;
        pub fn unplace_card_named(player_id: i32, cp: i32, cl: i32) -> i32;
        pub fn bump_mark(tile: i32, kp: i32, kl: i32, owner: i32, delta: i32) -> i32;
        pub fn tok_names(player_id: i32, p: i32, n: i32, buf: i32, cap: i32) -> i32;
        pub fn gate(player_id: i32, kind: i32) -> i32;
        pub fn card_settle_at(player_id: i32, tile: i32, main: i32) -> i32;
        pub fn card_offer_build(player_id: i32, buf: i32, n: i32) -> i32;
        pub fn do_move_roll(player_id: i32) -> i32;
        pub fn is_live_house_for(player_id: i32, tile: i32) -> i32;
        pub fn placed_cards(player_id: i32, buf: i32, cap: i32) -> i32;
        pub fn field_instances(player_id: i32, buf: i32, cap: i32) -> i32;
        pub fn crystals_at(uid: i32) -> i32;
        pub fn add_crystals_at(uid: i32, n: i32, max: i32) -> i32;
        pub fn unplace_at(uid: i32) -> i32;
        pub fn tile_at(uid: i32) -> i32;
        pub fn set_tile_at(uid: i32, tile: i32) -> i32;
        pub fn is_face_down_at(uid: i32) -> i32;
        pub fn set_face_down_at(uid: i32, on: i32) -> i32;
        pub fn is_immune_at(uid: i32) -> i32;
        pub fn set_immune_at(uid: i32, on: i32) -> i32;
        pub fn set_build_cost_pct(pct: i32);
        pub fn set_card_face_down(player_id: i32, cp: i32, cl: i32, down: i32) -> i32;
        pub fn set_extra_color(player_id: i32, tile: i32, group: i32);
        pub fn add_card_crystals(player_id: i32, cp: i32, cl: i32, n: i32, max: i32) -> i32;
        pub fn card_face_down(player_id: i32, cp: i32, cl: i32) -> i32;
    }
}

#[cfg(target_arch = "wasm32")]
fn s(v: &str) -> (i32, i32) {
    (v.as_ptr() as i32, v.len() as i32)
}

#[cfg(not(target_arch = "wasm32"))]
fn s(v: &str) -> (i32, i32) {
    crate::native::intern(v.as_bytes())
}

/// The i32 wire form of a message: `postcard` bytes leaked for the host to read
/// (same lifetime convention as every other buffer crossing the ABI).
#[cfg(target_arch = "wasm32")]
fn mj(m: &Msg) -> (i32, i32) {
    let b = m.to_bytes();
    let p = b.as_ptr() as i32;
    let l = b.len() as i32;
    core::mem::forget(b);
    (p, l)
}

#[cfg(not(target_arch = "wasm32"))]
fn mj(m: &Msg) -> (i32, i32) {
    crate::native::intern(&m.to_bytes())
}

// ------------------------------------------------------------- dice & log

/// `H.Roll(seat, count, sides, what)` -- sum of `count` d`sides`, logged as a dice event.
pub fn roll(player_id: i32, count: i32, sides: i32) -> i32 {
    unsafe { sys::roll(player_id, count, sides) }
}

/// `H.Log("text", seat, text)`.
pub fn log(player_id: i32, msg: &Msg) {
    let (p, l) = mj(msg);
    unsafe { sys::log(player_id, p, l) }
}

/// `H.Effect` -- announce which effect was applied. Reaches the player as a
/// popup as well as a log line; use this where the card has just picked a
/// branch (e.g. a 1d10 that names the branch taken) and the player should see
/// which. Additive with [`log`], not a replacement for it.
pub fn effect(player_id: i32, msg: &Msg) {
    let (p, l) = mj(msg);
    unsafe { sys::effect(player_id, p, l) }
}

// ------------------------------------------------------------------ board

pub fn tile_count() -> i32 {
    unsafe { sys::tile_count() }
}

/// Tile index of a name (a data key), or -1 (`H.TileNamed`).
pub fn tile_named(name: &str) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::tile_named(p, l) }
}

pub fn tile_owner(tile: i32) -> i32 {
    unsafe { sys::tile_owner(tile) }
}

/// The player's tile on the ring (`H.State.seats[seat].pos`).
pub fn player_pos(player_id: i32) -> i32 {
    unsafe { sys::player_pos(player_id) }
}

/// Tile `steps` ahead of a player around the ring.
pub fn tile_steps_ahead(player_id: i32, steps: i32) -> i32 {
    unsafe { sys::tile_steps_ahead(player_id, steps) }
}

pub fn rent_of(tile: i32) -> i32 {
    unsafe { sys::rent_of(tile) }
}

pub fn buy_price(tile: i32) -> i32 {
    unsafe { sys::buy_price(tile) }
}

pub fn build_cost(tile: i32) -> i32 {
    unsafe { sys::build_cost(tile) }
}

pub fn mortgage_value(tile: i32) -> i32 {
    unsafe { sys::mortgage_value(tile) }
}

pub fn owned_count(player_id: i32) -> i32 {
    unsafe { sys::owned_count(player_id) }
}

pub fn owned_at(player_id: i32, index: i32) -> i32 {
    unsafe { sys::owned_at(player_id, index) }
}

/// The player's deeds as tile indices (`H.OwnedBy`-style walks).
pub fn owned_tiles(player_id: i32) -> Vec<i32> {
    (0..owned_count(player_id))
        .map(|i| owned_at(player_id, i))
        .collect()
}

/// `TileData.IsBuyable` -- a deed tile (property / RiNG).
pub fn is_buyable(tile: i32) -> bool {
    unsafe { sys::is_buyable(tile) != 0 }
}

/// C# `H.IsShop` -- a 商店街 deed (buyable and colour group 10).
pub fn is_shop(tile: i32) -> bool {
    unsafe { sys::is_shop(tile) != 0 }
}

/// `TileData.kind == "ring"` -- a RiNG deed (never holds houses).
pub fn is_ring(tile: i32) -> bool {
    unsafe { sys::is_ring(tile) != 0 }
}

/// `TileData.kind == "circle"` -- the CiRCLE tile.
pub fn is_circle(tile: i32) -> bool {
    unsafe { sys::is_circle(tile) != 0 }
}

/// C# `H.IsLiveHouse` -- a Live House deed (buyable, colour group 6). The
/// `ExtraColor` band-skill colours are not visible here (TODO in the cards).
pub fn is_live_house(tile: i32) -> bool {
    unsafe { sys::is_live_house(tile) != 0 }
}

/// `TileData.group` -- the colour group (-1 for no tile).
pub fn tile_group(tile: i32) -> i32 {
    unsafe { sys::tile_group(tile) }
}

/// `H._tiles[t].price` -- the land price alone (houses are extra; cf. [`buy_price`]).
pub fn tile_price(tile: i32) -> i32 {
    unsafe { sys::tile_price(tile) }
}

/// `H.State.houses[t]` -- houses standing on the tile.
pub fn houses_of(tile: i32) -> i32 {
    unsafe { sys::houses_of(tile) }
}

/// Set the tile's house count directly (house transfer effects).
pub fn set_houses(tile: i32, n: i32) {
    unsafe { sys::set_houses(tile, n) }
}

/// `H.AddHouse` -- move the house count by `n`; returns the new count.
pub fn add_house(tile: i32, n: i32) -> i32 {
    unsafe { sys::add_house(tile, n) }
}

/// Is the deed mortgaged? (C# `H.State.mortgaged[t]`.)
pub fn mortgaged_of(tile: i32) -> bool {
    unsafe { sys::mortgaged_of(tile) != 0 }
}

/// Mortgage or redeem a deed outright (transfer effects; no money moves).
pub fn set_mortgaged(tile: i32, v: bool) {
    unsafe { sys::set_mortgaged(tile, v as i32) }
}

/// Hand a deed to another player outright (C# `H.State.owners[t] = seat`; deed
/// transfer effects). The caller logs the transfer.
pub fn set_owner(tile: i32, player_id: i32) {
    unsafe { sys::set_owner(tile, player_id) }
}

/// C# `H.Dist` -- undirected ring distance between two tiles.
pub fn dist(a: i32, b: i32) -> i32 {
    unsafe { sys::dist(a, b) }
}

/// C# `H.Forward` -- steps forward from `a` to `b` around the ring.
pub fn tile_forward(a: i32, b: i32) -> i32 {
    unsafe { sys::tile_forward(a, b) }
}

/// C# `H.Neighbor(seat, dir)` -- the next present player in turn order (`dir` ±1), or -1.
pub fn neighbor(player_id: i32, dir: i32) -> i32 {
    unsafe { sys::neighbor(player_id, dir) }
}

/// C# `H.SeatsOn(tile, except)` -- present players standing on a tile.
pub fn players_on(tile: i32, except: i32) -> Vec<i32> {
    (0..players_on_count(tile, except))
        .map(|i| players_on_at(tile, except, i))
        .collect()
}

fn players_on_count(tile: i32, except: i32) -> i32 {
    unsafe { sys::players_on_count(tile, except) }
}

fn players_on_at(tile: i32, except: i32, index: i32) -> i32 {
    unsafe { sys::players_on_at(tile, except, index) }
}

// ------------------------------------------------------------------ players

pub fn player_count() -> i32 {
    unsafe { sys::player_count() }
}

/// 1 when the player is out of the game (`H.Out`).
pub fn player_out(player_id: i32) -> bool {
    unsafe { sys::player_out(player_id) != 0 }
}

pub fn others_count(player_id: i32) -> i32 {
    unsafe { sys::others_count(player_id) }
}

pub fn others_at(player_id: i32, index: i32) -> i32 {
    unsafe { sys::others_at(player_id, index) }
}

/// The other players still in the game (`H.Others`).
pub fn others(player_id: i32) -> Vec<i32> {
    (0..others_count(player_id))
        .map(|i| others_at(player_id, i))
        .collect()
}

/// The player's current cash (C# `H.Money`).
pub fn money_of(player_id: i32) -> i32 {
    unsafe { sys::money(player_id) }
}

/// `H.GainR` -- money in, logged with its reason (`src` is a message key).
pub fn gain(player_id: i32, amount: i32, src: &Msg) -> i32 {
    let (p, l) = mj(src);
    unsafe { sys::gain(player_id, amount, p, l) }
}

/// `H.PayR` -- money out (what the player could pay), logged.
pub fn pay(player_id: i32, amount: i32, src: &Msg) -> Result<i32, Prompt> {
    let (p, l) = mj(src);
    asked(unsafe { sys::pay(player_id, amount, p, l) })
}

/// Player-to-player money: the receiver gets exactly what the payer could pay.
/// The usual shape of `H.PayR` + `H.GainR` in a transfer.
pub fn transfer(from: i32, to: i32, amount: i32, src: &Msg) -> Result<i32, Prompt> {
    let got = pay(from, amount, src)?;
    gain(to, got, src);
    Ok(got)
}

// ------------------------------------------------------------- hand / deck

/// `H.DrawR` -- draw `n` cards; returns how many were drawn.
pub fn draw(player_id: i32, n: i32) -> i32 {
    unsafe { sys::draw(player_id, n) }
}

pub fn add_to_hand(player_id: i32, card: &str) {
    let (p, l) = s(card);
    unsafe { sys::add_to_hand(player_id, p, l) }
}

pub fn add_to_deck(player_id: i32, card: &str, shuffle: bool) {
    let (p, l) = s(card);
    unsafe { sys::add_to_deck(player_id, p, l, shuffle as i32) }
}

/// Where `add_to_deck_at` inserts into the draw pile (C# `H.AddToDeck`'s
/// `where` argument -- note the C# default is [`DeckPos::Random`]).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DeckPos {
    /// `"top"` -- the next `draw` gets it.
    Top = 0,
    /// `"bottom"` -- the pile is emptied first.
    Bottom = 1,
    /// `"shuffle"` -- mixed into the pile.
    Random = 2,
}

/// C# `H.AddToDeck(seat, card, where)` -- put one specific card into the draw
/// pile at a position. (`add_to_deck(player_id, card, shuffle)` is the older
/// top/shuffle shape and stays.)
pub fn add_to_deck_at(player_id: i32, card: &str, pos: DeckPos) {
    let (p, l) = s(card);
    unsafe { sys::add_to_deck_at(player_id, p, l, pos as i32) }
}

/// Take one copy of `card` out of `pile` *without* sending it anywhere (C#
/// `_hidden[s].hand.Remove(card)` / `.discard.Remove(card)`) -- the caller
/// decides where it goes next. Returns whether it was there.
pub fn take_card(player_id: i32, pile: CardPile, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::take_card(player_id, pile as i32, p, l) != 0 }
}

/// [`take_card`] from the hand -- for moving a card straight to the field or
/// the deck without discarding it.
pub fn take_from_hand(player_id: i32, card: &str) -> bool {
    take_card(player_id, CardPile::Hand, card)
}

/// Every card id in a player's `pile`, in pile order (the deck's top card
/// first). Duplicates are listed once per copy.
pub fn cards_in(player_id: i32, pile: CardPile) -> Vec<String> {
    let need = unsafe { sys::cards_in(player_id, pile as i32, 0, 0) };
    if need <= 0 {
        return Vec::new();
    }
    let mut buf: Vec<u8> = Vec::new();
    buf.resize(need as usize, 0);
    let got = unsafe { sys::cards_in(player_id, pile as i32, buf.as_mut_ptr() as i32, need) };
    if got != need {
        return Vec::new();
    }
    postcard::from_bytes(&buf).unwrap_or_default()
}

pub fn to_discard(player_id: i32, card: &str) {
    let (p, l) = s(card);
    unsafe { sys::to_discard(player_id, p, l) }
}

/// Copies of `card` in the player's hand (C# `_hidden[s].hand` count).
pub fn hand_count(player_id: i32, card: &str) -> i32 {
    let (p, l) = s(card);
    unsafe { sys::hand_count(player_id, p, l) }
}

/// Total cards in hand (C# `_hidden[s].hand.Count`).
pub fn hand_size(player_id: i32) -> i32 {
    unsafe { sys::hand_size(player_id) }
}

/// Copies of `card` in the player's discard pile.
pub fn discard_count(player_id: i32, card: &str) -> i32 {
    let (p, l) = s(card);
    unsafe { sys::discard_count(player_id, p, l) }
}

/// Draw-pile size (C# `_hidden[s].draw.Count`).
pub fn deck_count(player_id: i32) -> i32 {
    unsafe { sys::deck_count(player_id) }
}

/// Discard-pile size (all ids).
pub fn discard_size(player_id: i32) -> i32 {
    unsafe { sys::discard_size(player_id) }
}

/// C# `H.DiscardFromHand` -- drop one copy of `card` from hand into the discard
/// pile. True when the player held one.
pub fn discard_from_hand(player_id: i32, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::discard_from_hand(player_id, p, l) != 0 }
}

/// C# `H.ShuffleAllIntoDeck(seat, hand: true, discard: true)` -- sweep hand +
/// discard into the draw pile and shuffle. Returns how many cards moved.
pub fn sweep_to_deck(player_id: i32) -> i32 {
    shuffle_into_deck(player_id, true, true)
}

/// C# `H.ShuffleAllIntoDeck(seat, hand, discard)` -- move the hand and/or the
/// discard pile into the draw pile, then shuffle it. Returns how many moved.
pub fn shuffle_into_deck(player_id: i32, hand: bool, discard: bool) -> i32 {
    unsafe { sys::shuffle_into_deck(player_id, hand as i32, discard as i32) }
}

// ------------------------------------------------------------ field cards

/// `H.PlaceFromPlay` -- this card stays in play at the player.
pub fn place_card(player_id: i32, card: &str, note: &Msg) -> i32 {
    place_card_at(player_id, card, note)
}

/// `H.PlaceCard` -- put a specific card (a derived one) into play at a player.
/// Returns the new instance's uid, which is what addresses it afterwards: the
/// *name* is not an identity, so a rule meaning "the copy I just placed" has to
/// hold this rather than re-resolving by name.
pub fn place_card_at(player_id: i32, card: &str, note: &Msg) -> i32 {
    let (cp, cl) = s(card);
    let (p, l) = mj(note);
    unsafe { sys::place_card_at(player_id, cp, cl, p, l) }
}

/// `PlayCtx.Dest` -- where this card goes afterwards (see [`Dest`]). For a
/// play it is the hand card's fate; for a field effect, the instance's.
pub fn set_dest(dest: Dest) {
    unsafe { sys::set_dest(dest as i32) }
}

/// [`set_dest`] aimed at another player's pile -- 「将此卡放入[使用者]弃卡区」
/// when [使用者] is not the one holding the card. `to` is whose discard / hand
/// / deck it lands in.
pub fn set_transfer_to_dest(to: i32, dest: Dest) {
    unsafe { sys::set_transfer_to_dest(to, dest as i32) }
}

/// [`set_dest`] applied **now** rather than when this effect finishes: for a
/// card that must be gone before the rest of the effect runs (a move or a
/// settle follows). Lands in its own owner's pile. Returns the owner it left,
/// or `None` when it was not in play.
pub fn send_to_dest(dest: Dest) -> Option<i32> {
    let v = unsafe { sys::send_to_dest(dest as i32) };
    (v >= 0).then_some(v)
}

/// [`send_to_dest`] aimed at another player's pile: `to` is whose.
pub fn transfer_to_dest(to: i32, dest: Dest) -> Option<i32> {
    let v = unsafe { sys::transfer_to_dest(to, dest as i32) };
    (v >= 0).then_some(v)
}

/// `H.RingMultiplier`.
pub fn ring_multiplier() -> i32 {
    unsafe { sys::ring_multiplier() }
}

/// `H._ringBonus += n`; returns the new multiplier.
pub fn add_ring_bonus(n: i32) -> i32 {
    unsafe { sys::add_ring_bonus(n) }
}

/// `H.ForceTeleport(..., resolve: false)` -- move a player without settling.
pub fn teleport_to(player_id: i32, tile: i32) {
    unsafe { sys::teleport_to(player_id, tile) }
}

/// `H.Unplace` -- take this card out of play. True when it was there.
/// Take **this instance** off the field; returns the owner it left (or -1).
/// [`unplace_card_named`] is the form for some *other* card.
pub fn unplace_self() -> i32 {
    unsafe { sys::unplace_card() }
}

/// Is this card in play at the player?
pub fn is_placed() -> bool {
    unsafe { sys::is_placed() != 0 }
}

/// Miracle crystals on **this card instance** (C# `Card.Crystals`). The
/// instance is the one running -- its placement is not a parameter, because the
/// host knows it from the dispatch and the same card id can sit on several
/// players' fields at once. [`card_crystals`] is the form for a *named* other
/// card, which does need to say whose field to look on.
pub fn crystals() -> i32 {
    unsafe { sys::crystals() }
}

/// Set this card instance's crystals (C# `Card.Crystals = n`); returns the new
/// count.
pub fn set_crystals(n: i32) -> i32 {
    unsafe { sys::set_crystals(n) }
}

/// `H.AddCrystals` -- adjust this card instance's crystals by `n`, clamped at 0
/// and at `max` (`0` = uncapped); returns the new count.
pub fn add_crystals(n: i32, max: i32) -> i32 {
    unsafe { sys::add_crystals(n, max) }
}

// --------------------------------------------------------- marks & tokens

/// `H.AddMark` -- a marker on a tile. `kind` names it (an i18n key), `note` explains it.
pub fn add_mark(tile: i32, player_id: i32, kind: &str, note: &Msg) {
    let (kp, kl) = s(kind);
    let (np, nl) = mj(note);
    unsafe { sys::add_mark(tile, player_id, kp, kl, np, nl) }
}

/// `H.CountMarks` -- marks on a tile (`kind` "" = any; `owner` -2 = any).
pub fn count_marks(tile: i32, kind: &str, owner: i32) -> i32 {
    let (kp, kl) = s(kind);
    unsafe { sys::count_marks(tile, kp, kl, owner) }
}

pub fn remove_marks(tile: i32, kind: &str, owner: i32) -> i32 {
    let (kp, kl) = s(kind);
    unsafe { sys::remove_marks(tile, kp, kl, owner) }
}

pub fn tok(player_id: i32, name: &str) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::tok(player_id, p, l) }
}

pub fn set_tok(player_id: i32, name: &str, value: i32) {
    let (p, l) = s(name);
    unsafe { sys::set_tok(player_id, p, l, value) }
}

/// `H.AddTok` -- returns how much the counter actually moved by.
pub fn add_tok(player_id: i32, name: &str, by: i32, max: i32) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::add_tok(player_id, p, l, by, max) }
}

// ---------------------------------------------------------- keyed state

/// The keyed state surface: `{value, min, max, expires}` items the engine holds
/// and enforces **nothing**.
///
/// Which keys mean what, and what their caps are, belongs to whoever uses them:
/// a character skill that mandates a fire-pot cap writes [`Self::max`] on
/// [`state_key::FIRE`], and the card that gains pots is free to honour it (the
/// `fire` / `gain_fire` helpers do) or ignore it. There is no engine-side
/// `match key`, and a raw [`Self::set`] / [`Self::add`] is never clamped.
pub mod state {
    use super::{s, sys};

    /// Which column of a state item.
    pub const VALUE: i32 = 0;
    pub const MIN: i32 = 1;
    /// The cap a consumer may enforce. 0 when none has been mandated.
    pub const MAX: i32 = 2;
    /// When it wears off: [`NEVER`] / [`TURN_START`] / [`TURN_END`].
    pub const EXPIRES: i32 = 3;

    /// Never wears off.
    pub const NEVER: i32 = 0;
    /// Loses a layer at its owner's start of turn.
    pub const TURN_START: i32 = 1;
    /// Loses a layer at its owner's end of turn.
    pub const TURN_END: i32 = 2;

    fn get_field(player_id: i32, key: &str, field: i32) -> i32 {
        let (p, l) = s(key);
        unsafe { sys::state_get(player_id, p, l, field) }
    }

    fn set_field(player_id: i32, key: &str, field: i32, value: i32) -> i32 {
        let (p, l) = s(key);
        unsafe { sys::state_set(player_id, p, l, field, value) }
    }

    /// The value.
    pub fn get(player_id: i32, key: &str) -> i32 {
        get_field(player_id, key, VALUE)
    }

    pub fn min(player_id: i32, key: &str) -> i32 {
        get_field(player_id, key, MIN)
    }

    /// The mandated cap -- `0` when no skill has declared one.
    pub fn max(player_id: i32, key: &str) -> i32 {
        get_field(player_id, key, MAX)
    }

    /// [`NEVER`] / [`TURN_START`] / [`TURN_END`].
    pub fn expires(player_id: i32, key: &str) -> i32 {
        get_field(player_id, key, EXPIRES)
    }

    /// Write the value raw. Not clamped to `min`/`max`.
    pub fn set(player_id: i32, key: &str, value: i32) -> i32 {
        set_field(player_id, key, VALUE, value)
    }

    /// Move the value by `delta` raw. No clamping, no logging.
    pub fn add(player_id: i32, key: &str, delta: i32) -> i32 {
        let (p, l) = s(key);
        unsafe { sys::state_add(player_id, p, l, delta) }
    }

    /// Declare the bounds a consumer may enforce. Stored, not applied.
    pub fn set_bounds(player_id: i32, key: &str, min: i32, max: i32) {
        set_field(player_id, key, MIN, min);
        set_field(player_id, key, MAX, max);
    }

    /// Raise (or lower) the cap. Does not touch the value -- the caller is the
    /// one that cares whether it should.
    pub fn set_max(player_id: i32, key: &str, max: i32) -> i32 {
        set_field(player_id, key, MAX, max)
    }

    /// Declare when this counter wears off.
    pub fn set_expires(player_id: i32, key: &str, expires: i32) -> i32 {
        set_field(player_id, key, EXPIRES, expires)
    }
}

// ------------------------------------------------------ per-player slots (V)

/// A free-form counter (C# `H.V`). Sugar over [`state`].
pub fn slot(player_id: i32, key: &str) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::slot(player_id, p, l) }
}

pub fn set_slot(player_id: i32, key: &str, value: i32) {
    let (p, l) = s(key);
    unsafe { sys::set_slot(player_id, p, l, value) }
}

pub fn inc_slot(player_id: i32, key: &str, by: i32) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::inc_slot(player_id, p, l, by) }
}

// ----------------------------------------------------------- pots & status

pub fn band_crystals(player_id: i32) -> i32 {
    unsafe { sys::band_crystals(player_id) }
}

pub fn add_band_crystals(player_id: i32, n: i32, max: i32) -> i32 {
    unsafe { sys::add_band_crystals(player_id, n, max) }
}

pub fn fire(player_id: i32) -> i32 {
    unsafe { sys::fire(player_id) }
}

pub fn fire_max(player_id: i32) -> i32 {
    unsafe { sys::fire_max(player_id) }
}

/// `H.GainFire` -- fire pots, capped by the player's own cap.
pub fn gain_fire(player_id: i32, n: i32, why: &Msg) -> i32 {
    let (p, l) = mj(why);
    unsafe { sys::gain_fire(player_id, n, p, l) }
}

pub fn give_stay(player_id: i32, n: i32) {
    unsafe { sys::give_stay(player_id, n) }
}

pub fn give_stun(player_id: i32, n: i32) {
    unsafe { sys::give_stun(player_id, n) }
}

/// `H.GiveExile(seat, layers, back_to)`.
pub fn give_exile(player_id: i32, n: i32, to: i32) {
    unsafe { sys::give_exile(player_id, n, to) }
}

/// `H.GiveExtraTurn`.
pub fn give_extra_turn(player_id: i32) {
    unsafe { sys::give_extra_turn(player_id) }
}

/// `H.CanPay` -- not out, not stunned, not exiled.
pub fn can_pay(player_id: i32) -> bool {
    unsafe { sys::can_pay(player_id) != 0 }
}

/// C# `H.MoveWhyNot` -- `None` when the player may still make this turn's main
/// move, else the shared refusal (`status.move_*` messages).
pub fn cant_move(player_id: i32) -> Option<Msg> {
    match unsafe { sys::cant_move(player_id) } {
        0 => None,
        1 => Some(Msg::new("status.move_not_your_turn")),
        2 => Some(Msg::new("status.move_already")),
        _ => Some(Msg::new("status.move_blocked")),
    }
}

/// C# `H.CardMove(c, m)` -- run the move being planned **now**. Shape it with
/// [`plan`] first (`set_steps` / `set_kind` / `set_tag` / ...), then call this:
/// the engine runs the move (it may prompt, and the usual move triggers fire)
/// and the effect continues after it. Like `play_card`, the run pauses for it.
pub fn card_move(player_id: i32) -> bool {
    unsafe { sys::card_move(player_id) != 0 }
}

/// C# `H.AgentLanding` -- `player_id` lands on `agent` as a 「星光代理」: the
/// buy-or-pay-rent routine runs engine-side.
pub fn agent_landing(player_id: i32, agent: i32) -> bool {
    unsafe { sys::agent_landing(player_id, agent) != 0 }
}

/// C# `H.SpendFire` -- spend `n` [火罐]; false when the player has fewer. Logs
/// the spend (`why` is the reason message).
pub fn spend_fire(player_id: i32, n: i32, why: &Msg) -> bool {
    let (p, l) = mj(why);
    unsafe { sys::spend_fire(player_id, n, p, l) != 0 }
}

/// `[停留]` layers on the player (C# `H.State.seats[s].stay`).
pub fn stay_of(player_id: i32) -> i32 {
    unsafe { sys::stay_of(player_id) }
}

/// `[晕眩]` layers on the player (C# `H.State.seats[s].stun`).
pub fn stun_of(player_id: i32) -> i32 {
    unsafe { sys::stun_of(player_id) }
}

/// `H.State.turn` -- whose turn it is (-1 when none).
pub fn turn_player() -> i32 {
    unsafe { sys::turn_player() }
}

/// `H.State.round` -- the round counter.
pub fn round_no() -> i32 {
    unsafe { sys::round_no() }
}

/// C# `H.TurnKey` -- `round * 100 + turn + 1`: a per-turn id for once-per-turn
/// latches (`set_slot(player_id, key, turn_key())` then compare).
pub fn turn_key() -> i32 {
    unsafe { sys::turn_key() }
}

/// Is the player's character exactly `name`? (C# `H.CharacterOf`-style checks.)
pub fn character_is(player_id: i32, name: &str) -> bool {
    let (p, l) = s(name);
    unsafe { sys::character_is(player_id, p, l) != 0 }
}

/// C# `H.BandOf(seat) == name` -- is the player's character in this band?
pub fn in_band(player_id: i32, name: &str) -> bool {
    let (p, l) = s(name);
    unsafe { sys::in_band(player_id, p, l) != 0 }
}

/// `PlayCtx.Dest` -- where a card goes when its effect finishes (C# fates
/// "discard" / "hand" / "placed" / "gone"). The port names them Graveyard /
/// Hand / Field / Banished; wire values 0..=3 are the C# order.
///
/// One fate, two movers. A **play** names where its hand card ends and the
/// engine applies it (`play_from_hand`); a **field effect** names where the
/// instance it is running for ends and the host applies it when the run
/// commits (C# `H.Unplace(this, "discard")` and kin). 「将此卡放入[使用者]
/// 弃卡区」 is `Dest::Graveyard` either way -- no separate unplace-plus-discard
/// dance. A run that names nothing leaves a placed card where it is.
///
/// Planned: the draw-pile fates, one per insert position -- C# `c.Dest =
/// "deck"` -> `H.AddToDeck(player, card, where)` with `where` = `"top"` /
/// `"bottom"` / `"shuffle"` (note the C# default is **shuffle**, not top):
/// `DeckTop` = 4, `DeckBottom` = 5, `DeckRandom` = 6. When they land they need
/// arms in `game-rules`'s `dest_from` and `game-core`'s play-from-hand dest
/// handling. (`ctx::add_to_deck`'s `shuffle` flag already covers top vs random;
/// bottom-insert is still missing there.)
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dest {
    /// The discard pile (弃卡区) -- the default.
    Graveyard = 0,
    /// Back to the hand (手牌).
    Hand = 1,
    /// Stays in play (场上) -- a persistent effect.
    Field = 2,
    /// 「[移除]」 -- out of the game entirely.
    Banished = 3,
    /// Onto the top of the draw pile (C# `H.AddToDeck(seat, card, "top")`).
    DeckTop = 4,
    /// Under the draw pile (C# `"bottom"`).
    DeckBottom = 5,
    /// Shuffled into the draw pile (C# `"shuffle"`, the `AddToDeck` default).
    DeckRandom = 6,
}

impl Dest {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Hand,
            2 => Self::Field,
            3 => Self::Banished,
            4 => Self::DeckTop,
            5 => Self::DeckBottom,
            6 => Self::DeckRandom,
            _ => Self::Graveyard,
        }
    }
}

// ------------------------------------------------------------- prompts

/// The host answers `EXIT_NEED_INPUT` when it has published a question and wants
/// the run to stop so it can be replayed with the answer. Every prompt-shaped
/// call funnels through here so the sentinel becomes `Err(Prompt)` once and the
/// card body just `?`s it.
fn asked(i: i32) -> Result<i32, Prompt> {
    if i == crate::abi::EXIT_NEED_INPUT {
        Err(Prompt)
    } else {
        Ok(i)
    }
}

fn ask_raw(kind: PromptKind, player_id: i32, title: &Msg, text: &Msg) -> Result<i32, Prompt> {
    let (tp, tl) = mj(title);
    let (xp, xl) = mj(text);
    asked(unsafe { sys::ask(kind as i32, player_id, tp, tl, xp, xl) })
}

/// `H.AskTileOf` -- returns the chosen **tile**, not the index.
/// Falls back to the first option if the answer is out of range.
pub fn ask_tile(player_id: i32, title: &Msg, text: &Msg, tiles: &[i32]) -> Result<i32, Prompt> {
    for &t in tiles {
        unsafe { sys::opt_int(t) }
    }
    let i = ask_raw(PromptKind::Tile, player_id, title, text)?;
    Ok(tiles[(i.max(0) as usize).min(tiles.len().saturating_sub(1))])
}

/// `H.AskPick` -- returns the chosen option index.
pub fn ask_pick(player_id: i32, title: &Msg, text: &Msg, options: &[Msg]) -> Result<usize, Prompt> {
    for o in options {
        let (p, l) = mj(o);
        unsafe { sys::opt_str(p, l) }
    }
    let i = ask_raw(PromptKind::Choice, player_id, title, text)?;
    Ok((i.max(0) as usize).min(options.len().saturating_sub(1)))
}

/// `H.AskYes`.
pub fn ask_yes(player_id: i32, title: &Msg, text: &Msg) -> Result<bool, Prompt> {
    Ok(ask_raw(PromptKind::YesNo, player_id, title, text)? == 0)
}

/// `H.AskSeat` -- returns the chosen player.
pub fn ask_player(player_id: i32, title: &Msg, text: &Msg, players: &[i32]) -> Result<i32, Prompt> {
    for &x in players {
        unsafe { sys::opt_int(x) }
    }
    let i = ask_raw(PromptKind::Player, player_id, title, text)?;
    Ok(players[(i.max(0) as usize).min(players.len().saturating_sub(1))])
}

/// `H.AskCard` -- pick one of `cards` (ids); returns the index.
pub fn ask_card(player_id: i32, title: &Msg, text: &Msg, cards: &[&str]) -> Result<usize, Prompt> {
    for c in cards {
        let (p, l) = s(c);
        unsafe { sys::opt_str(p, l) }
    }
    let i = ask_raw(PromptKind::Card, player_id, title, text)?;
    Ok((i.max(0) as usize).min(cards.len().saturating_sub(1)))
}

/// `H.AskNumber` -- a number in `min..=max`. The C# builds this as an `AskPick`
/// over the range, so the option list is the faithful shape (ranges in the card
/// pool are small; for a wide range, narrow the candidates yourself first).
pub fn ask_number(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    min: i32,
    max: i32,
) -> Result<i32, Prompt> {
    let min = min.min(max);
    let max = max.max(min);
    let mut options: Vec<Msg> = Vec::new();
    for n in min..=max {
        options.push(Msg::new("ask.intOption").i("n", n as i64));
    }
    if options.is_empty() {
        return Ok(min);
    }
    let i = ask_pick(player_id, title, text, &options)?;
    Ok(min + i as i32)
}

/// Run another card's `play` effect right now (C# `NewCard` + `Play`).
/// `H.PlayCard` -- run another card's `play` inside this run, as that card.
/// Returns where it says it goes (its `Dest`); moving it there is the caller's
/// job, since only the caller knows where the card came from.
pub fn play_card(id: &str, player_id: i32) -> Result<Dest, Prompt> {
    let (p, l) = s(id);
    let v = asked(unsafe { sys::play_card(p, l, player_id) })?;
    Ok(Dest::from_i32(v))
}

/// `H.CanReplay` -- could `player_id` play `id` right now (it has a `play` effect and
/// its `cant_play` gate is open)?
pub fn card_replayable(player_id: i32, id: &str) -> bool {
    let (p, l) = s(id);
    unsafe { sys::card_replayable(player_id, p, l) != 0 }
}

/// The move being planned (C# `TurnCtx.Plan`, a `MoveCtx`): what an
/// `On::RollPlan` body shapes before the dice, and what a card shapes before
/// running a move of its own. (`trigger::move_*` is different: it describes the
/// move that *caused* the current trigger.)
pub mod plan {
    use super::*;

    /// C# `SetSteps` -- the planned length; keeps the sign of a reverse walk.
    pub fn set_steps(n: i32) {
        unsafe { sys::set_steps(n) }
    }

    /// C# `Reverse` -- walk backwards.
    pub fn set_reverse(on: bool) {
        unsafe { sys::set_reverse(on as i32) }
    }

    /// C# `Signed` -- a negative roll walks backwards instead of clamping to 0.
    pub fn set_signed(on: bool) {
        unsafe { sys::set_signed(on as i32) }
    }

    /// C# `StopAt` -- force the walk to stop on this tile; -1 clears.
    pub fn set_stop_at(n: i32) {
        unsafe { sys::set_stop_at(n) }
    }

    /// C# `Parity` -- only odd (1) / even (0) tiles count; -1 either.
    pub fn set_parity(n: i32) {
        unsafe { sys::set_parity(n) }
    }

    /// C# `Resolve` -- settle where the walk lands.
    pub fn set_resolve(on: bool) {
        unsafe { sys::set_resolve(on as i32) }
    }

    /// C# `SettleTile` -- settle on this tile instead of the landing; -1 clears.
    pub fn set_settle_tile(n: i32) {
        unsafe { sys::set_settle_tile(n) }
    }

    /// C# `PayFactor` in milli-units (500 = x0.5): money paid on this walk.
    pub fn set_pay_factor(n: i32) {
        unsafe { sys::set_pay_factor(n) }
    }

    /// C# `RentFactor` in milli-units (500 = x0.5): rent paid on this walk.
    pub fn set_rent_factor(n: i32) {
        unsafe { sys::set_rent_factor(n) }
    }

    /// C# `NoBuy` -- the walk cannot buy where it lands.
    pub fn set_no_buy(on: bool) {
        unsafe { sys::set_no_buy(on as i32) }
    }

    /// C# `NoBuild` -- the walk cannot build where it lands.
    pub fn set_no_build(on: bool) {
        unsafe { sys::set_no_build(on as i32) }
    }

    /// C# `BuildAnywhere` -- the walk may build anywhere it lands.
    pub fn set_build_anywhere(on: bool) {
        unsafe { sys::set_build_anywhere(on as i32) }
    }

    /// How the move gets there (C# `m.Teleport`): [`MoveKind::Walk`] goes by
    /// the path, [`MoveKind::Teleport`] jumps to its destination (with
    /// `set_teleport_to` naming it, or the roll deriving it -- the old
    /// `TeleportWalk`, 「视为 [传送]（只触发终点）」).
    pub fn set_kind(kind: MoveKind) {
        unsafe { sys::set_kind(kind as i32) }
    }

    /// C# `MinRoll` -- clamp the final face up to this, after the reactions.
    pub fn set_min_roll(n: i32) {
        unsafe { sys::set_min_roll(n) }
    }

    /// C# `ExtraSteps`.
    pub fn set_extra_steps(n: i32) {
        unsafe { sys::set_extra_steps(n) }
    }

    /// C# `MoreSteps`.
    pub fn set_more_steps(n: i32) {
        unsafe { sys::set_more_steps(n) }
    }

    /// `MoveCtx.Tags` -- a free-form per-card counter on this move. Card rules
    /// own effects like [火罐] rolls through these instead of an engine flag:
    /// the card that arms one tags the move (`set_tag("fireRoll", 1)`) and
    /// readers ask [`trigger::move_tag`].
    pub fn set_tag(key: &str, value: i32) {
        let (p, l) = s(key);
        unsafe { sys::set_tag(p, l, value) }
    }

    /// C# `NoCircleReward` -- passing CiRCLE pays nothing on this walk.
    pub fn set_no_circle_reward(on: bool) {
        unsafe { sys::set_no_circle_reward(on as i32) }
    }

    /// C# `SettleAsAgent`.
    pub fn set_settle_as_agent(on: bool) {
        unsafe { sys::set_settle_as_agent(on as i32) }
    }

    /// C# `TeleportTo` -- the teleport's destination. -1 derives it from the
    /// roll (the 「视为 [传送]（只触发终点）」 shape).
    pub fn set_teleport_to(tile: i32) {
        unsafe { sys::set_teleport_to(tile) }
    }

    /// The tile the walk begins on instead of the player's own; -1 for the
    /// player's own. `why` is shown on the move's log line.
    pub fn set_start(tile: i32, why: &str) {
        let (p, l) = s(why);
        unsafe { sys::set_start(tile, p, l) }
    }

    /// C# `Base` -- replace the roll's dice table with `count`d`sides`.
    pub fn set_base_dice(count: i32, sides: i32, why: &str) {
        let (p, l) = s(why);
        unsafe { sys::set_base_dice(count, sides, p, l) }
    }

    /// Append `count`d`sides` to the roll's base dice.
    pub fn add_base_dice(count: i32, sides: i32, why: &str) {
        let (p, l) = s(why);
        unsafe { sys::add_base_dice(count, sides, p, l) }
    }

    /// Append `count`d`sides` to the roll as an extra term. A flat add is a
    /// `0`-sided term: `add_extra_dice(n, 0, why)` is what C# `Bonus` did.
    pub fn add_extra_dice(count: i32, sides: i32, why: &str) {
        let (p, l) = s(why);
        unsafe { sys::add_extra_dice(count, sides, p, l) }
    }

    /// Did the walk stop before its full length? Read-only: the walk loop sets
    /// it. To force a stop, name the tile with [`Self::set_stop_at`].
    pub fn stopped() -> bool {
        unsafe { sys::move_stopped() != 0 }
    }

    /// C# `StopAt`, or -1.
    pub fn stop_at() -> i32 {
        unsafe { sys::move_stop_at() }
    }

    /// C# `Parity`: -1 either, 0 even, 1 odd.
    pub fn parity() -> i32 {
        unsafe { sys::move_parity() }
    }

    /// C# `Resolve`.
    pub fn resolve() -> bool {
        unsafe { sys::move_resolve() != 0 }
    }

    /// The planned length (`MovePlan.Landing` is where it ends).
    pub fn steps() -> i32 {
        unsafe { sys::move_steps() }
    }

    /// C# `Remaining` -- steps left to walk.
    pub fn remaining() -> i32 {
        unsafe { sys::move_remaining() }
    }

    /// C# `Total` -- steps walked (`lastWalk` is written from this).
    pub fn total() -> i32 {
        unsafe { sys::move_total() }
    }

    /// C# `Dir` -- +1 forwards, -1 backwards.
    pub fn dir() -> i32 {
        unsafe { sys::move_dir() }
    }
    /// names the whole face means exactly that face.
    pub fn clear_dice() {
        unsafe { sys::clear_dice() }
    }
    /// C# `MoveCtx.CanBuild` -- may this move build where it lands?
    pub fn set_can_build(on: bool) {
        unsafe { sys::set_can_build(on as i32) }
    }
}

/// C# `H.Target(c, seat)` for a single-target card -- try to target `player_id`.
/// `None` when the targeting failed (out, exiled, immune, untargetable, or the
/// `target` [反击] window cancelled it); otherwise the player actually targeted,
/// which a field `redirect` hook may have changed (C# `IRedirect`).
pub fn target(player_id: i32) -> Option<i32> {
    let r = unsafe { sys::target(player_id, -1, 1) };
    (r >= 0).then_some(r)
}

/// C# `H.TargetAll(c, seats, got)` -- target each player in turn (no redirect);
/// returns the players that were targeted, in order, without duplicates.
pub fn target_all(players: &[i32]) -> Vec<i32> {
    let mut got = Vec::new();
    for &s in players {
        let r = unsafe { sys::target(s, -1, 0) };
        if r >= 0 && !got.contains(&r) {
            got.push(r);
        }
    }
    got
}

/// C# `H.TargetTile(c, tile)` -- try to target `tile` (and so its owner, when
/// someone else owns it). False when the targeting failed.
pub fn target_tile(tile: i32) -> bool {
    unsafe { sys::target(-1, tile, 0) >= 0 }
}

/// C# `H._targeted[player_id]` -- times other players' cards targeted `player_id` since its
/// own turn last started.
pub fn targeted_count(player_id: i32) -> i32 {
    unsafe { sys::targeted_count(player_id) }
}

/// C# `_abnormalTurn[player_id]` -- abnormal effects that got through to `player_id`
/// this turn (reset for every player at each turn start).
pub fn abnormal_count(player_id: i32) -> i32 {
    unsafe { sys::abnormal_count(player_id) }
}

/// Is `id` placed on `player_id`'s field? `Some(tile)` with the tile it is bound to
/// (-1 when it is not on a tile), `None` when the player has no such card in play.
pub fn placed_tile(player_id: i32, id: &str) -> Option<i32> {
    let (p, l) = s(id);
    let v = unsafe { sys::placed_tile(player_id, p, l) };
    (v >= -1).then_some(v)
}

/// Arm the doubling for the run in progress -- the *playing* card's
/// `PlayCtx.Doubled`. CiRCLE's band skill sets it from outside that card, which
/// is why it is a world setter and not something the card owns. `-1` clears.
pub fn set_play_doubled(n: i32) {
    unsafe { sys::set_play_doubled(n) }
}

/// C# `PlayCtx.N(k, value)` -- a number from this card's text, doubled when the
/// play doubles its `k`-th number (C# `PlayCtx.Doubled`). Use it instead of a
/// bare literal for the numbers a doubling effect can target.
pub fn n(k: i32, value: i32) -> i32 {
    if unsafe { sys::play_doubled() } == k {
        value * 2
    } else {
        value
    }
}

// ------------------------------------------------- turn plan & scheduling

/// C# `TurnCtx.AfterEnd` -- run this card's `On::AtEnd` (for `player_id`) when the
/// current turn ends, *after* the end-of-turn status wear-off (so a [停留] it
/// grants lasts through the next turn). The card need not be in play.
/// Scheduling twice runs it twice.
pub fn at_turn_end(player_id: i32) {
    unsafe { sys::schedule_turn_end(player_id, 0) }
}

/// C# `TurnCtx.AtEnd` -- run this card's `On::AtEnd` (for `player_id`) when the
/// current turn ends, *before* the status wear-off (「回合结束时」 money losses,
/// discarding down to a hand size).
pub fn before_turn_end(player_id: i32) {
    unsafe { sys::schedule_turn_end(player_id, 2) }
}

/// 「你的下回合结束时」 -- run this card's `On::AtEnd` (for `player_id`) at the end of
/// `player_id`'s next turn (not the current one, if it is `player_id`'s), after its wear-off.
pub fn at_next_turn_end(player_id: i32) {
    unsafe { sys::schedule_turn_end(player_id, 1) }
}

/// C# `TurnCtx.NoMoneyLoss` -- `player_id`'s money cannot drop for the rest of this
/// turn (payments it would make are waived; auctions are not payments).
pub fn set_no_money_loss(player_id: i32) {
    unsafe { sys::set_no_money_loss(player_id) }
}

/// C# `TurnCtx.Plan.FixedRoll` -- this turn's main-move roll is `n`.
pub fn set_fixed_roll(n: i32) {
    unsafe { sys::set_fixed_roll(n) }
}

/// This turn's fixed main-move roll, if a card set one.
pub fn fixed_roll() -> Option<i32> {
    let v = unsafe { sys::fixed_roll() };
    (v >= 0).then_some(v)
}

/// C# `NextStepsFx` -- `player_id`'s next main move walks exactly `n` steps.
pub fn set_next_steps(player_id: i32, n: i32) {
    unsafe { sys::set_next_steps(player_id, n) }
}

/// C# `TurnCtx.LastMain` -- steps this turn's main move walked (0 = none yet).
pub fn turn_main_steps() -> i32 {
    unsafe { sys::turn_main_steps() }
}

/// C# `Card.FireMaxDelta` -- raise (or, negative, lower) `player_id`'s [火罐] cap;
/// returns the new cap.
pub fn add_fire_max(player_id: i32, n: i32) -> i32 {
    unsafe { sys::add_fire_max(player_id, n) }
}

/// C# `DecayCard` tick -- burn one of this card's crystals. Returns the count
/// left.
///
/// The 「…为0时放入弃牌堆」 half of a decay clause is **not** here: it is the
/// card's `HookKind::CrystalsChanged` handler, so a count emptied by *any*
/// write (another card's `add_card_crystals`, say) leaves the field too. This
/// used to unplace-and-discard at 0, which only covered the tick.
pub fn decay() -> i32 {
    add_crystals(-1, 0)
}

/// The trigger a reaction is being checked against (C# `Trigger`).
pub mod trigger {
    use super::*;

    pub fn kind() -> TriggerKind {
        TriggerKind::from_i32(unsafe { sys::trig_kind() })
    }

    /// `t.Seat` -- whose action this is (the mover, the payer, the player of the card).
    pub fn player_id() -> i32 {
        unsafe { sys::trig_player() }
    }

    /// `t.Target` -- who it is aimed at (`t.Pay.to` on pay triggers), or -1.
    pub fn target() -> i32 {
        unsafe { sys::trig_target() }
    }

    /// `t.Tile` -- the tile involved (settled on, passed, mortgaged), or -1.
    pub fn tile() -> i32 {
        unsafe { sys::trig_tile() }
    }

    /// `t.Value` -- the amount involved (`t.Pay.amount` on pay triggers), or 0.
    pub fn value() -> i32 {
        unsafe { sys::trig_value() }
    }

    /// `t.Step` -- the turn stage (0 = no turn; 1 开始 / 2 运营 / 3 移动 / 4 结束) active when this trigger fired.
    /// Only meaningful for kinds that aren't step-specific (e.g. `mortgage`
    /// can fire during both step 1 and step 3).
    pub fn step() -> i32 {
        unsafe { sys::trig_step() }
    }

    /// `t.ByCard` -- the player whose card caused this trigger, or `None` when the
    /// trigger was not card-caused (board-driven: rent, buy, build, turn flow).
    /// This is what C# `H.HitByOtherCard` keys on: `by_card().is_some_and(|by|
    /// by != player)` means "another player's card did this to me". Distinct from
    /// `player_id()` (the mover/payer/target), which on a `pay` trigger is the
    /// *payer*, not the card that forced the payment.
    pub fn by_card() -> Option<i32> {
        let v = unsafe { sys::trig_by_card() };
        (v >= 0).then_some(v)
    }

    /// `t.Pay.IsRent` -- is this `pay`/`paid` trigger rent (C# `t.Pay.kind == "rent"`),
    /// as opposed to a buy, a build, or a forced loss. Always false on a
    /// card-driven payment.
    pub fn pay_is_rent() -> bool {
        unsafe { sys::trig_pay_is_rent() != 0 }
    }

    /// `t.Move` -- how the move that caused this trigger got there (C#
    /// `m.Teleport`): `None` when the move did not cause it. Present on
    /// `moveRoll` / `pass` / `settleBefore` / `settle` / `settleAfter` raised
    /// while a player is moving. A walk enters every tile it steps on; a teleport
    /// enters only its destination.
    pub fn move_kind() -> Option<MoveKind> {
        MoveKind::from_i32(unsafe { sys::trig_move_kind() })
    }

    /// `t.Move.Resolve` -- does that move settle where it lands (C#
    /// `m.Resolve`)? False = a card effect prevented settle at all.
    pub fn move_resolve() -> bool {
        unsafe { sys::trig_move_resolve() != 0 }
    }

    /// `t.Move.Tags[key]` -- a per-card counter the move carries (C# `m.Tags`).
    /// A [火罐] roll is card-owned state: whoever armed it tagged the move
    /// (`"fireRoll"` by convention).
    pub fn move_tag(key: &str) -> i32 {
        let (p, l) = s(key);
        unsafe { sys::trig_move_tag(p, l) }
    }

    /// `t.Move.Main` -- was this the turn's main move (C# `MoveCtx.main`)?
    pub fn move_is_main() -> bool {
        unsafe { sys::trig_move_main() != 0 }
    }

    /// `t.Move.Dir` -- the direction of the move: 1 forward, -1 backward.
    /// Only meaningful when the move caused the trigger ([`move_kind`]`().is_some()`).
    pub fn move_dir() -> i32 {
        unsafe { sys::trig_move_dir() }
    }

    /// The abnormal effect on an `abnormalGuard` / `abnormal` trigger.
    pub fn abnormal_kind() -> Option<AbKind> {
        match kind() {
            TriggerKind::Abnormal | TriggerKind::AbnormalGuard => AbKind::from_i32(value()),
            _ => None,
        }
    }

    /// The cards a `drew` trigger is about, in draw order (empty otherwise).
    pub fn cards() -> Vec<String> {
        let need = unsafe { sys::trig_cards(0, 0) };
        if need <= 0 {
            return Vec::new();
        }
        let mut buf: Vec<u8> = Vec::new();
        buf.resize(need as usize, 0);
        if unsafe { sys::trig_cards(buf.as_mut_ptr() as i32, need) } != need {
            return Vec::new();
        }
        postcard::from_bytes(&buf).unwrap_or_default()
    }

    /// `t.Move.Remaining` -- steps the move has left to walk.
    pub fn move_remaining() -> i32 {
        unsafe { sys::trig_move_remaining() }
    }

    /// `t.Move.Path.Count` -- the move's path length so far.
    pub fn move_total() -> i32 {
        unsafe { sys::trig_move_total() }
    }

    /// `t.Move.Roll` -- the roll being reacted to, or -1.
    pub fn move_roll() -> Option<i32> {
        let v = unsafe { sys::trig_move_roll() };
        (v >= 0).then_some(v)
    }

    pub fn set_move_roll(v: i32) {
        unsafe { sys::trig_set_move_roll(v) }
    }

    /// Rewrite the amount of a pending `pay`/`paid` trigger (C# `PayCtx.amount`).
    /// `0` cancels the payment outright (C# `t.Pay.cancel = true`). The engine
    /// honours whatever this leaves on the trigger once the reaction window
    /// resolves.
    pub fn set_pay_amount(v: i32) {
        unsafe { sys::trig_set_pay_amount(v) }
    }

    /// Redirect the payee of a pending `pay` (C# `PayCtx.to`); `-1` sends the
    /// money to the bank instead. Combine with [`set_pay_amount`] to reshape a
    /// payment completely.
    pub fn set_pay_target(to: i32) {
        unsafe { sys::trig_set_pay_target(to) }
    }

    /// Rewrite `t.target` -- on a `redirect` hook, the player that takes the hit
    /// instead (C# `IRedirect`).
    pub fn set_target(player_id: i32) {
        unsafe { sys::trig_set_pay_target(player_id) }
    }

    /// Negate this link's **activation** (Yu-Gi-Oh: the card "did not
    /// activate"). It never happened -- nothing settles -- and a listener on the
    /// effect does not see it. The effect body is skipped while the point's
    /// Before/After hooks still fire.
    pub fn set_cancelled() {
        unsafe { sys::trig_set_cancelled() }
    }

    /// Negate this link's **effect** (Yu-Gi-Oh: it activated, its effect does
    /// nothing). A listener on the effect still sees it; it just settles to
    /// nothing. Weaker than [`set_cancelled`], and does not undo it.
    pub fn negate_effect() {
        unsafe { sys::trig_set_negate_effect() }
    }

    /// Take one recipient out of settlement. The effect still settles for
    /// everyone else -- this is *not* a complete invalidation; use
    /// [`set_cancelled`] for that. Counter cards differ on which they mean.
    pub fn spare(seat: i32) {
        unsafe { sys::trig_set_spare(seat) }
    }

    /// Has a reaction already cancelled this trigger (`Trigger.Cancelled`)?
    pub fn cancelled() -> bool {
        unsafe { sys::trig_cancelled() != 0 }
    }

    /// `t.Card == id` / `t.Play.Id == id` -- is this trigger about that card?
    pub fn card_is(id: &str) -> bool {
        let (p, l) = s(id);
        unsafe { sys::trig_card_is(p, l) != 0 }
    }

    /// Position of this link within the current chain, 1-based. 0 = not on a
    /// chain (a bare hook or gate raise).
    pub fn seq() -> i32 {
        unsafe { sys::trig_seq() }
    }

    /// Which link this one answers. 0 = this is the effect declaration itself.
    pub fn answers() -> i32 {
        unsafe { sys::trig_answers() }
    }
}

/// The effects a chain link declares, with their recipients already named.
///
/// A [反击] card chooses how much of this to read. For the rulebook's recurring
/// 「被其他玩家的卡效果影响」 clause the *whole list* is the right view -- any
/// effect of that play touching me -- while a card like
/// `CRYCHIC:主唱太拼命了` (「一次性向其他玩家支付5000以上资金时」) wants one
/// entry. Both are here; the card picks.
pub mod effect {
    use super::sys;
    use crate::abi::TriggerKind;

    /// How many effects this link declares.
    pub fn count() -> i32 {
        unsafe { sys::trig_effect_count() }
    }

    /// What effect `i` does, as a [`TriggerKind`] wire value.
    pub fn kind(i: i32) -> TriggerKind {
        TriggerKind::from_i32(unsafe { sys::trig_effect_kind(i) })
    }

    /// The seat effect `i` is aimed at (-1 for none).
    pub fn target(i: i32) -> i32 {
        unsafe { sys::trig_effect_target(i) }
    }

    /// The other party to effect `i`, where it has one. A payment touches both
    /// its payer and its payee.
    pub fn from(i: i32) -> i32 {
        unsafe { sys::trig_effect_from(i) }
    }

    /// The tile effect `i` is aimed at (-1 for none).
    pub fn tile(i: i32) -> i32 {
        unsafe { sys::trig_effect_tile(i) }
    }

    /// The amount effect `i` carries, where it has one.
    pub fn value(i: i32) -> i32 {
        unsafe { sys::trig_effect_value(i) }
    }

    /// Does any declared effect touch `seat`? The whole-list view: 「被…效果
    /// 影响」 without asking which effect. A payment touches both its payer and
    /// its payee.
    pub fn hits(seat: i32) -> bool {
        (0..count()).any(|i| target(i) == seat || from(i) == seat)
    }

    /// Is any declared effect of `kind`? The per-effect view, for a card that
    /// means one thing specifically.
    pub fn has(kind: TriggerKind) -> bool {
        (0..count()).any(|i| self::kind(i) == kind)
    }

    /// Append an effect to the current link, naming its recipient now -- at
    /// declaration, not at settlement.
    pub fn declare(kind: TriggerKind, target: i32, from: i32, tile: i32, value: i32) {
        unsafe { sys::declare_effect(kind as i32, target, from, tile, value) }
    }
}

// ------------------------------------------------------------- placement & skills

/// C# `H.IsColor` -- does `tile` count as colour `group` for `player_id`?
pub fn is_color(player_id: i32, tile: i32, group: i32) -> bool {
    unsafe { sys::is_color(player_id, tile, group) != 0 }
}

/// `H.Unplace` -- take this card out of play. True when it was there.
/// Take a *named* card off the player's field (C# `H.Unplace(card, ...)`).
/// [`unplace_card`] is the special case where the named card is the one running.
pub fn unplace_card_named(player_id: i32, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::unplace_card_named(player_id, p, l) != 0 }
}

/// Move one matching mark's `count` by `delta`, dropping it at 0 (C#
/// `mark.count--`). [`remove_marks`] clears every match; this is the single-tick
/// form the 「移除一个」 clauses want. Returns the count now stored.
pub fn bump_mark(tile: i32, kind: &str, owner: i32, delta: i32) -> i32 {
    let (kp, kl) = s(kind);
    unsafe { sys::bump_mark(tile, kp, kl, owner, delta) }
}

/// Names of the player's non-zero counters whose name starts with `prefix`.
/// The listing half of the counter query; [`tok`] reads one by name.
pub fn tok_names(player_id: i32, prefix: &str) -> Vec<String> {
    let (p, l) = s(prefix);
    let cap = 4096;
    let mut buf = alloc::vec![0u8; cap as usize];
    let n = unsafe { sys::tok_names(player_id, p, l, buf.as_mut_ptr() as i32, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// The C# `H.AbnormalGate` -- ask the engine whether an abnormal effect of
/// `kind` (`[强制停下]`, `[传送]`, ...) may land on `player_id`. Returns `false`
/// when a field card guarded it, the player is immune, or they are
/// `unstoppable`. Call this **before** applying the effect, and skip on `false`.
pub fn gate(player_id: i32, kind: crate::abi::AbKind) -> bool {
    unsafe { sys::gate(player_id, kind as i32) != 0 }
}

/// `H.SettleAt` -- a full [触发结算] of `tile` for this player. The player does
/// not move; the tile's own effect resolves. `main` marks it as the turn's
/// landing (it writes `landed`).
pub fn card_settle_at(player_id: i32, tile: i32, main: bool) -> bool {
    unsafe { sys::card_settle_at(player_id, tile, main as i32) != 0 }
}

/// `H.OfferBuildAmong` -- prompt to build on one of `tiles`, then build there.
/// Silently skips when none of them can take a house.
pub fn card_offer_build(player_id: i32, tiles: &[i32]) -> bool {
    let mut buf = alloc::vec![0u8; tiles.len() * 4];
    for (i, &t) in tiles.iter().enumerate() {
        buf[i * 4..i * 4 + 4].copy_from_slice(&t.to_le_bytes());
    }
    unsafe { sys::card_offer_build(player_id, buf.as_ptr() as i32, buf.len() as i32) != 0 }
}

/// `H.DoMoveRoll` -- sum the planned move's dice tables into one face. The
/// `rollAfter` / `moveRoll` hooks are **not** raised: a re-roll is usually being
/// requested from inside one of them, and re-raising would recurse.
pub fn do_move_roll(player_id: i32) -> i32 {
    unsafe { sys::do_move_roll(player_id) }
}

/// `H.IsLiveHouse` for one player: the base check plus their `Fx.ExtraColor`.
pub fn is_live_house_for(player_id: i32, tile: i32) -> bool {
    unsafe { sys::is_live_house_for(player_id, tile) != 0 }
}

/// Is this card in play at the player?
/// Ids of the player's placed field cards, in placement order.
pub fn placed_cards(player_id: i32) -> Vec<String> {
    let cap = 4096;
    let mut buf = alloc::vec![0u8; cap as usize];
    let n = unsafe { sys::placed_cards(player_id, buf.as_mut_ptr() as i32, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// 「加盖房屋时半价」 / 「本回合加盖房屋变为免费」 -- the build cost as a
/// percentage of its table price (100 = full, 50 = half, 0 = free).
pub fn set_build_cost_pct(pct: i32) {
    unsafe { sys::set_build_cost_pct(pct) }
}

/// Flip a placed card face-down / face-up (C# `H.SwitchState`).
pub fn set_card_face_down(player_id: i32, card: &str, down: bool) -> bool {
    let (cp, cl) = s(card);
    unsafe { sys::set_card_face_down(player_id, cp, cl, down as i32) != 0 }
}

/// C# `Fx.ExtraColor` -- re-colour a tile for one player (「使其对你视为live
/// house格子」). `-1` clears.
pub fn set_extra_color(player_id: i32, tile: i32, group: i32) {
    unsafe { sys::set_extra_color(player_id, tile, group) }
}

/// C# `Card.AddCrystals` on a named placed card; `max` caps (0 = uncapped).
pub fn add_card_crystals(player_id: i32, card: &str, n: i32, max: i32) -> i32 {
    let (cp, cl) = s(card);
    unsafe { sys::add_card_crystals(player_id, cp, cl, n, max) }
}

/// Is this placed card face-down? The `!p.FaceDown` half of the C# field
/// filters; [`cards_in`] lists face-down cards too.
pub fn card_face_down(player_id: i32, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::card_face_down(player_id, p, l) != 0 }
}

// ------------------------------------------------------------- placement & skills

/// `H.PlaceCard` -- put a specific card (a derived one) into play at a player.
/// `WhyNotBuildOn` -- may this player build on this tile? The same gate the
/// build step uses, so a card choosing a destination cannot pick one the engine
/// would then refuse.
pub fn can_build_on(player_id: i32, tile: i32) -> bool {
    unsafe { sys::can_build_on(player_id, tile) != 0 }
}

/// `H.BuyRoutine` -- buy `tile` now (the 「必须购买」 clauses).
pub fn card_buy(player_id: i32, tile: i32) -> bool {
    unsafe { sys::card_buy(player_id, tile) != 0 }
}

/// Is this placed field card immune to other effects?
pub fn card_immune(player_id: i32, card: &str) -> bool {
    let (cp, cl) = s(card);
    unsafe { sys::card_immune(player_id, cp, cl) != 0 }
}

/// `H.MortgageRoutine` -- mortgage one of the player's deeds.
pub fn card_mortgage(player_id: i32, tile: i32) -> bool {
    unsafe { sys::card_mortgage(player_id, tile) != 0 }
}

/// Does this card's rule text mention `needle`? 「所有效果包含[奇迹水晶]的卡」
/// is this query.
pub fn card_text_mentions(card: &str, needle: &str) -> bool {
    let (cp, cl) = s(card);
    let (np, nl) = s(needle);
    unsafe { sys::card_text_mentions(cp, cl, np, nl) != 0 }
}

/// A gain skills and crits may not bend (C# `fixedAmount`). The money moves;
/// the `payAdd` / `payMul` / `payChoose` hooks do not see it.
pub fn gain_fixed(player_id: i32, amount: i32, why: &Msg) -> i32 {
    let (p, l) = mj(why);
    unsafe { sys::gain_fixed(player_id, amount, p, l) }
}

/// Is this the 地产商 tile (C# `kind == "agent"`)?
pub fn is_agent(tile: i32) -> bool {
    unsafe { sys::is_agent(tile) != 0 }
}

/// What this turn's [触发结算]s have cost the player so far (C#
/// `TurnCtx.PaidInSettle`).
pub fn paid_in_settle() -> i32 {
    unsafe { sys::paid_in_settle() }
}

/// Place a field card **on a tile** (the mark sits on the board at `tile`
/// rather than with its owner). Returns the new instance's uid.
pub fn place_card_on(player_id: i32, tile: i32, card: &str, note: &Msg) -> i32 {
    let (cp, cl) = s(card);
    let (p, l) = mj(note);
    unsafe { sys::place_card_on(player_id, tile, cp, cl, p, l) }
}

/// Did the play being resolved come from the hand? `false` when the card was
/// played from somewhere else -- 「若此卡从手牌以外的地方打出」.
pub fn play_from_hand() -> bool {
    unsafe { sys::play_from_hand() != 0 }
}

/// 「下次盖房时减免N（可溢出），盖房后减少1层」 -- a layered cut on the build
/// cost. Each build pops one layer.
pub fn set_build_discount(n: i32, layers: i32) {
    unsafe { sys::set_build_discount(n, layers) }
}

/// 「本回合购买格子时[消耗]资金时降低N（最低0）」 (C# `TurnCtx.BuyDiscount`).
pub fn set_buy_discount(n: i32) {
    unsafe { sys::set_buy_discount(n) }
}

/// C# `Card.Immune` -- 「此卡不受…效果影响」. Set it at play time; effects that
/// would touch the card read [`card_immune`] and skip.
pub fn set_card_immune(player_id: i32, card: &str, on: bool) -> bool {
    let (cp, cl) = s(card);
    unsafe { sys::set_card_immune(player_id, cp, cl, on as i32) != 0 }
}

/// Move a placed field card to `tile` (C# `card.Tile = t`). The card is already
/// in play; this only changes where it sits. `tile: -1` puts it back with its
/// owner.
pub fn set_card_tile(player_id: i32, card: &str, tile: i32) -> bool {
    let (cp, cl) = s(card);
    unsafe { sys::set_card_tile(player_id, cp, cl, tile) != 0 }
}

/// Force the play's number ranges to their theoretical max (`1`) or min
/// (`-1`) -- 「以理论最大值或最小值结算」. `0` clears.
pub fn set_extreme(v: i32) {
    unsafe { sys::set_extreme(v) }
}

/// 「本回合购买格子不[消耗]资金」 (C# `TurnCtx.FreeBuy`).
pub fn set_free_buy(on: bool) {
    unsafe { sys::set_free_buy(on as i32) }
}

/// 「如果购买则拆除那个格子上的所有房屋」 (C# `TurnCtx.RazeOnBuy`).
pub fn set_raze_on_buy(on: bool) {
    unsafe { sys::set_raze_on_buy(on as i32) }
}

/// Re-colour a tile for everyone.
pub fn set_tile_color(tile: i32, group: i32) {
    unsafe { sys::set_tile_color(tile, group) }
}

/// C# `_turnCtx.Rolls` -- every face rolled this turn, in order.
/// 「与本回合内你骰出过的所有骰点都不同」 compares against this.
pub fn turn_rolls() -> Vec<i32> {
    let cap = 4096;
    #[cfg(target_arch = "wasm32")]
    let (p, buf) = {
        let mut buf = alloc::vec![0u8; cap as usize];
        (buf.as_mut_ptr() as i32, buf)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let (p, mut buf) = {
        let (h, l) = crate::native::reserve(cap as usize);
        (h, alloc::vec![0u8; l as usize])
    };
    let n = unsafe { sys::turn_rolls(p, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    #[cfg(not(target_arch = "wasm32"))]
    let buf = crate::native::read(p, n as usize);
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// C# `_turnSnap[i]` -- `(pos, stay, stun, exile)` when the turn started. The
/// four things 「回到起始地点并取消所有受到的效果」 restores.
pub fn turn_snap(player_id: i32) -> (i32, i32, i32, i32) {
    #[cfg(target_arch = "wasm32")]
    let (p, mut buf) = {
        let mut buf = [0u8; 16];
        (buf.as_mut_ptr() as i32, buf)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let (p, mut buf) = {
        let (h, _) = crate::native::reserve(16);
        (h, [0u8; 16])
    };
    let n = unsafe { sys::turn_snap(player_id, p) };
    #[cfg(not(target_arch = "wasm32"))]
    let buf = crate::native::read(p, n.max(0) as usize);
    if n < 16 {
        return (-1, 0, 0, 0);
    }
    let g = |i: usize| i32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);
    (g(0), g(4), g(8), g(12))
}

/// Where the player stood when this turn started (C# `_turnSnap[i].pos`).
/// 「在Livehouse地块开始回合时」 is a question about that square.
pub fn turn_start_pos(player_id: i32) -> i32 {
    unsafe { sys::turn_start_pos(player_id) }
}

/// C# `H.IsLiveHouse` -- a Live House deed (buyable, colour group 6). The
/// `ExtraColor` band-skill colours are not visible here (TODO in the cards).
/// C# `_tileColors[t]` -- re-colour a tile for everyone. `-1` clears;
/// [`ALL_COLORS`] means it counts as every colour (「该格获得所有颜色」).
pub const ALL_COLORS: i32 = -2;

// --------------------------------------------- card crystals & build

/// C# `Card.Crystals` on a named placed card.
pub fn card_crystals(player_id: i32, card: &str) -> i32 {
    let (cp, cl) = s(card);
    unsafe { sys::card_crystals(player_id, cp, cl) }
}

/// `H.BuildRoutine` -- pay `tile`'s build cost and raise one house.
pub fn card_build(player_id: i32, tile: i32) -> bool {
    unsafe { sys::card_build(player_id, tile) != 0 }
}

/// 「位于此卡所在格子上的玩家无法使用角色及乐队技能」 / 「不可使用任何<BAND>
/// 角色的（2）技能」 -- the shared skill-press gate. `skillBlock` on the player
/// suppresses every skill; `skillBlock:<band>` suppresses one band's. A
/// `skillBlock` mark on the tile the player stands on suppresses everyone there.
/// `band` is the skill's own band, or `""` when it does not have one.
pub fn skill_blocked(player_id: i32, band: &str) -> bool {
    if tok(player_id, "skillBlock") > 0 {
        return true;
    }
    if !band.is_empty() {
        let key = alloc::format!("skillBlock:{}", band);
        if tok(player_id, &key) > 0 {
            return true;
        }
    }
    let t = player_pos(player_id);
    t >= 0 && count_marks(t, "skillBlock", -2) > 0
}

/// Where **this instance** sits, or -1 for "with its owner" / gone
/// (C# `Card.Tile`). [`placed_tile`] is the form for some *other* card.
pub fn self_tile() -> Option<i32> {
    let v = unsafe { sys::self_tile() };
    (v >= -1).then_some(v)
}

/// Move **this instance** to `tile` (-1 = back with its owner).
pub fn set_self_tile(tile: i32) -> bool {
    unsafe { sys::set_self_tile(tile) != 0 }
}

/// Is **this instance** face-down?
pub fn self_face_down() -> bool {
    unsafe { sys::self_face_down() != 0 }
}

/// Flip **this instance**; returns whether it is on the field.
pub fn set_self_face_down(on: bool) -> bool {
    unsafe { sys::set_self_face_down(on as i32) != 0 }
}

/// Is **this instance** marked 「不受任何效果影响」?
pub fn self_immune() -> bool {
    unsafe { sys::self_immune() != 0 }
}

/// Mark **this instance** 「不受任何效果影响」; returns whether it is on the field.
pub fn set_self_immune(on: bool) -> bool {
    unsafe { sys::set_self_immune(on as i32) != 0 }
}

/// Every card instance on `player_id`'s field, as `(uid, id)` in placement
/// order. Walk this rather than [`placed_cards`] whenever the next step
/// addresses the instance -- the *name* is not an identity, so a name-keyed
/// op would hit the first copy twice when one player holds two.
pub fn field_instances(player_id: i32) -> Vec<(i32, String)> {
    let cap = 8192;
    let mut buf = alloc::vec![0u8; cap as usize];
    let n = unsafe { sys::field_instances(player_id, buf.as_mut_ptr() as i32, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// The instance at `uid`, wherever it sits. [`crystals`] / [`self_tile`] are
/// the forms for the running instance; these address some *other* one.
pub fn crystals_at(uid: i32) -> i32 {
    unsafe { sys::crystals_at(uid) }
}

/// `H.AddCrystals` on the instance at `uid`; `max` caps (0 = uncapped).
pub fn add_crystals_at(uid: i32, n: i32, max: i32) -> i32 {
    unsafe { sys::add_crystals_at(uid, n, max) }
}

/// Take the instance at `uid` off the field; returns the owner it left.
pub fn unplace_at(uid: i32) -> i32 {
    unsafe { sys::unplace_at(uid) }
}

/// Where the instance at `uid` sits (`Some(-1)` = with its owner, `None` = gone).
pub fn tile_at(uid: i32) -> Option<i32> {
    let v = unsafe { sys::tile_at(uid) };
    (v >= -1).then_some(v)
}

/// Move the instance at `uid` to `tile` (-1 = back with its owner).
pub fn set_tile_at(uid: i32, tile: i32) -> bool {
    unsafe { sys::set_tile_at(uid, tile) != 0 }
}

/// Is the instance at `uid` face-down?
pub fn is_face_down_at(uid: i32) -> bool {
    unsafe { sys::is_face_down_at(uid) != 0 }
}

/// Flip the instance at `uid`; returns whether it exists.
pub fn set_face_down_at(uid: i32, on: bool) -> bool {
    unsafe { sys::set_face_down_at(uid, on as i32) != 0 }
}

/// Is the instance at `uid` marked 「不受任何效果影响」?
pub fn is_immune_at(uid: i32) -> bool {
    unsafe { sys::is_immune_at(uid) != 0 }
}

/// Mark the instance at `uid` 「不受任何效果影响」; returns whether it exists.
pub fn set_immune_at(uid: i32, on: bool) -> bool {
    unsafe { sys::set_immune_at(uid, on as i32) != 0 }
}

/// The uid of the instance named `name` on `player_id`'s field, or `None`.
/// A *resolver*, not an identity: with several copies in play it answers the
/// first, so a rule that means "a specific copy" has to hold the uid from
/// [`place_card`] instead of asking by name.
pub fn find_card(player_id: i32, name: &str) -> Option<i32> {
    field_instances(player_id)
        .into_iter()
        .find(|(_, id)| id == name)
        .map(|(uid, _)| uid)
}
