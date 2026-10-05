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

use crate::abi::{PromptKind, TriggerKind};
use crate::msg::Msg;

mod sys {
    // The link attribute is wasm-only; on other targets the imports stay
    // declared so the analyzer resolves `ctx::*` in card sources.
    #[cfg_attr(target_arch = "wasm32", link(wasm_import_module = "bandori"))]
    extern "C" {
        // dice & log
        pub fn roll(seat: i32, count: i32, sides: i32) -> i32;
        pub fn log(seat: i32, ptr: i32, len: i32);
        // board
        pub fn tile_count() -> i32;
        pub fn tile_named(ptr: i32, len: i32) -> i32;
        pub fn tile_owner(tile: i32) -> i32;
        pub fn seat_pos(seat: i32) -> i32;
        pub fn tile_steps_ahead(seat: i32, steps: i32) -> i32;
        pub fn rent_of(tile: i32) -> i32;
        pub fn buy_price(tile: i32) -> i32;
        pub fn build_cost(tile: i32) -> i32;
        pub fn mortgage_value(tile: i32) -> i32;
        pub fn owned_count(seat: i32) -> i32;
        pub fn owned_at(seat: i32, index: i32) -> i32;
        pub fn is_buyable(tile: i32) -> i32;
        pub fn is_shop(tile: i32) -> i32;
        pub fn tile_group(tile: i32) -> i32;
        pub fn tile_price(tile: i32) -> i32;
        pub fn houses_of(tile: i32) -> i32;
        pub fn set_houses(tile: i32, n: i32);
        pub fn add_house(tile: i32, n: i32) -> i32;
        pub fn mortgaged_of(tile: i32) -> i32;
        pub fn set_mortgaged(tile: i32, v: i32);
        pub fn set_owner(tile: i32, seat: i32);
        pub fn dist(a: i32, b: i32) -> i32;
        pub fn tile_forward(a: i32, b: i32) -> i32;
        pub fn neighbor(seat: i32, dir: i32) -> i32;
        pub fn seats_on_count(tile: i32, except: i32) -> i32;
        pub fn seats_on_at(tile: i32, except: i32, index: i32) -> i32;
        // seats & money
        pub fn seat_count() -> i32;
        pub fn seat_out(seat: i32) -> i32;
        pub fn others_count(seat: i32) -> i32;
        pub fn others_at(seat: i32, index: i32) -> i32;
        pub fn money(seat: i32) -> i32;
        pub fn gain(seat: i32, amount: i32, ptr: i32, len: i32) -> i32;
        pub fn pay(seat: i32, amount: i32, ptr: i32, len: i32) -> i32;
        // hand & deck
        pub fn draw(seat: i32, n: i32) -> i32;
        pub fn add_to_hand(seat: i32, ptr: i32, len: i32);
        pub fn add_to_deck(seat: i32, ptr: i32, len: i32, shuffle: i32);
        pub fn add_to_deck_at(seat: i32, ptr: i32, len: i32, pos: i32);
        pub fn to_discard(seat: i32, ptr: i32, len: i32);
        pub fn hand_count(seat: i32, ptr: i32, len: i32) -> i32;
        pub fn discard_count(seat: i32, ptr: i32, len: i32) -> i32;
        pub fn deck_count(seat: i32) -> i32;
        pub fn discard_size(seat: i32) -> i32;
        pub fn discard_from_hand(seat: i32, ptr: i32, len: i32) -> i32;
        pub fn sweep_to_deck(seat: i32) -> i32;
        // field cards
        pub fn place_card_at(seat: i32, cp: i32, cl: i32, ptr: i32, len: i32);
        pub fn set_dest(dest: i32);
        pub fn ring_multiplier() -> i32;
        pub fn add_ring_bonus(n: i32) -> i32;
        pub fn teleport_to(seat: i32, tile: i32);
        pub fn unplace_card(seat: i32) -> i32;
        pub fn is_placed(seat: i32) -> i32;
        // marks & tokens
        pub fn add_mark(tile: i32, seat: i32, kp: i32, kl: i32, np: i32, nl: i32);
        pub fn count_marks(tile: i32, kp: i32, kl: i32, owner: i32) -> i32;
        pub fn remove_marks(tile: i32, kp: i32, kl: i32, owner: i32) -> i32;
        pub fn tok(seat: i32, ptr: i32, len: i32) -> i32;
        pub fn set_tok(seat: i32, ptr: i32, len: i32, v: i32);
        pub fn add_tok(seat: i32, ptr: i32, len: i32, by: i32, max: i32) -> i32;
        // per-seat slots
        pub fn slot(seat: i32, ptr: i32, len: i32) -> i32;
        pub fn set_slot(seat: i32, ptr: i32, len: i32, v: i32);
        pub fn inc_slot(seat: i32, ptr: i32, len: i32, by: i32) -> i32;
        // pots & status
        pub fn band_crystals(seat: i32) -> i32;
        pub fn add_band_crystals(seat: i32, n: i32, max: i32) -> i32;
        pub fn fire(seat: i32) -> i32;
        pub fn fire_max(seat: i32) -> i32;
        pub fn gain_fire(seat: i32, n: i32, ptr: i32, len: i32) -> i32;
        pub fn give_stay(seat: i32, n: i32);
        pub fn give_stun(seat: i32, n: i32);
        pub fn give_exile(seat: i32, n: i32, to: i32);
        pub fn give_extra_turn(seat: i32);
        pub fn can_pay(seat: i32) -> i32;
        pub fn spend_fire(seat: i32, n: i32, ptr: i32, len: i32) -> i32;
        pub fn stay_of(seat: i32) -> i32;
        pub fn stun_of(seat: i32) -> i32;
        pub fn turn_seat() -> i32;
        pub fn round_no() -> i32;
        pub fn turn_key() -> i32;
        pub fn character_is(seat: i32, ptr: i32, len: i32) -> i32;
        pub fn in_band(seat: i32, ptr: i32, len: i32) -> i32;
        // prompts & trigger
        pub fn opt_int(v: i32);
        pub fn opt_str(ptr: i32, len: i32);
        pub fn ask(kind: i32, seat: i32, title_ptr: i32, title_len: i32, text_ptr: i32, text_len: i32) -> i32;
        pub fn trig_kind() -> i32;
        pub fn trig_seat() -> i32;
        pub fn trig_target() -> i32;
        pub fn trig_tile() -> i32;
        pub fn trig_value() -> i32;
        pub fn trig_move_roll() -> i32;
        pub fn trig_set_move_roll(v: i32);
        pub fn trig_card_is(ptr: i32, len: i32) -> i32;
        pub fn play_card(id_ptr: i32, id_len: i32, seat: i32);
    }
}

