//! Host functions available to card effects (the host surface a card body sees).
//!
//! Every piece of text a card shows (log lines, mark notes, prompts, options) is a
//! [`Msg`]: a key in the card crate's `locales/*.json` plus typed arguments. The
//! host never sees display text, and each player reads it in their own language.
//!
//! Ops that would need a player decision do not exist here on purpose: `ask_*`
//! blocks, the host publishes the question, and the run is replayed with the
//! answer -- so a card reads like straight-line code while remaining deterministic.

#[cfg(target_arch = "wasm32")]
use alloc::{string::String, vec::Vec};

pub use crate::abi::{
    AbKind, CardPile, DistanceKind, OutCause, PlayOutcome, Zone, ZoneChange, ZoneChangeCause,
};
use crate::abi::{MoveKind, PromptKind, TriggerKind};
use crate::msg::Msg;
use crate::Prompt;

/// Interned name ids returned by the identity-list seams ([`cards_in`],
/// [`held_names`], …). Re-exported here so a body that names the item type can
/// say `use card_sdk::ctx::NameId` without reaching into `intern`.
/// [`intern`] is re-exported alongside it for pushing literals into a
/// `Vec<NameId>` (`out.push(intern("PP:…"))`).
pub use crate::intern::{intern, NameId};

pub mod sys;

#[cfg(target_arch = "wasm32")]
#[cfg_attr(kani, inline(always))]
fn s(v: &str) -> (i32, i32) {
    in_bytes(v.as_bytes())
}

/// ABI handle for a **name** the host looks up by identity.
///
/// Under `cfg(kani)` this is `intern::encode_handle(intern(v))` — the handle
/// *is* the interned `NameId`, so the host never re-hashes or heap-copies the
/// bytes. Card bodies keep calling `ctx::held_count(p, "literal")` unchanged; the
/// literal's FNV hash constant-folds through the zero-loop `intern::fnv1a`.
/// Production wasm / native keep the real `ptr, len` arena ABI.
#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(kani, inline(always))]
fn s(v: &str) -> (i32, i32) {
    #[cfg(kani)]
    {
        let id = crate::intern::intern(v);
        #[cfg(kani)]
        kani::assert(
            id.is_valid(),
            "name intern table exhausted or name too long",
        );
        (crate::intern::encode_handle(id), v.len() as i32)
    }
    #[cfg(not(kani))]
    {
        in_bytes(v.as_bytes())
    }
}

/// The ABI handle for an **input** buffer the host reads during the call.
///
/// On wasm32 the handle *is* the linear-memory pointer. On a native build a
/// 64-bit pointer does not fit in the `i32` wire form, so the bytes are
/// interned in the per-call arena and the handle is the arena index
/// ([`crate::native`]). The bytes must outlive the call -- a `&[u8]` borrowed
/// from a local is fine because the host copies them out before returning.
fn in_bytes(v: &[u8]) -> (i32, i32) {
    #[cfg(target_arch = "wasm32")]
    {
        (v.as_ptr() as i32, v.len() as i32)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::native::intern(v)
    }
}

/// The i32 wire form of a message: `postcard` bytes leaked for the host to read
/// (same lifetime convention as every other buffer crossing the ABI).
#[cfg(all(target_arch = "wasm32", not(kani)))]
fn msg_bytes(m: &Msg) -> (i32, i32) {
    let b = m.to_bytes();
    let out = (b.as_ptr() as i32, b.len() as i32);
    core::mem::forget(b);
    out
}

/// Kani: the host model records narration as a counter only (the book's frame
/// is about game effects). Hand it the interned-key handle + arg count — two
/// ints, no `Vec<u8>`, no arena intern (which would push a heap `Vec` per log
/// and unwind as `drop_glue`). Production builds keep the real bytes.
#[cfg(kani)]
fn msg_bytes(m: &Msg) -> (i32, i32) {
    m.abi_pair()
}

#[cfg(all(not(target_arch = "wasm32"), not(kani)))]
fn msg_bytes(m: &Msg) -> (i32, i32) {
    in_bytes(&m.to_bytes())
}

/// Reserve an **out-buffer** the host writes into during the call.
///
/// Returns `(handle, scratch)`. On wasm the handle is the scratch's
/// linear-memory pointer (the host writes straight into it) and `scratch`
/// **is** the buffer [`out_read`] hands to the decoder. On native / kani the
/// handle is an arena slot the host writes through [`crate::native::write`];
/// `scratch` is empty and [`out_read`] moves the slot's bytes out (native) or
/// copies them under the kani unwind bounds.
fn out_buf(cap: usize) -> (i32, alloc::vec::Vec<u8>) {
    #[cfg(kani)]
    {
        // Fixed arena slot — never `vec![0u8; cap]` (`extend_with` cliff when
        // `cap` is a host return). `out_read` ignores the scratch and pulls
        // the bytes from the arena, but it still gets dropped: a cap-0 `Vec`
        // drops a dangling pointer through the Kani 0.68 niche-`Cap`
        // read-back (false positive, see `owned_tiles`), so keep a concrete
        // capacity (64 = `native::arena::MAX_BUF`).
        let (h, _l) = crate::native::reserve(cap);
        (h, alloc::vec::Vec::with_capacity(64))
    }
    #[cfg(not(kani))]
    #[cfg(target_arch = "wasm32")]
    {
        let mut buf = alloc::vec![0u8; cap];
        (buf.as_mut_ptr() as i32, buf)
    }
    #[cfg(not(kani))]
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Arena slot is the real buffer (the host writes there via the handle).
        // No dummy of the same length -- `out_read` moves the slot out, so the
        // decoder walks that one allocation in place. See [`crate::native::take`].
        let (h, _l) = crate::native::reserve(cap);
        (h, alloc::vec::Vec::new())
    }
}

/// Read back an out-buffer the host just wrote (`n` = the host's return).
///
/// Returns the single buffer the decoder walks -- no second `Vec` copy.
/// On wasm that is `scratch` itself (the host wrote into it via the pointer);
/// on native it is the reserved slot moved out of the arena
/// ([`crate::native::take`]); on kani it is the bounded-copy result of
/// [`crate::native::read`] (the fixed arena cannot hand out a borrow, and a
/// `vec![0u8; n]` scratch would hit the unwind cliff).
fn out_read(p: i32, n: i32, scratch: alloc::vec::Vec<u8>) -> alloc::vec::Vec<u8> {
    #[cfg(kani)]
    {
        let _ = scratch;
        crate::native::read(p, n.max(0) as usize)
    }
    #[cfg(not(kani))]
    #[cfg(target_arch = "wasm32")]
    {
        let _ = p;
        scratch
    }
    #[cfg(not(kani))]
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = scratch;
        crate::native::take(p, n.max(0) as usize)
    }
}

/// Raw ABI helpers for the game sugar crate (`bandori-std`). Card bodies use
/// the typed [`super`] wrappers; `bandori-std` implements this ruleset's
/// readable names on top of these plus the generic [`tile_is`] family.
pub mod raw {
    pub use super::sys;
    use super::*;
    pub fn s(v: &str) -> (i32, i32) {
        super::s(v)
    }
    pub fn msg_bytes(m: &Msg) -> (i32, i32) {
        super::msg_bytes(m)
    }
    pub fn asked(i: i32) -> Result<i32, Prompt> {
        super::asked(i)
    }
    pub fn out_buf(cap: usize) -> (i32, alloc::vec::Vec<u8>) {
        super::out_buf(cap)
    }
    pub fn out_read(p: i32, n: i32, scratch: alloc::vec::Vec<u8>) -> alloc::vec::Vec<u8> {
        super::out_read(p, n, scratch)
    }
    pub fn in_bytes(v: &[u8]) -> (i32, i32) {
        super::in_bytes(v)
    }
}

// ------------------------------------------------------------- dice & log

/// Roll, raising the 「掷骰结算前」 [反击] window on the face (the default).
/// `source` is a [`crate::abi::roll_source`] code (NONE/CHARGE/CARD/SKILL).
/// Always logs a `"dice"` event (the roll animation).
/// Returns the face a counteraction left; `Err(Prompt)` pauses the run.
///
/// [`RollPurpose::Effect`] rolls `count`d`sides`; [`RollPurpose::Move`] sums
/// the move plan's dice tables and ignores `count`/`sides`.
pub fn roll(
    player_id: i32,
    count: i32,
    sides: i32,
    purpose: crate::abi::RollPurpose,
    source: i32,
) -> Result<i32, Prompt> {
    crate::pure::refuse_if_filter("ctx::roll");
    asked(unsafe { sys::roll(player_id, count, sides, purpose as i32, source) })
}

/// [`roll`] without the [反击] window — the face is final as cast.
/// Still logs a `"dice"` event (all rolls animate).
pub fn roll_direct(
    player_id: i32,
    count: i32,
    sides: i32,
    purpose: crate::abi::RollPurpose,
    source: i32,
) -> Result<i32, Prompt> {
    crate::pure::refuse_if_filter("ctx::roll_direct");
    asked(unsafe { sys::roll_direct(player_id, count, sides, purpose as i32, source) })
}

/// Log a message line for `player_id`.
pub fn log(player_id: i32, msg: &Msg) {
    let (p, l) = msg_bytes(msg);
    unsafe { sys::log(player_id, p, l) }
}

/// Announce which effect was applied. Reaches the player as a
/// popup as well as a log line; use this where the card has just picked a
/// branch (e.g. a 1d10 that names the branch taken) and the player should see
/// which. Additive with [`log`], not a replacement for it.
pub fn effect(player_id: i32, msg: &Msg) {
    let (p, l) = msg_bytes(msg);
    unsafe { sys::effect(player_id, p, l) }
}

// ---------------------------------------------------------------- flash

/// Request the card **flash** for THIS activation at this point (overrides the
/// `Auto` scope classification; a no-op under `Flash::Never`). The lazy
/// 「效果适用」 header is posted first -- a flash is a visible outcome and the
/// header names which clause is firing. A `QuietWhenEmpty` entry keeps its
/// header once a flash was requested.
///
/// Once-per-flash: identified by `(drive, flash ordinal within the drive)`.
/// A body re-run after a prompt must not flash the same ordinal again; the
/// already-posted flash remains shown and is not dropped on halt.
pub fn flash() {
    unsafe { sys::flash() }
}

/// Suppress the card flash for THIS activation (decided at runtime). Wins over
/// [`flash`] / `Flash::Always`, loses to `Flash::Never`.
pub fn no_flash() {
    unsafe { sys::no_flash() }
}

/// Proactively flash another card as a "look here" cue -- its own `"flash"`
/// event in the stream, parent-linked under the activation. Usable by pseudo
/// cards (tiles, `mark:*`). Identified by the next flash ordinal within the
/// drive; use [`flash_card_keyed`] when the position can shift between runs.
pub fn flash_card(card_id: &str, msg: &Msg) {
    let (cp, cl) = s(card_id);
    let (p, l) = msg_bytes(msg);
    unsafe { sys::flash_card(cp, cl, p, l) }
}

/// Like [`flash_card`] but with an explicit identity key: the same `key` in one
/// drive flashes once, even if its position shifts between re-runs.
pub fn flash_card_keyed(key: &str, card_id: &str, msg: &Msg) {
    let (kp, kl) = s(key);
    let (cp, cl) = s(card_id);
    let (p, l) = msg_bytes(msg);
    unsafe { sys::flash_card_keyed(kp, kl, cp, cl, p, l) }
}

/// Proactively flash a tile as a "look here" cue -- its own `"flash"` event,
/// parent-linked under the activation. Identified by the next flash ordinal.
pub fn flash_tile(tile: i32, msg: &Msg) {
    let (p, l) = msg_bytes(msg);
    unsafe { sys::flash_tile(tile, p, l) }
}

// ------------------------------------------------------------------ board

#[cfg_attr(kani, inline(always))]
pub fn tile_count() -> i32 {
    unsafe { sys::tile_count() }
}

/// Tile index of a name (a data key), or -1.
#[cfg_attr(kani, inline(always))]
pub fn tile_named(name: &str) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::tile_named(p, l) }
}

pub fn tile_owner(tile: i32) -> i32 {
    unsafe { sys::tile_owner(tile) }
}

/// The tile standing at board position `position` (`position` itself when
/// nothing is bound there).
pub fn tile_at(position: i32) -> i32 {
    unsafe { sys::tile_at(position) }
}

/// The board position tile `tile` currently stands at (`tile` itself when
/// nothing is bound to it).
pub fn position_of_tile(tile: i32) -> i32 {
    unsafe { sys::position_of_tile(tile) }
}

/// How many *extra* instances (non-`tile:` cards) lie on tile `tile`. The
/// tile carries its extras, so the count is identity-keyed.
pub fn tile_extra_count(tile: i32) -> i32 {
    unsafe { sys::tile_extra_count(tile) }
}

/// The board position **this instance** sits at (its placement slot).
/// [`self_tile`] is the tile standing there.
pub fn self_position() -> i32 {
    unsafe { sys::self_position() }
}

/// The player's tile on the ring.
pub fn player_pos(player_id: i32) -> i32 {
    unsafe { sys::player_pos(player_id) }
}

/// Tile `steps` ahead of a player around the ring.
pub fn tile_steps_ahead(player_id: i32, steps: i32) -> i32 {
    unsafe { sys::tile_steps_ahead(player_id, steps) }
}

/// Rent charged when landing on `tile` at its current build level.
pub fn rent_of(tile: i32) -> i32 {
    unsafe { sys::rent_of(tile) }
}

/// Price to purchase unowned `tile`.
pub fn buy_price(tile: i32) -> i32 {
    unsafe { sys::buy_price(tile) }
}

/// Cost to build the next house/hotel on `tile`.
pub fn build_cost(tile: i32) -> i32 {
    unsafe { sys::build_cost(tile) }
}

/// Cash received for mortgaging `tile`.
pub fn mortgage_value(tile: i32) -> i32 {
    unsafe { sys::mortgage_value(tile) }
}

/// Number of deeds held by `player_id`.
pub fn owned_count(player_id: i32) -> i32 {
    unsafe { sys::owned_count(player_id) }
}

/// The `index`-th deed of `player_id` as a tile index (see [`owned_tiles`]).
pub fn owned_at(player_id: i32, index: i32) -> i32 {
    unsafe { sys::owned_at(player_id, index) }
}

/// The player's deeds as tile indices.
#[cfg(kani)]
pub fn owned_tiles(player_id: i32) -> Vec<i32> {
    // Bounded loop instead of a symbolic-length Range fold (the `collect`
    // → `extend_trusted` path is an unwinding cliff at 32). Same result.
    let n = owned_count(player_id).max(0).min(8) as usize;
    // Concrete capacity, not `with_capacity(n)`: `n` crosses the host
    // function-pointer call, so CBMC keeps it symbolic and the `__rust_dealloc`
    // layout check cannot relate the allocation size to the drop (timeout or
    // spurious FAIL). A fixed ≤8 capacity makes both sides constant.
    let mut v = Vec::with_capacity(8);
    let mut i = 0usize;
    while i < n {
        v.push(owned_at(player_id, i as i32));
        i += 1;
    }
    v
}

#[cfg(not(kani))]
pub fn owned_tiles(player_id: i32) -> Vec<i32> {
    (0..owned_count(player_id))
        .map(|i| owned_at(player_id, i))
        .collect()
}

/// Does the tile's **data kind** equal `kind` (the `TileKind` wire spelling:
/// `"ring"`, `"circle"`, `"agent"`, `"property"`, …)? Generic host query —
/// game sugar (`is_deed` / `is_start` / …) lives in `bandori-std`.
#[cfg_attr(kani, inline(always))]
pub fn tile_is(tile: i32, kind: &str) -> bool {
    let (p, l) = s(kind);
    unsafe { sys::tile_is(tile, p, l) != 0 }
}

/// Generic tile field read (`"group"` / `"area"` / `"tier"` / `"price"` /
/// `"house"` / `"rent"` / `"index"`). See ABI v56. Game named-tile helpers
/// live in `bandori-std`.
#[cfg_attr(kani, inline(always))]
pub fn tile_attr(tile: i32, attr: &str) -> i32 {
    let (p, l) = s(attr);
    unsafe { sys::tile_attr(tile, p, l) }
}

/// Is `tile` in the ruleset **class** `class` for `player_id` (`"buyable"` /
/// `"shop"` / `"livehouse"` / `"color:<g>"`)? Per-player overrides are generic
/// class-membership props. Game sugar lives in `bandori-std`.
#[cfg_attr(kani, inline(always))]
pub fn tile_in(player_id: i32, tile: i32, class: &str) -> bool {
    let (p, l) = s(class);
    unsafe { sys::tile_in(player_id, tile, p, l) != 0 }
}

/// The colour group (-1 for no tile).
pub fn tile_group(tile: i32) -> i32 {
    unsafe { sys::tile_group(tile) }
}

/// The land price alone (houses are extra; cf. [`buy_price`]).
pub fn tile_price(tile: i32) -> i32 {
    unsafe { sys::tile_price(tile) }
}

/// Houses standing on the tile.
pub fn houses_of(tile: i32) -> i32 {
    unsafe { sys::houses_of(tile) }
}

/// The house count a **rent** lookup reads -- the counted
/// value, which a 「房屋数视为…」 override may lift above [`houses_of`].
/// Real houses are untouched; build caps, raze and sale still see
/// [`houses_of`]. 「X为你收费格上的房屋数」 reads this.
pub fn rent_houses_of(tile: i32) -> i32 {
    unsafe { sys::rent_houses_of(tile) }
}

/// Set the tile's house count directly (house transfer effects).
pub fn set_houses(tile: i32, n: i32) {
    unsafe { sys::set_houses(tile, n) }
}

/// Move the house count by `n`; returns the new count.
pub fn add_house(tile: i32, n: i32) -> i32 {
    unsafe { sys::add_house(tile, n) }
}

/// Is the deed mortgaged?
pub fn mortgaged_of(tile: i32) -> bool {
    unsafe { sys::mortgaged_of(tile) != 0 }
}

/// Mortgage or redeem a deed outright (transfer effects; no money moves).
pub fn set_mortgaged(tile: i32, v: bool) {
    unsafe { sys::set_mortgaged(tile, v as i32) }
}

/// Hand a deed to another player outright (deed transfer effects). The caller
/// logs the transfer.
pub fn set_owner(tile: i32, player_id: i32) {
    unsafe { sys::set_owner(tile, player_id) }
}

/// Undirected ring distance between two tiles.
pub fn dist(a: i32, b: i32) -> i32 {
    unsafe { sys::dist(a, b) }
}

/// Steps forward from `a` to `b` around the ring.
pub fn tile_forward(a: i32, b: i32) -> i32 {
    unsafe { sys::tile_forward(a, b) }
}

/// The next present player in turn order (`dir` ±1), or -1.
pub fn neighbor(player_id: i32, dir: i32) -> i32 {
    unsafe { sys::neighbor(player_id, dir) }
}

/// Present players standing on a tile.
pub fn players_on(tile: i32, except: i32) -> Vec<i32> {
    // Concrete cap, same host-call reason as `others`.
    let n = players_on_count(tile, except);
    let mut v = Vec::with_capacity(4);
    let mut i = 0;
    while i < n {
        v.push(players_on_at(tile, except, i));
        i += 1;
    }
    v
}

fn players_on_count(tile: i32, except: i32) -> i32 {
    unsafe { sys::players_on_count(tile, except) }
}

fn players_on_at(tile: i32, except: i32, index: i32) -> i32 {
    unsafe { sys::players_on_at(tile, except, index) }
}

// ------------------------------------------------------------------ players

/// Model bound N ∈ 2..=4.
///
/// Under Kani this returns the **constant 4** so the unmodified card bodies'
/// `(0..ctx::player_count())` Range is a literal `0..4` that CBMC can finish
/// in 4 unrolls. A table / if-else clamp on the real `n` was not enough:
/// CBMC does not connect a function return's value range to `Range::try_fold`
/// (poorest unwound 24× and hit the limit).
///
/// Soundness: seats `>= N` must read as **out**, so `poorest`'s
/// `p == player_id || player_out(p) || …` short-circuits before looking at
/// `money_of(p)`. `player_out` does that (see below). The real `n` is still
/// asserted to be in 2..=4 — a violation is loud.
#[cfg_attr(kani, inline(always))]
pub fn player_count() -> i32 {
    let n = unsafe { sys::player_count() };
    #[cfg(kani)]
    {
        kani::assert(n >= 2 && n <= 4, "player_count outside model bound N∈2..=4");
        return 4;
    }
    #[cfg(not(kani))]
    n
}

/// 1 when the player is out of the game (or not a seat at all).
///
/// Under Kani `player_count` returns the constant 4, so `0..4` loops visit
/// seats `>= N`. Those must read as out — `poorest` short-circuits on this
/// before `money_of(p)`.
#[cfg_attr(kani, inline(always))]
pub fn player_out(player_id: i32) -> bool {
    #[cfg(kani)]
    {
        let n = unsafe { sys::player_count() };
        if player_id < 0 || player_id >= n {
            return true;
        }
    }
    unsafe { sys::player_out(player_id) != 0 }
}

pub fn others_count(player_id: i32) -> i32 {
    unsafe { sys::others_count(player_id) }
}

pub fn others_at(player_id: i32, index: i32) -> i32 {
    unsafe { sys::others_at(player_id, index) }
}

/// The other players still in the game.
pub fn others(player_id: i32) -> Vec<i32> {
    // Concrete cap: `others_count` crosses a host call, so `collect` would
    // size the Vec symbolically and its drop trips the Kani 0.68
    // `__rust_dealloc` layout check (see `owned_tiles`). A match has at most
    // four seats.
    let n = others_count(player_id);
    let mut v = Vec::with_capacity(4);
    let mut i = 0;
    while i < n {
        v.push(others_at(player_id, i));
        i += 1;
    }
    v
}

/// The player's current cash.
pub fn money_of(player_id: i32) -> i32 {
    unsafe { sys::money(player_id) }
}

/// Money in, logged with its reason (`src` is a message key).
/// Runs the same `Money` pipeline as [`pay`] (print, game -> player), so
/// `paySplitAdd` / `paySplitChoose` and the `effect` [反击] window see it; the answer is
/// the amount that actually moved (0 = cancelled).
#[cfg_attr(kani, inline(always))]
pub fn gain(player_id: i32, amount: i32, src: &Msg) -> Result<i32, Prompt> {
    crate::pure::refuse_if_filter("ctx::gain");
    let (p, l) = msg_bytes(src);
    asked(unsafe { sys::gain(player_id, amount, p, l) })
}

/// A bank gain whose log line is `text` (a full `Pay::text`) and whose money
/// event type is `typ` (the `Pay`'s event `typ`, e.g. `"pass"`). Runs the same
/// `Money` pipeline as [`gain`], as `Pay::new(amount, "gain")` so the 「支付」
/// scalars (`scale_settle_payment`) do not reach a print.
#[cfg_attr(kani, inline(always))]
pub fn gain_typed(player_id: i32, amount: i32, typ: &str, text: &Msg) -> Result<i32, Prompt> {
    let (tp, tl) = s(typ);
    let (xp, xl) = msg_bytes(text);
    asked(unsafe { sys::gain_typed(player_id, amount, tp, tl, xp, xl) })
}

/// A **raw** money adjustment -- `money += delta` with a `log.gain` /
/// `log.lose` line and **no** payment pipeline: no `paySplitAdd` / `paySplitMul` /
/// `paySplitChoose` / `paySplitAt`, no `effect` [反击] window, no `payAfter`. 规则书
/// 「不受其他任何效果影响」 -- the amount moves as-is (ONE OF US's 「被分享方
/// 资金直接增加」). Positive `delta` is a gain, negative a loss.
///
/// A gain is non-pausing and applies raw. A loss pauses and raises the
/// `spendBefore` counteraction window at the full cost before the debit
/// (G02 §2.21); a responder reduces it with `set_pay_amount`, and a cancel
/// leaves the cost in place. Returns the amount actually moved (0 = the seat
/// was out); `Err(Prompt)` pauses the run on the loss window.
#[cfg_attr(kani, inline(always))]
pub fn adjust_money(player_id: i32, delta: i32, src: &Msg) -> Result<i32, Prompt> {
    let (p, l) = msg_bytes(src);
    asked(unsafe { sys::adjust_money(player_id, delta, p, l) })
}

/// [`adjust_money`] with a positive `amount` -- a raw gain. Convenience.
#[cfg_attr(kani, inline(always))]
pub fn gain_raw(player_id: i32, amount: i32, src: &Msg) -> Result<i32, Prompt> {
    adjust_money(player_id, amount.max(0), src)
}

/// [`adjust_money`] with a positive `amount` taken away -- a raw loss. It
/// pauses on the `spendBefore` window like [`adjust_money`] does for a
/// negative `delta`.
#[cfg_attr(kani, inline(always))]
pub fn lose_raw(player_id: i32, amount: i32, src: &Msg) -> Result<i32, Prompt> {
    adjust_money(player_id, -amount.max(0), src)
}

/// Money out (what the player could pay), logged.
pub fn pay(player_id: i32, amount: i32, src: &Msg) -> Result<i32, Prompt> {
    PayCommand::new(player_id, -1, amount, src)
        .send()
        .map(|r| r.amount())
}

/// A payment command: the `pay` / `transfer` verb with its context (shape,
/// kind, flags, tile) packed into the host call. `send` pauses for the
/// windows and answers a [`PayReceipt`] with the amount actually moved.
///
/// `from < 0` = print (the bank pays), `to < 0` = delete (the bank is paid);
/// both `>= 0` = a transfer.
pub struct PayCommand {
    from: i32,
    to: i32,
    amount: i32,
    kind: crate::abi::PayKind,
    shape: crate::abi::PayShape,
    flags: i32,
    tile: i32,
    src: Msg,
}

impl PayCommand {
    /// A command with today's defaults: `MUST` and the command-wide pre-split
    /// stage, kind `Card`, shape `Normal`.
    pub fn new(from: i32, to: i32, amount: i32, src: &Msg) -> Self {
        Self {
            from,
            to,
            amount,
            kind: crate::abi::PayKind::Card,
            shape: crate::abi::PayShape::Normal,
            flags: crate::abi::pay_flag::MUST | crate::abi::pay_flag::TOTAL_STAGE,
            tile: -1,
            src: src.clone(),
        }
    }

