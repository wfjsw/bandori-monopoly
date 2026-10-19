//! **Status rule hosts** -- the `status:*` category: one standing pseudo card
//! per rulebook status marker, bound **once** to the neutral board owner
//! (`BOARD_OWNER`, `tile = -1`) via `game_core::data::status_rule_ids` ->
//! `bind_tiles`. See [`docs/STATUS-HOSTS.md`].
//!
//! ## Taxonomy (rulebook 「标志物解释」)
//!
//! | kind | ids | what they are |
//! |---|---|---|
//! | **状态 / conditions** | `status:stay`, `status:stun`, `status:exile`, `status:unstoppable` | 「处于该状态时…」 gates + ticks. The real status hosts. |
//! | **companions** | `status:no_hand` | `noHand` card-text slot (turn-flow 2→1→0 dance), not a glossary status. |
//!
//! 火罐 「技能指示物」 is **not** a status -- its kernel lives in
//! `bandori_std::fire` (same taxonomy as CP点 / 奇迹水晶).
//!
//! | id | file | rulebook (`data/rules.txt`) |
//! |---|---|---|
//! | `status:stay` | `stay.rs` | 48 「[停留]」 + 47 「[无法移动]」 |
//! | `status:stun` | `stun.rs` | 49 「[晕眩]」 |
//! | `status:exile` | `exile.rs` | 50 「[除外]」 |
//! | `status:unstoppable` | `unstoppable.rs` | 51 「[不可阻挡]」 |
//! | `status:no_hand` | `no_hand.rs` | `noHand` companion (card text) |
//!
//! The `mark:cp` precedent (`rules/markers/src/cp.rs`, `docs/TILES.md`):
//! cards never special-case a status. They call this crate's canonical API
//! (`status::stay::add`, …) or message the host. The
//! engine keeps only generic primitives (keyed state, turn-flow verbs, player
//! gates) and **no named mirror** of stay / stun / exile -- the frozen
//! spellings live in `bandori_std::state_key` / `bandori_std::status` (raise
//! names, i18n keys) and ride the manifest as data (`gate_msgs` /
//! `gate_skip_msgs` / `exemption_prop`).
//!
//! ## Storage (recorded in `docs/STATUS-HOSTS.md` §3.1)
//!
//! The named state keys `stay` / `stun` / `stunStart` / `exile` / `exileTo` /
//! `exileMain` / `noHand` / `unstoppable` remain the player-bound
//! counters **and** the frozen webui wire (`MatchPlayer.state`). Each host is
//! the creating rule of its key(s): it owns wear-off, clamping, gates and
//! clauses. Wear-off is item-carried (`ctx::state::set_expires`,
//! `Tick::TurnEnd` / `Tick::TurnStart`). (`charge` / `tempFire` are
//! `bandori_std::fire`'s, not a status host's.)
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
pub mod no_hand;
pub mod stay;
pub mod stun;
pub mod unstoppable;

pub use exile::STATUS_EXILE;
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
// bandori_std::fire::{get, max, gain, spend, add_max};
// ```
//
// Thin wrappers over the generic `ctx::state` surface and the still-shipped
// engine verbs (Phase B shims; Phase C drops the old `ctx::{give_stay, …}`
// names once every card has moved).

// ----------------------------------------------------------- status gates
//
// [无法移动] / 「无法收付款」 / 「无法使用[主动]技能…」 are **gates**, not markers.
// Each host declares the ones it holds as CEL-only `On::Gate` entries
// (`stay.rs` / `stun.rs` / `exile.rs`: `ActGate`, `PayGate`, `MoveGate`). The
// condition is the whole answer, so the engine and the host evaluate it
// natively in pure reads -- including from a card's guard. A gate entry with a
// residual guard or no condition is refused at ruleset load.

/// May `player_id` pay or receive payment (「无法收付款」)? Not out, and no
/// standing `PayGate` condition holds ([晕眩], [除外]). Pure; safe in guards.
pub fn can_pay(player_id: i32) -> bool {
    card_sdk::ctx::can_pay(player_id)
}

/// `None` when the player may still make this turn's [主要移动], else the
/// shared refusal (`status.move_*` messages). This is the generic turn-flow
/// question (not your turn / already moved / the turn's move is closed); the
/// status part is the `MoveGate` conditions, applied to the turn's latch at
/// turn start and when a layer is granted mid-turn.
pub fn move_blocked(player_id: i32) -> Option<card_sdk::Msg> {
    card_sdk::ctx::cant_move(player_id)
}

// TODO(ABI): `ActGate` has no guest reader (no card asks "may I act?" -- the
// engine applies it to [手] plays and turn start). A pure reader needs a host
// import (`gate_open(kind, player)`), which would replace `can_pay` too.