fn s(v: &str) -> (i32, i32) {
    (v.as_ptr() as i32, v.len() as i32)
}

/// The i32 wire form of a message: `postcard` bytes leaked for the host to read
/// (same lifetime convention as every other buffer crossing the ABI).
fn mj(m: &Msg) -> (i32, i32) {
    let b = m.to_bytes();
    let p = b.as_ptr() as i32;
    let l = b.len() as i32;
    core::mem::forget(b);
    (p, l)
}

// ------------------------------------------------------------- dice & log

/// `H.Roll(seat, count, sides, what)` -- sum of `count` d`sides`, logged as a dice event.
pub fn roll(seat: i32, count: i32, sides: i32) -> i32 {
    unsafe { sys::roll(seat, count, sides) }
}

/// `H.Log("text", seat, text)`.
pub fn log(seat: i32, msg: &Msg) {
    let (p, l) = mj(msg);
    unsafe { sys::log(seat, p, l) }
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

/// The seat's tile on the ring (`H.State.seats[seat].pos`).
pub fn seat_pos(seat: i32) -> i32 {
    unsafe { sys::seat_pos(seat) }
}

/// Tile `steps` ahead of a seat around the ring.
pub fn tile_steps_ahead(seat: i32, steps: i32) -> i32 {
    unsafe { sys::tile_steps_ahead(seat, steps) }
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

pub fn owned_count(seat: i32) -> i32 {
    unsafe { sys::owned_count(seat) }
}

pub fn owned_at(seat: i32, index: i32) -> i32 {
    unsafe { sys::owned_at(seat, index) }
}

/// The seat's deeds as tile indices (`H.OwnedBy`-style walks).
pub fn owned_tiles(seat: i32) -> Vec<i32> {
    (0..owned_count(seat)).map(|i| owned_at(seat, i)).collect()
}

/// `TileData.IsBuyable` -- a deed tile (property / RiNG).
pub fn is_buyable(tile: i32) -> bool {
    unsafe { sys::is_buyable(tile) != 0 }
}

/// C# `H.IsShop` -- a 商店街 deed (buyable and colour group 10).
pub fn is_shop(tile: i32) -> bool {
    unsafe { sys::is_shop(tile) != 0 }
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

/// Hand a deed to another seat outright (C# `H.State.owners[t] = seat`; deed
/// transfer effects). The caller logs the transfer.
pub fn set_owner(tile: i32, seat: i32) {
    unsafe { sys::set_owner(tile, seat) }
}

/// C# `H.Dist` -- undirected ring distance between two tiles.
pub fn dist(a: i32, b: i32) -> i32 {
    unsafe { sys::dist(a, b) }
}

/// C# `H.Forward` -- steps forward from `a` to `b` around the ring.
pub fn tile_forward(a: i32, b: i32) -> i32 {
    unsafe { sys::tile_forward(a, b) }
}

/// C# `H.Neighbor(seat, dir)` -- the next present seat in turn order (`dir` ±1), or -1.
pub fn neighbor(seat: i32, dir: i32) -> i32 {
    unsafe { sys::neighbor(seat, dir) }
}

/// C# `H.SeatsOn(tile, except)` -- present seats standing on a tile.
pub fn seats_on(tile: i32, except: i32) -> Vec<i32> {
    (0..seats_on_count(tile, except)).map(|i| seats_on_at(tile, except, i)).collect()
}

fn seats_on_count(tile: i32, except: i32) -> i32 {
    unsafe { sys::seats_on_count(tile, except) }
}

fn seats_on_at(tile: i32, except: i32, index: i32) -> i32 {
    unsafe { sys::seats_on_at(tile, except, index) }
}

// ------------------------------------------------------------------ seats

pub fn seat_count() -> i32 {
    unsafe { sys::seat_count() }
}

/// 1 when the seat is out of the game (`H.Out`).
pub fn seat_out(seat: i32) -> bool {
    unsafe { sys::seat_out(seat) != 0 }
}

pub fn others_count(seat: i32) -> i32 {
    unsafe { sys::others_count(seat) }
}

pub fn others_at(seat: i32, index: i32) -> i32 {
    unsafe { sys::others_at(seat, index) }
}

/// The other seats still in the game (`H.Others`).
pub fn others(seat: i32) -> Vec<i32> {
    (0..others_count(seat)).map(|i| others_at(seat, i)).collect()
}

pub fn money(seat: i32) -> i32 {
    unsafe { sys::money(seat) }
}

/// `H.GainR` -- money in, logged with its reason (`src` is a message key).
pub fn gain(seat: i32, amount: i32, src: &Msg) -> i32 {
    let (p, l) = mj(src);
    unsafe { sys::gain(seat, amount, p, l) }
}

/// `H.PayR` -- money out (what the seat could pay), logged.
pub fn pay(seat: i32, amount: i32, src: &Msg) -> i32 {
    let (p, l) = mj(src);
    unsafe { sys::pay(seat, amount, p, l) }
}

/// Seat-to-seat money: the receiver gets exactly what the payer could pay.
/// The usual shape of `H.PayR` + `H.GainR` in a transfer.
pub fn transfer(from: i32, to: i32, amount: i32, src: &Msg) -> i32 {
    let got = pay(from, amount, src);
    gain(to, got, src);
    got
}

// ------------------------------------------------------------- hand / deck

/// `H.DrawR` -- draw `n` cards; returns how many were drawn.
pub fn draw(seat: i32, n: i32) -> i32 {
    unsafe { sys::draw(seat, n) }
}

pub fn add_to_hand(seat: i32, card: &str) {
    let (p, l) = s(card);
    unsafe { sys::add_to_hand(seat, p, l) }
}

pub fn add_to_deck(seat: i32, card: &str, shuffle: bool) {
    let (p, l) = s(card);
    unsafe { sys::add_to_deck(seat, p, l, shuffle as i32) }
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
/// pile at a position. (`add_to_deck(seat, card, shuffle)` is the older
/// top/shuffle shape and stays.)
pub fn add_to_deck_at(seat: i32, card: &str, pos: DeckPos) {
    let (p, l) = s(card);
    unsafe { sys::add_to_deck_at(seat, p, l, pos as i32) }
}

pub fn to_discard(seat: i32, card: &str) {
    let (p, l) = s(card);
    unsafe { sys::to_discard(seat, p, l) }
}

/// Copies of `card` in the seat's hand (C# `_hidden[s].hand` count).
pub fn hand_count(seat: i32, card: &str) -> i32 {
    let (p, l) = s(card);
    unsafe { sys::hand_count(seat, p, l) }
}

/// Copies of `card` in the seat's discard pile.
pub fn discard_count(seat: i32, card: &str) -> i32 {
    let (p, l) = s(card);
    unsafe { sys::discard_count(seat, p, l) }
}

/// Draw-pile size (C# `_hidden[s].draw.Count`).
pub fn deck_count(seat: i32) -> i32 {
    unsafe { sys::deck_count(seat) }
}

/// Discard-pile size (all ids).
pub fn discard_size(seat: i32) -> i32 {
    unsafe { sys::discard_size(seat) }
}

/// C# `H.DiscardFromHand` -- drop one copy of `card` from hand into the discard
/// pile. True when the seat held one.
pub fn discard_from_hand(seat: i32, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::discard_from_hand(seat, p, l) != 0 }
}

/// C# `H.ShuffleAllIntoDeck` -- sweep hand + discard into the draw pile and
/// shuffle. Returns how many cards moved.
pub fn sweep_to_deck(seat: i32) -> i32 {
    unsafe { sys::sweep_to_deck(seat) }
}

// ------------------------------------------------------------ field cards

/// `H.PlaceFromPlay` -- this card stays in play at the seat.
pub fn place_card(seat: i32, card: &str, note: &Msg) {
    place_card_at(seat, card, note)
}

/// `H.PlaceCard` -- put a specific card (a derived one) into play at a seat.
pub fn place_card_at(seat: i32, card: &str, note: &Msg) {
    let (cp, cl) = s(card);
    let (p, l) = mj(note);
    unsafe { sys::place_card_at(seat, cp, cl, p, l) }
}

/// `PlayCtx.Dest` -- where this card goes afterwards (see [`Dest`]).
pub fn set_dest(dest: Dest) {
    unsafe { sys::set_dest(dest as i32) }
}

/// `H.RingMultiplier`.
pub fn ring_multiplier() -> i32 {
    unsafe { sys::ring_multiplier() }
}

/// `H._ringBonus += n`; returns the new multiplier.
pub fn add_ring_bonus(n: i32) -> i32 {
    unsafe { sys::add_ring_bonus(n) }
}

/// `H.ForceTeleport(..., resolve: false)` -- move a seat without settling.
pub fn teleport_to(seat: i32, tile: i32) {
    unsafe { sys::teleport_to(seat, tile) }
}

/// `H.Unplace` -- take this card out of play. True when it was there.
pub fn unplace_card(seat: i32) -> bool {
    unsafe { sys::unplace_card(seat) != 0 }
}

/// Is this card in play at the seat?
pub fn is_placed(seat: i32) -> bool {
    unsafe { sys::is_placed(seat) != 0 }
}

// --------------------------------------------------------- marks & tokens

/// `H.AddMark` -- a marker on a tile. `kind` names it (an i18n key), `note` explains it.
pub fn add_mark(tile: i32, seat: i32, kind: &str, note: &Msg) {
    let (kp, kl) = s(kind);
    let (np, nl) = mj(note);
    unsafe { sys::add_mark(tile, seat, kp, kl, np, nl) }
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

pub fn tok(seat: i32, name: &str) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::tok(seat, p, l) }
}

pub fn set_tok(seat: i32, name: &str, value: i32) {
    let (p, l) = s(name);
    unsafe { sys::set_tok(seat, p, l, value) }
}

/// `H.AddTok` -- returns how much the counter actually moved by.
pub fn add_tok(seat: i32, name: &str, by: i32, max: i32) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::add_tok(seat, p, l, by, max) }
}