    pub fn kind(mut self, kind: crate::abi::PayKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn shape(mut self, shape: crate::abi::PayShape) -> Self {
        self.shape = shape;
        self
    }

    /// An immutable leg: no responder can change its figure or payee and no
    /// factor applies.
    pub fn fixed(mut self) -> Self {
        self.shape = crate::abi::PayShape::Fixed;
        self
    }

    /// The declared payee pays half of what the payer lost, as a fresh fixed
    /// child leg after this one settles.
    pub fn payee_half(mut self) -> Self {
        self.shape = crate::abi::PayShape::PayeeHalf;
        self
    }

    pub fn tile(mut self, tile: i32) -> Self {
        self.tile = tile;
        self
    }

    /// Optional: no raise-funds / bankruptcy, but a cancellable mortgage offer.
    pub fn optional(mut self) -> Self {
        self.flags &= !crate::abi::pay_flag::MUST;
        self.flags |= crate::abi::pay_flag::ALLOW_MORTGAGE;
        self
    }

    pub fn total_stage(mut self, on: bool) -> Self {
        if on {
            self.flags |= crate::abi::pay_flag::TOTAL_STAGE;
        } else {
            self.flags &= !crate::abi::pay_flag::TOTAL_STAGE;
        }
        self
    }

    /// Run the command. Load-bearing: it pauses for the pay windows.
    pub fn send(self) -> Result<PayReceipt, Prompt> {
        let mut spec = (self.shape.as_i32() & 0xF)
            | ((self.kind.as_i32() & 0xF) << 4)
            | ((self.flags & 0xF) << 8);
        if self.tile >= 0 {
            spec |= (self.tile + 1) << 12;
        }
        let (p, l) = msg_bytes(&self.src);
        asked(unsafe { sys::pay_command(self.from, self.to, self.amount, spec, p, l) })
            .map(PayReceipt)
    }
}

/// What a [`PayCommand`] actually did, packed.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PayReceipt(pub i32);

impl PayReceipt {
    /// The amount actually moved (0 for a cancel, a shortfall or a locked
    /// zero commit).
    pub fn amount(self) -> i32 {
        self.0 & crate::abi::pay_receipt::AMOUNT_MASK
    }

    /// The payment committed (a locked zero included).
    pub fn settled(self) -> bool {
        self.0 & crate::abi::pay_receipt::SETTLED != 0
    }

    /// Nothing moved for a no-settle reason (immunity, gate, zero, cancel,
    /// payer off-board).
    pub fn cancelled(self) -> bool {
        self.0 & crate::abi::pay_receipt::CANCELLED != 0
    }

    /// A mandatory payment could not be completed (the payer went out).
    pub fn shortfall(self) -> bool {
        self.0 & crate::abi::pay_receipt::SHORTFALL != 0
    }

    /// The command was a fixed leg.
    pub fn fixed(self) -> bool {
        self.0 & crate::abi::pay_receipt::FIXED != 0
    }
}

/// Player-to-player money -- one pipeline entry (`pay_to`), so the `effect`
/// declaration names both the payer and the payee and 「向其他玩家支付」 sees it.
/// The receiver gets exactly what the payer could pay.
pub fn transfer(from: i32, to: i32, amount: i32, src: &Msg) -> Result<i32, Prompt> {
    PayCommand::new(from, to, amount, src)
        .send()
        .map(|r| r.amount())
}

/// A rent-shaped 「收费」 at `milli_scale` (1000 = ×1.0) of `tile`'s
/// ordinary-settle rent, outside a full [触发结算].
///
/// 规则书 (`RAS:练习室里的风暴` (2)): 「对那个玩家进行一次相当于此卡所在格子
/// 普通[结算]的(4-X)/4倍价格的收费」 -- pass `250 * (4 - x)` as `milli_scale`.
/// The figure is the same one `pay_rent` charges (table / houses / dice-rent
/// roll) and the scale rides the money pipeline's `paySplitMul` stage
/// (`scale_settle_payment`) with the rent factors. Runs as `PayKind::Rent`
/// (so `PaySplitAdd` / `pay_is_rent` see a rent), but it is a 「收费」, not a
/// 「结算」: no settle hooks, no buy/build offer.
///
/// Returns the amount actually paid. Refuses (0) when the tile is unowned or
/// mortgaged (the mortgaged case is `TODO(规则书)[ruling]` -- 「普通[结算]」
/// there is a forced-buy offer with no obvious 收费 recipient).
pub fn charge_rent(payer: i32, tile: i32, milli_scale: i32) -> Result<i32, Prompt> {
    asked(unsafe { sys::charge_rent(payer, tile, milli_scale) })
}

// ------------------------------------------------------------- 「[分摊]」 / 分摊前

/// The command-wide **pre-split** stage (`PIPELINE-AUDIT` Q2): `payCombinedAdd` →
/// `payCombinedMul` → `payCombinedCancel` shape `total` **before** any 「[分摊]」
/// divides it into shares. This is the 「分摊前」 figure of
/// 「此次支付的分摊前资金减少Y×100」 and kin (规则书 支付阶段 2/4/5 applied to
/// the command rather than to one pair).
///
/// Returns the shaped total, or `None` when a `payCombinedCancel` hook dropped the
/// whole command. A single-pair payment runs the same three stages inside
/// [`transfer`] / [`pay`] / [`gain`] -- call this only when *you* are about to
/// split, and follow it with [`pay_leg`] per share (see [`split_pay`]).
pub fn pay_total(from: i32, to: i32, total: i32, src: &Msg) -> Result<Option<i32>, Prompt> {
    let (p, l) = msg_bytes(src);
    let shaped = asked(unsafe { sys::pay_total(from, to, total, p, l) })?;
    Ok((shaped >= 0).then_some(shaped))
}

/// One 「[分摊]」 leg: the same payment pipeline as [`transfer`] minus the
/// command-wide pre-split stage (the body already ran [`pay_total`]).
/// `from < 0` / `to < 0` name the bank side.
pub fn pay_leg(from: i32, to: i32, amount: i32, src: &Msg) -> Result<i32, Prompt> {
    PayCommand::new(from, to, amount, src)
        .total_stage(false)
        .send()
        .map(|r| r.amount())
}

/// 「[分摊][支付]」: shape `total` with the command-wide
/// pre-split stage, then charge each of `payers` `ceil10(ceil(total/n))` to
/// `to` (the 规则书 「向上取整10」 share). Returns the per-leg amounts, or an
/// empty list when the command was cancelled.
///
/// The legs run [`pay_leg`], so a `payCombinedAdd` hook hits the **total** once
/// rather than each share (`PIPELINE-AUDIT` Q2 / `rb_money::
/// pre_split_modifier_shapes_the_total_not_each_leg`).
pub fn split_pay(payers: &[i32], to: i32, total: i32, src: &Msg) -> Result<Vec<i32>, Prompt> {
    #[cfg(kani)]
    {
        // Zero-loop filter + leg walk (a `filter().collect()` + `for p in list`
        // over a symbolic-length slice is the extend_with / drop_glue cliff).
        let mut list: [i32; 4] = [0; 4];
        let mut n = 0usize;
        macro_rules! push {
            ($i:expr) => {
                if n < 4 {
                    if let Some(&p) = payers.get($i) {
                        if p != to && !player_out(p) {
                            list[n] = p;
                            n += 1;
                        }
                    }
                }
            };
        }
        push!(0);
        push!(1);
        push!(2);
        push!(3);
        if payers.len() > 4 {
            kani::assert(false, "split_pay: more than 4 payers under Kani");
        }
        // Concrete cap on every returned `Vec` (including the empty ones):
        // the caller drops it, and a cap-0 `Vec` is dropped through the
        // Kani 0.68 niche-`Cap` read-back (false positive, see
        // `owned_tiles`).
        if n == 0 || total <= 0 {
            return Ok(Vec::with_capacity(4));
        }
        let Some(shaped) = pay_total(list[0], to, total, src)? else {
            return Ok(Vec::with_capacity(4));
        };
        let ln = n as i32;
        let per = (shaped + ln - 1) / ln;
        let share = (per + 9) / 10 * 10;
        let mut moved = Vec::with_capacity(4);
        macro_rules! leg {
            ($i:expr) => {
                if $i < n {
                    moved.push(pay_leg(list[$i], to, share, src)?);
                }
            };
        }
        leg!(0);
        leg!(1);
        leg!(2);
        leg!(3);
        return Ok(moved);
    }
    #[cfg(not(kani))]
    {
        let list: Vec<i32> = payers
            .iter()
            .copied()
            .filter(|&p| p != to && !player_out(p))
            .collect();
        if list.is_empty() || total <= 0 {
            return Ok(Vec::new());
        }
        let Some(shaped) = pay_total(list[0], to, total, src)? else {
            return Ok(Vec::new());
        };
        let n = list.len() as i32;
        let per = (shaped + n - 1) / n;
        let share = (per + 9) / 10 * 10;
        let mut moved = Vec::with_capacity(list.len());
        for p in list {
            moved.push(pay_leg(p, to, share, src)?);
        }
        Ok(moved)
    }
}

// ------------------------------------------------------------- hand / deck

/// Draw `n` cards; returns how many were drawn.
///
/// One draw **instruction**: `n` cards, one host call (v114 -- it used to be
/// one call per card). The engine adjudicates each card (`drewBefore` may
/// replace it) and moves the plain ones. The per-card after points
/// (`drawn` / `drew`) fire once the run commits, and `drawAfter` fires **once**
/// with the whole batch after the last of them. Pauses like [`pay`] does; the
/// body must `?` the result.
pub fn draw(player_id: i32, n: i32) -> Result<i32, Prompt> {
    draw_flags(player_id, n, 0)
}

/// Draw `n` cards **without** the immediate hand-limit trim (「可超过上限」).
/// The excess is allowed until the hand fits again or the turn ends: the
/// card's own scheduled discard is the usual disposer, and `end` is accepted
/// meanwhile. The limit state itself is untouched. Pauses like [`draw`].
pub fn draw_over_hand_limit(player_id: i32, n: i32) -> Result<i32, Prompt> {
    draw_flags(player_id, n, crate::abi::draw_flag::OVER_HAND_LIMIT)
}

fn draw_flags(player_id: i32, n: i32, flags: i32) -> Result<i32, Prompt> {
    crate::pure::refuse_if_filter("ctx::draw");
    #[cfg(kani)]
    {
        // Zero-loop (card-pool draws are 1..=3; a `for _ in 0..n` unrolls to
        // the global bound when `n` is a register value).
        let mut got = 0;
        let mut taken = 0i32;
        macro_rules! draw1 {
            () => {
                if taken < n.max(0) {
                    got += asked(unsafe { sys::draw(player_id, 1, flags) })?;
                    taken += 1;
                }
            };
        }
        draw1!();
        draw1!();
        draw1!();
        draw1!();
        if n.max(0) > 4 {
            kani::assert(false, "draw: more than 4 cards under Kani");
        }
        return Ok(got);
    }
    #[cfg(not(kani))]
    {
        // One host call for the whole instruction: the host draws the cards
        // (replacements, refill, the over-limit flag and pausing included) and
        // raises `drawAfter` once for the batch.
        asked(unsafe { sys::draw(player_id, n.max(0), flags) })
    }
}

/// Draw the top event and settles it (「抽取一个事件卡」).
/// The card does not enter the hand: it is revealed to every player, its
/// effect takes effect at once, and it goes to the event discard (reshuffling
/// that into a new event deck when the deck runs dry). Rulebook 「基础[结算]规则」.
/// Pauses like [`draw`]; the body must `?` the result.
pub fn draw_event(player_id: i32) -> Result<(), Prompt> {
    asked(unsafe { sys::draw_event(player_id) })?;
    Ok(())
}

/// Draw the top event and attribute it to `player_id` at `tile`, instead of to
/// the drawing card's own owner and square (「视为其他玩家在“流星堂”抽取」).
/// `tile = -1` means the drawer's current position. Runs the whole event
/// pipeline like [`draw_event`] (including a deferral, if one is claimed).
/// Answers nothing; the body must `?` it.
pub fn draw_event_as(player_id: i32, tile: i32) -> Result<(), Prompt> {
    asked(unsafe { sys::draw_event_as(player_id, tile) })?;
    Ok(())
}

/// Suspend the `event` link this hook runs on: the event card is parked and
/// resolves at the deferring instance's owner's next turn start.
/// `true` = this instance claimed it (the record now exists), `false` = not an
/// `event` trigger or another instance already claimed it. The caller spends
/// its cost for the claim. Pauses like every request.
pub fn event_defer() -> Result<bool, Prompt> {
    Ok(asked(unsafe { sys::event_defer() })? != 0)
}

/// Checkpoint the board and answer the key (1-based; `0` = no world).
/// Covers every player's position, keyed state and money and the board's
/// owners, houses and mortgage flags. Pauses.
pub fn world_snapshot() -> Result<i32, Prompt> {
    asked(unsafe { sys::world_snapshot() })
}

/// Write a snapshot back (absolute, idempotent). `false` = no such key.
/// Not a move: no abnormal gate, no move bookkeeping. Pauses.
pub fn world_restore(key: i32) -> Result<bool, Prompt> {
    Ok(asked(unsafe { sys::world_restore(key) })? != 0)
}

/// Drop a snapshot and forget its key. Pauses.
pub fn world_snapshot_drop(key: i32) -> Result<(), Prompt> {
    asked(unsafe { sys::world_snapshot_drop(key) })?;
    Ok(())
}

/// 「若无手牌则弃掉下一张抽到的牌」 -- arm a one-shot on `player_id`: the next
/// card that seat draws is discarded instead of kept. Per seat, so two armed
/// cards discard two draws; the flag clears on that draw. Pauses (ABI-08).
pub fn discard_next_fx(player_id: i32) -> Result<(), Prompt> {
    asked(unsafe { sys::discard_next_fx(player_id) })?;
    Ok(())
}

/// Put one copy of `card` into `player_id`'s pile at `dest`.
///
/// The named-card counterpart of [`send_to_dest`] / [`transfer_to_dest`], which
/// move the *running instance*. `Dest::Hand` is 「加入手牌」; `Dest::Graveyard` is
/// 「放入弃卡区」 (prefer [`to_discard`] when you want the discarded hooks); the
/// deck fates name the draw-pile insert position (`Dest::DeckTop` /
/// `DeckBottom` / `DeckRandom` -- the default is **shuffle**).
/// `Dest::Field` / `Dest::Banished` are not piles and trap.
///
/// Replaces `add_to_hand` / `add_to_deck` / `add_to_deck_at`.
///
/// `counted` marks the arrival as a draw for the `handChanged` listener; only
/// a draw routine passes `true`. Every other caller leaves it `false`.
#[cfg_attr(kani, inline(always))]
pub fn add_to_dest(player_id: i32, card: &str, dest: Dest, counted: bool) {
    let (p, l) = s(card);
    unsafe { sys::add_to_dest(player_id, p, l, dest as i32, counted as i32) }
}

/// Take one copy of `card` out of `player_id`'s `src` pile *without* sending it
/// anywhere -- the caller decides where it goes next. Returns whether it was
/// there. Card before zone, matching [`add_to_dest`].
///
/// Replaces `take_from_hand` (that was `src = CardPile::Hand`).
#[cfg_attr(kani, inline(always))]
pub fn take_card(player_id: i32, card: &str, src: CardPile) -> bool {
    let (p, l) = s(card);
    unsafe { sys::take_card(player_id, p, l, src as i32) != 0 }
}

/// Turn up to `count` cards of `owner`'s draw pile over into a new inspection
/// frame for `looker`. The cards land in [`CardPile::Inspection`], top-first;
/// `public` reveals each one to the table as it is turned. Returns how many
/// were held (0 on an empty deck).
///
/// Raises `inspected` for every card **now**, dispatched to the card itself, so
/// a card that places itself out of the frame is gone before the next
/// statement. Pauses on a hook that prompts; the body must `?` it. Always
/// close the frame with [`inspection_end`] when the body is done.
#[must_use]
#[cfg_attr(kani, inline(always))]
pub fn inspection_begin(looker: i32, owner: i32, count: i32, public: bool) -> Result<i32, Prompt> {
    asked(unsafe { sys::inspection_begin(looker, owner, count, public as i32) })
}

/// Close `player`'s own innermost inspection frame -- `player` is the
/// **looker**, whatever seat owns the deck. The leftovers go back to the deck
/// top in their original order, or shuffled in when `shuffle`. Returns how many
/// were returned. No open frame is a no-op returning 0.
#[cfg_attr(kani, inline(always))]
pub fn inspection_end(player: i32, shuffle: bool) -> i32 {
    unsafe { sys::inspection_end(player, shuffle as i32) }
}

/// 「公开…」 -- publish a pile's ids to the table (hand, draw pile or discard).
/// A draw-pile reveal includes the open inspection frames on that seat,
/// innermost first. Writes nothing to any pile. `reason` is the log line shown
/// with the reveal.
pub fn reveal_pile(player: i32, pile: CardPile, reason: &Msg) {
    let (rp, rl) = msg_bytes(reason);
    unsafe { sys::reveal_pile(player, pile as i32, rp, rl) }
}

/// 「展示给所有玩家」 -- publish one card id to the table. `reason` as in
/// [`reveal_pile`].
pub fn reveal_card(player: i32, card: &str, reason: &Msg) {
    let (cp, cl) = s(card);
    let (rp, rl) = msg_bytes(reason);
    unsafe { sys::reveal_card(player, cp, cl, rp, rl) }
}

/// Corrupt-prefix guard for the decode-failure fallback below. Real piles are
/// far smaller (the callers already reject `n > cap` with `cap` ∈ 16..8192
/// *bytes*); this only bounds a garbage length varint so a bad buffer cannot
/// ask for a gigabyte of `INVALID`s.
const MAX_NAME_LIST: usize = 4096;

/// Decode a host name list (`postcard::to_allocvec(&Vec<String>)`) into
/// interned [`NameId`]s.
///
/// **This is where the `String` clone dies.** The host still encodes
/// `Vec<String>` (wire format unchanged — Batch 3 may re-encode as ids);
/// the guest deserializes borrowed `&str` out of the scratch buffer and
/// interns each one. Zero `String` construction. A name that fails to intern
/// (table full / too long) becomes [`NameId::INVALID`] so the list length
/// stays exact — bodies that read `.len()` must see the true count
/// (`docs/rulebook/spec/kani/ras.md` `ranger_01_02_steps_by_count_iff`).
fn decode_name_list(buf: &[u8]) -> Vec<NameId> {
    match postcard::from_bytes::<Vec<&str>>(buf) {
        Ok(borrowed) => borrowed.into_iter().map(crate::intern::intern).collect(),
        // Decode failed mid-list. The `Vec`'s varint length prefix is still
        // readable; keep that many [`NameId::INVALID`] so `.len()` stays exact
        // (the invariant above). An empty list would tell `ranger`'s 「数量」 and
        // `riot`'s redraw that the pile is empty when it is not. The cap is a
        // corrupt-prefix guard, not a design bound — callers already reject
        // `n > cap` before we get here.
        Err(_) => {
            let n = postcard::from_bytes::<usize>(buf).unwrap_or(0);
            alloc::vec![crate::intern::NameId::INVALID; n.min(MAX_NAME_LIST)]
        }
    }
}

/// Decode a host `(uid, name)` list (`postcard::to_allocvec(&Vec<(i32, String)>)`).
/// No `String` construction — see [`decode_name_list`]. Same length invariant.
fn decode_uid_name_list(buf: &[u8]) -> Vec<(i32, NameId)> {
    match postcard::from_bytes::<Vec<(i32, &str)>>(buf) {
        Ok(borrowed) => borrowed
            .into_iter()
            .map(|(uid, s)| (uid, crate::intern::intern(s)))
            .collect(),
        Err(_) => {
            let n = postcard::from_bytes::<usize>(buf).unwrap_or(0);
            (0..n.min(MAX_NAME_LIST))
                .map(|i| (i as i32, crate::intern::NameId::INVALID))
                .collect()
        }
    }
}

/// Every card id in a player's `pile`, in pile order (the deck's top card
/// first). Duplicates are listed once per copy.
///
/// Under `cfg(kani)` the result is a `Vec<NameId>` of **exact** length `n`
/// (the real pile count, `0..=MAX_CARDS`). Host `arm_cards_in` encodes
/// `[n, id…]` and the first probe returns `1 + n`. Short static names
/// (`"c0"`..`"c3"`, interned — no `String`, no `from_raw_parts`, no
/// `postcard`). Bodies that read `hand.len()` / `placed_count` (riot redraw
/// amount, ranger's 「数量」) therefore see the true count, not a padded 0|4
/// shape. A bounded `while` keeps the unwind at `MAX_CARDS`.
#[cfg(kani)]
pub fn cards_in(player_id: i32, pile: CardPile) -> Vec<NameId> {
    let need = unsafe { sys::cards_in(player_id, pile as i32, 0, 0) };
    // `need` is the encoded buffer size `1 + n`; `n = 0` is the empty pile.
    // Empty still carries the concrete capacity: a cap-0 `Vec` drops a
    // dangling pointer whose dealloc layout travels through the Kani 0.68
    // niche-`Cap` read-back (false positive, see `owned_tiles`).
    if need < 2 {
        return Vec::with_capacity(8);
    }
    // Known exact push count: size the Vec once (bounded ≤ 8) so the pushes
    // below never grow it -- a realloc here trips the CBMC dealloc layout
    // checks (same fix as `owned_tiles`).
    let n = (need - 1).min(8) as usize;
    let (p, scratch) = out_buf(need as usize);
    // (capacity is made concrete at `with_capacity` below, see `owned_tiles`.)
    let got = unsafe { sys::cards_in(player_id, pile as i32, p, need) };
    if got != need {
        return Vec::with_capacity(8);
    }
    let buf = out_read(p, got, scratch);
    // Interned short static names. The literal FNV hashes constant-fold;
    // no `String::from`, no `memcmp` on the proof path.
    const SLOTS: [&str; 4] = ["c0", "c1", "c2", "c3"];
    // Concrete cap (see `owned_tiles`): `need` is symbolic through the host
    // call even when the pile is empty on every feasible path.
    let mut out: Vec<NameId> = Vec::with_capacity(8);
    let mut i = 0usize;
    while i < n {
        let slot = (if 1 + i < buf.len() { buf[1 + i] } else { 2 }) & 3;
        out.push(crate::intern::intern(SLOTS[slot as usize]));
        i += 1;
    }
    out
}

#[cfg(not(kani))]
pub fn cards_in(player_id: i32, pile: CardPile) -> Vec<NameId> {
    let need = unsafe { sys::cards_in(player_id, pile as i32, 0, 0) };
    if need <= 0 {
        return Vec::new();
    }
    let (p, scratch) = out_buf(need as usize);
    let got = unsafe { sys::cards_in(player_id, pile as i32, p, need) };
    if got != need {
        return Vec::new();
    }
    let buf = out_read(p, got, scratch);
    decode_name_list(&buf)
}

/// Distinct card ids in `pile`, first-seen order. A pile can hold several
/// copies of one card; when the player is picking a *title*, list each id
/// once.
pub fn distinct_cards_in(player_id: i32, pile: CardPile) -> Vec<NameId> {
    // Concrete cap (see `owned_tiles`): the mixed push/realloc growth is
    // capped at `cards_in`'s own 8 anyway.
    let mut ids: Vec<NameId> = Vec::with_capacity(8);
    // By reference: consuming the `cards_in` `Vec` into an `IntoIter` makes
    // the drop go through the Kani 0.68 niche-`Cap` read-back, which can
    // deallocate with a bogus size (allocator-check false positive).
    for &c in &cards_in(player_id, pile) {
        if !ids.contains(&c) {
            ids.push(c);
        }
    }
    ids
}

#[cfg_attr(kani, inline(always))]
pub fn to_discard(player_id: i32, card: &str) {
    let (p, l) = s(card);
    unsafe { sys::to_discard(player_id, p, l) }
}

/// Copies of `card` in the player's hand.
#[cfg_attr(kani, inline(always))]
pub fn hand_count(player_id: i32, card: &str) -> i32 {
    let (p, l) = s(card);
    unsafe { sys::hand_count(player_id, p, l) }
}

/// Total cards in hand.
pub fn hand_size(player_id: i32) -> i32 {
    unsafe { sys::hand_size(player_id) }
}

/// The authoritative opening hand size (`state_key::START_HAND`; the engine
/// seeds it to 2 before the before-match-start point). Effects lower it with
/// [`inc_start_hand`]; the opening draw reads the final value.
pub fn start_hand(player_id: i32) -> i32 {
    state::get(player_id, crate::abi::state_key::START_HAND)
}

/// 「初始手牌减1」 and kin: move the opening hand size by `delta`, floored at 0.
/// Live only at the before-match-start point (`DeckBeforeGame`) -- the opening
/// draw reads the value once, afterwards.
#[cfg_attr(kani, inline(always))]
pub fn inc_start_hand(player_id: i32, delta: i32) {
    let v = (state::get(player_id, crate::abi::state_key::START_HAND) + delta).max(0);
    state::set(player_id, crate::abi::state_key::START_HAND, v);
}

/// Copies of `card` in the player's discard pile.
#[cfg_attr(kani, inline(always))]
pub fn discard_count(player_id: i32, card: &str) -> i32 {
    let (p, l) = s(card);
    unsafe { sys::discard_count(player_id, p, l) }
}

/// Draw-pile size.
pub fn deck_count(player_id: i32) -> i32 {
    unsafe { sys::deck_count(player_id) }
}

/// Discard-pile size (all ids).
#[cfg_attr(kani, inline(always))]
pub fn discard_size(player_id: i32) -> i32 {
    unsafe { sys::discard_size(player_id) }
}

/// Drop one copy of `card` from hand into the discard
/// pile. True when the player held one.
#[cfg_attr(kani, inline(always))]
pub fn discard_from_hand(player_id: i32, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::discard_from_hand(player_id, p, l) != 0 }
}

/// Sweep hand + discard into the draw pile and shuffle. Returns how many
/// cards moved.
pub fn sweep_to_deck(player_id: i32) -> i32 {
    shuffle_into_deck(player_id, true, true)
}

/// Move the hand and/or the discard pile into the draw pile, then shuffle it.
/// Returns how many moved.
pub fn shuffle_into_deck(player_id: i32, hand: bool, discard: bool) -> i32 {
    unsafe { sys::shuffle_into_deck(player_id, hand as i32, discard as i32) }
}

// ------------------------------------------------------------ field cards

/// This card remains in play at the player.
#[cfg_attr(kani, inline(always))]
pub fn place_card(player_id: i32, card: &str, note: &Msg) -> i32 {
    crate::pure::refuse_if_filter("ctx::place_card");
    place_card_at(player_id, card, note)
}

