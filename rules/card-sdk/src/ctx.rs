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

mod sys {
    // The link attribute is wasm-only; on other targets the imports stay
    // declared so the analyzer resolves `ctx::*` in card sources.
    #[cfg_attr(target_arch = "wasm32", link(wasm_import_module = "bandori"))]
    extern "C" {
        // dice & log
        pub fn roll(player_id: i32, count: i32, sides: i32) -> i32;
        pub fn log(player_id: i32, ptr: i32, len: i32);
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
        pub fn place_card_at(player_id: i32, cp: i32, cl: i32, ptr: i32, len: i32);
        pub fn set_dest(dest: i32);
        pub fn ring_multiplier() -> i32;
        pub fn add_ring_bonus(n: i32) -> i32;
        pub fn teleport_to(player_id: i32, tile: i32);
        pub fn unplace_card(player_id: i32) -> i32;
        pub fn is_placed(player_id: i32) -> i32;
        pub fn crystals(player_id: i32) -> i32;
        pub fn set_crystals(player_id: i32, n: i32) -> i32;
        pub fn add_crystals(player_id: i32, n: i32, max: i32) -> i32;
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
        pub fn ask(kind: i32, player_id: i32, title_ptr: i32, title_len: i32, text_ptr: i32, text_len: i32) -> i32;
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
        pub fn trig_cancelled() -> i32;
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
        // turn plan & scheduling
        pub fn schedule_turn_end(player_id: i32, mode: i32);
        pub fn set_no_money_loss(player_id: i32);
        pub fn set_fixed_roll(n: i32);
        pub fn fixed_roll() -> i32;
        pub fn set_next_steps(player_id: i32, n: i32);
        pub fn turn_main_steps() -> i32;
        pub fn add_fire_max(player_id: i32, n: i32) -> i32;
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
pub fn roll(player_id: i32, count: i32, sides: i32) -> i32 {
    unsafe { sys::roll(player_id, count, sides) }
}

/// `H.Log("text", seat, text)`.
pub fn log(player_id: i32, msg: &Msg) {
    let (p, l) = mj(msg);
    unsafe { sys::log(player_id, p, l) }
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
    (0..owned_count(player_id)).map(|i| owned_at(player_id, i)).collect()
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
    (0..players_on_count(tile, except)).map(|i| players_on_at(tile, except, i)).collect()
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
    (0..others_count(player_id)).map(|i| others_at(player_id, i)).collect()
}

pub fn money(player_id: i32) -> i32 {
    unsafe { sys::money(player_id) }
}

/// `H.GainR` -- money in, logged with its reason (`src` is a message key).
pub fn gain(player_id: i32, amount: i32, src: &Msg) -> i32 {
    let (p, l) = mj(src);
    unsafe { sys::gain(player_id, amount, p, l) }
}

/// `H.PayR` -- money out (what the player could pay), logged.
pub fn pay(player_id: i32, amount: i32, src: &Msg) -> i32 {
    let (p, l) = mj(src);
    unsafe { sys::pay(player_id, amount, p, l) }
}

/// Player-to-player money: the receiver gets exactly what the payer could pay.
/// The usual shape of `H.PayR` + `H.GainR` in a transfer.
pub fn transfer(from: i32, to: i32, amount: i32, src: &Msg) -> i32 {
    let got = pay(from, amount, src);
    gain(to, got, src);
    got
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
pub fn place_card(player_id: i32, card: &str, note: &Msg) {
    place_card_at(player_id, card, note)
}

/// `H.PlaceCard` -- put a specific card (a derived one) into play at a player.
pub fn place_card_at(player_id: i32, card: &str, note: &Msg) {
    let (cp, cl) = s(card);
    let (p, l) = mj(note);
    unsafe { sys::place_card_at(player_id, cp, cl, p, l) }
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

/// `H.ForceTeleport(..., resolve: false)` -- move a player without settling.
pub fn teleport_to(player_id: i32, tile: i32) {
    unsafe { sys::teleport_to(player_id, tile) }
}

/// `H.Unplace` -- take this card out of play. True when it was there.
pub fn unplace_card(player_id: i32) -> bool {
    unsafe { sys::unplace_card(player_id) != 0 }
}

/// Is this card in play at the player?
pub fn is_placed(player_id: i32) -> bool {
    unsafe { sys::is_placed(player_id) != 0 }
}

/// Miracle crystals on this card while it is in play at the player
/// (C# `Card.Crystals`).
pub fn crystals(player_id: i32) -> i32 {
    unsafe { sys::crystals(player_id) }
}

/// Set this card's crystals at the player (C# `Card.Crystals = n`); returns the
/// new count.
pub fn set_crystals(player_id: i32, n: i32) -> i32 {
    unsafe { sys::set_crystals(player_id, n) }
}

/// `H.AddCrystals` -- adjust this card's crystals at the player by `n`, clamped
/// at 0 and at `max` (`0` = uncapped); returns the new count.
pub fn add_crystals(player_id: i32, n: i32, max: i32) -> i32 {
    unsafe { sys::add_crystals(player_id, n, max) }
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

fn ask_raw(kind: PromptKind, player_id: i32, title: &Msg, text: &Msg) -> i32 {
    let (tp, tl) = mj(title);
    let (xp, xl) = mj(text);
    unsafe { sys::ask(kind as i32, player_id, tp, tl, xp, xl) }
}

/// `H.AskTileOf` -- returns the chosen **tile**, not the index.
/// Falls back to the first option if the answer is out of range.
pub fn ask_tile(player_id: i32, title: &Msg, text: &Msg, tiles: &[i32]) -> i32 {
    for &t in tiles {
        unsafe { sys::opt_int(t) }
    }
    let i = ask_raw(PromptKind::Tile, player_id, title, text);
    tiles[(i.max(0) as usize).min(tiles.len().saturating_sub(1))]
}

/// `H.AskPick` -- returns the chosen option index.
pub fn ask_pick(player_id: i32, title: &Msg, text: &Msg, options: &[Msg]) -> usize {
    for o in options {
        let (p, l) = mj(o);
        unsafe { sys::opt_str(p, l) }
    }
    let i = ask_raw(PromptKind::Choice, player_id, title, text);
    (i.max(0) as usize).min(options.len().saturating_sub(1))
}

/// `H.AskYes`.
pub fn ask_yes(player_id: i32, title: &Msg, text: &Msg) -> bool {
    ask_raw(PromptKind::YesNo, player_id, title, text) == 0
}

/// `H.AskSeat` -- returns the chosen player.
pub fn ask_player(player_id: i32, title: &Msg, text: &Msg, players: &[i32]) -> i32 {
    for &x in players {
        unsafe { sys::opt_int(x) }
    }
    let i = ask_raw(PromptKind::Player, player_id, title, text);
    players[(i.max(0) as usize).min(players.len().saturating_sub(1))]
}

/// `H.AskCard` -- pick one of `cards` (ids); returns the index.
pub fn ask_card(player_id: i32, title: &Msg, text: &Msg, cards: &[&str]) -> usize {
    for c in cards {
        let (p, l) = s(c);
        unsafe { sys::opt_str(p, l) }
    }
    let i = ask_raw(PromptKind::Card, player_id, title, text);
    (i.max(0) as usize).min(cards.len().saturating_sub(1))
}

/// `H.AskNumber` -- a number in `min..=max`. The C# builds this as an `AskPick`
/// over the range, so the option list is the faithful shape (ranges in the card
/// pool are small; for a wide range, narrow the candidates yourself first).
pub fn ask_number(player_id: i32, title: &Msg, text: &Msg, min: i32, max: i32) -> i32 {
    let min = min.min(max);
    let max = max.max(min);
    let mut options: Vec<Msg> = Vec::new();
    for n in min..=max {
        options.push(Msg::new("ask.intOption").i("n", n as i64));
    }
    if options.is_empty() {
        return min;
    }
    let i = ask_pick(player_id, title, text, &options);
    min + i as i32
}

/// Run another card's `play` effect right now (C# `NewCard` + `Play`).
/// `H.PlayCard` -- run another card's `play` inside this run, as that card.
/// Returns where it says it goes (its `Dest`); moving it there is the caller's
/// job, since only the caller knows where the card came from.
pub fn play_card(id: &str, player_id: i32) -> Dest {
    let (p, l) = s(id);
    Dest::from_i32(unsafe { sys::play_card(p, l, player_id) })
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

    /// C# `Bonus` -- add `n` to the roll; `why` is shown on the move's log line.
    pub fn set_bonus(n: i32, why: &Msg) {
        let (p, l) = mj(why);
        unsafe { sys::set_bonus(n, p, l) }
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

/// C# `DecayCard` tick -- burn one of this card's crystals at `player_id`; at 0 the
/// card leaves the field for its owner's discard pile (`H.Unplace(this,
/// "discard")`). Returns the crystals left (0 = it just decayed away).
pub fn decay(player_id: i32, id: &str) -> i32 {
    let left = add_crystals(player_id, -1, 0);
    if left == 0 {
        unplace_card(player_id);
        to_discard(player_id, id);
    }
    left
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

    /// `t.Step` -- the turn step (0/1/2/3) active when this trigger fired.
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

    /// Negate this trigger's effect outright (C# `trigger.Cancelled = true`).
    /// The engine then skips the effect body -- the landed tile does not
    /// resolve, the event does not run, the played card has no effect -- while
    /// the point's Before/After hooks still fire.
    pub fn set_cancelled() {
        unsafe { sys::trig_set_cancelled() }
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
}