// ------------------------------------------------------ per-seat slots (V)

pub fn slot(seat: i32, key: &str) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::slot(seat, p, l) }
}

pub fn set_slot(seat: i32, key: &str, value: i32) {
    let (p, l) = s(key);
    unsafe { sys::set_slot(seat, p, l, value) }
}

pub fn inc_slot(seat: i32, key: &str, by: i32) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::inc_slot(seat, p, l, by) }
}

// ----------------------------------------------------------- pots & status

pub fn band_crystals(seat: i32) -> i32 {
    unsafe { sys::band_crystals(seat) }
}

pub fn add_band_crystals(seat: i32, n: i32, max: i32) -> i32 {
    unsafe { sys::add_band_crystals(seat, n, max) }
}

pub fn fire(seat: i32) -> i32 {
    unsafe { sys::fire(seat) }
}

pub fn fire_max(seat: i32) -> i32 {
    unsafe { sys::fire_max(seat) }
}

/// `H.GainFire` -- fire pots, capped by the seat's own cap.
pub fn gain_fire(seat: i32, n: i32, why: &Msg) -> i32 {
    let (p, l) = mj(why);
    unsafe { sys::gain_fire(seat, n, p, l) }
}

pub fn give_stay(seat: i32, n: i32) {
    unsafe { sys::give_stay(seat, n) }
}