/// Put a specific card (a derived one) into play at a player.
/// Returns the new instance's uid, which is what addresses it afterwards: the
/// *name* is not an identity, so a rule meaning "the copy I just placed" has to
/// hold this rather than re-resolving by name.
#[cfg_attr(kani, inline(always))]
pub fn place_card_at(player_id: i32, card: &str, note: &Msg) -> i32 {
    let (cp, cl) = s(card);
    let (p, l) = msg_bytes(note);
    unsafe { sys::place_card_at(player_id, cp, cl, p, l) }
}

/// 「将此卡放置在场上」 -- [`set_dest`]`(`[`Dest::Field`]`)` then [`place_card`],
/// the fold every play body that stays in play wants. Two axes, one call:
/// `place_card` creates the field instance **now**; the `Dest::Field` dest
/// tells the commit that the played hand card *became* that instance, so it is
/// not also spent to the default dest (the discard). Returns the new
/// instance's uid ([`place_card_at`]'s contract).
///
/// Only for the body that places **its own** card (its `ID`). A body placing
/// some *other* card still calls [`place_card`] alone -- that instance is not
/// the played hand card's dest -- and a play whose hand card retires elsewhere
/// (「…然后此卡[移除]」) names its own [`Dest`].
#[cfg_attr(kani, inline(always))]
pub fn place_self_to_field(player_id: i32, card: &str, note: &Msg, cause: ZoneChangeCause) -> i32 {
    crate::pure::refuse_if_filter("ctx::place_self_to_field");
    set_dest(Dest::Field, cause);
    place_card(player_id, card, note)
}

/// Where this card goes afterwards (see [`Dest`]). For a
/// play it is the hand card's dest; for a field effect, the instance's.
/// `cause` rides the transition's `ZoneChange` — say why it leaves
/// (`Effect`, `Cost`, `Expire`, …), never `Other` where a real reason exists.
pub fn set_dest(dest: Dest, cause: ZoneChangeCause) {
    crate::pure::refuse_if_filter("ctx::set_dest");
    unsafe { sys::set_dest(dest as i32, cause.as_i32()) }
}

/// [`set_dest`] aimed at another player's pile -- 「将此卡放入[使用者]弃卡区」
/// when [使用者] is not the one holding the card. `to` is whose discard / hand
/// / deck it lands in.
pub fn set_transfer_to_dest(to: i32, dest: Dest, cause: ZoneChangeCause) {
    unsafe { sys::set_transfer_to_dest(to, dest as i32, cause.as_i32()) }
}

/// [`set_dest`] applied **now** rather than when this effect finishes: for a
/// card that must be gone before the rest of the effect runs (a move or a
/// settle follows). Lands in its own owner's pile. Returns the owner it left,
/// or `None` when it was not in play.
pub fn send_to_dest(dest: Dest, cause: ZoneChangeCause) -> Option<i32> {
    let v = unsafe { sys::send_to_dest(dest as i32, cause.as_i32()) };
    (v >= 0).then_some(v)
}

/// [`send_to_dest`] aimed at another player's pile: `to` is whose.
pub fn transfer_to_dest(to: i32, dest: Dest, cause: ZoneChangeCause) -> Option<i32> {
    let v = unsafe { sys::transfer_to_dest(to, dest as i32, cause.as_i32()) };
    (v >= 0).then_some(v)
}

/// Suppress the arrival hook for the next [`set_dest`] (a batch's own moves).
pub fn set_dest_unwatched(dest: Dest) {
    unsafe { sys::set_dest_unwatched(dest as i32) }
}

/// Move a player without settling (no [触发结算] at the landing).
pub fn teleport_to(player_id: i32, tile: i32) {
    unsafe { sys::teleport_to(player_id, tile) }
}

/// Take **this instance** off the field; returns the owner it left (or -1).
/// [`unplace_card_named`] is the form for some *other* card.
#[cfg_attr(kani, inline(always))]
pub fn unplace_self() -> i32 {
    unsafe { sys::unplace_card() }
}

/// Expire the active event `id` (`docs/EVENTS.md`): unbind its rule instance
/// and file it away. `removed = true` is 「永久移除」; `false` is 「放入事件弃牌」.
/// A one-shot event never calls this -- `draw_event` files it away itself when
/// the body does not remain.
#[cfg_attr(kani, inline(always))]
pub fn event_expire(id: &str, removed: bool) {
    let (p, l) = s(id);
    unsafe { sys::event_expire(p, l, removed as i32) }
}

/// Is `id` active and face-up? 「此卡在场上则…」
#[cfg_attr(kani, inline(always))]
pub fn event_is_active(id: &str) -> bool {
    let (p, l) = s(id);
    unsafe { sys::event_is_active(p, l) != 0 }
}

/// Put `id` on the top of the event deck. `face_down` is 「背面朝上放置于事件
/// 牌堆顶部」 -- the draw still reveals it (「向所有玩家公开」).
#[cfg_attr(kani, inline(always))]
pub fn event_deck_push(id: &str, face_down: bool) {
    let (p, l) = s(id);
    unsafe { sys::event_deck_push(p, l, face_down as i32) }
}

/// Take `id` out of the event deck / discard / active list for good --
/// 「从所有非衍生事件中选择3个移除」.
#[cfg_attr(kani, inline(always))]
pub fn event_banish(id: &str) {
    let (p, l) = s(id);
    unsafe { sys::event_banish(p, l) }
}

/// 「本次移动…是否触发结算」 -- arm `player_id`'s answer for its **current
/// move**. The engine consumes it once; `Unanswered` leaves it alone.
pub fn set_move_settle_answer(player_id: i32, answer: crate::abi::SettleAnswer) {
    unsafe { sys::set_move_settle_answer(player_id, answer as i32) }
}

/// The armed answer for `player_id`'s current move.
pub fn move_settle_answer(player_id: i32) -> crate::abi::SettleAnswer {
    match unsafe { sys::move_settle_answer(player_id) } {
        1 => crate::abi::SettleAnswer::Settle,
        2 => crate::abi::SettleAnswer::Skip,
        _ => crate::abi::SettleAnswer::Unanswered,
    }
}

/// 「（或无法移动的回合结束时）」 -- the turn-end half for `player_id`. The
/// answer belongs to that seat across turns (the blocking effect usually lands
/// on someone else's turn).
pub fn set_turn_end_settle_answer(player_id: i32, answer: crate::abi::SettleAnswer) {
    unsafe { sys::set_turn_end_settle_answer(player_id, answer as i32) }
}

/// `player_id`'s armed turn-end answer.
pub fn turn_end_settle_answer(player_id: i32) -> crate::abi::SettleAnswer {
    match unsafe { sys::turn_end_settle_answer(player_id) } {
        1 => crate::abi::SettleAnswer::Settle,
        2 => crate::abi::SettleAnswer::Skip,
        _ => crate::abi::SettleAnswer::Unanswered,
    }
}

/// The innermost open roll transaction, if any. `None` outside a roll window
/// and when a body resumed after a dropped stack -- callers treat it as "open
/// a fresh preview".
pub fn roll_current() -> Option<i32> {
    let id = unsafe { sys::roll_current() };
    (id >= 0).then_some(id)
}

/// Every face `roll` has cast, in cast order.
pub fn roll_faces(roll: i32) -> Vec<i32> {
    let mut out = Vec::new();
    loop {
        let v = unsafe { sys::roll_face(roll, out.len() as i32) };
        if v < 0 {
            break;
        }
        out.push(v);
    }
    out
}

/// Every face `player_id` has cast this turn, in cast order (one entry per
/// real throw and per fixed face).
pub fn rolls_by_player(player_id: i32) -> Vec<i32> {
    const CAP: usize = 64;
    let (p, scratch) = out_buf(CAP);
    let n = unsafe { sys::rolls_by_player(player_id, p, CAP as i32) };
    if n <= 0 {
        return Vec::new();
    }
    // Over the cap the host writes nothing and returns the **full** length
    // (`hostfns::write_postcard`), so size a second buffer to it and ask
    // again -- a long turn (taki rerolls, starry, 运动的天赋) must not read as
    // "no casts".
    if n as usize > CAP {
        let (p2, scratch2) = out_buf(n as usize);
        let m = unsafe { sys::rolls_by_player(player_id, p2, n) };
        if m <= 0 || m > n {
            return Vec::new();
        }
        let buf = out_read(p2, m, scratch2);
        return postcard::from_bytes(&buf[..m as usize]).unwrap_or_default();
    }
    let buf = out_read(p, n, scratch);
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// The face the latest pick did not take, if any (the reroll's spare face).
pub fn roll_unused_face(roll: i32) -> Option<i32> {
    let v = unsafe { sys::roll_unused_face(roll) };
    (v >= 0).then_some(v)
}

/// Recast `roll` with `source`; the enclosing transaction settles the chosen
/// face. Returns the new raw face.
pub fn roll_recast(roll: i32, source: i32) -> Result<i32, Prompt> {
    crate::pure::refuse_if_filter("ctx::roll_recast");
    let v = asked(unsafe { sys::roll_recast(roll, source) })?;
    // The recast chooses its own cast; keep the trigger's move view in step so
    // the window's write-back cannot undo the choice.
    crate::ctx::trigger::set_move_roll(v);
    Ok(v)
}

/// Choose a cast of `roll`. Clamped; records the other face of the pair as
/// `unused_face`.
pub fn roll_pick(roll: i32, index: i32) {
    unsafe { sys::roll_pick(roll, index) };
    // See `roll_recast`: the pick may take an earlier cast, and the window's
    // write-back must carry the picked face.
    let faces = roll_faces(roll);
    if !faces.is_empty() {
        let i = (index.max(0) as usize).min(faces.len() - 1);
        crate::ctx::trigger::set_move_roll(faces[i]);
    }
}

/// The result of [`roll_reroll`].
pub struct RerollOutcome {
    /// The **absolute** cast index that will settle (the prompt's option
    /// mapped onto the transaction's casts -- the offered pair is the last
    /// two casts, not casts 0/1).
    pub chosen_index: i32,
    /// The face the pick left behind.
    pub unused_face: Option<i32>,
}

/// 「放弃第一次的结果重骰一次」: cast a new face on the current roll, let the
/// player pick old/new and choose it. The enclosing transaction settles the
/// chosen face. With no open transaction, the first cast is a fresh preview.
pub fn roll_reroll(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    keep_old: &Msg,
    keep_new: &Msg,
    source: i32,
) -> Result<RerollOutcome, Prompt> {
    let roll = match roll_current() {
        Some(r) => r,
        None => {
            let _ = roll(player_id, 0, 0, crate::abi::RollPurpose::Move, source)?;
            roll_current().unwrap_or(-1)
        }
    };
    roll_recast(roll, source)?;
    let pick = ask_choice(
        player_id,
        title,
        text,
        &[keep_old.clone(), keep_new.clone()],
    )?;
    // The prompt's options are the pair the recast left as the last two casts
    // (old = n-2, new = n-1); `roll_pick` wants an absolute cast index, so a
    // transaction that already carries earlier casts (taki's rerolls, layer's
    // fire-pot rerolls) must not pass the option index straight through.
    let n = roll_faces(roll).len().max(1);
    let chosen = if pick == 1 {
        n - 1
    } else {
        n.saturating_sub(2)
    };
    roll_pick(roll, chosen as i32);
    Ok(RerollOutcome {
        chosen_index: chosen as i32,
        unused_face: roll_unused_face(roll),
    })
}

/// A standalone move roll for `player_id` (Doll Garden's victim): its own
/// transaction, the whole Move ladder, the settled face.
pub fn roll_move(player_id: i32, source: i32) -> Result<i32, Prompt> {
    crate::pure::refuse_if_filter("ctx::roll_move");
    asked(unsafe { sys::roll_move(player_id, source) })
}

/// Is this card in play at the player?
pub fn is_placed() -> bool {
    unsafe { sys::is_placed() != 0 }
}

/// One declared **property** of the running rule instance (`FieldCard::props`,
/// see [`crate::abi::prop`]). A tile rule body reads its tile data this way
/// (`prop::PRICE`, `prop::RENT_PREFIX` + level, …); a key the instance does
/// not carry reads as its defined default, `0`. The instance is the one
/// running -- same reasoning as [`crystals`].
#[cfg_attr(kani, inline(always))]
pub fn prop(key: &str) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::self_prop(p, l) }
}

/// Write a **property** on the running rule instance. What a card that bends a
/// tile does instead of an engine flag: 黑衣人的补给 sets `prop::NO_REWARD`,
/// （soyo） sets `prop::GROUP`, and so on. `docs/TILES.md`.
#[cfg_attr(kani, inline(always))]
pub fn set_prop(key: &str, value: i32) -> i32 {
    crate::pure::refuse_if_filter("ctx::set_prop");
    let (p, l) = s(key);
    unsafe { sys::set_self_prop(p, l, value) }
}

/// A **property of the rule instance governing `tile`** (`docs/TILES.md`) --
/// what a card that bends a *tile* writes instead of an engine flag. The
/// reader is the tile instance; the source owns the arming and the disarming.
/// A tile with no rule instance reads as the key's default (`0`).
#[cfg_attr(kani, inline(always))]
pub fn tile_prop(tile: i32, key: &str) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::tile_prop(tile, p, l) }
}

/// Write [`tile_prop`] on the rule instance(s) governing `tile`; returns the
/// stored value. 「无法获取[CiRCLE奖励]」 arms `prop::NO_REWARD` on CiRCLE's
/// `tile:circle` instance this way, and the reward step reads it back.
#[cfg_attr(kani, inline(always))]
pub fn set_tile_prop(tile: i32, key: &str, value: i32) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::set_tile_prop(tile, p, l, value) }
}

/// One declared property of the instance at `uid` (`FieldCard::props`,
/// [`crate::abi::prop`] keys). Default 0. Mirrors [`crystals_at`]'s naming:
/// the instance is named by uid, not by tile or by the running card.
#[cfg_attr(kani, inline(always))]
pub fn prop_at(uid: i32, key: &str) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::prop_at(uid, p, l) }
}

/// Write [`prop_at`] on the instance at `uid`. Returns the stored value.
#[cfg_attr(kani, inline(always))]
pub fn set_prop_at(uid: i32, key: &str, value: i32) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::set_prop_at(uid, p, l, value) }
}

/// 「那格视为与…交换位置」 -- bind **this instance's** slot to `partner`, one
/// level, both directions. `-1` clears. Sugar over
/// [`set_prop`]`(prop::SWAPPED_TILE, …)`, so it dies with the instance.
pub fn set_swapped_tile(partner: i32) -> i32 {
    set_prop(crate::abi::prop::SWAPPED_TILE, partner)
}

// ------------------------------------------------------ counters & marks
// Named counters on a card instance, and their units bound to a tile (a tile
// mark) or a holder (a player token). One API; see `bandori_std::counter` /
// `abi::MarkFilter`. User ruling 2026-10-10: every mark/token is a unit of
// some card instance's counter and dies with that instance.

/// On-card named counter of **this** instance (`bandori_std::counter`
/// names, or a card-invented name).
#[cfg_attr(kani, inline(always))]
pub fn counter(name: &str) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::counter_self(p, l) }
}

/// Adjust this instance's on-card counter `name` by `n`, clamped at 0 / `max`
/// (`0` = uncapped). Raises `CounterChanged` with `t.name = name`. `?` it
/// (the marker window can pause).
#[cfg_attr(kani, inline(always))]
pub fn add_counter(name: &str, n: i32, max: i32) -> Result<i32, Prompt> {
    crate::pure::refuse_if_filter("ctx::add_counter");
    let (p, l) = s(name);
    asked(unsafe { sys::add_counter_self(p, l, n, max) })
}

/// Set this instance's on-card counter `name` to `n`.
#[cfg_attr(kani, inline(always))]
pub fn set_counter(name: &str, n: i32) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::set_counter_self(p, l, n) }
}

/// The hard per-instance maximum stored for the counter `name` on `uid` (`0` =
/// uncapped / absent). `add_counter*` clamps at it in addition to the
/// call-local `max`; the static form is a `counter_max.<name>` placement prop
/// ([`crate::abi::prop::COUNTER_MAX_PREFIX`]).
#[cfg_attr(kani, inline(always))]
pub fn counter_max_at(uid: i32, name: &str) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::counter_max_at(uid, p, l) }
}

/// Write the per-instance maximum of the counter `name` on `uid`; `0` clears
/// it. Returns the stored value. The key is removed with the instance.
#[cfg_attr(kani, inline(always))]
pub fn set_counter_max_at(uid: i32, name: &str, max: i32) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::set_counter_max_at(uid, p, l, max) }
}

/// On-card named counter of the instance at `uid`.
#[cfg_attr(kani, inline(always))]
pub fn counter_at(uid: i32, name: &str) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::counter_at(uid, p, l) }
}

/// Adjust the on-card named counter of the instance at `uid`. Raises
/// `CounterChanged` against that instance. Cross-instance **writes** prefer
/// `send` (the owner writes its own counters); this is the engine-level form
/// used when the engine itself is the actor (e.g. the `mark:cp` landing hook
/// spending the placer's on-card stock).
#[cfg_attr(kani, inline(always))]
pub fn add_counter_at(uid: i32, name: &str, n: i32, max: i32) -> i32 {
    crate::pure::refuse_if_filter("ctx::add_counter_at");
    let (p, l) = s(name);
    unsafe { sys::add_counter_at(uid, p, l, n, max) }
}

/// On-card named counter of the placed card `card` on `player`'s field
/// (first copy).
#[cfg_attr(kani, inline(always))]
pub fn card_counter(player_id: i32, card: &str, name: &str) -> i32 {
    let (cp, cl) = s(card);
    let (np, nl) = s(name);
    unsafe { sys::card_counter(player_id, cp, cl, np, nl) }
}

/// Adjust the on-card named counter of a placed card. Prefer `send` when the
/// receiver should decide; this is the engine-level form.
#[cfg_attr(kani, inline(always))]
pub fn add_card_counter(player_id: i32, card: &str, name: &str, n: i32, max: i32) -> i32 {
    let (cp, cl) = s(card);
    let (np, nl) = s(name);
    unsafe { sys::add_card_counter(player_id, cp, cl, np, nl, n, max) }
}

// -- bound units (tile marks / held tokens) --------------------------------

/// Bind `count` units of **this** instance's counter `kind` to `tile`.
/// `owner` colours the mark (a seat, or -1 neutral); `category` is the display
/// category (`""` player / `bandori_std::mark::CP_CATEGORY`); `src` is provenance
/// (defaults to this instance when -1). `note` explains it. `stack` picks
/// [`abi::Stack::Merge`] (onto the first match) or [`abi::Stack::Fresh`] (a
/// new row per call).
///
/// Returns a durable [`crate::abi::MarkRef`] to the row that now holds the
/// units: the new row's uid on `Fresh`, the merged row's uid on `Merge`, or
/// [`crate::abi::MarkRef::NONE`] on failure. Store the ref in card state
/// (`set_slot` / `state::set`) and retrieve it later via [`mark_tile`] /
/// [`mark_alive`] / [`remove_mark`].
///
/// This is the one place-mark verb: the graveyard rule asks `mark:cp` to bind
/// units this way via `send`; a skill like 要乐奈 binds 抹茶芭菲 units here.
#[cfg_attr(kani, inline(always))]
pub fn place_mark(
    tile: i32,
    kind: &str,
    category: &str,
    owner: i32,
    src: i32,
    count: i32,
    note: &Msg,
    stack: crate::abi::Stack,
) -> crate::abi::MarkRef {
    let (kp, kl) = s(kind);
    let (cp, cl) = s(category);
    let (np, nl) = msg_bytes(note);
    crate::abi::MarkRef::from(unsafe {
        sys::place_mark(
            tile,
            kp,
            kl,
            cp,
            cl,
            owner,
            src,
            count,
            stack as i32,
            np,
            nl,
        )
    })
}

/// Place **one** paired mark spanning tiles `a` and `b` (the miracle-crystal
/// shape: one unit bound to two tiles). One row per tile, each row's
/// `partner` naming the other. Place-or-refuse atomic: returns `false` and
/// places **nothing** when either tile already carries a matching mark, any
/// owner (the pair cap 「上限1」 is on the pair; a tile cannot sit in two
/// links at once). `owner` / `category` / `src` / `note` / `stack` speak as in
/// [`place_mark`]; both rows die with the creating instance like any mark.
#[cfg_attr(kani, inline(always))]
pub fn place_mark_pair(
    a: i32,
    b: i32,
    kind: &str,
    category: &str,
    owner: i32,
    src: i32,
    count: i32,
    note: &Msg,
    stack: crate::abi::Stack,
) -> bool {
    let (kp, kl) = s(kind);
    let (cp, cl) = s(category);
    let (np, nl) = msg_bytes(note);
    unsafe {
        sys::place_mark_pair(
            a,
            b,
            kp,
            kl,
            cp,
            cl,
            owner,
            src,
            count,
            stack as i32,
            np,
            nl,
        ) != 0
    }
}

/// Units on `tile` matching `filter` (empty kind/category, `mark::ANY` owner
/// / src / instance = any).
pub fn count_marks(tile: i32, filter: &crate::abi::MarkFilter) -> i32 {
    let (fp, fl) = mf(filter);
    unsafe { sys::count_marks_f(tile, fp, fl) }
}

/// Move one matching mark's count by `delta`; the row drops at 0.
pub fn bump_mark(tile: i32, filter: &crate::abi::MarkFilter, delta: i32) -> i32 {
    let (fp, fl) = mf(filter);
    unsafe { sys::bump_mark_f(tile, fp, fl, delta) }
}

/// Drop every matching mark on `tile` (`-1` = every tile, one log line);
/// returns rows removed.
pub fn remove_marks(tile: i32, filter: &crate::abi::MarkFilter) -> i32 {
    let (fp, fl) = mf(filter);
    unsafe { sys::remove_marks_f(tile, fp, fl) }
}

/// Provenance (`TileMark.src`) of the first matching mark on `tile`, or -1.
pub fn mark_src_at(tile: i32, filter: &crate::abi::MarkFilter) -> i32 {
    let (fp, fl) = mf(filter);
    unsafe { sys::mark_src_at(tile, fp, fl) }
}

/// Owning instance of the first matching mark on `tile`, or -1.
pub fn mark_instance_at(tile: i32, filter: &crate::abi::MarkFilter) -> i32 {
    let (fp, fl) = mf(filter);
    unsafe { sys::mark_instance_at(tile, fp, fl) }
}

/// Partner tile (`TileMark.partner`) of the first matching mark on `tile`,
/// or -1 (none / unpaired). The cross-end of a [`place_mark_pair`] row.
pub fn mark_partner(tile: i32, filter: &crate::abi::MarkFilter) -> i32 {
    let (fp, fl) = mf(filter);
    unsafe { sys::mark_partner(tile, fp, fl) }
}

/// The marked tile of the row at `r`, or -1 when no such row (the handle is
/// dead -- never recycled onto a later mark).
#[cfg_attr(kani, inline(always))]
pub fn mark_tile(r: crate::abi::MarkRef) -> i32 {
    unsafe { sys::mark_tile(r.into()) }
}

/// Is the row at `r` still on the board? No extra syscall -- this is just
/// [`mark_tile`] `>= 0`.
#[cfg_attr(kani, inline(always))]
pub fn mark_alive(r: crate::abi::MarkRef) -> bool {
    mark_tile(r) >= 0
}

/// Drop the exact mark row at `r`. Returns whether that row was present.
#[cfg_attr(kani, inline(always))]
pub fn remove_mark(r: crate::abi::MarkRef) -> bool {
    unsafe { sys::remove_mark(r.into()) != 0 }
}

/// Units of **this** instance's counter `kind` bound to `tile`.
pub fn count_on_tile(kind: &str, tile: i32) -> i32 {
    count_marks(
        tile,
        &crate::abi::MarkFilter::any()
            .kind(kind)
            .instance(self_uid()),
    )
}

// -- tile designations ([指定] a tile / 地契 / 地块 while this rule is in play) --
// A designation is a mark of the dedicated [`abi::mark::DESIGNATE_CATEGORY`],
// owned by the designating instance (`instance` = `self_uid()`, `src` = this
// run). It dies with the instance (`destroy_instance_units`), so 「指定」
// lasting while the card / skill is in play is one mark and one cleanup path.
// Generic: no game-specific names live in card-sdk.

/// Record that **this** instance designates `tile` (「指定」 that tile / 地契 /
/// 地块). `owner` is the designating seat (the label's colour in the view);
/// `note` explains the pick. One row per call ([`abi::Stack::Fresh`]), so a
/// card that designates several tiles stacks one label each.
///
/// Returns the [`crate::abi::MarkRef`] to that designation row. The mark is
/// the displayed record; the stored ref is the retrievable handle -- keep it
/// in card state (`set_slot` / `state::set`) and later settles the pick
/// (`MarkRef::tile`), check liveness (`MarkRef::is_live`), or drop it
/// ([`undesignate`]).
pub fn designate_tile(tile: i32, owner: i32, note: &Msg) -> crate::abi::MarkRef {
    place_mark(
        tile,
        crate::abi::mark::DESIGNATE_KIND,
        crate::abi::mark::DESIGNATE_CATEGORY,
        owner,
        self_uid(),
        1,
        note,
        crate::abi::Stack::Fresh,
    )
}

/// Drop **this one** designation row (the designation ended early -- the card
/// is still in play). Pass the [`crate::abi::MarkRef`] returned by
/// [`designate_tile`]. Returns whether that row was present. When the instance
/// itself leaves the field, `destroy_instance_units` already clears every
/// designation; call this only for an early end of one pick. The batch /
/// early-end-of-everything verb is [`undesignate_tile`].
#[cfg_attr(kani, inline(always))]
pub fn undesignate(r: crate::abi::MarkRef) -> bool {
    remove_mark(r)
}

/// Drop **this** instance's designation mark(s) on `tile` (the designation
/// ended early -- the card is still in play). Returns rows removed. When the
/// instance itself leaves the field, `destroy_instance_units` already clears
/// every designation; call this only for an early end.
pub fn undesignate_tile(tile: i32) -> i32 {
    remove_marks(
        tile,
        &crate::abi::MarkFilter::any()
            .category(crate::abi::mark::DESIGNATE_CATEGORY)
            .instance(self_uid()),
    )
}

