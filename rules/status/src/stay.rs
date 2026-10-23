//! `status:stay` -- the [停留] **owner**.
//!
//! 规则书（`data/rules.txt` 48）:
//! > · [停留]：处于该状态时[无法移动]。重复受到该效果时可叠加，玩家的每回合结束时移除一层。
//!
//! and the gate it shares with [晕眩] / [除外] (`data/rules.txt` 47):
//! > · [无法移动]：处于该状态时[主要移动]的[移动终点]视为当前所在格子且不[结算]。
//! >   无法使用非[传送]的移动效果（如果因效果随机到非[传送]的移动效果则抵消该效果）。
//!
//! ## Clause-by-clause
//!
//! * **「处于该状态时[无法移动]」** -- while `stay` layers > 0 and the player
//!   is not [不可阻挡], the generic **move gate** is set. The engine reads the
//!   gate; it does not name [停留]. (`game_core`'s `status_gates` derives it;
//!   see `docs/STATUS-HOSTS.md` §3.4.)
//! * **「重复受到该效果时可叠加」** -- `status::stay::add` is a layer add
//!   (`bandori_std::status::stay::add`, over `ctx::apply_abnormal`; the keyed
//!   state floors at 0).
//! * **「玩家的每回合结束时移除一层」** -- wear-off is the item's own
//!   `Tick::TurnEnd`, **declared by this host's grant verb**
//!   (`ctx::state::set_expires`). `default_expiry` in `game-core` is the
//!   safety net for raw writes; `tick_state` is key-agnostic.
//! * **Un-latch** -- dropping the last layer (with no [除外] holding the move
//!   either) un-skips the turn's main move. That is this host's
//!   count-changed clause (memory: *state clauses belong on events*), not a
//!   side effect of the grant site. The verb lives in
//!   `bandori_std::status::stay::{add,unlatch}` over the generic
//!   `skip_main_move` / `clear_move_skip`. **exile is not part of the latch**
//!   (closeout-2 ruling).
//!
//! [不可阻挡] (`data/rules.txt` 51) 「无视已存在的[停留]」 is a
//! cross-host clause: the move gate stays clear while `unstoppable` > 0.

use bandori_std::kind::abnormal;
use card_sdk::abi::{ActionKind, FilterKind};
use card_sdk::ctx::trigger;
use card_sdk::{Action, CardDef, Filter, GateValue, On};

/// The board-owned [停留] rule. One instance on `BOARD_OWNER`, `tile = -1`.
/// Registered in [`crate::CARDS`]; `bind_tiles` places it.
pub const STATUS_STAY: CardDef = CardDef::new(
    "status:stay",
    &[
        // 「处于该状态时[无法移动]」 -- when [停留] lands, the move gate is
        // already derived from the counter. This hook is the **clause**: a
        // card that wants to know 「受到[停留]时」 hears it here rather than
        // sniffing the grant site.
        On::Action(Action::Abnormal(
            "",
            None,
            Some(stay_landed),
            on_stay_landed,
        )),
        // 「处于该状态时[无法移动]」 + [不可阻挡] 「无视已存在的[停留]」
        // (rules.txt 47/48/51) -- the move gate, **CEL-only**: the condition is
        // the whole answer, so the host evaluates it natively in pure reads
        // (`Gate::Move` at turn start, the view's `can_roll_here`). Holding
        // another status (`unstoppable`) is why the vocabulary exposes
        // per-seat status functions.
        On::Filter(Filter::MoveGate(STAY_HOLDS_MOVE, None, refuse)),
    ],
)
.board_role("status")
// Turn-skip line for the move gate (`play.rs` closed-move skip). Stay
// closes no play path, so the play table is empty and the skip table
// carries the frozen key.
.gate_skip_msgs(&[(FilterKind::MoveGate.gate_id(), "log.stay_skip")]);

/// The state key this host owns (frozen webui wire).
pub const KEY: &str = "stay";

/// The condition under which [停留] closes the move gate. [停留] closes
/// neither the act nor the pay gate.
pub const STAY_HOLDS_MOVE: &str = "state(actor,'stay') > 0 && state(actor,'unstoppable') == 0";

/// The answer of a closed gate (only run if a gate is ever raised; pure reads
/// use the condition alone).
fn refuse(_owner: i32, mut g: GateValue) -> GateValue {
    g.allowed = Some(false); // deny
    g
}

/// 规则书: the hook fires for an [停留] abnormal landing on anyone.
/// `trigger::abnormal_kind()` names the kind; `t.player_id` is the target.
fn stay_landed(_owner: i32) -> bool {
    trigger::abnormal_kind() == Some(abnormal::Stay)
}

/// The [停留] landing clause. The layer itself is already on the player (the
/// engine's generic state write); this is where a host-side effect would run.
/// Today the only host-side effect is the un-latch, which the shared verb
/// (`bandori_std::status::stay::unlatch`) performs so it also covers
/// `tick_state` and StubRules matches. Logged as one 效果适用 entry when
/// anything happens.
fn on_stay_landed(_owner: i32) -> card_sdk::Asked {
    // Intentionally empty beyond the reason log: the move gate is derived
    // from the counter, and un-latch is the count-changed clause below.
    Ok(())
}

// ------------------------------------------------------------- canonical API
// Verb bodies live in `bandori_std::status::stay` (shared by cards / events);
// re-exported here so `rules_status::stay::…` call sites keep working.

pub use bandori_std::status::stay::{
    add, blocks_act, blocks_move, blocks_pay, clear, get, unlatch, unlatch_skip_move,
};

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn on_stay_landed_kani(player_id: i32) -> card_sdk::Asked {
    on_stay_landed(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn refuse_kani(player_id: i32, v: card_sdk::GateValue) -> card_sdk::GateValue {
    refuse(player_id, v)
}