pub fn give_stun(seat: i32, n: i32) {
    unsafe { sys::give_stun(seat, n) }
}

/// `H.GiveExile(seat, layers, back_to)`.
pub fn give_exile(seat: i32, n: i32, to: i32) {
    unsafe { sys::give_exile(seat, n, to) }
}

/// `H.GiveExtraTurn`.
pub fn give_extra_turn(seat: i32) {
    unsafe { sys::give_extra_turn(seat) }
}

/// `H.CanPay` -- not out, not stunned, not exiled.
pub fn can_pay(seat: i32) -> bool {
    unsafe { sys::can_pay(seat) != 0 }
}

/// C# `H.SpendFire` -- spend `n` [火罐]; false when the seat has fewer. Logs
/// the spend (`why` is the reason message).
pub fn spend_fire(seat: i32, n: i32, why: &Msg) -> bool {
    let (p, l) = mj(why);
    unsafe { sys::spend_fire(seat, n, p, l) != 0 }
}

/// `[停留]` layers on the seat (C# `H.State.seats[s].stay`).
pub fn stay_of(seat: i32) -> i32 {
    unsafe { sys::stay_of(seat) }
}

/// `[晕眩]` layers on the seat (C# `H.State.seats[s].stun`).
pub fn stun_of(seat: i32) -> i32 {
    unsafe { sys::stun_of(seat) }
}