/// Tiles **this** instance currently designates (one entry per tile that
/// carries at least one of its designation marks).
pub fn designated_tiles() -> alloc::vec::Vec<i32> {
    let f = crate::abi::MarkFilter::any()
        .category(crate::abi::mark::DESIGNATE_CATEGORY)
        .instance(self_uid());
    (0..tile_count())
        .filter(|&t| count_marks(t, &f) > 0)
        .collect()
}

/// Does **this** instance currently designate `tile`?
#[cfg_attr(kani, inline(always))]
pub fn designates(tile: i32) -> bool {
    count_marks(
        tile,
        &crate::abi::MarkFilter::any()
            .category(crate::abi::mark::DESIGNATE_CATEGORY)
            .instance(self_uid()),
    ) > 0
}

/// Units of **this** instance's counter `name` held by `player_id`.
#[cfg_attr(kani, inline(always))]
pub fn count_held(name: &str, player_id: i32) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::count_held(p, l, player_id) }
}

impl crate::abi::MarkRef {
    /// The marked tile, or -1 when the row is gone.
    #[cfg_attr(kani, inline(always))]
    pub fn tile(self) -> i32 {
        mark_tile(self)
    }
    /// Is the row still on the board?
    #[cfg_attr(kani, inline(always))]
    pub fn is_live(self) -> bool {
        mark_alive(self)
    }
}

/// Bind `n` more units of this instance's counter `name` to `player_id`
/// (a rule-created token). Returns how much actually moved. Opens the
/// `markerSpend` / `markerGain` window (user ruling 2026-10-07).
#[cfg_attr(kani, inline(always))]
pub fn add_held(name: &str, player_id: i32, n: i32, max: i32) -> Result<i32, Prompt> {
    let (p, l) = s(name);
    asked(unsafe { sys::add_held(p, l, player_id, n, max) })
}

/// Move `n` units of this instance's counter `name` between locations.
/// A location is a tile (`tile >= 0`), a holder (`player >= 0`), or the
/// on-card pool (both -1). Returns how many actually moved.
///
/// 「将自己拥有的一个"抹茶芭菲"转移到该格上」 (要乐奈 (2)) is
/// `move_units(name, -1, other, space, -1, 1)`.
#[cfg_attr(kani, inline(always))]
pub fn move_units(
    name: &str,
    from_tile: i32,
    from_player: i32,
    to_tile: i32,
    to_player: i32,
    n: i32,
) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::move_units(p, l, from_tile, from_player, to_tile, to_player, n) }
}

/// Announce the body about to run as `card` (the one dispatcher `rt::on` calls;
/// card code does not). `1` runs the body, `0` skips it (the body's
/// `cardFired` raise was cancelled), and the pause sentinel
/// [`crate::abi::EXIT_NEED_INPUT`] halts the body too -- the raise asked a
/// question, and the replayed action re-enters.
///
/// The frame itself is the host's: the host pushes it before the drive, and
/// the raise that carried the body truncates it afterwards (the hooks a body
/// raises are walked *after* it returns, so the guest cannot time the pop).
pub fn body_enter(card: &str) -> i32 {
    let (p, l) = s(card);
    unsafe { sys::body_enter(p, l) }
}

/// Close the body's scope. Called after every [`body_enter`] that ran or
/// parked; the frame outlives it (see there), so today this is a no-op.
pub fn body_leave() {
    unsafe { sys::body_leave() }
}

/// The uid of the running instance (`FieldCard::uid`). Provenance default for
/// [`place_mark`]; addressing default for [`send`].
pub fn self_uid() -> i32 {
    unsafe { sys::self_uid() }
}

// -- cross-card messages ---------------------------------------------------

/// Send a message to another card instance. Nested in the caller's run (the
/// handler may prompt and may raise triggers). Returns the handler's reply,
/// or 0 when no receiver admitted the message.
///
/// Addressing (`abi::Target`): a specific uid, the instance of a card id on a
/// seat, or a standing board-owned pseudo card (`mark:cp`). Multiple receivers
/// of a name: the first admitted one in field order answers. A handler whose
/// `condition` / guard rejects is not a receiver.
#[cfg_attr(kani, inline(always))]
pub fn send(
    target: &crate::abi::Target,
    name: &str,
    payload: &crate::abi::Message,
) -> Result<i32, Prompt> {
    let (tp, tl) = target_bytes(target);
    let (np, nl) = s(name);
    let (pp, pl) = abi_msg_bytes(payload);
    asked(unsafe { sys::send(tp, tl, np, nl, pp, pl) })
}

/// The message being handled (`On::Message`). Sender + payload + reply.
pub mod message {
    use super::*;

    /// The sender's instance uid.
    pub fn sender_uid() -> i32 {
        unsafe { sys::msg_sender_uid() }
    }

    /// The sender's seat.
    pub fn sender_seat() -> i32 {
        unsafe { sys::msg_sender_seat() }
    }

    /// The message name being handled.
    pub fn name() -> String {
        unsafe { sys::msg_name(0, 0) };
        read_str(|b, c| unsafe { sys::msg_name(b, c) })
    }

    /// The payload ints.
    pub fn a() -> i32 {
        unsafe { sys::msg_arg0() }
    }
    pub fn b() -> i32 {
        unsafe { sys::msg_arg1() }
    }
    pub fn c() -> i32 {
        unsafe { sys::msg_arg2() }
    }
    pub fn tile() -> i32 {
        unsafe { sys::msg_tile() }
    }
    pub fn seat() -> i32 {
        unsafe { sys::msg_seat() }
    }

    /// The payload text.
    pub fn text() -> String {
        read_str(|b, c| unsafe { sys::msg_text(b, c) })
    }

    /// Set the reply value the sender receives.
    pub fn reply(v: i32) {
        unsafe { sys::msg_reply(v) }
    }
}

// -- small serialisation helpers -------------------------------------------

fn mf(f: &crate::abi::MarkFilter) -> (i32, i32) {
    let bytes = postcard::to_allocvec(f).unwrap_or_default();
    in_bytes(&bytes)
}

fn target_bytes(t: &crate::abi::Target) -> (i32, i32) {
    #[cfg(kani)]
    {
        // Fixed pack, no postcard/heap: kind (0=Uid, 1=Card, 2=Board),
        // player/uid, card-name NameId (0 when Board / unused).
        let (kind, who, card) = match t {
            crate::abi::Target::Uid(u) => (0i32, *u, 0u16),
            crate::abi::Target::Card { player, card } => {
                (1i32, *player, crate::intern::intern(card).0)
            }
            crate::abi::Target::Board(id) => (2i32, -1i32, crate::intern::intern(id).0),
        };
        let mut buf = [0u8; 12];
        buf[0..4].copy_from_slice(&kind.to_le_bytes());
        buf[4..8].copy_from_slice(&who.to_le_bytes());
        buf[8..10].copy_from_slice(&card.to_le_bytes());
        in_bytes(&buf)
    }
    #[cfg(not(kani))]
    {
        let bytes = postcard::to_allocvec(t).unwrap_or_default();
        in_bytes(&bytes)
    }
}

fn abi_msg_bytes(m: &crate::abi::Message) -> (i32, i32) {
    #[cfg(kani)]
    {
        // Fixed pack, no postcard/heap. The message **name** rides the
        // `send()` `np` slot (interned via `s`); this is only the typed ints.
        let mut buf = [0u8; 20];
        buf[0..4].copy_from_slice(&m.seat.to_le_bytes());
        buf[4..8].copy_from_slice(&m.arg0.to_le_bytes());
        buf[8..12].copy_from_slice(&m.arg1.to_le_bytes());
        buf[12..16].copy_from_slice(&m.arg2.to_le_bytes());
        buf[16..20].copy_from_slice(&m.tile.to_le_bytes());
        in_bytes(&buf)
    }
    #[cfg(not(kani))]
    {
        let bytes = postcard::to_allocvec(m).unwrap_or_default();
        in_bytes(&bytes)
    }
}

fn read_str(fill: impl Fn(i32, i32) -> i32) -> String {
    let need = fill(0, 0);
    if need <= 0 {
        return String::new();
    }
    // `out_buf` / `out_read` -- the same two-call out-buffer pattern every
    // other host string return uses. A raw `buf.as_mut_ptr() as i32` truncates
    // a 64-bit pointer on native (the host writes into an arena slot, not the
    // guest's stack buffer), so the read came back as NULs and
    // `trigger::name() == bandori_std::counter::CRYSTALS` never matched -- the native
    // build kept a crystal-empty card on the field (drift.rs).
    let (p, scratch) = out_buf(need as usize);
    if fill(p, need) != need {
        return String::new();
    }
    let buf = out_read(p, need, scratch);
    #[cfg(kani)]
    {
        // Skip UTF-8 *validation* (the CBMC unwind cliff) -- the host writes
        // ASCII kind spellings / interned names only.
        return unsafe { String::from_utf8_unchecked(buf) };
    }
    #[cfg(not(kani))]
    {
        String::from_utf8(buf).unwrap_or_default()
    }
}

/// Units of counter `name` held by `player_id` (name-keyed; the name is
/// unique to its creating rule).
#[cfg_attr(kani, inline(always))]
pub fn held_count(player_id: i32, name: &str) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::count_held_name(p, l, player_id) }
}

/// Set the held count of `name` on `player_id`. Prefer [`add_held`] /
/// [`move_units`]. `max` is the live ceiling (`0` = uncapped). `instance`
/// stamps or rebinds the owning card instance (`< 0` keeps an existing row's
/// owner and falls back to the running card on a fresh row).
#[cfg_attr(kani, inline(always))]
pub fn set_held(player_id: i32, name: &str, value: i32, max: i32, instance: i32) {
    let (p, l) = s(name);
    unsafe { sys::set_held_name(p, l, player_id, value, max, instance) }
}

/// Live ceiling stored on the held row (`0` = uncapped / undeclared).
#[cfg_attr(kani, inline(always))]
pub fn held_max(player_id: i32, name: &str) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::held_max(p, l, player_id) }
}

/// Bind `n` more units of `name` to `player_id`. Returns how
/// much actually moved. Marker window can pause.
pub fn add_held_by_name(player_id: i32, name: &str, n: i32, max: i32) -> Result<i32, Prompt> {
    add_held(name, player_id, n, max)
}

/// [`held_count`] for a [`NameId`] already in hand (an entry of
/// [`held_names`]): no name re-hash. Under `cfg(kani)` the id is passed as the
/// intern handle (the host's name facade); elsewhere the resolved text goes
/// back through the ordinary name path.
#[cfg(kani)]
#[cfg_attr(kani, inline(always))]
pub fn held_count_id(player_id: i32, id: crate::intern::NameId) -> i32 {
    unsafe { sys::count_held_name(crate::intern::encode_handle(id), 0, player_id) }
}

#[cfg(not(kani))]
#[cfg_attr(kani, inline(always))]
pub fn held_count_id(player_id: i32, id: crate::intern::NameId) -> i32 {
    held_count(player_id, id.as_str())
}

/// [`add_held_by_name`] for a [`NameId`] already in hand.
#[cfg(kani)]
#[cfg_attr(kani, inline(always))]
pub fn add_held_id(
    player_id: i32,
    id: crate::intern::NameId,
    n: i32,
    max: i32,
) -> Result<i32, Prompt> {
    let p = crate::intern::encode_handle(id);
    asked(unsafe { sys::add_held(p, 0, player_id, n, max) })
}

#[cfg(not(kani))]
#[cfg_attr(kani, inline(always))]
pub fn add_held_id(
    player_id: i32,
    id: crate::intern::NameId,
    n: i32,
    max: i32,
) -> Result<i32, Prompt> {
    add_held(id.as_str(), player_id, n, max)
}

// ---------------------------------------------------------- keyed state

/// The keyed state surface: `{value, min, max, expires}` items the engine holds
/// and enforces **nothing**.
///
/// Which keys mean what, and what their caps are, belongs to whoever uses them:
/// a character skill that mandates a marker cap writes [`Self::max`] on that
/// marker's wire key, and the card that gains the marker is free to honour it
/// (the marker helpers do) or ignore it. There is no engine-side
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
    /// The free-form flag word (host `state_flag` bits).
    pub const FLAGS: i32 = 4;

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
        crate::pure::refuse_if_filter("state::set");
        set_field(player_id, key, VALUE, value)
    }

    /// Move the value by `delta` raw. No clamping, no logging.
    pub fn add(player_id: i32, key: &str, delta: i32) -> i32 {
        let (p, l) = s(key);
        unsafe { sys::state_add(player_id, p, l, delta) }
    }

    /// Declare the bounds a consumer may enforce. Stored, not applied.
    pub fn set_bounds(player_id: i32, key: &str, min: i32, max: i32) {
        crate::pure::refuse_if_filter("plan::set_bounds");
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

    /// Write the item's free-form flag word. Rules-layer bit meanings live
    /// outside this SDK; the host just stores the `u32`.
    pub fn set_flags(player_id: i32, key: &str, flags: u32) {
        crate::pure::refuse_if_filter("state::set_flags");
        set_field(player_id, key, FLAGS, flags as i32);
    }
}

// ------------------------------------------------------ per-player slots (V)

/// A free-form counter. Sugar over [`state`]: translated to state calls
/// before crossing the ABI -- there is no dedicated slot export.
#[cfg_attr(kani, inline(always))]
pub fn slot(player_id: i32, key: &str) -> i32 {
    state::get(player_id, key)
}

#[cfg_attr(kani, inline(always))]
pub fn set_slot(player_id: i32, key: &str, value: i32) {
    crate::pure::refuse_if_filter("ctx::set_slot");
    state::set(player_id, key, value);
}

/// Replace the player's persona **name** -- the spelling `persona_is`
/// matches. Does not rebind skills; the caller swaps the persona skill
/// separately (Two-in-One 「替换为sumimi的另一名角色」).
#[cfg_attr(kani, inline(always))]
pub fn set_persona(player_id: i32, name: &str) {
    let (p, l) = s(name);
    unsafe { sys::set_persona(player_id, p, l) }
}

#[cfg_attr(kani, inline(always))]
pub fn inc_slot(player_id: i32, key: &str, n: i32) -> i32 {
    crate::pure::refuse_if_filter("ctx::inc_slot");
    state::add(player_id, key, n)
}

// ----------------------------------------------------------- pots & status

/// Announce that `player_id` just became the owner of
/// `tile`, so the `bought` hook chain (「购买」 hooks: Afterglow's free
/// house, ...) hears it. A card that hands a deed over outside the buy routine
/// (tomoe_savior's 「从该玩家处收购该地契」) calls this after the ownership
/// change; `buy()` raises the same hook itself. The run's own player is the
/// cause (`t.by_card`).
pub fn raise_bought(player_id: i32, tile: i32) -> bool {
    unsafe { sys::raise_bought(player_id, tile) != 0 }
}

/// Run an auction over `tile` among every present player who may pay, starting
/// at the current turn seat. Each seat names a raised bid (or passes); the
/// highest bidder pays that bid and takes the deed. Answers the winner (`-1`
/// when nobody bid). 「所有玩家进行竞价，出价最大的玩家支付选择数量的资金并
/// 获得该格子的地契」 -- 弦卷集团地产开发.
pub fn auction(player_id: i32, tile: i32) -> Result<i32, Prompt> {
    asked(unsafe { sys::auction(player_id, tile) })
}

/// The cosmetic skins of `player_id`'s character: every roster persona that
/// shares the player's base character (`variant_of`), the current one
/// excluded. A skin keeps the bound skills, so the swap is name / art only
/// (「更换角色皮肤（技能相同）」) -- apply it with [`set_persona`].
pub fn persona_skins(player_id: i32) -> Vec<alloc::string::String> {
    let n = unsafe { sys::persona_skins(player_id, 0, 0) };
    if n <= 0 {
        return Vec::new();
    }
    let need = n as usize;
    let (bp, scratch) = out_buf(need);
    if unsafe { sys::persona_skins(player_id, bp, need as i32) } != n {
        return Vec::new();
    }
    let buf = out_read(bp, need as i32, scratch);
    let mut out: Vec<alloc::string::String> = Vec::new();
    for part in buf.split(|&b| b == b'\n') {
        if !part.is_empty() {
            out.push(alloc::string::String::from_utf8_lossy(part).into_owned());
        }
    }
    out
}

/// Raise a trigger point from a guest body. `kind` is a [`TriggerKind`]
/// (e.g. `bandori_std::kind::PassRewardAffected`); `value` is the trigger's `value`.
/// Returns true when the link settled (nobody cancelled it). A hook that
/// prompts pauses the run (`Err(Prompt)`).
pub fn raise(player_id: i32, kind: impl Into<TriggerKind>, value: i32) -> Result<bool, Prompt> {
    raise_from(player_id, kind, value, crate::abi::ChargeSource::Other)
}

/// [`raise`] with a source marker. `Skill` sets `charge_is_skill` on the
/// window (`fire::spend_skill` is the one caller that means it).
pub fn raise_from(
    player_id: i32,
    kind: impl Into<TriggerKind>,
    value: i32,
    source: crate::abi::ChargeSource,
) -> Result<bool, Prompt> {
    let kind = kind.into();
    let (p, l) = s(&kind.spelling());
    asked(unsafe { sys::raise(player_id, p, l, value, source.as_i32()) }).map(|v| v != 0)
}

// ---------------------------------------------------------- generic status verbs
// Engine services the `rules/status` hosts (and `bandori-std::status`) drive.
// Cards call the ruleset status hosts (`bandori_std::status` /
// `rules_status`, `docs/STATUS-HOSTS.md`) with opaque key names as data.
// No game vocabulary here: compose `gate` + `state::*` + `raise_after`.

/// Queue a post-commit raise with a caller-supplied wire name (opaque string).
/// This is how a rule raises a ruleset-declared event after the current effect
/// commits (e.g. the wire id 21 event). `value` is the trigger's payload.
pub fn raise_after(player_id: i32, name: &str, value: i32) {
    let (p, l) = s(name);
    unsafe { sys::raise_after(player_id, p, l, value) }
}

/// Set this turn's main-move latch. `locked != 0` skips the move; `0`
/// releases it. The turn-flow latch is **not** part of every layer-count
/// host's clause -- closeout-2 ruling.
pub fn skip_main_move(player_id: i32, locked: bool) {
    unsafe { sys::skip_main_move(player_id, locked as i32) }
}

/// [`skip_main_move`] with `locked = false`.
pub fn clear_move_skip(player_id: i32) {
    unsafe { sys::clear_move_skip(player_id) }
}

/// Give the player an extra turn.
pub fn give_extra_turn(player_id: i32) {
    unsafe { sys::give_extra_turn(player_id) }
}

/// End this player's turn now (generic auto-skip latch). The engine honors it
/// after the current `turnStartBefore` / `turnStart` raise: log nothing here,
/// just ask; the host paces and closes the turn.
pub fn end_turn_now(player_id: i32) {
    unsafe { sys::end_turn_now(player_id) }
}

/// Teleport with the move's `is_main_move` flag named. Opens **no** abnormal window
/// (the caller's own clause). `is_main_move` marks the move as the turn's
/// [主要移动] and consumes the main-move slot.
pub fn teleport_as(player_id: i32, tile: i32, settles: bool, is_main_move: bool) {
    unsafe { sys::teleport_as(player_id, tile, settles as i32, is_main_move as i32) }
}

/// [`log`] with an explicit event kind (`"status"` for host skip / over lines).
pub fn log_as(player_id: i32, kind: &str, msg: &Msg) {
    let (kp, kn) = s(kind);
    let (p, l) = msg_bytes(msg);
    unsafe { sys::log_as(player_id, kp, kn, p, l) }
}

/// Can the player pay? Not out and not under a no-act lock.
#[doc(hidden)]
pub fn can_pay(player_id: i32) -> bool {
    unsafe { sys::can_pay(player_id) != 0 }
}

/// [`cant_move`]'s raw code: 0 = free, 1 = not your turn, 2 = the main move
/// is already spent, 3 = the move gate holds (a [停留] latch).
#[doc(hidden)]
pub fn cant_move_code(player_id: i32) -> i32 {
    unsafe { sys::cant_move(player_id) }
}

/// [`cant_move_code`] from the named activation: "already moved" is suppressed
/// when that activation drove this turn's main move, so a re-run of a body
/// reads back as "not spent".
#[doc(hidden)]
pub fn cant_move_code_for_activation(player_id: i32, activation_ordinal: i32) -> i32 {
    unsafe { sys::cant_move_code_for_activation(player_id, activation_ordinal) }
}

/// `None` when the player may still make this turn's main
/// move, else the shared refusal (`status.move_*` messages).
#[doc(hidden)]
pub fn cant_move(player_id: i32) -> Option<Msg> {
    match unsafe { sys::cant_move(player_id) } {
        0 => None,
        1 => Some(Msg::new("status.move_not_your_turn")),
        2 => Some(Msg::new("status.move_already")),
        _ => Some(Msg::new("status.move_blocked")),
    }
}

/// Run the move being planned **now**. Shape it with
/// [`plan`] first (`set_steps` / `set_kind` / `set_tag` / ...), then call this:
/// the engine runs the move (it may prompt, and the usual move triggers fire)
/// and the effect continues after it. Like `play_card`, the run pauses for it.
pub fn run_planned_move(player_id: i32) -> bool {
    unsafe { sys::run_planned_move(player_id) != 0 }
}

// ------------------------------------------------------------- named markers
// Name-as-data. Readers are `state::get` / `state::max` with the marker's
// wire key; writers open the generic `markerGain` / `markerSpend` windows.

/// Dumb add of `n` to the named counter; returns how many landed. Opens
/// `markerGain` first. No cap policy: the rules-layer kernel clamps and
/// narrates.
pub fn gain_marker(player_id: i32, name: &str, n: i32, reason: &Msg) -> Result<i32, Prompt> {
    let (np, nl) = s(name);
    let (p, l) = msg_bytes(reason);
    asked(unsafe { sys::gain_marker(player_id, np, nl, n, p, l) })
}

/// Spend `n` of the named counter; 1 when the player had enough. Opens
/// `markerSpend` first (a cancelled window spends nothing).
pub fn spend_marker(player_id: i32, name: &str, n: i32, reason: &Msg) -> Result<i32, Prompt> {
    let (np, nl) = s(name);
    let (p, l) = msg_bytes(reason);
    asked(unsafe { sys::spend_marker(player_id, np, nl, n, p, l) })
}

/// Raise (or lower) the named counter's advertised cap; returns the new cap.
/// Storage-only: the pull-under is the rules-layer kernel's job.
pub fn add_marker_max(player_id: i32, name: &str, n: i32) -> i32 {
    let (np, nl) = s(name);
    unsafe { sys::add_marker_max(player_id, np, nl, n) }
}

/// Whose turn it is (-1 when none).
pub fn turn_player() -> i32 {
    unsafe { sys::turn_player() }
}

/// The round counter.
pub fn round_no() -> i32 {
    unsafe { sys::round_no() }
}

/// `round * 100 + turn + 1`: a per-turn id for once-per-turn
/// latches (`set_slot(player_id, key, turn_key())` then compare).
#[cfg_attr(kani, inline(always))]
pub fn turn_key() -> i32 {
    unsafe { sys::turn_key() }
}

/// Is the turn's dice immunity up (「打出此卡的回合…」)?
#[doc(hidden)]
pub fn dice_immune() -> bool {
    unsafe { sys::dice_immune() != 0 }
}

/// Raise the turn's dice immunity (idempotent; for dynamic grants).
#[doc(hidden)]
pub fn set_dice_immune(on: bool) -> bool {
    unsafe { sys::set_dice_immune(on as i32) != 0 }
}

/// Where a card goes when its effect finishes. Wire values 0..=3 are the
/// Graveyard / Hand / Field / Banished order; 4..=6 are the draw-pile fates.
///
/// One dest, two movers. A **play** names where its hand card ends and the
/// engine applies it (`play_from_hand`); a **field effect** names where the
/// instance it is running for ends and the host applies it when the run
/// commits. 「将此卡放入[使用者]弃卡区」 is `Dest::Graveyard` either way -- no
/// separate unplace-plus-discard dance. A run that names nothing leaves a
/// placed card where it is.
///
/// Planned: the draw-pile fates, one per insert position -- `DeckTop` = 4,
/// `DeckBottom` = 5, `DeckRandom` = 6 (the default is **shuffle**, not top).
/// When they land they need arms in `game-rules`'s `dest_from` and
/// `game-core`'s play-from-hand dest handling.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dest {
    /// The discard pile (弃卡区) -- the default.
    Graveyard = 0,
    /// Back to the hand (手牌).
    Hand = 1,
    /// Remains in play (场上) -- a persistent effect.
    Field = 2,
    /// 「[移除]」 -- out of the game entirely.
    Banished = 3,
    /// Onto the top of the draw pile.
    DeckTop = 4,
    /// Under the draw pile.
    DeckBottom = 5,
    /// Shuffled into the draw pile (the default insert position).
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
    let (tp, tl) = msg_bytes(title);
    let (xp, xl) = msg_bytes(text);
    asked(unsafe { sys::ask(kind as i32, player_id, tp, tl, xp, xl) })
}

/// Returns the chosen **tile**, not the index.
/// Falls back to the first option if the answer is out of range.
///
/// The answer is logged as the **tile id** (`PromptKind::TileId`), like
/// [`ask_tiles`]: a re-run recomputes the option list from the world the
/// body's own landing changed (a nearest-tile list walks with the position),
/// so a recorded index would slide to a different tile.
pub fn ask_tile(player_id: i32, title: &Msg, text: &Msg, tiles: &[i32]) -> Result<i32, Prompt> {
    #[cfg(kani)]
    {
        // Zero-loop option list (see `ask_choice`).
        if let Some(&t) = tiles.get(0) {
            unsafe { sys::opt_int(t) }
        }
        if let Some(&t) = tiles.get(1) {
            unsafe { sys::opt_int(t) }
        }
        if let Some(&t) = tiles.get(2) {
            unsafe { sys::opt_int(t) }
        }
        if let Some(&t) = tiles.get(3) {
            unsafe { sys::opt_int(t) }
        }
        if tiles.len() > 4 {
            kani::assert(false, "ask_tile: more than 4 tiles under Kani");
        }
        let t = ask_raw(PromptKind::TileId, player_id, title, text)?;
        return Ok(if t < 0 {
            tiles.first().copied().unwrap_or(-1)
        } else {
            t
        });
    }
    #[cfg(not(kani))]
    {
        for &t in tiles {
            unsafe { sys::opt_int(t) }
        }
        let t = ask_raw(PromptKind::TileId, player_id, title, text)?;
        Ok(if t < 0 {
            tiles.first().copied().unwrap_or(-1)
        } else {
            t
        })
    }
}

