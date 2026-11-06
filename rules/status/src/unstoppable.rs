//! `status:unstoppable` -- the [不可阻挡] **owner**.
//!
//! 规则书（`data/rules.txt` 51）:
//! > · [不可阻挡]：处于该状态时无视已存在的[停留]，[晕眩]效果也不会获得新的层数；
//! >   无法获得新的[除外]层数。[停留]，[晕眩]，[除外]在适当时机依旧掉层。
//! >   处于该状态时可选择受到的[传送]，[强制移动]，[强制停下]效果是否生效。
//!
//! ## Clause-by-clause
//!
//! * **「无视已存在的[停留]」** -- the stay host's move gate stays clear
//!   while `unstoppable` > 0 (`status::stay::blocks_move`).
//! * **「[晕眩]效果也不会获得新的层数；无法获得新的[除外]层数」** --
//!   the stun host's grant verbs (`status::stun::add` / `add_start`) and the
//!   exile host's grant (`rules/status/src/exile.rs::grant`) refuse while
//!   `unstoppable > 0` (STA-01, done for both halves).
//! * **「[停留]，[晕眩]，[除外]在适当时机依旧掉层」** -- wear-off is
//!   unchanged (the items keep their ticks).
//! * **「可选择受到的[传送]，[强制移动]，[强制停下]效果是否生效」** -- the
//!   abnormal gate: a received abnormal of the four movement kinds (teleport,
//!   forced move, forced stop, reverse) is refused outright, so it never lands;
//!   a landed layer (stay / stun / exile) is exempted from holding the move.

use card_sdk::abi::FilterKind;
use card_sdk::{CardDef, Filter, GateValue, On};

/// The board-owned [不可阻挡] rule. One instance on `BOARD_OWNER`, `tile = -1`.
pub const STATUS_UNSTOPPABLE: CardDef = CardDef::new(
    "status:unstoppable",
    &[
        // 「可选择受到的…是否生效」 -- a received movement abnormal (kinds 4-7)
        // is refused: it does not land and opens no `effect` window. A
        // self-caused move (`by == actor`) is chosen, not 「受到的」, and walks.
        On::Filter(Filter::AbnormalGate(
            "step == 0 && state(actor,'unstoppable') > 0 && value >= 4 && by != actor",
            None,
            refuse,
        )),
        // 「无视已存在的[停留]」 / the blocked-ness read. A gate entry's
        // condition is its **closed** case (a matching condition closes the
        // gate), so this names the kinds that *hold* the move: forced / stop /
        // reverse always do; a landed layer (kinds 1-3) does unless the mover
        // is [不可阻挡]; teleport (4) never does.
        On::Filter(Filter::AbnormalGate(
            "step == 1 && value != 4 && (value >= 5 || state(actor,'unstoppable') == 0)",
            None,
            exempt,
        )),
    ],
)
.board_role("status")
// Skip line for the refused landing (`abnormal_gate` on a closed gate).
.gate_skip_msgs(&[(FilterKind::AbnormalGate.gate_id(), "log.unstoppable_skip")]);

/// Does [不可阻挡] suppress a new [晕眩] / [除外] layer?
/// (「也不会获得新的层数；无法获得新的[除外]层数」.)
pub fn refuses_layers(unstoppable: i32) -> bool {
    unstoppable > 0
}

/// Does [不可阻挡] clear the stay half of the move gate?
/// (「无视已存在的[停留]」.)
pub fn ignores_stay(unstoppable: i32) -> bool {
    unstoppable > 0
}

// Both bodies are the shape the filter constructor requires. Gate entries are
// CEL-only: the runtime host never calls them, the **condition** decides and a
// matching condition is the closed case. The `allowed` write is the cancel
// signal `GateValue::write` raises (`Some(false)` -> `set_cancelled`); the
// Kani body dispatch (`*_kani`, at the bottom) still exercises it, and
// `Some(true)` is a no-op.
//
// The refusal answer: a received movement abnormal does not land.
fn refuse(_owner: i32, mut g: GateValue) -> GateValue {
    g.allowed = Some(false);
    g
}

// The exemption answer: an exempt kind (a landed layer, or any teleport) does
// not hold this mover's move (the landing still happens).
fn exempt(_owner: i32, mut g: GateValue) -> GateValue {
    g.allowed = Some(true);
    g
}
// ------------------------------------------------------------- canonical API

/// The state key this host owns.
pub const KEY: &str = "unstoppable";

/// Add `n` [不可阻挡] layers.
pub fn add(player_id: i32, n: i32) {
    if n == 0 {
        return;
    }
    card_sdk::ctx::state::add(player_id, KEY, n);
    bandori_std::status::declare_companion(player_id, KEY);
}

pub fn get(player_id: i32) -> i32 {
    card_sdk::ctx::state::get(player_id, KEY)
}

// `build.rs` emits these into the Kani body-dispatch table
// (`wrapper_check` requires one per `On::*` body).
#[cfg(kani)]
#[inline(always)]
pub fn refuse_kani(player_id: i32, v: card_sdk::GateValue) -> card_sdk::GateValue {
    refuse(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn exempt_kani(player_id: i32, v: card_sdk::GateValue) -> card_sdk::GateValue {
    exempt(player_id, v)
}