/// `H.State.turn` -- whose turn it is (-1 when none).
pub fn turn_seat() -> i32 {
    unsafe { sys::turn_seat() }
}

/// `H.State.round` -- the round counter.
pub fn round_no() -> i32 {
    unsafe { sys::round_no() }
}

/// C# `H.TurnKey` -- `round * 100 + turn + 1`: a per-turn id for once-per-turn
/// latches (`set_slot(seat, key, turn_key())` then compare).
pub fn turn_key() -> i32 {
    unsafe { sys::turn_key() }
}

/// Is the seat's character exactly `name`? (C# `H.CharacterOf`-style checks.)
pub fn character_is(seat: i32, name: &str) -> bool {
    let (p, l) = s(name);
    unsafe { sys::character_is(seat, p, l) != 0 }
}

/// C# `H.BandOf(seat) == name` -- is the seat's character in this band?
pub fn in_band(seat: i32, name: &str) -> bool {
    let (p, l) = s(name);
    unsafe { sys::in_band(seat, p, l) != 0 }
}

/// `PlayCtx.Dest` -- where a card goes when its effect finishes (C# fates
/// "discard" / "hand" / "placed" / "gone"). The port names them Graveyard /
/// Hand / Field / Banished; wire values 0..=3 are the C# order.
///
/// Planned: the draw-pile fates, one per insert position -- C# `c.Dest =
/// "deck"` -> `H.AddToDeck(seat, card, where)` with `where` = `"top"` /
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
}