/// One option of [`ask_tile_index`]: a tile and its label.
#[derive(Clone)]
pub struct TileOption {
    pub tile: i32,
    pub label: Msg,
}

/// Like [`ask_tile`], but one option per `(tile, label)` pair and the answer is
/// the **option index** -- repeated tiles stay distinct options, and each maps
/// to its own cast (Hagumi's per-roll options).
pub fn ask_tile_index(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    options: &[TileOption],
) -> Result<usize, Prompt> {
    #[cfg(kani)]
    {
        // Bounded list (16 slots), same cap as `ask_choice`.
        const CAP: usize = 16;
        let mut i = 0usize;
        while i < CAP {
            if let Some(o) = options.get(i) {
                let (lp, ll) = msg_bytes(&o.label);
                unsafe { sys::opt_tile(o.tile, lp, ll) }
            }
            i += 1;
        }
        if options.len() > CAP {
            kani::assert(false, "ask_tile_index: more than 16 options under Kani");
        }
        if options.is_empty() {
            let _ = ask_raw(PromptKind::Tile, player_id, title, text)?;
            return Ok(0);
        }
        let i = ask_raw(PromptKind::Tile, player_id, title, text)?;
        return Ok((i.max(0) as usize).min(options.len() - 1));
    }
    #[cfg(not(kani))]
    {
        for o in options {
            let (lp, ll) = msg_bytes(&o.label);
            unsafe { sys::opt_tile(o.tile, lp, ll) }
        }
        let i = ask_raw(PromptKind::Tile, player_id, title, text)?;
        Ok((i.max(0) as usize).min(options.len().saturating_sub(1)))
    }
}

/// [`ask_tile`] for a pick the player then walks to: the board lights the
/// path to the hovered tile as well as the tile itself.
pub fn ask_tile_walk(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    tiles: &[i32],
) -> Result<i32, Prompt> {
    #[cfg(kani)]
    {
        if let Some(&t) = tiles.get(0) {
            unsafe { sys::opt_int(t) }
        }
        if let Some(&t) = tiles.get(1) {
            unsafe { sys::opt_int(t) }
        }
        if let Some(&t) = tiles.get(2) {
            unsafe { sys::opt_int(t) }
        }
        if let Some(&t) = tiles.get(3) {
            unsafe { sys::opt_int(t) }
        }
        if let Some(&t) = tiles.get(4) {
            unsafe { sys::opt_int(t) }
        }
        if let Some(&t) = tiles.get(5) {
            unsafe { sys::opt_int(t) }
        }
        if let Some(&t) = tiles.get(6) {
            unsafe { sys::opt_int(t) }
        }
        if let Some(&t) = tiles.get(7) {
            unsafe { sys::opt_int(t) }
        }
        if tiles.len() > 8 {
            kani::assert(false, "ask_tile_walk: more than 8 tiles under Kani");
        }
        let i = ask_raw(PromptKind::TileWalk, player_id, title, text)?;
        return Ok(tiles[(i.max(0) as usize).min(tiles.len().saturating_sub(1))]);
    }
    #[cfg(not(kani))]
    {
        for &t in tiles {
            unsafe { sys::opt_int(t) }
        }
        let i = ask_raw(PromptKind::TileWalk, player_id, title, text)?;
        Ok(tiles[(i.max(0) as usize).min(tiles.len().saturating_sub(1))])
    }
}

/// Tile ask with per-option labels, prices, and an AI choice hint
/// (the agent-landing prompt's `ask.view.prices` + `with_ai`).
///
/// Returns the chosen **tile id**, or `-1` for the 「不选」 / none answer
/// (`Ask::tile`'s `fallback = tiles.len()`). The id is what the answer log
/// carries, so it is stable across the commit-pass re-run. Do **not** clamp.
pub fn ask_tiles(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    tiles: &[i32],
    labels: &[Msg],
    prices: &[i32],
    ai: i32,
) -> Result<i32, Prompt> {
    for (i, &t) in tiles.iter().enumerate() {
        let (lp, ll) = msg_bytes(&labels[i]);
        unsafe { sys::opt_tile(t, lp, ll) }
        unsafe { sys::opt_price(*prices.get(i).unwrap_or(&0)) }
    }
    unsafe { sys::opt_ai(ai) }
    // `PromptKind::TileId` answers with the **tile id** (or -1 for 「不选」),
    // not the option index: the commit-pass re-run of a body whose options
    // list the just-committed host effect reshaped still maps the pick to the
    // same tile -- an index would slide and log a spurious skip.
    ask_raw(PromptKind::TileId, player_id, title, text)
}

/// Returns the chosen option index.
pub fn ask_choice(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    options: &[Msg],
) -> Result<usize, Prompt> {
    #[cfg(kani)]
    {
        // Bounded option list (16 slots). Covers the widest card range seen
        // so far. A wider list is a loud FAIL (coverage limit).
        const CAP: usize = 16;
        let mut i = 0usize;
        while i < CAP {
            if let Some(o) = options.get(i) {
                let (p, l) = msg_bytes(o);
                unsafe { sys::opt_str(p, l) }
            }
            i += 1;
        }
        if options.len() > CAP {
            kani::assert(false, "ask_choice: more than 16 options under Kani");
        }
        // 0 or 1 options: the pick is forced. Take the ask so `opt_n`
        // accounting matches the host, but return a **literal** index — a
        // symbolic `i.max(0).min(len-1)` leaves `who` symbolic in the body and
        // a later `format!("{p}")` then unrolls Display + memchr (class-E
        // `rec_key` cliff on tsukushi-01).
        if options.len() <= 1 {
            let _ = ask_raw(PromptKind::Choice, player_id, title, text)?;
            return Ok(0);
        }
        let i = ask_raw(PromptKind::Choice, player_id, title, text)?;
        return Ok((i.max(0) as usize).min(options.len().saturating_sub(1)));
    }
    #[cfg(not(kani))]
    {
        for o in options {
            let (p, l) = msg_bytes(o);
            unsafe { sys::opt_str(p, l) }
        }
        let i = ask_raw(PromptKind::Choice, player_id, title, text)?;
        Ok((i.max(0) as usize).min(options.len().saturating_sub(1)))
    }
}

/// Name a free amount of money in `0..=max`. No option list is built -- the
/// bound rides `opt_count`; the answer is the raw amount. `ai` is the bot's
/// preferred amount (`-1` = unset). 「支付选择数量的资金」 (弦卷集团地产开发's
/// bid) is the shape this exists for: an `ask_choice` over every integer would
/// carry thousands of options.
pub fn ask_money(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    max: i32,
    ai: i32,
) -> Result<i32, Prompt> {
    unsafe { sys::opt_count(max.max(0)) }
    if ai >= 0 {
        unsafe { sys::opt_ai(ai) }
    }
    let v = ask_raw(PromptKind::Money, player_id, title, text)?;
    Ok(v.clamp(0, max.max(0)))
}

/// Pick exactly `count` distinct options in one multi-select prompt. The answer
/// is a bitmask (bit *i* = option *i*); the chosen indices come back
/// **ascending**. `count` is clamped to `1..=options.len()`.
///
/// The decode never trusts the host: a mask with too few bits is padded with
/// the lowest unset indices, too many keeps the lowest `count` set bits, so
/// the result is always exactly `count` distinct in-range indices -- including
/// under Kani's symbolic answers.
pub fn ask_choices(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    options: &[Msg],
    count: u32,
) -> Result<Vec<usize>, Prompt> {
    let n = options.len().min(31);
    let want = (count as usize).clamp(1, n.max(1));
    #[cfg(kani)]
    {
        // Same bounded zero-loop push as `ask_choice`; 5 options (this card's
        // widest multi) sit well under the 16-slot cap, so the existing
        // proofs keep compiling.
        const CAP: usize = 16;
        let mut i = 0usize;
        while i < CAP {
            if let Some(o) = options.get(i) {
                let (p, l) = msg_bytes(o);
                unsafe { sys::opt_str(p, l) }
            }
            i += 1;
        }
        if options.len() > CAP {
            kani::assert(false, "ask_choices: more than 16 options under Kani");
        }
    }
    #[cfg(not(kani))]
    {
        for o in options {
            let (p, l) = msg_bytes(o);
            unsafe { sys::opt_str(p, l) }
        }
    }
    unsafe { sys::opt_count(want as i32) }
    let mask = ask_raw(PromptKind::ChoiceMulti, player_id, title, text)?;
    Ok(decode_multi_mask(mask, n, want))
}

/// Turn a multi-select answer mask into exactly `count` distinct indices in
/// `0..n`, ascending. Pads short masks with the lowest unset indices; keeps
/// the lowest `count` bits of a fat mask. Host answers that are negative or
/// name bits past `n` are treated as empty and padded.
fn decode_multi_mask(mask: i32, n: usize, count: usize) -> Vec<usize> {
    let count = count.min(n);
    let mut picked: Vec<usize> = Vec::with_capacity(count);
    let bits = if mask < 0 { 0u32 } else { mask as u32 };
    for i in 0..n {
        if picked.len() >= count {
            break;
        }
        if bits & (1u32 << i) != 0 {
            picked.push(i);
        }
    }
    // Pad with the lowest unset indices so the result is always `count` long.
    for i in 0..n {
        if picked.len() >= count {
            break;
        }
        if bits & (1u32 << i) == 0 {
            picked.push(i);
        }
    }
    picked.sort_unstable();
    picked
}

/// A yes/no ask.
pub fn ask_yes(player_id: i32, title: &Msg, text: &Msg) -> Result<bool, Prompt> {
    Ok(ask_raw(PromptKind::YesNo, player_id, title, text)? == 0)
}

/// Returns the chosen player.
pub fn ask_player(player_id: i32, title: &Msg, text: &Msg, players: &[i32]) -> Result<i32, Prompt> {
    #[cfg(kani)]
    {
        if let Some(&x) = players.get(0) {
            unsafe { sys::opt_int(x) }
        }
        if let Some(&x) = players.get(1) {
            unsafe { sys::opt_int(x) }
        }
        if let Some(&x) = players.get(2) {
            unsafe { sys::opt_int(x) }
        }
        if let Some(&x) = players.get(3) {
            unsafe { sys::opt_int(x) }
        }
        if players.len() > 4 {
            kani::assert(false, "ask_player: more than 4 players under Kani");
        }
        let i = ask_raw(PromptKind::Player, player_id, title, text)?;
        return Ok(players[(i.max(0) as usize).min(players.len().saturating_sub(1))]);
    }
    #[cfg(not(kani))]
    {
        for &x in players {
            unsafe { sys::opt_int(x) }
        }
        let i = ask_raw(PromptKind::Player, player_id, title, text)?;
        Ok(players[(i.max(0) as usize).min(players.len().saturating_sub(1))])
    }
}

/// The forced play target (ABI-04): the seat a forced play is made against,
/// written by the forcing card as the plan tag `"force"` before it plays the
/// card (and cleared afterwards). `-1` when unset.
///
/// The tag holds the seat **plus one**: `trig_move_tag` answers 0 for a
/// missing name and seat 0 is a valid holder, so the bare value cannot tell
/// "nobody" from "player 0".
#[cfg_attr(kani, inline(always))]
pub fn forced_target() -> i32 {
    let v = trigger::move_tag("force");
    if v > 0 {
        v - 1
    } else {
        -1
    }
}

/// [`ask_player`] that skips the prompt when a forced play already named the
/// target (ABI-04). `pool` is the card's own candidate set: when the forced
/// seat is in it, the designation is the force's, so the seat is returned
/// without pausing; otherwise this asks as usual. The caller still puts the
/// answer through its targeting gate, so Immune / Untargetable / Redirect
/// behave.
#[cfg_attr(kani, inline(always))]
pub fn ask_player_target(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    pool: &[i32],
) -> Result<i32, Prompt> {
    let forced = forced_target();
    if forced >= 0 && pool.contains(&forced) {
        return Ok(forced);
    }
    ask_player(player_id, title, text, pool)
}

/// A player ask where each option also names a **card** to show with it (returns
/// 「复制哪名玩家的团卡？」 -- the player's band card rides the option). The
/// answer is still the chosen player; only the label gains a `card` argument,
/// so a client renders the card in the option instead of a bare name.
///
/// Built on [`ask_choice`] (`PromptKind::Choice`) rather than `PromptKind::Player`
/// because only the string-option path can carry a card per option. Bot /
/// autopilot answering is unchanged (an index either way).
pub fn ask_player_cards(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    players: &[i32],
    cards: &[&str],
) -> Result<i32, Prompt> {
    #[cfg(kani)]
    {
        // Fixed 4-slot option array — no `Vec<Msg>` grow/drop.
        let mut options: [Msg; 4] = [Msg::new("ask.player"); 4];
        let mut n = 0usize;
        macro_rules! push {
            ($i:expr) => {
                if n < 4 {
                    if let (Some(&p), Some(&c)) = (players.get($i), cards.get($i)) {
                        options[n] = Msg::new("ask.player").player_id("who", p).card("card", c);
                        n += 1;
                    }
                }
            };
        }
        push!(0);
        push!(1);
        push!(2);
        push!(3);
        if n == 0 {
            return Ok(-1);
        }
        let i = ask_choice(player_id, title, text, &options[..n])?;
        return Ok(players[(i.max(0) as usize).min(players.len().saturating_sub(1))]);
    }
    #[cfg(not(kani))]
    {
        let n = players.len().min(cards.len());
        let mut options: Vec<Msg> = Vec::with_capacity(n);
        for (p, c) in players.iter().zip(cards.iter()).take(n) {
            options.push(Msg::new("ask.player").player_id("who", *p).card("card", c));
        }
        if options.is_empty() {
            return Ok(-1);
        }
        let i = ask_choice(player_id, title, text, &options)?;
        Ok(players[(i.max(0) as usize).min(players.len().saturating_sub(1))])
    }
}

/// Pick one of `cards` (ids); returns the index.
pub fn ask_card(player_id: i32, title: &Msg, text: &Msg, cards: &[&str]) -> Result<usize, Prompt> {
    #[cfg(kani)]
    {
        // Zero-loop option list (same 4-slot tree as `ask_choice`). The prompt
        // profile showed `ctx::ask_card` 31 — the `for c in cards` over a
        // symbolic-length slice.
        macro_rules! opt {
            ($i:expr) => {
                if let Some(c) = cards.get($i) {
                    // `opt_str` takes a serialized `Msg`, not a raw string -- a
                    // card id wrapped as `{{card}}` is the label. Under kani
                    // `Msg` is Copy and `msg_bytes` is two ints (no postcard Vec).
                    let (p, l) = msg_bytes(&Msg::new("ask.cardOption").card("card", c));
                    unsafe { sys::opt_str(p, l) }
                }
            };
        }
        opt!(0);
        opt!(1);
        opt!(2);
        opt!(3);
        if cards.len() > 4 {
            kani::assert(false, "ask_card: more than 4 cards under Kani");
        }
        let i = ask_raw(PromptKind::Card, player_id, title, text)?;
        return Ok((i.max(0) as usize).min(cards.len().saturating_sub(1)));
    }
    #[cfg(not(kani))]
    {
        for c in cards {
            // `opt_str` takes a serialized `Msg`, not a raw string -- a card id
            // wrapped as `{{card}}` is the label.
            let (p, l) = msg_bytes(&Msg::new("ask.cardOption").card("card", c));
            unsafe { sys::opt_str(p, l) }
        }
        let i = ask_raw(PromptKind::Card, player_id, title, text)?;
        Ok((i.max(0) as usize).min(cards.len().saturating_sub(1)))
    }
}

/// Ask which of `cards` (ids) to take out of `pile`, then remove that copy.
/// Returns the id taken.
///
/// `None` when `cards` is empty -- `empty`, if given, is logged first -- or
/// when the take found nothing. Pass a pre-filtered list to narrow the
/// candidates; [`pick_from_pile`] is the unfiltered pile-wide shape.
pub fn pick_take_from(
    player_id: i32,
    pile: CardPile,
    title: &Msg,
    text: &Msg,
    empty: Option<&Msg>,
    cards: &[NameId],
) -> Result<Option<NameId>, Prompt> {
    if cards.is_empty() {
        if let Some(m) = empty {
            log(player_id, m);
        }
        return Ok(None);
    }
    // Concrete cap: `cards.len()` is symbolic and the `collect` sizing
    // crosses the `ask_card` host call before the drop (Kani 0.68
    // `__rust_dealloc` layout check, see `owned_tiles`); callers cap at 8.
    let mut refs: Vec<&str> = Vec::with_capacity(8);
    let mut i = 0usize;
    while i < cards.len() {
        refs.push(cards[i].as_str());
        i += 1;
    }
    let pick = ask_card(player_id, title, text, &refs)?;
    let id = cards[pick.min(cards.len() - 1)];
    Ok(take_card(player_id, id.as_str(), pile).then_some(id))
}

/// [`pick_take_from`] over [`distinct_cards_in`]: the player picks one card
/// out of `pile` and that copy leaves it -- the usual 「从弃牌堆中选择一张牌」.
pub fn pick_from_pile(
    player_id: i32,
    pile: CardPile,
    title: &Msg,
    text: &Msg,
    empty: Option<&Msg>,
) -> Result<Option<NameId>, Prompt> {
    let ids = distinct_cards_in(player_id, pile);
    pick_take_from(player_id, pile, title, text, empty, &ids)
}

/// A number in `min..=max`, built as an [`ask_choice`] over the range (ranges in
/// the card pool are small; for a wide range, narrow the candidates yourself
/// first).
pub fn ask_number(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    min: i32,
    max: i32,
) -> Result<i32, Prompt> {
    let min = min.min(max);
    let max = max.max(min);
    #[cfg(kani)]
    {
        // Fixed 16-slot options. Covers the widest card range seen so far
        // (`fire_bird`'s 1..=10). A wider range is a loud FAIL (coverage limit).
        const CAP: usize = 16;
        let mut options: [Msg; CAP] = [Msg::new("ask.intOption"); CAP];
        let mut n = 0usize;
        let mut v = min;
        while v <= max && n < CAP {
            options[n] = Msg::new("ask.intOption").i("n", v as i64);
            n += 1;
            v += 1;
        }
        if v <= max {
            kani::assert(false, "ask_number: range wider than 16 under Kani");
        }
        if n == 0 {
            return Ok(min);
        }
        let i = ask_choice(player_id, title, text, &options[..n])?;
        return Ok(min + i as i32);
    }
    #[cfg(not(kani))]
    {
        let mut options: Vec<Msg> = Vec::new();
        for n in min..=max {
            options.push(Msg::new("ask.intOption").i("n", n as i64));
        }
        if options.is_empty() {
            return Ok(min);
        }
        for o in &options {
            let (p, l) = msg_bytes(o);
            unsafe { sys::opt_str(p, l) }
        }
        // Answered as an option *index*, but deliberately **not** clamped to
        // the current list the way `ask_choice` is: a body re-runs after its
        // own host writes, and those writes often shrink the range (a count
        // bounded by houses the body has already moved). The logged index was
        // taken against the longer list, so `min + index` recovers the value
        // the player chose; clamping would collapse it to `min`.
        let i = ask_raw(PromptKind::Choice, player_id, title, text)?;
        Ok(min + i.max(0))
    }
}

/// Run another card's `play` inside this run, as that card.
/// Returns where it says it goes (its `Dest`); moving it there is the caller's
/// job, since only the caller knows where the card came from.
#[cfg_attr(kani, inline(always))]
pub fn play_card(id: &str, player_id: i32) -> Result<Dest, Prompt> {
    let (p, l) = s(id);
    let v = asked(unsafe { sys::play_card(p, l, player_id) })?;
    Ok(Dest::from_i32(v))
}

/// Run one named entry of `id` nested, as `player_id`, with its field instance
/// bound. Body only -- ask [`query_guard`] first when the gate matters. The
/// invoke frame seeds `trigger::card` / `trigger::cards` with the invoked card
/// instead of raising a counteraction category.
#[cfg_attr(kani, inline(always))]
pub fn invoke_entry(player_id: i32, id: &str, entry_id: &str) -> Result<Dest, Prompt> {
    invoke_entry_flags(player_id, id, entry_id, 0)
}

/// [`invoke_entry`] for an entry whose cost the caller has already paid: the
/// body sees [`invoke_paid`] and must not collect it again.
#[cfg_attr(kani, inline(always))]
pub fn invoke_entry_paid(player_id: i32, id: &str, entry_id: &str) -> Result<Dest, Prompt> {
    invoke_entry_flags(player_id, id, entry_id, crate::abi::invoke::COST_PAID)
}

fn invoke_entry_flags(
    player_id: i32,
    id: &str,
    entry_id: &str,
    flags: i32,
) -> Result<Dest, Prompt> {
    let (p, l) = s(id);
    let (ep, el) = s(entry_id);
    let v = asked(unsafe { sys::invoke_entry(player_id, p, l, ep, el, flags) })?;
    Ok(Dest::from_i32(v))
}

/// The innermost invocation frame, if this body is running inside one.
#[cfg(kani)]
#[cfg_attr(kani, inline(always))]
pub fn invoke() -> Option<crate::abi::InvokeFrame> {
    // Under `cfg(kani)` the host writes a fixed 24-byte LE record
    // `[card, entry, by, by_instance, instance, paid, depth]` (two interning
    // `u16`s, then five `i32`s) -- the `field_instances` convention, no
    // postcard and no `String`.
    const RECORD: i32 = 24;
    if unsafe { sys::invoke_frame(0, 0) } != RECORD {
        return None;
    }
    let (p, scratch) = out_buf(RECORD as usize);
    if unsafe { sys::invoke_frame(p, RECORD) } != RECORD {
        return None;
    }
    let buf = out_read(p, RECORD, scratch);
    // `.get` (not `buf[i]`): no slice-index panic path for CBMC to explore.
    let byte = |i: usize| buf.get(i).copied().unwrap_or(0);
    let u16_at = |i: usize| u16::from_le_bytes([byte(i), byte(i + 1)]);
    let i32_at = |i: usize| i32::from_le_bytes([byte(i), byte(i + 1), byte(i + 2), byte(i + 3)]);
    Some(crate::abi::InvokeFrame {
        card: crate::intern::NameId(u16_at(0)),
        entry: crate::intern::NameId(u16_at(2)),
        by: i32_at(4),
        by_instance: i32_at(8),
        instance: i32_at(12),
        paid: i32_at(16) != 0,
        depth: i32_at(20),
    })
}

/// See the `cfg(kani)` variant above; this decodes postcard `InvokeFrame`.
#[cfg(not(kani))]
pub fn invoke() -> Option<crate::abi::InvokeFrame> {
    let n = unsafe { sys::invoke_frame(0, 0) };
    if n <= 0 {
        return None;
    }
    let (p, scratch) = out_buf(n as usize);
    if unsafe { sys::invoke_frame(p, n) } != n {
        return None;
    }
    let buf = out_read(p, n, scratch);
    postcard::from_bytes(&buf[..n as usize]).ok()
}

/// `invoke().is_some_and(|f| f.paid)` -- sugar, not a second import. A read,
/// not a decision.
pub fn invoke_paid() -> bool {
    invoke().is_some_and(|f| f.paid)
}

/// Run a skill's press entry for `player_id` -- sugar over
/// [`invoke_entry`] on [`entry::PRESS`], the card's first Play.
#[cfg_attr(kani, inline(always))]
pub fn invoke_skill(player_id: i32, id: &str) -> Result<Dest, Prompt> {
    invoke_entry(player_id, id, crate::abi::entry::PRESS)
}

/// Reserved entry ids -- see [`query_guard`]. Re-exported from
/// [`crate::abi::entry`] so the guest and the host share one spelling.
pub use crate::abi::entry;

/// Does the **named entry** of `card_id` admit for `player_id` right now?
///
/// Evaluates that entry's **condition + residual guard / play gate** through
/// `admits` (docs/GUARDS.md §4.4 item 6) and does **not** run its body. The
/// query half next to [`play_card`] / [`invoke_skill`] / [`send`] -- see
/// docs/ACTIVATION.md §3.
///
/// There is one query, not two. "May I press this card / use this skill?" is
/// "does the press entry admit?", asked of [`entry::PRESS`]:
///
/// ```ignore
/// ctx::query_guard(p, "skill:美竹兰:叛逆的红挑染", ctx::entry::PRESS)
/// ctx::query_guard(p, "AG:回家的路上绕个道", "decay")
/// ```
///
/// The entry is addressed by a stable **internal** slug the target card
/// declared (`On::….id("decay")`) or by a reserved [`entry`] id -- never by a
/// positional `(kind, trigger)` pair. That is what removes "which of this
/// card's three hooks?" ambiguity. Slugs are identity, not display text, and
/// are unique per card.
///
/// **Probe mode** -- never prompts. An entry whose residual guard would ask
/// answers `false`, the same way an eligibility probe folds
/// `RuleError::GuardPrompted` to "not eligible". Unknown card, unknown or
/// unnamed entry, or nesting overflow answer `false` too.
#[cfg_attr(kani, inline(always))]
pub fn query_guard(player_id: i32, card_id: &str, entry_id: &str) -> bool {
    let (p, l) = s(card_id);
    let (ep, el) = s(entry_id);
    unsafe { sys::query_guard(player_id, p, l, ep, el) != 0 }
}

/// The activation argument the running entry's `OP_DECLARE` collected
/// (docs/ACTIVATION.md §2), as the card's own `T`. `None` when nothing was
/// collected (no `declare` on the entry, or the host has no store).
///
/// Only the body's `drive` normally calls this -- it hands `T` on as the
/// body's second parameter. A body that wants the value directly may call it.
pub fn declared_arg<T: crate::ActivationArg>() -> Option<T> {
    let need = unsafe { sys::declared_arg(0, 0) };
    if need <= 0 {
        return None;
    }
    let (p, scratch) = out_buf(need as usize);
    let got = unsafe { sys::declared_arg(p, need) };
    if got != need {
        return None;
    }
    let buf = out_read(p, got, scratch);
    postcard::from_bytes(&buf).ok()
}

/// The move being planned: what an
/// `On::RollPlan` body shapes before the dice, and what a card shapes before
/// running a move of its own. (`trigger::move_*` is different: it describes the
/// move that *caused* the current trigger.)
pub mod plan {
    use super::*;

    /// The planned length; keeps the sign of a reverse walk.
    pub fn set_steps(n: i32) {
        unsafe { sys::set_steps(n) }
    }

