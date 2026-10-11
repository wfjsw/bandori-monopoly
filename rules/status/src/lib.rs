//! **Status rule hosts** -- the `status:*` category: one standing pseudo card
//! per rulebook status marker, bound **once** to the neutral board owner
//! (`BOARD_OWNER`, `tile = -1`) via `game_core::data::status_rule_ids` ->
//! `bind_tiles`. See [`docs/STATUS-HOSTS.md`].
//!
//! | id | file | rulebook (`data/rules.txt`) |
//! |---|---|---|
//! | `status:stay` | `stay.rs` | 48 「[停留]」 + 47 「[无法移动]」 |
//! | `status:stun` | `stun.rs` | 49 「[晕眩]」 |
//! | `status:exile` | `exile.rs` | 50 「[除外]」 |
//! | `status:fire` | `fire.rs` | 126 「火罐」 |
//! | `status:unstoppable` | `unstoppable.rs` | 51 「[不可阻挡]」 |
//! | `status:no_hand` | `no_hand.rs` | `noHand` companion (card text) |
//!
//! The `mark:cp` precedent (`rules/tile_marks/src/cp.rs`, `docs/TILES.md`):
//! cards never special-case a status. They call this crate's canonical API
//! (`status::stay::add`, `status::fire::spend`, …) or message the host. The
//! engine keeps only generic primitives (keyed state, turn-flow verbs, player
//! gates) and stops naming stay / stun / exile / fire at policy sites.
//!
//! ## Storage (recorded in `docs/STATUS-HOSTS.md` §3.1)
//!
//! The named state keys `stay` / `stun` / `stunStart` / `exile` / `exileTo` /
//! `exileMain` / `fire` / `noHand` / `unstoppable` remain the player-bound
//! counters **and** the frozen webui wire (`MatchPlayer.state`). Each host is
//! the creating rule of its key(s): it owns wear-off, clamping, gates and
//! clauses. Wear-off is item-carried (`ctx::state::set_expires`,
//! `Tick::TurnEnd` / `Tick::TurnStart`).
//!
//! ## Gates
//!
//! `[无法移动]` (rules.txt 47) is a **generic move gate**, not a marker.
//! Stay / stun / exile all set it while their layers > 0. The engine reads
//! gates (`act` / `move` / `pay` / `on_board` / `targetable`); the hosts'
//! clauses say what the counters mean to each gate. Because `tick_state` and
//! the turn flow run outside a card run, the gate **derivation** lives
//! engine-side in one function that only the status keys touch
//! (`game_core::engine::ops::status_gates`). The hosts own the meaning; the
//! engine owns the timing.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod exile;
pub mod fire;
pub mod no_hand;
pub mod stay;
pub mod stun;
pub mod unstoppable;

pub use exile::STATUS_EXILE;
pub use fire::STATUS_FIRE;
pub use no_hand::STATUS_NO_HAND;
pub use stay::STATUS_STAY;
pub use stun::STATUS_STUN;
pub use unstoppable::STATUS_UNSTOPPABLE;

/// Every status rule, in registration order. The shipped module
/// (`card-all`) concatenates these with the card, skill, tile, mark and event
/// tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    STATUS_STAY,
    STATUS_STUN,
    STATUS_EXILE,
    STATUS_FIRE,
    STATUS_UNSTOPPABLE,
    STATUS_NO_HAND,
];

// --------------------------------------------------------------- canonical API
//
// One shape for every status, on the host module that owns it:
//
// ```ignore
// status::stay::add(p, n);  status::stay::get(p);  status::stay::clear(p);
// status::stun::add(p, n);  status::stun::get(p);  status::stun::clear(p);
// status::exile::send(p, n, to);  status::exile::get(p);  status::exile::clear(p);
// status::fire::{get, max, gain, spend, add_max};
// ```
//
// Thin wrappers over the generic `ctx::state` surface and the still-shipped
// engine verbs (Phase B shims; Phase C drops the old `ctx::{give_stay, …}`
// names once every card has moved).

// -------------------------------------------------------- generic gate readers
//
// Replacements for `ctx::can_pay` / `ctx::cant_move` (Phase C deletes those
// names). These are **generic** -- no status vocabulary -- and read the same
// thing the engine's policy sites read.

pub use card_sdk::ctx::{can_pay, cant_move};

/// `true` when the player may use [主动] skills and [手] effects.
/// Not out, not [晕眩], not [除外].
pub fn can_act(player_id: i32) -> bool {
    can_pay(player_id)
}

/// `None` when the player may still make this turn's [主要移动], else the
/// shared refusal (`status.move_*` messages).
pub fn move_blocked(player_id: i32) -> Option<card_sdk::Msg> {
    cant_move(player_id)
}