// ------------------------------------------------------------- prompts

fn ask_raw(kind: PromptKind, seat: i32, title: &Msg, text: &Msg) -> i32 {
    let (tp, tl) = mj(title);
    let (xp, xl) = mj(text);
    unsafe { sys::ask(kind as i32, seat, tp, tl, xp, xl) }
}

/// `H.AskTileOf` -- returns the chosen **tile**, not the index.
/// Falls back to the first option if the answer is out of range.
pub fn ask_tile(seat: i32, title: &Msg, text: &Msg, tiles: &[i32]) -> i32 {
    for &t in tiles {
        unsafe { sys::opt_int(t) }
    }
    let i = ask_raw(PromptKind::Tile, seat, title, text);
    tiles[(i.max(0) as usize).min(tiles.len().saturating_sub(1))]
}

/// `H.AskPick` -- returns the chosen option index.
pub fn ask_pick(seat: i32, title: &Msg, text: &Msg, options: &[Msg]) -> usize {
    for o in options {
        let (p, l) = mj(o);
        unsafe { sys::opt_str(p, l) }
    }
    let i = ask_raw(PromptKind::Pick, seat, title, text);
    (i.max(0) as usize).min(options.len().saturating_sub(1))
}

/// `H.AskYes`.
pub fn ask_yes(seat: i32, title: &Msg, text: &Msg) -> bool {
    ask_raw(PromptKind::Yes, seat, title, text) == 0
}

/// `H.AskSeat` -- returns the chosen seat.
pub fn ask_seat(seat: i32, title: &Msg, text: &Msg, seats: &[i32]) -> i32 {
    for &x in seats {
        unsafe { sys::opt_int(x) }
    }
    let i = ask_raw(PromptKind::Seat, seat, title, text);
    seats[(i.max(0) as usize).min(seats.len().saturating_sub(1))]
}

/// `H.AskCard` -- pick one of `cards` (ids); returns the index.
pub fn ask_card(seat: i32, title: &Msg, text: &Msg, cards: &[&str]) -> usize {
    for c in cards {
        let (p, l) = s(c);
        unsafe { sys::opt_str(p, l) }
    }
    let i = ask_raw(PromptKind::Card, seat, title, text);
    (i.max(0) as usize).min(cards.len().saturating_sub(1))
}

/// `H.AskNumber` -- a number in `min..=max`. The C# builds this as an `AskPick`
/// over the range, so the option list is the faithful shape (ranges in the card
/// pool are small; for a wide range, narrow the candidates yourself first).
pub fn ask_number(seat: i32, title: &Msg, text: &Msg, min: i32, max: i32) -> i32 {
    let min = min.min(max);
    let max = max.max(min);
    let mut options: Vec<Msg> = Vec::new();
    for n in min..=max {
        options.push(Msg::new("ask.intOption").i("n", n as i64));
    }
    if options.is_empty() {
        return min;
    }
    let i = ask_pick(seat, title, text, &options);
    min + i as i32
}

/// Run another card's `play` effect right now (C# `NewCard` + `Play`).
pub fn play_card(id: &str, seat: i32) {
    let (p, l) = s(id);
    unsafe { sys::play_card(p, l, seat) }
}

/// The trigger a reaction is being checked against (C# `Trigger`).
pub mod trigger {
    use super::*;

    pub fn kind() -> TriggerKind {
        TriggerKind::from_i32(unsafe { sys::trig_kind() })
    }

    /// `t.Seat` -- whose action this is (the mover, the payer, the player of the card).
    pub fn seat() -> i32 {
        unsafe { sys::trig_seat() }
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

    /// `t.Move.Roll` -- the roll being reacted to, or -1.
    pub fn move_roll() -> Option<i32> {
        let v = unsafe { sys::trig_move_roll() };
        (v >= 0).then_some(v)
    }

    pub fn set_move_roll(v: i32) {
        unsafe { sys::trig_set_move_roll(v) }
    }

    /// `t.Card == id` / `t.Play.Id == id` -- is this trigger about that card?
    pub fn card_is(id: &str) -> bool {
        let (p, l) = s(id);
        unsafe { sys::trig_card_is(p, l) != 0 }
    }
}