    /// A card has claimed this move's plan at the pre-dice pass; later offers
    /// at the same pass stand down. One-shot: it lives on the move's plan
    /// frame and dies with the move. Written outside a move (a turn-start hook,
    /// a pressed skill) it lands on the staging plan and is consumed by the
    /// next main move.
    pub fn claim() {
        unsafe { sys::plan_claim() }
    }

    /// Has any card claimed the move in flight? `false` outside a move unless
    /// an out-of-move claim is still waiting for the next main move.
    pub fn is_claimed() -> bool {
        unsafe { sys::plan_claimed() != 0 }
    }

    /// 「上一名玩家代替进行此次投掷」 (幻觉来了) -- attribute the planned roll to
    /// another seat. The log line shows 「by」; the move still belongs to the
    /// original player, and 「所有影响投掷的效果服从于原本进行投掷的玩家」.
    pub fn set_roller(player_id: i32) {
        unsafe { sys::set_roller(player_id) }
    }

    /// Walk backwards.
    pub fn set_reverse(on: bool) {
        unsafe { sys::set_reverse(on as i32) }
    }

    /// A negative roll walks backwards instead of clamping to 0.
    pub fn set_signed(on: bool) {
        unsafe { sys::set_signed(on as i32) }
    }

    /// Force the walk to stop on this tile; -1 clears.
    pub fn set_stop_at(n: i32) {
        unsafe { sys::set_stop_at(n) }
    }

    /// Only odd (1) / even (0) tiles count; -1 either.
    pub fn set_parity(n: i32) {
        unsafe { sys::set_parity(n) }
    }

    /// Settle where the walk lands.
    pub fn set_settles(on: bool) {
        unsafe { sys::set_settles(on as i32) }
    }

    /// Settle on this tile instead of the landing; -1 clears.
    pub fn set_settle_tile(n: i32) {
        unsafe { sys::set_settle_tile(n) }
    }

    /// Money paid on the innermost live scope, in milli-units (500 = x0.5): the
    /// open settle, else the move in flight, else the next move. A second live
    /// write multiplies into the first (MONEY-01), and a repeated write of one
    /// body (STACK-01 re-run) composes once.
    pub fn set_pay_factor(n: i32) {
        unsafe { sys::set_pay_factor(n) }
    }

    /// Compose a pay factor into `scope` (milli). An unopened `this` scope
    /// falls forward to its next instance. A repeated write of one body
    /// (STACK-01 re-run) composes once -- see `Run::factor_write_key`.
    pub fn multiply_pay_factor(milli: i32, scope: crate::abi::PayFactorScope) {
        unsafe { sys::multiply_pay_factor(milli, scope.as_i32()) }
    }

    /// The scoped pay-factor product in milli: the move plan × every open
    /// settle. The per-payment rent factor, tile props and `milli_scale` are
    /// not in it.
    pub fn pay_factor() -> i32 {
        unsafe { sys::pay_factor() }
    }

    /// Rent paid on the open settle (or the move in flight), in milli-units
    /// (500 = x0.5). A second live write multiplies into the first (MONEY-01),
    /// and a repeated write of one body (STACK-01 re-run) composes once.
    pub fn set_rent_factor(n: i32) {
        unsafe { sys::set_rent_factor(n) }
    }

    /// The walk cannot buy where it lands.
    pub fn set_no_buy(on: bool) {
        unsafe { sys::set_no_buy(on as i32) }
    }

    /// The walk cannot build where it lands.
    pub fn set_no_build(on: bool) {
        unsafe { sys::set_no_build(on as i32) }
    }

    /// The walk may build anywhere it lands.
    pub fn set_build_anywhere(on: bool) {
        unsafe { sys::set_build_anywhere(on as i32) }
    }

    /// How the move gets there: [`MoveKind::Walk`] goes by the path,
    /// [`MoveKind::Teleport`] jumps to its destination (with `set_teleport_to`
    /// naming it, or the roll deriving it -- 「视为 [传送]（只触发终点）」).
    pub fn set_kind(kind: MoveKind) {
        unsafe { sys::set_kind(kind.as_i32()) }
    }

    /// Clamp the final face up to this, after the counteractions.
    pub fn set_min_roll(n: i32) {
        unsafe { sys::set_min_roll(n) }
    }

    /// Extra steps added to the walk length.
    pub fn set_extra_steps(n: i32) {
        unsafe { sys::set_extra_steps(n) }
    }

    /// Add to the walk's extra steps (signed).
    pub fn add_extra_steps(n: i32) {
        unsafe { sys::add_extra_steps(n) }
    }

    /// Drop a fixed step count so the next move rolls.
    pub fn clear_steps() {
        unsafe { sys::clear_steps() }
    }

    /// A forced move: not the main move unless [`set_main_override`].
    pub fn set_forced(on: bool) {
        unsafe { sys::set_forced(on as i32) }
    }

    /// 「视为主要移动」 -- keep a forced move of the turn player the main move.
    pub fn set_main_override(on: bool) {
        unsafe { sys::set_main_override(on as i32) }
    }

    /// This teleport replaces the pending walk instead of following it.
    pub fn void_walk() {
        unsafe { sys::void_walk() }
    }

    /// The planned teleport destination, `-1` when none.
    pub fn planned_teleport() -> i32 {
        unsafe { sys::move_planned_teleport() }
    }

    /// The plan half of the pay factor, in milli.
    pub fn move_pay_factor() -> i32 {
        unsafe { sys::move_pay_factor() }
    }

    /// The move's rent factor, in milli.
    pub fn rent_factor() -> i32 {
        unsafe { sys::move_rent_factor() }
    }

    /// The length this move will take.
    pub fn effective_steps() -> i32 {
        unsafe { sys::move_effective_steps() }
    }

    /// Is this a forced move?
    pub fn forced() -> bool {
        unsafe { sys::move_forced() != 0 }
    }

    /// Further steps appended to the walk length.
    pub fn set_more_steps(n: i32) {
        unsafe { sys::set_more_steps(n) }
    }

    /// A free-form per-card counter on this move. Card rules
    /// own effects like [火罐] rolls through these instead of an engine flag:
    /// the card that arms one tags the move (`set_tag("fireRoll", 1)`) and
    /// readers ask [`trigger::move_tag`].
    pub fn set_tag(key: &str, value: i32) {
        crate::pure::refuse_if_filter("plan::set_tag");
        let (p, l) = s(key);
        unsafe { sys::set_tag(p, l, value) }
    }

    /// `set_tag` with the key as an intern id (v78). Kept for host call sites
    /// that still name a tag by intern id; payload write-back uses the string
    /// form of [`Self::set_tag`].
    pub fn set_tag_id(key_id: i32, value: i32) {
        unsafe { sys::set_tag_id(key_id, value) }
    }

    /// Host-supplied recorded roll (v78), or `None` when the host did not
    /// pre-roll one for this raise.
    pub fn recorded_roll() -> Option<i32> {
        let v = unsafe { sys::recorded_roll() };
        // Flatten boundary: the wasm import is `i32`; `i32::MIN` is the wire
        // spelling of "no recorded roll".
        if v == i32::MIN {
            None
        } else {
            Some(v)
        }
    }

    /// Passing CiRCLE pays nothing on this walk.

    /// Settle this move as the 地产商 landing.
    pub fn set_settle_as_agent(on: bool) {
        unsafe { sys::set_settle_as_agent(on as i32) }
    }

    /// 「使你的下次主要移动结果对那些玩家一起执行」 -- record `player_id` as a
    /// **follower** of the move being planned.
    /// After the mover settles, the engine replays this move's result for each
    /// follower in the order they were added (「你先触发结算，此后其他玩家按
    /// 行动顺序依次触发结算」 -- add them in action order). The follower's
    /// replay carries the same plan (steps / kind / destination / `pay_factor`),
    /// so 「触发结算时进行的支付价格减半」 reaches them too.
    pub fn add_follower(player_id: i32) {
        unsafe { sys::plan_add_follower(player_id) }
    }

    /// The teleport's destination. -1 derives it from the
    /// roll (the 「视为 [传送]（只触发终点）」 shape).
    pub fn set_teleport_to(tile: i32) {
        unsafe { sys::set_teleport_to(tile) }
    }

    /// The tile the walk begins on instead of the player's own; -1 for the
    /// player's own. `reason` is shown on the move's log line.
    pub fn set_start(tile: i32, reason: &str) {
        let (p, l) = s(reason);
        unsafe { sys::set_start(tile, p, l) }
    }

    /// Replace the roll's dice table with `count`d`sides`.
    pub fn set_base_dice(count: i32, sides: i32, reason: &str) {
        let (p, l) = s(reason);
        unsafe { sys::set_base_dice(count, sides, p, l) }
    }

    /// Append `count`d`sides` to the roll's base dice.
    pub fn add_base_dice(count: i32, sides: i32, reason: &str) {
        let (p, l) = s(reason);
        unsafe { sys::add_base_dice(count, sides, p, l) }
    }

    /// Append `count`d`sides` to the roll as an extra term. A flat add is a
    /// A flat add is a `0`-sided term (`add_extra_dice(n, 0, reason)`).
    pub fn add_extra_dice(count: i32, sides: i32, reason: &str) {
        let (p, l) = s(reason);
        unsafe { sys::add_extra_dice(count, sides, p, l) }
    }

    /// Did the walk stop before its full length? Read-only: the walk loop sets
    /// it. To force a stop, name the tile with [`Self::set_stop_at`].
    pub fn stopped() -> bool {
        unsafe { sys::move_stopped() != 0 }
    }

    /// The forced stop tile, or -1.
    pub fn stop_at() -> i32 {
        unsafe { sys::move_stop_at() }
    }

    /// The parity filter: -1 either, 0 even, 1 odd.
    pub fn parity() -> i32 {
        unsafe { sys::move_parity() }
    }

    /// Does the walk settle where it lands?
    pub fn settles() -> bool {
        unsafe { sys::move_settles() != 0 }
    }

    /// The planned length.
    pub fn steps() -> i32 {
        unsafe { sys::move_steps() }
    }

    /// Steps left to walk.
    pub fn remaining() -> i32 {
        unsafe { sys::move_remaining() }
    }

    /// Steps walked.
    pub fn total() -> i32 {
        unsafe { sys::move_total() }
    }

    /// Walk direction: +1 forwards, -1 backwards.
    pub fn dir() -> i32 {
        unsafe { sys::move_dir() }
    }
    /// names the whole face means exactly that face.
    pub fn clear_dice() {
        unsafe { sys::clear_dice() }
    }
    /// May this move build where it lands?
    pub fn set_can_build(on: bool) {
        unsafe { sys::set_can_build(on as i32) }
    }
}

/// For a single-target card -- try to target `player_id`.
/// `None` when the targeting failed (out, off-board, immune, untargetable, or the
/// `target` [反击] window cancelled it); otherwise the player actually targeted,
/// which a field `redirect` hook may have changed.
pub fn target(player_id: i32) -> Option<i32> {
    let r = unsafe { sys::target(player_id, -1, 1) };
    (r >= 0).then_some(r)
}

/// Target **yourself** through the full pipeline (「指定」 yourself).
/// `Some(hit)` is the seat actually named — a `redirect` may still move it
/// elsewhere; `None` when the designation was refused or cancelled.
/// Sugar over [`target`]; the engine path is identical.
pub fn target_self(player_id: i32) -> Option<i32> {
    target(player_id)
}

/// Target each player in turn (no redirect);
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

/// Try to target `tile` (and so its owner, when
/// someone else owns it). False when the targeting failed.
pub fn target_tile(tile: i32) -> bool {
    unsafe { sys::target(-1, tile, 0) >= 0 }
}

/// The nearest other present player(s) to `by` by `kind`, seat order on ties.
/// `Forward` excludes a co-located seat; `Shortest` includes it. Never a
/// first-found seat: every seat at the minimal distance is returned.
#[cfg(kani)]
pub fn nearest(by: i32, kind: DistanceKind) -> Vec<i32> {
    nearest_out(by, kind, None)
}

