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
//!   gate closes for a receiver's movement abnormal (the four kinds 4–7:
//!   teleport, forced move, forced stop, reverse) and the engine raises the
//!   `abnormalChoice` window on the holder. The [`on_abnormal_choice`] body
//!   asks the holder yes/no: yes lets the effect land (and still holds the
//!   move); no (and a timeout / the standard AI) keeps the refusal. A landed
//!   layer (stay / stun / exile) is exempted from holding the move.

use card_sdk::abi::{ActionKind, FilterKind};
use card_sdk::ctx::trigger;
use card_sdk::{Action, CardDef, Filter, GateValue, On};

/// The board-owned [不可阻挡] rule. One instance on `BOARD_OWNER`, `tile = -1`.
pub const STATUS_UNSTOPPABLE: CardDef = CardDef::new(
    "status:unstoppable",
    &[
        // 「可选择受到的…是否生效」 -- the engine raises `abnormalChoice` on
        // the holder when the gate below closed for a received movement
        // abnormal. This body asks the holder and, on yes, lets it land. A
        // self-caused move (`by == actor`) is chosen, not 「受到的」, and never
        // reaches here (the engine does not raise it).
        On::Action(Action::AbnormalChoice(
            "state(actor,'unstoppable') > 0 && value >= 4 && by != actor",
            None,
            on_abnormal_choice,
        )),
        // 「可选择受到的…是否生效」 -- the gate the choice answers: a received
        // movement abnormal (kinds 4-7) is refused unless the holder opted in,
        // so it does not land and opens no `effect` window. A self-caused move
        // (`by == actor`) is chosen, not 「受到的」, and walks.
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

/// 「可选择受到的…效果是否生效」 -- the holder's opt-in on the engine's
/// `abnormalChoice` window. The body runs for the board-owned instance, so the
/// holder is the raise seat (`trigger::player_id`), not `_owner`. Yes writes
/// the allow flag the gate reads back; no leaves the refusal (and its
/// `log.unstoppable_skip` line) unchanged. The ask's fallback is 「否」, so a
/// timeout and the standard bot both refuse.
fn on_abnormal_choice(_owner: i32) -> card_sdk::Asked {
    let p = trigger::player_id();
    let title = card_sdk::Msg::new("ask.unstoppable_choice.title");
    let text = card_sdk::Msg::new("ask.unstoppable_choice.text").player_id("who", p);
    if card_sdk::ctx::ask_yes(p, &title, &text)? {
        trigger::allow_abnormal();
    }
    Ok(())
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

#[cfg(kani)]
#[inline(always)]
pub fn on_abnormal_choice_kani(player_id: i32) -> card_sdk::Asked {
    on_abnormal_choice(player_id)
}