#[cfg(not(kani))]
pub fn nearest(by: i32, kind: DistanceKind) -> Vec<i32> {
    let cap = 4096;
    let (p, scratch) = out_buf(cap as usize);
    let n = unsafe { sys::nearest(by, kind.as_i32(), p, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    let buf = out_read(p, n, scratch);
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// [`nearest`], plus every present seat holding an instance whose prop `key`
/// equals `by`, in distance-then-seat order and deduplicated. The raw nearest
/// set stays first. Generic: the caller supplies the key.
#[cfg(kani)]
pub fn nearest_with_prop(by: i32, kind: DistanceKind, key: &str) -> Vec<i32> {
    nearest_out(by, kind, Some(key))
}

#[cfg(not(kani))]
pub fn nearest_with_prop(by: i32, kind: DistanceKind, key: &str) -> Vec<i32> {
    let cap = 4096;
    let (kp, kl) = s(key);
    let (p, scratch) = out_buf(cap as usize);
    let n = unsafe { sys::nearest_with_prop(by, kind.as_i32(), kp, kl, p, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    let buf = out_read(p, n, scratch);
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// The shared `cfg(kani)` body: the host writes LE `i32`s (no postcard — its
/// `Vec` deserializer is a BMC cliff), the length is asked first.
#[cfg(kani)]
fn nearest_out(by: i32, kind: DistanceKind, key: Option<&str>) -> Vec<i32> {
    const MAX_N: usize = 8;
    let (n, kp, kl) = match key {
        Some(k) => {
            let (kp, kl) = s(k);
            let n = unsafe { sys::nearest_with_prop(by, kind.as_i32(), kp, kl, 0, 0) };
            (n, kp, kl)
        }
        None => (unsafe { sys::nearest(by, kind.as_i32(), 0, 0) }, 0, 0),
    };
    if n <= 0 || n as usize > MAX_N {
        return Vec::new();
    }
    let need = (n as usize) * 4;
    let (p, scratch) = out_buf(need);
    let got = unsafe {
        match key {
            Some(_) => sys::nearest_with_prop(by, kind.as_i32(), kp, kl, p, need as i32),
            None => sys::nearest(by, kind.as_i32(), p, need as i32),
        }
    };
    if got != n {
        return Vec::new();
    }
    let buf = out_read(p, need as i32, scratch);
    (0..n as usize)
        .map(|i| {
            let b = i * 4;
            i32::from_le_bytes([
                buf.get(b).copied().unwrap_or(0),
                buf.get(b + 1).copied().unwrap_or(0),
                buf.get(b + 2).copied().unwrap_or(0),
                buf.get(b + 3).copied().unwrap_or(0),
            ])
        })
        .collect()
}

/// Designations of `player_id` by other sources since `player_id`'s own turn
/// ended -- 「两个你的回合之间」 (`Mor:离心力，不为所动`). Zeroed at its own turn
/// start, not counted during its own turn, and a named-then-negated
/// designation still counts.
pub fn targeted_count(player_id: i32) -> i32 {
    unsafe { sys::targeted_count(player_id) }
}

/// 「当前回合内你每获得过一次资金」 -- money-ins for `player_id` during the
/// current turn (any cause: a print, a pay-player credit, a `gain_fixed`).
/// Zeroed for everyone at each turn start. This is what a hand card reads when
/// it has no field stand-in observing the gains (HHW:（育美）(2)).
pub fn gains_this_turn(player_id: i32) -> i32 {
    unsafe { sys::gains_this_turn(player_id) }
}

/// The **static targeting query**: which players the play being resolved (by
/// `player_id`) designates. Empty
/// when the play names nobody. This is what 「有[指定]目标」 /
/// 「取消其对目标之一的[指定]」 branches on, before the play's body has run.
pub fn designations(player_id: i32) -> Vec<i32> {
    let cap = 4096;
    let (p, scratch) = out_buf(cap as usize);
    let n = unsafe { sys::designations(player_id, p, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    let buf = out_read(p, n, scratch);
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// Per-pair cancel (「取消其对目标之一的[指定]」):
/// mark `seat`'s designation on the play being resolved as cancelled. The rest
/// of the play's designations still land.
pub fn cancel_designation(seat: i32) {
    unsafe { sys::cancel_designation(seat) }
}

/// Is `seat`'s designation on the play being resolved cancelled?
pub fn designation_cancelled(seat: i32) -> bool {
    unsafe { sys::designation_cancelled(seat) != 0 }
}

/// 真奈 join (CH-05): the seat self-targeted effects of counters answering
/// this link are redirected to (「改为你」). `None` outside a join.
pub fn join_redirect_target() -> Option<i32> {
    let v = unsafe { sys::join_redirect_target() };
    (v >= 0).then_some(v)
}

/// 真奈 join: mark that a joiner used the redirect (shielded the target), so
/// the join opener knows not to take its fallback draw.
pub fn set_join_used() {
    unsafe { sys::set_join_used() }
}

/// The seat a self-targeted effect of this counter should aim at: the join
/// redirect target when under a join (「改为你」), else `player_id` itself.
pub fn self_target(player_id: i32) -> i32 {
    join_redirect_target().unwrap_or(player_id)
}

/// Abnormal effects that got through to `player_id`
/// this turn (reset for every player at each turn start).
#[cfg_attr(kani, inline(always))]
pub fn abnormal_count(player_id: i32) -> i32 {
    unsafe { sys::abnormal_count(player_id) }
}

/// Is `id` placed on `player_id`'s field? `Some(tile)` with the tile it is bound to
/// (-1 when it is not on a tile), `None` when the player has no such card in play.
#[cfg_attr(kani, inline(always))]
pub fn placed_tile(player_id: i32, id: &str) -> Option<i32> {
    let (p, l) = s(id);
    let v = unsafe { sys::placed_tile(player_id, p, l) };
    (v >= -1).then_some(v)
}

/// Arm the doubling for the run in progress -- the *playing* card's
/// doubled-number slot. CiRCLE's band skill sets it from outside that card, which
/// is why it is a world setter and not something the card owns. `-1` clears.
pub fn set_play_doubled(n: i32) {
    unsafe { sys::set_play_doubled(n) }
}

/// A number from this card's text, doubled when the play doubles its `k`-th
/// number (「将…一个数字变为两倍」). Use it instead of a bare literal for the
/// numbers a doubling effect can target.
pub fn n(k: i32, value: i32) -> i32 {
    if unsafe { sys::play_doubled() } == k {
        // One-shot: 「将…一个数字变为两倍」 doubles a single tagged number,
        // then the flag is spent so a later `n` on the same play remains put.
        unsafe { sys::set_play_doubled(-1) };
        value * 2
    } else {
        value
    }
}

// ------------------------------------------------- turn plan & scheduling

/// Run this card's `On::Schedule(id)` entry (for `player_id`) when the
/// current turn ends, *after* the end-of-turn status wear-off (so a [停留] it
/// grants lasts through the next turn). The card need not be in play.
/// Scheduling twice runs it twice. An unknown / unmatching `id` is a no-op.
pub fn at_turn_end(player_id: i32, id: &str) {
    let (p, l) = s(id);
    unsafe { sys::schedule_turn_end(player_id, 0, p, l) }
}

/// Run this card's `On::Schedule(id)` entry (for `player_id`) when the
/// current turn ends, *before* the status wear-off (「回合结束时」 money losses,
/// discarding down to a hand size). An unknown / unmatching `id` is a no-op.
pub fn before_turn_end(player_id: i32, id: &str) {
    let (p, l) = s(id);
    unsafe { sys::schedule_turn_end(player_id, 2, p, l) }
}

/// 「你的下回合结束时」 -- run this card's `On::Schedule(id)` entry (for
/// `player_id`) at the end of `player_id`'s next turn (not the current one, if
/// it is `player_id`'s), after its wear-off. An unknown / unmatching `id` is a
/// no-op.
pub fn at_next_turn_end(player_id: i32, id: &str) {
    let (p, l) = s(id);
    unsafe { sys::schedule_turn_end(player_id, 1, p, l) }
}

/// `player_id`'s money cannot drop for the rest of this
/// turn (payments it would make are waived; auctions are not payments).
pub fn set_no_money_loss(player_id: i32) {
    unsafe { sys::set_no_money_loss(player_id) }
}

/// This turn's [主要移动] roll is `n`.
pub fn set_fixed_roll(n: i32, source: i32) {
    unsafe { sys::set_fixed_roll(n, source) }
}

/// This turn's fixed main-move roll, if a card set one.
pub fn fixed_roll() -> Option<i32> {
    let v = unsafe { sys::fixed_roll() };
    (v >= 0).then_some(v)
}

/// `player_id`'s next [主要移动] walks exactly `n` steps.
pub fn set_next_steps(player_id: i32, n: i32) {
    unsafe { sys::set_next_steps(player_id, n) }
}

/// Steps this turn's [主要移动] walked (0 = none yet).
pub fn turn_main_steps() -> i32 {
    unsafe { sys::turn_main_steps() }
}

/// The trigger a counteraction is being checked against.
/// The closest tile strictly along `axis` from `tile`, or `-1`. Board-only.
pub fn nearest_tile_on_axis(tile: i32, axis: i32) -> i32 {
    unsafe { sys::nearest_tile_on_axis(tile, axis) }
}

/// The farthest tile strictly along `axis` from `tile`, or `-1`. Board-only.
pub fn farthest_tile_on_axis(tile: i32, axis: i32) -> i32 {
    unsafe { sys::farthest_tile_on_axis(tile, axis) }
}

pub mod trigger {
    use super::*;

    pub fn kind() -> TriggerKind {
        let s = read_str(|b, c| unsafe { sys::trig_kind(b, c) });
        TriggerKind::from_str(&s)
    }

    pub fn player_id() -> i32 {
        unsafe { sys::trig_player() }
    }

    pub fn target() -> i32 {
        unsafe { sys::trig_target() }
    }

    /// The board position of this event (the slot stepped onto / settled at),
    /// `-1` where there is none. [`tile`](Self::tile) is the governing tile;
    /// the two are equal with no swap. Use the position for co-location and
    /// "was this physical slot the one settled"; identity readers take the tile.
    pub fn position() -> i32 {
        unsafe { sys::trig_position() }
    }

    pub fn tile() -> i32 {
        unsafe { sys::trig_tile() }
    }

    pub fn value() -> i32 {
        unsafe { sys::trig_value() }
    }

    /// How many tiles this move has traversed so far (0 when the trigger
    /// carries no move).
    pub fn move_path_length() -> i32 {
        unsafe { sys::trig_move_path_length() }
    }

    /// Index `index` of the move's path: the governing tile of that step.
    pub fn move_path(index: i32) -> i32 {
        unsafe { sys::trig_move_path(index) }
    }

    /// Empty otherwise.
    pub fn name() -> String {
        read_str(|b, c| unsafe { sys::trig_name(b, c) })
    }

    /// The raise's unique id, shared by every job the raise
    /// dispatches and stable across a replay of the same action. `0` outside a
    /// raise and on a hand-built test trigger. A hook keys one-shot claims on
    /// it (`ctx::prop(CLAIM) != trigger::uid()`).
    pub fn uid() -> i32 {
        unsafe { sys::trig_uid() }
    }

    /// The instance/row this raise is **about**: the changed
    /// instance on `counterChanged`, the mark's creating instance on
    /// `markChanged`, `-1` otherwise. Never restamped per hook job, so copy A
    /// can ignore a raise about copy B (`trigger::instance() != ctx::self_uid()`).
    pub fn instance() -> i32 {
        unsafe { sys::trig_instance() }
    }

    /// The transition this raise is about, `None` outside the zone family.
    pub fn zone() -> Option<ZoneChange> {
        let n = unsafe { sys::trigger_zone(0, 0) };
        if n <= 0 {
            return None;
        }
        let (p, scratch) = out_buf(n as usize);
        if unsafe { sys::trigger_zone(p, n) } != n {
            return None;
        }
        let buf = out_read(p, n, scratch);
        // The seam carries the `Option` as postcard wrote it (a `Some` tag
        // byte precedes the payload; `None` is the single `0`).
        postcard::from_bytes::<Option<ZoneChange>>(&buf[..n as usize])
            .ok()
            .flatten()
    }

    /// The play's outcome on `card` / `cardAfter`; `None` elsewhere.
    pub fn play_outcome() -> Option<PlayOutcome> {
        let v = unsafe { sys::trig_play_outcome() };
        (v >= 0).then(|| PlayOutcome::from_i32(v))
    }

    /// Why the player left on `beforeOut`; `None` elsewhere.
    pub fn out_cause() -> Option<OutCause> {
        let v = unsafe { sys::trig_out_cause() };
        (v >= 0).then(|| OutCause::from_i32(v))
    }

    /// The turn stage (0 = no turn; 1 开始 / 2 运营 / 3 移动 / 4 结束) active when this trigger fired.
    /// Only meaningful for kinds that aren't step-specific (e.g. `mortgage`
    /// can fire during both step 1 and step 3).
    pub fn step() -> i32 {
        unsafe { sys::trig_step() }
    }

    /// `by_card().is_some_and(|by| by != player)` means "another player's card did
    /// this to me". Distinct from `player_id()` (the mover/payer/target), which on a
    /// `pay` trigger is the *payer*, not the card that forced the payment.
    pub fn by_card() -> Option<i32> {
        let v = unsafe { sys::trig_by_card() };
        (v >= 0).then_some(v)
    }

    /// The **instance** whose card caused this trigger, `None` on a board
    /// raise. `by_card` is the seat; this is the identity that separates two
    /// copies of one card (a copy excludes only its own release).
    pub fn by_card_instance() -> Option<i32> {
        let v = unsafe { sys::trig_by_card_instance() };
        (v >= 0).then_some(v)
    }

    /// The `settle` link was negated: no body could run.
    pub fn settle_cancelled() -> bool {
        let facts = unsafe { sys::trig_settle_facts() };
        facts & crate::abi::settle_fact::CANCELLED != 0
    }

    /// The `settleBody` link was negated: another card replaced the body.
    pub fn settle_body_replaced() -> bool {
        let facts = unsafe { sys::trig_settle_facts() };
        facts & crate::abi::settle_fact::BODY_REPLACED != 0
    }

    /// A rent payment to this settle's owner, on the settle's own tile,
    /// committed.
    pub fn settle_paid() -> bool {
        let facts = unsafe { sys::trig_settle_facts() };
        facts & crate::abi::settle_fact::PAID != 0
    }

    /// What that payment moved (0 without [`settle_paid`]).
    pub fn settle_paid_value() -> i32 {
        unsafe { sys::trig_settle_paid_value() }
    }

    /// The seat a landed `redirect` moved this designation from, else `None`.
    pub fn redirected_from() -> Option<i32> {
        let v = unsafe { sys::trig_redirected_from() };
        (v >= 0).then_some(v)
    }

    /// Did a redirect land on this designation?
    pub fn redirected() -> bool {
        redirected_from().is_some()
    }

    /// Always false on a card-driven payment.
    pub fn pay_is_rent() -> bool {
        unsafe { sys::trig_pay_is_rent() != 0 }
    }

    /// Present on `diceRollResult` / `pass` / `settleBefore` / `settle` /
    /// `settleAfter` raised while a player is moving. A walk enters every tile it
    /// steps on; a teleport enters only its destination.
    pub fn move_kind() -> Option<MoveKind> {
        MoveKind::from_i32(unsafe { sys::trig_move_kind() })
    }

    /// False = a card effect prevented settle at all.
    pub fn move_settles() -> bool {
        unsafe { sys::trig_move_settles() != 0 }
    }

    /// A [火罐] roll is card-owned state: whoever armed it tagged the move
    /// (`"fireRoll"` by convention).
    pub fn move_tag(key: &str) -> i32 {
        let (p, l) = s(key);
        unsafe { sys::trig_move_tag(p, l) }
    }

    pub fn move_is_main() -> bool {
        unsafe { sys::trig_move_main() != 0 }
    }

    ///
    /// A `tileResolved` / `settleBody` hook uses this to tell a card's own
    /// redirected settle from a real player settle on the same tile (笑容大游行
    /// （3）'s 「在此卡放置的格子上[触发结算]后」 must not fire for the redirect
    /// that swap itself caused).
    pub fn is_settle_redirect() -> bool {
        unsafe { sys::trig_settle_redirect() != 0 }
    }

    /// Only meaningful when the move caused the trigger
    /// ([`move_kind`]`().is_some()`).
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

    /// The cards the trigger is about: the drawn cards on a `drew` trigger (in
    /// draw order), the skill id on a `skillUsed` trigger, the played card on
    /// `card` / `cardAfter` / `cardPlayed`. Empty otherwise.
    pub fn cards() -> Vec<NameId> {
        let need = unsafe { sys::trig_cards(0, 0) };
        if need <= 0 {
            return Vec::new();
        }
        let (p, scratch) = out_buf(need as usize);
        if unsafe { sys::trig_cards(p, need) } != need {
            return Vec::new();
        }
        let buf = out_read(p, need, scratch);
        decode_name_list(&buf)
    }

    pub fn move_remaining() -> i32 {
        unsafe { sys::trig_move_remaining() }
    }

    pub fn move_from() -> i32 {
        unsafe { sys::trig_move_from() }
    }

    pub fn move_total() -> i32 {
        unsafe { sys::trig_move_total() }
    }

    /// Was the triggering move a rolled walk?
    pub fn move_rolled() -> bool {
        unsafe { sys::trig_move_rolled() != 0 }
    }

    /// The running activation's ordinal; 0 outside a routine.
    #[doc(hidden)]
    pub fn activation_ordinal() -> i32 {
        unsafe { sys::trigger_activation_ordinal() }
    }

    /// Did the abnormal this trigger announces hold the move? Meaningful only
    /// on the `effect` and `abnormal` raises of an abnormal.
    #[doc(hidden)]
    pub fn abnormal_blocked() -> bool {
        unsafe { sys::trigger_abnormal_blocked() != 0 }
    }

    /// Did an abnormal land during the triggering move?
    #[doc(hidden)]
    pub fn move_abnormal() -> bool {
        unsafe { sys::trigger_move_abnormal() != 0 }
    }

    /// The triggering move's effective length.
    pub fn move_effective_steps() -> i32 {
        unsafe { sys::trig_move_effective_steps() }
    }

    pub fn move_roll() -> Option<i32> {
        let v = unsafe { sys::trig_move_roll() };
        (v >= 0).then_some(v)
    }

    /// 「当你使用火罐进行掷骰时」 reads this. [`crate::abi::roll_source::NONE`] on every non-roll
    /// trigger.
    pub fn roll_source() -> i32 {
        unsafe { sys::trig_roll_source() }
    }

    /// The roll transaction this trigger belongs to (`Trigger.roll`), `None`
    /// on every non-roll trigger. v109.
    pub fn roll_payload() -> Option<crate::abi::RollPayload> {
        const CAP: usize = 512;
        let (p, scratch) = out_buf(CAP);
        let n = unsafe { sys::trigger_roll(p, CAP as i32) };
        if n <= 0 || n as usize > CAP {
            return None;
        }
        let buf = out_read(p, n, scratch);
        postcard::from_bytes::<Option<crate::abi::RollPayload>>(&buf[..n as usize])
            .ok()
            .flatten()
    }

    /// The transaction id, `-1` outside a roll trigger.
    pub fn roll_id() -> i32 {
        roll_payload().map(|r| r.id).unwrap_or(-1)
    }

    /// The [`crate::abi::RollPurpose`] code, `None` outside a roll trigger.
    pub fn roll_purpose() -> Option<i32> {
        roll_payload().map(|r| r.purpose)
    }

    /// Has the transaction more than one cast (a reroll)?
    pub fn roll_rerolled() -> bool {
        roll_payload().map(|r| r.rerolled).unwrap_or(false)
    }

    /// The face a pick did not take, `None` when none.
    pub fn roll_unused_face() -> Option<i32> {
        roll_payload().map(|r| r.unused_face).filter(|&v| v >= 0)
    }

    /// The trigger's every cast face, in cast order.
    pub fn roll_faces() -> Vec<i32> {
        roll_payload()
            .map(|r| r.casts.into_iter().map(|c| c.value).collect())
            .unwrap_or_default()
    }

    // ---- v40 purchase payload (`docs/PURCHASE.md`) ------------------------

    pub fn buy_kind() -> i32 {
        unsafe { sys::trig_buy_kind() }
    }

    pub fn seller() -> i32 {
        unsafe { sys::trig_seller() }
    }

    pub fn price() -> i32 {
        unsafe { sys::trig_price() }
    }

    /// Rewrite the quoted price (the `BuyAdd` / `BuyMul` / `BuySet` stages).
    pub fn set_price(v: i32) {
        unsafe { sys::trig_set_price(v) }
    }

    pub fn deal_owner() -> i32 {
        unsafe { sys::trig_deal_owner() }
    }

    /// Rewrite the deal's post-commit owner (`BuyAssign`).
    pub fn set_deal_owner(v: i32) {
        unsafe { sys::trig_set_deal_owner(v) }
    }

    pub fn deal_houses() -> i32 {
        unsafe { sys::trig_deal_houses() }
    }

    /// Rewrite the deal's post-commit house count (`BuyAssign`, raze).
    pub fn set_deal_houses(v: i32) {
        unsafe { sys::trig_set_deal_houses(v) }
    }

    pub fn deal_mortgaged() -> bool {
        unsafe { sys::trig_deal_mortgaged() != 0 }
    }

    /// Rewrite the deal's post-commit mortgage flag (`BuyAssign`).
    pub fn set_deal_mortgaged(v: bool) {
        unsafe { sys::trig_set_deal_mortgaged(v as i32) }
    }

    /// A `BuyGate` refusal's reason key, shown instead of a bare cancel.
    pub fn set_reason(reason: &str) {
        let (p, l) = s(reason);
        unsafe { sys::trig_set_reason(p, l) }
    }

    pub fn set_move_roll(v: i32) {
        unsafe { sys::trig_set_move_roll(v) }
    }

    /// Rewrite the amount of a pending `pay`/`paid` trigger.
    /// `0` cancels the payment outright. The engine
    /// honours whatever this leaves on the trigger once the counteraction window
    /// resolves.
    pub fn set_pay_amount(v: i32) {
        unsafe { sys::trig_set_pay_amount(v) }
    }

    /// Combine with [`set_pay_amount`] to reshape a payment completely.
    pub fn set_pay_target(to: i32) {
        unsafe { sys::trig_set_pay_target(to) }
    }

    /// Rewrite the pending payment's kind. Valid in every pay window (a write
    /// at `paid` is observational only); last writer wins. `Loss` keeps the
    /// declared rent flag, the other kinds set / clear it.
    pub fn set_pay_kind(kind: crate::abi::PayKind) {
        unsafe { sys::trig_set_pay_kind(kind.as_i32()) }
    }

    /// Rewrite the pending payment's shape. Honoured only before the first
    /// share leg; a late write is dropped, logged, and answers `false`.
    pub fn set_pay_shape(shape: crate::abi::PayShape) -> bool {
        unsafe { sys::trig_set_pay_shape(shape.as_i32()) != 0 }
    }

    /// The tile of the innermost open settle, `-1` outside one.
    pub fn settle_tile() -> i32 {
        unsafe { sys::settle_tile() }
    }

    /// The settle's terminal outcome ([`crate::abi::SettleOutcome`]), `-1`
    /// outside `tileResolved`.
    pub fn settle_outcome() -> i32 {
        unsafe { sys::settle_outcome() }
    }

    /// Rewrite `t.target` -- on a `redirect` hook, the player that takes the hit
    /// instead.
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

    /// Answer an `AbnormalChoice` window with **yes**: the received movement
    /// abnormal lands after all (「可选择受到的…效果是否生效」, glossary 14).
    /// Without this write the gate stays refused; the body asks the holder
    /// with [`super::ask_yes`] and calls this on yes.
    pub fn allow_abnormal() {
        unsafe { sys::trig_set_abnormal_allowed() }
    }

    /// Take one recipient out of settlement. The effect still settles for
    /// everyone else -- this is *not* a complete invalidation; use
    /// [`set_cancelled`] for that. Counter cards differ on which they mean.
    pub fn spare(seat: i32) {
        unsafe { sys::trig_set_spare(seat) }
    }

    pub fn cancelled() -> bool {
        unsafe { sys::trig_cancelled() != 0 }
    }

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

    /// Declared, unresolved answers on this link whose entry declares
    /// [`crate::abi::AnswerEffect::NEGATES`] (design G06 §6.2).
    pub fn pending_negation() -> i32 {
        unsafe { sys::trigger_pending_negation() }
    }

    /// An extreme is armed on this play, or a declared answer on this link
    /// declares [`crate::abi::AnswerEffect::ARMS_EXTREME`].
    pub fn extreme_armed() -> bool {
        unsafe { sys::trigger_extreme_armed() != 0 }
    }

    /// The card whose effect this link is ([`Self`]'s `subject_card`): the
    /// played card on a root link, the counter's own card on a counter link.
    pub fn subject_card() -> Option<String> {
        let s = read_str(|b, c| unsafe { sys::trigger_subject_card(b, c) });
        (!s.is_empty()).then_some(s)
    }

    /// The card a `copied` counter link plays; `None` otherwise.
    pub fn link_copied() -> Option<String> {
        let s = read_str(|b, c| unsafe { sys::trigger_link_copied(b, c) });
        (!s.is_empty()).then_some(s)
    }

    /// 0 = the root link, n = n counters deep.
    pub fn chain_depth() -> i32 {
        unsafe { sys::trigger_chain_depth() }
    }

    /// The kind of the link at depth `i`, counting up from the root; `None`
    /// when `i` is out of range or the link is not on the chain.
    pub fn answered_kind(i: i32) -> Option<TriggerKind> {
        let s = read_str(|b, c| unsafe { sys::trigger_answered_kind(i, b, c) });
        (!s.is_empty()).then(|| TriggerKind::from_str(&s))
    }
}

/// 真奈 join window (design G06 §5.4). A `SELF_REDIRECT` counter opens a
/// window on the link it answers: the other seats may answer the same link as
/// if each were its target, and a landing counter shields the opener.
pub mod join {
    use alloc::vec::Vec;

    use crate::abi::PromptKind as _;
    use crate::Prompt;

    /// The opener when this counter answers inside a join window, `None`
    /// outside one.
    pub fn opener() -> Option<i32> {
        let v = unsafe { super::sys::join_opener() };
        (v >= 0).then_some(v)
    }

    /// The participating seats of the window this link answers, seat order.
    pub fn seats() -> Vec<i32> {
        const CAP: usize = 16;
        let (p, scratch) = super::out_buf(CAP);
        let n = unsafe { super::sys::join_seats(p, CAP as i32) };
        if n <= 0 {
            return Vec::new();
        }
        if n as usize > CAP {
            let (p2, scratch2) = super::out_buf(n as usize);
            let m = unsafe { super::sys::join_seats(p2, n) };
            if m <= 0 || m > n {
                return Vec::new();
            }
            let buf = super::out_read(p2, m, scratch2);
            return postcard::from_bytes(&buf[..m as usize]).unwrap_or_default();
        }
        let buf = super::out_read(p, n, scratch);
        postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
    }

    /// This joiner's counter shields the opener (「免于受到该影响」): marks the
    /// running seat and returns `true`. Idempotent; `false` outside a join.
    pub fn shield() -> bool {
        unsafe { super::sys::join_shield() != 0 }
    }

    /// Opener side: build and settle the nested join round on the answered
    /// link. `Err(Prompt)` if the round pauses; `true` when it completed.
    pub fn open() -> Result<bool, Prompt> {
        Ok(super::asked(unsafe { super::sys::join_open() })? != 0)
    }

    /// Opener side, after [`open`]: the round was built.
    pub fn opened() -> bool {
        unsafe { super::sys::join_opened() != 0 }
    }

    /// Opener side: how many seats declared (bars the fallback).
    pub fn joiner_count() -> i32 {
        unsafe { super::sys::join_joiner_count() }
    }

    /// Opener side: the `i`-th declarer, in declaration order (`-1` if none).
    pub fn joiner(i: i32) -> i32 {
        unsafe { super::sys::join_joiner(i) }
    }

    /// Opener side: did declaration `i` land and shield?
    pub fn joiner_shielded(i: i32) -> bool {
        unsafe { super::sys::join_joiner_shielded(i) != 0 }
    }

    /// Opener side: the designation declaration `i` named, `None` if none.
    pub fn designation(i: i32) -> Option<i32> {
        let v = unsafe { super::sys::join_designation(i) };
        (v >= 0).then_some(v)
    }
}

/// Distinct players who declared a [反击] against `seat` this turn, in
/// declaration order (「本回合内」 counters read it).
pub fn countered_by_this_turn(seat: i32) -> Vec<i32> {
    const CAP: usize = 8;
    let (p, scratch) = out_buf(CAP);
    let n = unsafe { sys::countered_by_this_turn(seat, p, CAP as i32) };
    if n <= 0 {
        return Vec::new();
    }
    if n as usize > CAP {
        let (p2, scratch2) = out_buf(n as usize);
        let m = unsafe { sys::countered_by_this_turn(seat, p2, n) };
        if m <= 0 || m > n {
            return Vec::new();
        }
        let buf = out_read(p2, m, scratch2);
        return postcard::from_bytes(&buf[..m as usize]).unwrap_or_default();
    }
    let buf = out_read(p, n, scratch);
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// The effects a chain link declares, with their recipients already named.
///
/// A [反击] card chooses how much of this to read. For the rulebook's recurring
/// 「被其他玩家的卡效果影响」 clause the *whole list* is the right view -- any
/// effect of that play touching me -- while a card like
/// `CRYCHIC:主唱太拼命了` (「一次性向其他玩家支付至少5000资金时」) wants one
/// entry. Both are here; the card picks.
pub mod effect {
    use super::{read_str, s, sys};
    use crate::abi::TriggerKind;

    /// How many effects this link declares.
    pub fn count() -> i32 {
        unsafe { sys::trig_effect_count() }
    }

    /// What effect `i` does, as a [`TriggerKind`] wire value.
    pub fn kind(i: i32) -> TriggerKind {
        let s = read_str(|b, c| unsafe { sys::trig_effect_kind(i, b, c) });
        TriggerKind::from_str(&s)
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
    ///
    /// A hand play's effects are not declared until the body runs; the play's
    /// own [反击] window is before that. Fall back to the card's static
    /// `DESIGNATES` list (`designations`) so 「被…效果影响」 of the play works
    /// there too (CROSS-TESTS C13: one timing on the play).
    pub fn hits(seat: i32) -> bool {
        if (0..count()).any(|i| target(i) == seat || from(i) == seat) {
            return true;
        }
        if count() == 0 {
            return super::designations(super::trigger::player_id()).contains(&seat);
        }
        false
    }

    /// Is any declared effect of `kind`? The per-effect view, for a card that
    /// means one thing specifically.
    pub fn has(kind: TriggerKind) -> bool {
        (0..count()).any(|i| self::kind(i) == kind)
    }

    /// Append an effect to the current link, naming its recipient now -- at
    /// declaration, not at settlement.
    pub fn declare(kind: TriggerKind, target: i32, from: i32, tile: i32, value: i32) {
        let (p, l) = s(&kind.spelling());
        unsafe { sys::declare_effect(p, l, target, from, tile, value) }
    }
}

// ------------------------------------------------------------- placement & skills

/// Take a *named* card off the player's field. True when it was there.
/// [`unplace_card`] is the special case where the named card is the one running.
#[cfg_attr(kani, inline(always))]
pub fn unplace_card_named(player_id: i32, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::unplace_card_named(player_id, p, l) != 0 }
}

/// Names of the player's non-zero counters whose name starts with `prefix`.
/// The listing half of the counter query; [`held`] reads one by name.
///
/// Under `cfg(kani)` the host answers LE `u16` [`NameId`]s (`buf == 0` is the
/// count probe) — never postcard, whose `Vec<&str>` decoder is a BMC cliff.
#[cfg(kani)]
pub fn held_names(player_id: i32, prefix: &str) -> Vec<NameId> {
    let (p, l) = s(prefix);
    let n = unsafe { sys::held_names(player_id, p, l, 0, 0) };
    if n <= 0 {
        return Vec::new();
    }
    let need = (n as usize) * 2;
    let (bp, scratch) = out_buf(need);
    if unsafe { sys::held_names(player_id, p, l, bp, need as i32) } != n {
        return Vec::new();
    }
    let buf = out_read(bp, need as i32, scratch);
    let mut out: Vec<NameId> = Vec::new();
    let mut i = 0usize;
    while i < n as usize {
        out.push(NameId(u16::from_le_bytes([
            buf.get(i * 2).copied().unwrap_or(0),
            buf.get(i * 2 + 1).copied().unwrap_or(0),
        ])));
        i += 1;
    }
    out
}

#[cfg(not(kani))]
pub fn held_names(player_id: i32, prefix: &str) -> Vec<NameId> {
    let (p, l) = s(prefix);
    let cap = 4096;
    let (bp, scratch) = out_buf(cap as usize);
    let n = unsafe { sys::held_names(player_id, p, l, bp, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    let buf = out_read(bp, n, scratch);
    decode_name_list(&buf[..n as usize])
}

/// Ask the engine whether an abnormal effect of
/// `kind` (`[强制停下]`, `[传送]`, ...) may land on `player_id`. Returns `false`
/// when a field card guarded it, the player is immune, or they are
/// the cannot-be-stopped counter. Call this **before** applying the effect, and skip on `false`.
pub fn gate(player_id: i32, kind: crate::abi::AbKind) -> bool {
    unsafe { sys::gate(player_id, kind.as_i32()) != 0 }
}

/// A full [触发结算] of `tile` for this player. The player does
/// not move; the tile's own effect resolves. `is_main_move` marks it as the turn's
/// landing (it writes `landed`).
pub fn settle_tile_at(player_id: i32, tile: i32, is_main_move: bool) -> bool {
    unsafe { sys::settle_tile_at(player_id, tile, is_main_move as i32) != 0 }
}

/// Prompt to build on one of `tiles`, then build there.
/// Silently skips when none of them can take a house.
pub fn card_offer_build(player_id: i32, tiles: &[i32]) -> bool {
    let mut buf = alloc::vec![0u8; tiles.len() * 4];
    for (i, &t) in tiles.iter().enumerate() {
        buf[i * 4..i * 4 + 4].copy_from_slice(&t.to_le_bytes());
    }
    let (p, l) = in_bytes(&buf);
    unsafe { sys::card_offer_build(player_id, p, l) != 0 }
}

/// Sum the planned move's dice tables into one face, without the [反击] window.
/// The `rollAfter` / `diceRollResult` hooks are **not** raised: a re-roll is
/// usually being requested from inside one of them, and re-raising would
/// recurse. Shorthand for [`roll_direct`] with [`crate::abi::RollPurpose::Move`];
/// still logs a `"dice"` event.
pub fn do_move_roll(player_id: i32) -> Result<i32, Prompt> {
    roll_direct(
        player_id,
        0,
        0,
        crate::abi::RollPurpose::Move,
        crate::abi::roll_source::CARD,
    )
}

/// Is this card in play at the player?
/// Ids of the player's placed field cards, in placement order.
#[cfg(kani)]
pub fn placed_cards(player_id: i32) -> Vec<NameId> {
    // Interned short static names — no `postcard`, no `from_raw_parts`.
    let cap = 16;
    let (p, scratch) = out_buf(cap as usize);
    let n = unsafe { sys::placed_cards(player_id, p, cap) };
    if n <= 0 {
        return Vec::new();
    }
    let buf = out_read(p, n, scratch);
    const SLOTS: [&str; 4] = ["p0", "p1", "p2", "p3"];
    let count = buf.first().copied().unwrap_or(0).min(4) as usize;
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < count {
        let slot = buf.get(1 + i).copied().unwrap_or(0) & 3;
        out.push(crate::intern::intern(SLOTS[slot as usize]));
        i += 1;
    }
    out
}

#[cfg(not(kani))]
pub fn placed_cards(player_id: i32) -> Vec<NameId> {
    let cap = 4096;
    let (p, scratch) = out_buf(cap as usize);
    let n = unsafe { sys::placed_cards(player_id, p, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    let buf = out_read(p, n, scratch);
    decode_name_list(&buf[..n as usize])
}

/// 「加盖房屋时半价」 / 「本回合加盖房屋变为免费」 -- the build cost as a
/// percentage of its table price (100 = full, 50 = half, 0 = free).
#[cfg_attr(kani, inline(always))]
pub fn set_build_cost_pct(pct: i32) {
    unsafe { sys::set_build_cost_pct(pct) }
}

/// Flip a placed card face-down / face-up.
#[cfg_attr(kani, inline(always))]
pub fn set_card_face_down(player_id: i32, card: &str, down: bool) -> bool {
    let (cp, cl) = s(card);
    unsafe { sys::set_card_face_down(player_id, cp, cl, down as i32) != 0 }
}

/// Is this placed card face-down? Field filters use this half of the
/// face-down test; [`cards_in`] lists face-down cards too.
#[cfg_attr(kani, inline(always))]
pub fn card_face_down(player_id: i32, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::card_face_down(player_id, p, l) != 0 }
}

// ------------------------------------------------------------- placement & skills

/// May this player build on this tile? The same gate the
/// build step uses, so a card choosing a destination cannot pick one the engine
/// would then refuse.
pub fn can_build_on(player_id: i32, tile: i32) -> bool {
    unsafe { sys::can_build_on(player_id, tile) != 0 }
}

/// Buy `tile` now (the 「必须购买」 clauses).
/// Kept as the P0 alias of [`buy`] with [`crate::abi::BuyKind::Card`].
/// `true` only when the deed actually changed hands (HOST-06 / ABI v53):
/// `false` on `BuyGate` / `buyBefore` cancel, already owned, unpaid quote or money.
pub fn card_buy(player_id: i32, tile: i32) -> bool {
    unsafe { sys::card_buy(player_id, tile) != 0 }
}

/// Batched purchase quote (`docs/PURCHASE.md`): what would `player_id` be
/// charged for each tile in `tiles`, and may they buy it at all?
///
/// Returns a postcard `Vec<(price, eligible)>` parallel to `tiles`
/// (`price = -1` when the tile is not buyable at all). The figure is the
/// same hook-aware quote [`buy`] charges (`BuyGate` / `BuyAdd` / `BuyMul` /
/// `BuySet`), so quote == charge (HOST-07 / ABI v53).
pub fn buy_quotes(player_id: i32, kind: i32, tiles: &[i32]) -> Vec<(i32, bool)> {
    let cap = 4096;
    let (op, scratch) = out_buf(cap as usize);
    // Raw i32 LE, matching the host's `chunks_exact(4)` reader.
    let mut raw = alloc::vec![0u8; tiles.len() * 4];
    for (i, &t) in tiles.iter().enumerate() {
        raw[i * 4..i * 4 + 4].copy_from_slice(&t.to_le_bytes());
    }
    let (ip, il) = in_bytes(&raw);
    let n = unsafe { sys::buy_quotes(player_id, kind, ip, il, op) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    let buf = out_read(op, n, scratch);
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// Which option `player_id`'s AI would take among `tiles` on an agent offer.
/// Returns the index; `tiles.len()` = skip.
pub fn ai_agent_choice(player_id: i32, tiles: &[i32]) -> i32 {
    let mut buf = alloc::vec![0u8; tiles.len() * 4];
    for (i, &t) in tiles.iter().enumerate() {
        buf[i * 4..i * 4 + 4].copy_from_slice(&t.to_le_bytes());
    }
    let (p, l) = in_bytes(&buf);
    unsafe { sys::ai_agent_choice(player_id, p, l) }
}

/// Buy `tile` as `kind` (`docs/PURCHASE.md`). Replaces [`card_buy`] for new
/// call sites; `kind` selects the pipeline shape (Land / Agent / Card run the
/// money pipeline, Force / Acquire name the owner as payee, Auction is the
/// direct win). `true` only when the deed actually changed hands (HOST-06).
pub fn buy(player_id: i32, tile: i32, kind: i32) -> bool {
    unsafe { sys::buy(player_id, tile, kind) != 0 }
}

/// 「收购」 -- take `tile` from `from` at `price`: pipeline pay, then assign →
/// `bought` → `buyAfter` (`docs/PURCHASE.md`). `true` only when the deed
/// actually changed hands (HOST-06).
pub fn acquire(player_id: i32, from: i32, tile: i32, price: i32) -> bool {
    unsafe { sys::acquire(player_id, from, tile, price) != 0 }
}

/// The agent offer's chosen branch: buy (`kind = 0`), build (`kind = 1`) or
/// half-rent settle on another player's deed (`kind = 2`) for `tile` from the
/// `agent` tile's colour set. `true` only when that branch actually committed
/// (HOST-06).
pub fn agent_offer(player_id: i32, agent: i32, tile: i32, kind: i32) -> bool {
    unsafe { sys::agent_offer(player_id, agent, tile, kind) != 0 }
}

/// Bind the running card's own def as a **turn-scoped** lingering instance
/// (`docs/PURCHASE.md`). `expires` is the turn count it
/// survives (`0` = this turn only). This is the hand-card home for 「本回合」
/// effects: a `BuyAdd` / `BuyMul` / `BuySet` / `BuyAssign` hook on the card's
/// own def reaches the buy pipeline, and a [`set_prop`] made before this call
/// lands on the instance's props (e.g. `prop::NO_BUILD`). Cleared at turn
/// start and carried across `NeedHost` by `adopt_turn_policy`.
#[cfg_attr(kani, inline(always))]
pub fn linger(player_id: i32, expires: i32) -> bool {
    unsafe { sys::linger(player_id, expires) != 0 }
}

/// Is this placed field card immune to **any** effect?
///
/// The cause-blind read: the any-flavour flag only, so an 「除拥有者以外」
/// card reads `false` here. A check that knows who is acting uses
/// [`is_immune_from_at`] instead, which also honours the owner exemption.
#[cfg_attr(kani, inline(always))]
pub fn card_immune(player_id: i32, card: &str) -> bool {
    let (cp, cl) = s(card);
    unsafe { sys::card_immune(player_id, cp, cl) != 0 }
}

/// Mortgage one of the player's deeds. `true` only when the deed actually
/// went onto mortgage (HOST-06).
#[cfg_attr(kani, inline(always))]
pub fn card_mortgage(player_id: i32, tile: i32) -> bool {
    unsafe { sys::card_mortgage(player_id, tile) != 0 }
}

/// Mechanism half of a mortgage the tile rule already approved: flip the
/// mortgage flag and credit `amount` directly (not through the money
/// pipeline -- 规则书 基础规则 6). Returns the amount actually credited
/// (`0` when the deed was already mortgaged / not owned).
#[cfg_attr(kani, inline(always))]
pub fn commit_mortgage(player_id: i32, tile: i32, amount: i32) -> i32 {
    unsafe { sys::commit_mortgage(player_id, tile, amount) }
}

/// Mechanism half of a redeem the tile rule already approved: clear the
/// mortgage flag and charge `amount`. Runs the `spendBefore` window (a raw
/// 消耗 debit), so it may prompt; answers the amount charged.
#[cfg_attr(kani, inline(always))]
pub fn commit_redeem(player_id: i32, tile: i32, amount: i32) -> Result<i32, Prompt> {
    asked(unsafe { sys::commit_redeem(player_id, tile, amount) })
}

/// Pay the buy price through the money pipeline (`PayKind::Buy`). `kind` is
/// the [`crate::abi::BuyKind`] wire value; Land / Agent allow the optional
/// mortgage-funding arm. Answers the amount actually paid (0 = refused).
/// May prompt (raise-funds / optional funding) and pause the run.
pub fn pay_buy(player_id: i32, tile: i32, amount: i32, kind: i32) -> Result<i32, Prompt> {
    asked(unsafe { sys::pay_buy(player_id, tile, amount, kind) })
}

/// Pay the build cost through the money pipeline (`PayKind::Build`).
/// Answers the amount actually paid (0 = refused). May prompt and pause.
pub fn pay_build(player_id: i32, tile: i32, amount: i32) -> Result<i32, Prompt> {
    asked(unsafe { sys::pay_build(player_id, tile, amount) })
}

/// Write the deed after a buy (`purchase::assign_deed`). Mechanism only --
/// the tile body already decided owner / houses / mortgaged (e.g. from a
/// `BuyAssign` filter fold).
pub fn assign_deed(player_id: i32, tile: i32, owner: i32, houses: i32, mortgaged: bool) -> i32 {
    unsafe { sys::assign_deed(player_id, tile, owner, houses, mortgaged as i32) }
}

/// Does this card's rule text mention `needle`? 「所有效果包含[奇迹水晶]的卡」
/// is this query.
#[cfg_attr(kani, inline(always))]
pub fn card_text_mentions(card: &str, needle: &str) -> bool {
    let (cp, cl) = s(card);
    let (np, nl) = s(needle);
    unsafe { sys::card_text_mentions(cp, cl, np, nl) != 0 }
}

/// One declared static property of the card rule `id` (`CardDef::props`,
/// [`crate::abi::prop`]); `0` when the card does not declare it (the defined
/// default for every key). Generic: the key is a wire name, no game
/// vocabulary in the query itself.
#[cfg_attr(kani, inline(always))]
pub fn card_prop(id: &str, key: &str) -> i32 {
    let (ip, il) = s(id);
    let (kp, kl) = s(key);
    unsafe { sys::card_prop(ip, il, kp, kl) }
}

/// A gain that goes through the full money pipeline.
///
/// `NEGATION-AUDIT` V4 (user ruling 2026-10-07): Tritone's 「立刻获得此次失去
/// 的资金金额」 is money movement and goes through the same pipeline as any
/// other gain -- the `effect` [反击] window, the modifier stages and the `pay`
/// settlement all see it. This is [`gain`] under a name the cards already use.
pub fn gain_fixed(player_id: i32, amount: i32, reason: &Msg) -> Result<i32, Prompt> {
    gain(player_id, amount, reason)
}

/// What this turn's [触发结算]s have cost the player so far.
#[cfg_attr(kani, inline(always))]
pub fn paid_in_settle() -> i32 {
    unsafe { sys::paid_in_settle() }
}

/// What `seat` paid other players during this turn's settles.
pub fn paid_in_settle_by(seat: i32) -> i32 {
    unsafe { sys::paid_in_settle_by(seat) }
}

/// The figure a fresh rent charge from `payer` on `tile` would move, scaled by
/// the tile's own factors and the standing plan / settle factors (not by any
/// in-flight payment's factor). `-1` = no rent charge. Rolls a fresh d20 on a
/// dice-rent tile.
pub fn rent_quote(payer: i32, tile: i32) -> i32 {
    unsafe { sys::rent_quote(payer, tile) }
}

/// Place a field card **on a tile** (the mark sits on the board at `tile`
/// rather than with its owner). Returns the new instance's uid.
#[cfg_attr(kani, inline(always))]
pub fn place_card_on(player_id: i32, tile: i32, card: &str, note: &Msg) -> i32 {
    let (cp, cl) = s(card);
    let (p, l) = msg_bytes(note);
    unsafe { sys::place_card_on(player_id, tile, cp, cl, p, l) }
}

/// Did the play being resolved come from the hand? `false` when the card was
/// played from somewhere else -- 「若此卡从手牌以外的地方打出」.
pub fn play_from_hand() -> bool {
    unsafe { sys::play_from_hand() != 0 }
}

/// 「下次盖房时减免N（可溢出），盖房后减少1层」 -- a layered cut on the build
/// cost. Each build pops one layer.
#[cfg_attr(kani, inline(always))]
pub fn set_build_discount(n: i32, layers: i32) {
    unsafe { sys::set_build_discount(n, layers) }
}

/// 「此卡不受…效果影响」. Set it at play time; effects that
/// would touch the card read [`card_immune`] and skip.
///
/// This writes the **any-flavour** flag. 「不受除拥有者以外的效果影响」 has no
/// name-based setter -- the card's own body uses
/// [`set_self_immune_except_owner`] -- and an effect from a given causer
/// checks with [`is_immune_from_at`] rather than [`card_immune`].
#[cfg_attr(kani, inline(always))]
pub fn set_card_immune(player_id: i32, card: &str, on: bool) -> bool {
    let (cp, cl) = s(card);
    unsafe { sys::set_card_immune(player_id, cp, cl, on as i32) != 0 }
}

/// Move a placed field card to `tile`. The card is already
/// in play; this only changes where it sits. `tile: -1` puts it back with its
/// owner.
#[cfg_attr(kani, inline(always))]
pub fn set_card_tile(player_id: i32, card: &str, tile: i32) -> bool {
    let (cp, cl) = s(card);
    unsafe { sys::set_card_tile(player_id, cp, cl, tile) != 0 }
}

/// Force the play's number ranges to their theoretical max (`1`) or min
/// (`-1`) -- 「以理论最大值或最小值结算」. `0` clears.
pub fn set_extreme(v: i32) {
    unsafe { sys::set_extreme(v) }
}

/// Every face rolled this turn, in order.
/// 「与本回合内你骰出过的所有骰点都不同」 compares against this.
pub fn turn_rolls() -> Vec<i32> {
    let cap = 4096;
    let (p, scratch) = out_buf(cap as usize);
    let n = unsafe { sys::turn_rolls(p, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    let buf = out_read(p, n, scratch);
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// Turn-start snapshot payload (`ruleset_turn_snap`): starting square, keyed
/// state map, that player's money and the board ownership arrays. Field order
/// mirrors `game_core::engine::TurnSnapshot` -- postcard carries no field names,
/// so the two must stay in step.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct TurnSnapshot {
    pub pos: i32,
    pub state: alloc::collections::BTreeMap<String, i32>,
    pub money: i32,
    pub owners: Vec<i32>,
    pub houses: Vec<i32>,
    pub mortgaged: Vec<bool>,
}

/// The host's postcard [`TurnSnapshot`]; `pos = -1` when there is no snapshot.
pub fn turn_snap_map(player_id: i32) -> TurnSnapshot {
    let cap = 16_384;
    let (p, scratch) = out_buf(cap);
    let n = unsafe { sys::turn_snapshot(player_id, p) };
    if n <= 0 || n as usize > cap {
        return TurnSnapshot {
            pos: -1,
            ..Default::default()
        };
    }
    let buf = out_read(p, n, scratch);
    postcard::from_bytes(&buf[..n as usize]).unwrap_or(TurnSnapshot {
        pos: -1,
        ..Default::default()
    })
}

/// `(pos, v0, v1, v2)` for the caller's three keys (the caller names them) --
/// the three layer counters 「回到起始地点并取消所有受到的效果」 restores.
pub fn turn_snapshot(player_id: i32, k0: &str, k1: &str, k2: &str) -> (i32, i32, i32, i32) {
    let snap = turn_snap_map(player_id);
    (
        snap.pos,
        snap.state.get(k0).copied().unwrap_or(0),
        snap.state.get(k1).copied().unwrap_or(0),
        snap.state.get(k2).copied().unwrap_or(0),
    )
}

/// Where the player stood when this turn started.
/// 「在Livehouse地块开始回合时」 is a question about that square.
pub fn turn_start_pos(player_id: i32) -> i32 {
    unsafe { sys::turn_start_pos(player_id) }
}

// --------------------------------------------- card crystals & build

/// Pay `tile`'s build cost and raise one house.
/// `true` only when a house was added (HOST-06): `false` on
/// `cant_build_on` / `buildBefore` cancel / unpaid cost / max houses.
pub fn card_build(player_id: i32, tile: i32) -> bool {
    unsafe { sys::card_build(player_id, tile) != 0 }
}

/// The price a house on `tile` would actually cost this turn:
/// `max(0, build_cost * build_cost_percent / 100 - build_discount)`.
pub fn build_price(tile: i32) -> i32 {
    unsafe { sys::build_price(tile) }
}

/// This turn's build percentage (100 = full).
pub fn build_cost_percent() -> i32 {
    unsafe { sys::build_cost_percent() }
}

/// This turn's layered build cut (absolute money, applied after the percent).
pub fn build_discount() -> i32 {
    unsafe { sys::build_discount() }
}

/// `cant_build_on` minus the ownership test: does the tile take a house at
/// all for `player_id`? For a free build on someone else's tile.
pub fn tile_buildable(player_id: i32, tile: i32) -> bool {
    unsafe { sys::tile_buildable(player_id, tile) != 0 }
}

/// Run the full build pipeline on `tile`: gate, hooks, price, commit.
/// `charge` chooses what is paid; `Fixed` uses `amount`, `Scaled` uses
/// `numerator / denominator` of the effective price; `owner_required` adds
/// the owner test to the gate. `true` iff a house committed.
pub fn card_build_as(
    player_id: i32,
    tile: i32,
    charge: crate::abi::BuildCharge,
    amount: i32,
    numerator: i32,
    denominator: i32,
    owner_required: bool,
) -> bool {
    unsafe {
        sys::card_build_as(
            player_id,
            tile,
            charge as i32,
            amount,
            numerator,
            denominator,
            owner_required as i32,
        ) != 0
    }
}

/// Where **this instance** sits, or -1 for "with its owner" / gone.
/// [`placed_tile`] is the form for some *other* card.
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
pub fn set_self_face_down(on: bool, cause: ZoneChangeCause) -> bool {
    unsafe { sys::set_self_face_down(on as i32, cause.as_i32()) != 0 }
}

/// Is **this instance** marked 「不受任何效果影响」?
pub fn self_immune() -> bool {
    unsafe { sys::self_immune() != 0 }
}

/// Mark **this instance** 「不受任何效果影响」; returns whether it is on the field.
pub fn set_self_immune(on: bool) -> bool {
    unsafe { sys::set_self_immune(on as i32) != 0 }
}

/// Mark **this instance** 「不受除拥有者以外的效果影响」; returns whether it
/// is on the field.
pub fn set_self_immune_except_owner(on: bool) -> bool {
    unsafe { sys::set_self_immune_except_owner(on as i32) != 0 }
}

/// Every card instance on `player_id`'s field, as `(uid, id)` in placement
/// order. Walk this rather than [`placed_cards`] whenever the next step
/// addresses the instance -- the *name* is not an identity, so a name-keyed
/// op would hit the first copy twice when one player holds two.
///
/// Under `cfg(kani)` the host writes LE `i32` pairs `[uid, name_id]` (never
/// postcard — the `Vec<(i32, String)>` deserializer is a BMC cliff). The
/// name is the interned [`NameId`] itself: no rebuild, no UTF-8 validation
/// loop, no `String`.
#[cfg(kani)]
pub fn field_instances(player_id: i32) -> Vec<(i32, NameId)> {
    const MAX_F: usize = 8;
    let n = unsafe { sys::field_instances(player_id, 0, 0) };
    if n <= 0 || n > MAX_F as i32 {
        return Vec::new();
    }
    let need = (n as usize) * 8;
    let (p, scratch) = out_buf(need);
    let got = unsafe { sys::field_instances(player_id, p, need as i32) };
    if got != n {
        return Vec::new();
    }
    let buf = out_read(p, need as i32, scratch);
    let mut out: Vec<(i32, NameId)> = Vec::new();
    let mut i = 0usize;
    while i < n as usize {
        let base = i * 8;
        let uid = i32::from_le_bytes([
            buf.get(base).copied().unwrap_or(0),
            buf.get(base + 1).copied().unwrap_or(0),
            buf.get(base + 2).copied().unwrap_or(0),
            buf.get(base + 3).copied().unwrap_or(0),
        ]);
        let nid = u16::from_le_bytes([
            buf.get(base + 4).copied().unwrap_or(0),
            buf.get(base + 5).copied().unwrap_or(0),
        ]);
        out.push((uid, NameId(nid)));
        i += 1;
    }
    out
}

#[cfg(not(kani))]
pub fn field_instances(player_id: i32) -> Vec<(i32, NameId)> {
    let cap = 8192;
    let (p, scratch) = out_buf(cap as usize);
    let n = unsafe { sys::field_instances(player_id, p, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    let buf = out_read(p, n, scratch);
    decode_uid_name_list(&buf[..n as usize])
}

/// The `uid` of `player_id`'s attached instance of `card`, `-1` when the seat
/// has none. The sim analogue of a `ExtraOf<T>` lookup: the next step addresses
/// the instance, so a *name* scan ([`placed_cards`]) is not enough when the
/// seat holds two copies (a borrowed skill row is one such instance).
pub fn extra_of(player_id: i32, card: &str) -> i32 {
    let id = intern(card);
    field_instances(player_id)
        .into_iter()
        .find(|&(_, nid)| nid == id)
        .map_or(-1, |(uid, _)| uid)
}

/// Take the instance at `uid` off play into `destination`; returns the owner
/// it left (`-1` when it was not in play). `destination` is a `Dest` code;
/// `Dest::Field` keeps it in play (a no-op move).
pub fn unplace_at(uid: i32, destination: Dest, cause: ZoneChangeCause) -> i32 {
    unsafe { sys::unplace_at(uid, destination as i32, cause.as_i32()) }
}

/// The zone of an instance; [`Zone::Nowhere`] for an unknown `uid`.
pub fn zone_of(instance: i32) -> Zone {
    Zone::from_i32(unsafe { sys::zone_of(instance) })
}

/// The instance `instance` sits on, `-1` when it is not attached.
pub fn attachment_host(instance: i32) -> i32 {
    unsafe { sys::attachment_host(instance) }
}

/// The instances attached to `host`, in filing order.
pub fn attached_cards(host: i32) -> Vec<i32> {
    uid_list(|b, c| unsafe { sys::attached_cards(host, b, c) })
}

/// The instances `creator` made, in creation order.
pub fn created_cards(creator: i32) -> Vec<i32> {
    uid_list(|b, c| unsafe { sys::created_cards(creator, b, c) })
}

/// Decode an `i32`-per-entry host list.
fn uid_list(call: impl Fn(i32, i32) -> i32) -> Vec<i32> {
    let n = call(0, 0);
    if n <= 0 {
        return Vec::new();
    }
    // The host writes LE `i32`s; the cap only bounds a garbage length.
    let need = ((n as usize) * 4).min(4096);
    let (p, scratch) = out_buf(need);
    if unsafe { call(p, need as i32) } != n {
        return Vec::new();
    }
    let buf = out_read(p, need as i32, scratch);
    let mut out = Vec::with_capacity(n as usize);
    let mut i = 0usize;
    while i < n as usize {
        let base = i * 4;
        out.push(i32::from_le_bytes([
            buf.get(base).copied().unwrap_or(0),
            buf.get(base + 1).copied().unwrap_or(0),
            buf.get(base + 2).copied().unwrap_or(0),
            buf.get(base + 3).copied().unwrap_or(0),
        ]));
        i += 1;
    }
    out
}

/// The instance's real owner (`FieldCard.owner`), `-1` when dead.
pub fn instance_owner(instance: i32) -> i32 {
    unsafe { sys::instance_owner(instance) }
}

/// Set the real owner without moving the row (「视为拥有」); `false` when dead.
pub fn set_instance_owner(instance: i32, owner: i32) -> bool {
    unsafe { sys::set_instance_owner(instance, owner) != 0 }
}

/// Record that `instance` was made by `creator` (`-1` clears); `false` when
/// `instance` is dead.
pub fn set_created_by_at(instance: i32, creator: i32) -> bool {
    unsafe { sys::set_created_by_at(instance, creator) != 0 }
}

/// Mark the instance as not having taken effect (or clear the mark). The
/// writer also reads the bit at the next Graveyard entry; `false` when dead.
pub fn set_ineffective_at(instance: i32, ineffective: bool) -> bool {
    unsafe { sys::set_ineffective_at(instance, ineffective as i32) != 0 }
}

/// Has the instance not taken effect?
pub fn is_ineffective_at(instance: i32) -> bool {
    unsafe { sys::is_ineffective_at(instance) != 0 }
}

/// The player's earliest-owned tile under the acquisition order, `-1` if none.
pub fn earliest_owned_tile(player_id: i32) -> i32 {
    unsafe { sys::earliest_owned_tile(player_id) }
}

/// Put a card (by name) onto `host`, face-down when `face_down`. A live
/// instance of the name is re-parented; otherwise the name comes off the
/// caller's hand into a fresh instance. Answers the attached uid, `-1` when
/// refused (a cycle, a dead host, no copy).
pub fn attach_card(host: i32, card: &str, cause: ZoneChangeCause, face_down: bool) -> i32 {
    let (p, l) = s(card);
    unsafe { sys::attach_card(host, p, l, cause.as_i32(), face_down as i32) }
}

/// Take `instance` off its host into `dest`; answers the instance's owner,
/// `-1` when it was not attached.
pub fn detach_card(instance: i32, dest: Dest, cause: ZoneChangeCause) -> i32 {
    unsafe { sys::detach_card(instance, dest as i32, cause.as_i32()) }
}

/// Move the deed row on `tile` to `owner` unchanged (identity, counters and
/// crystals stay). `false` when the tile has no row.
pub fn rehome_card(tile: i32, owner: i32) -> bool {
    unsafe { sys::rehome_card(tile, owner) != 0 }
}

/// Remove `card` from `player`'s hand and run its play now. `false` when the
/// card is not in that hand.
pub fn play_hand_card(player: i32, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::play_hand_card(player, p, l) != 0 }
}

/// Where the instance at `uid` sits (`Some(-1)` = with its owner, `None` = gone).
pub fn tile_of(uid: i32) -> Option<i32> {
    let v = unsafe { sys::tile_of(uid) };
    (v >= -1).then_some(v)
}

/// Move the instance at `uid` to `tile` (-1 = back with its owner).
pub fn set_tile_at(uid: i32, tile: i32) -> bool {
    unsafe { sys::set_tile_at(uid, tile) != 0 }
}

/// The card whose **printed surface** the instance at `uid` runs -- the
/// stand-in an effective-card marker names, else the row's own card. `None`
/// when `uid` is not in play. Not the same as the instance's own card: a body
/// that must name the current copy reads [`field_instances`] instead.
#[cfg(kani)]
pub fn effective_card_at(uid: i32) -> Option<NameId> {
    // Under `cfg(kani)` the host writes the interned id as LE `u16` (the
    // `field_instances` convention: no UTF-8 scan).
    let n = unsafe { sys::effective_card_at(uid, 0, 0) };
    if n <= 0 {
        return None;
    }
    let (p, scratch) = out_buf(n as usize);
    if unsafe { sys::effective_card_at(uid, p, n) } != n {
        return None;
    }
    let buf = out_read(p, n, scratch);
    let id = u16::from_le_bytes([
        buf.first().copied().unwrap_or(0),
        buf.get(1).copied().unwrap_or(0),
    ]);
    Some(crate::intern::NameId(id))
}

/// See the `cfg(kani)` variant above; this decodes UTF-8 bytes.
#[cfg(not(kani))]
pub fn effective_card_at(uid: i32) -> Option<NameId> {
    let n = unsafe { sys::effective_card_at(uid, 0, 0) };
    if n <= 0 {
        return None;
    }
    let (p, scratch) = out_buf(n as usize);
    if unsafe { sys::effective_card_at(uid, p, n) } != n {
        return None;
    }
    let buf = out_read(p, n, scratch);
    core::str::from_utf8(&buf[..n as usize])
        .ok()
        .map(crate::intern::intern)
}

/// Name another card's surface for this instance; `""` clears. Returns
/// whether the instance is on the field.
pub fn set_effective_card_at(uid: i32, card: &str) -> bool {
    // `s` is the name-handle facade (the intern handle under Kani, the arena
    // bytes on wasm) -- a raw `as_ptr` is untagged and the host model traps.
    let (p, l) = s(card);
    unsafe { sys::set_effective_card_at(uid, p, l) != 0 }
}

/// Did **this body** place the instance at `uid` in the current run? True for
/// a row the body itself minted (or re-minted) this drive, false for one that
/// was already on the field. A body that reacts to the field and also places
/// into it reads this so its own card does not trip the reaction on a re-run
/// (the replay re-executes the body against the world its own pass left).
pub fn is_own_placement(uid: i32) -> bool {
    unsafe { sys::is_own_placement(uid) != 0 }
}

/// Is the instance at `uid` face-down?
pub fn is_face_down_at(uid: i32) -> bool {
    unsafe { sys::is_face_down_at(uid) != 0 }
}

/// Flip the instance at `uid`; returns whether it exists. `cause` says why
/// (the flip's own `cardFaceFlipped` hooks may read it).
pub fn set_face_down_at(uid: i32, on: bool, cause: ZoneChangeCause) -> bool {
    unsafe { sys::set_face_down_at(uid, on as i32, cause.as_i32()) != 0 }
}

/// Is the instance at `uid` marked 「不受任何效果影响」?
///
/// The cause-blind read -- the any-flavour flag only. When the acting causer
/// is known, [`is_immune_from_at`] is the correct question.
pub fn is_immune_at(uid: i32) -> bool {
    unsafe { sys::is_immune_at(uid) != 0 }
}

/// Is the instance at `uid` immune to an effect from `by`? The any-flavour
/// flag, or the 「除拥有者以外」 flavour for a causer other than the owner.
pub fn is_immune_from_at(uid: i32, by: i32) -> bool {
    unsafe { sys::is_immune_from_at(uid, by) != 0 }
}

/// Mark the instance at `uid` 「不受任何效果影响」; returns whether it exists.
pub fn set_immune_at(uid: i32, on: bool) -> bool {
    unsafe { sys::set_immune_at(uid, on as i32) != 0 }
}

/// The uid of the instance named `name` on `player_id`'s field, or `None`.
/// A *resolver*, not an identity: with several copies in play it answers the
/// first, so a rule that means "a specific copy" has to hold the uid from
/// [`place_card`] instead of asking by name.
///
/// Under `cfg(kani)` this walks the compact `[uid, name_id]` host encoding and
/// compares intern ids — no postcard `Vec<(i32, String)>` decode at all.
#[cfg(kani)]
pub fn find_card(player_id: i32, name: &str) -> Option<i32> {
    find_card_id(player_id, crate::intern::intern(name))
}

/// [`find_card`] for an id already in hand: the resolver without the name
/// re-hash. Kani only — production decodes the text forms and has no id to give.
#[cfg(kani)]
#[cfg_attr(kani, inline(always))]
pub fn find_card_id(player_id: i32, want: NameId) -> Option<i32> {
    const MAX_F: usize = 8;
    let n = unsafe { sys::field_instances(player_id, 0, 0) };
    if n <= 0 || n > MAX_F as i32 {
        return None;
    }
    let need = (n as usize) * 8;
    let (p, scratch) = out_buf(need);
    let got = unsafe { sys::field_instances(player_id, p, need as i32) };
    if got != n {
        return None;
    }
    let buf = out_read(p, need as i32, scratch);
    let mut i = 0usize;
    while i < n as usize {
        let base = i * 8;
        let uid = i32::from_le_bytes([
            buf.get(base).copied().unwrap_or(0),
            buf.get(base + 1).copied().unwrap_or(0),
            buf.get(base + 2).copied().unwrap_or(0),
            buf.get(base + 3).copied().unwrap_or(0),
        ]);
        let nid = crate::intern::NameId(u16::from_le_bytes([
            buf.get(base + 4).copied().unwrap_or(0),
            buf.get(base + 5).copied().unwrap_or(0),
        ]));
        if nid == want {
            return Some(uid);
        }
        i += 1;
    }
    None
}

#[cfg(not(kani))]
pub fn find_card(player_id: i32, name: &str) -> Option<i32> {
    field_instances(player_id)
        .into_iter()
        .find(|(_, id)| id == name)
        .map(|(uid, _)| uid)
}
