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
//!   (`ctx::give_stay` today; the keyed state floors at 0).
//! * **「玩家的每回合结束时移除一层」** -- wear-off is the item's own
//!   `Tick::TurnEnd` (`default_expiry` stamps it on any write whose item has
//!   no tick). `tick_state` does the decrement; this host names no tick.
//! * **Un-latch** -- dropping the last layer (with no [除外] holding the move
//!   either) un-skips the turn's main move. That is this host's
//!   count-changed clause (memory: *state clauses belong on events*), not a
//!   side effect of the grant site. Phase B: the engine verb
//!   `give_stay` still calls the shared clause; the handler below is the
//!   documented owner and runs on `Stay` when the engine raises it.
//!
//! [不可阻挡] (`data/rules.txt` 51) 「无视已存在的[停留]」 is a
//! cross-host clause: the move gate stays clear while `unstoppable` > 0.

use card_sdk::abi::{AbKind, GateKind, HookKind};
use card_sdk::ctx::trigger;
use card_sdk::{CardDef, On};

/// The board-owned [停留] rule. One instance on `BOARD_OWNER`, `tile = -1`.
/// Registered in [`crate::CARDS`]; `bind_tiles` places it.
pub const STATUS_STAY: CardDef = CardDef::new(
    "status:stay",
    &[
        // 「处于该状态时[无法移动]」 -- when [停留] lands, the move gate is
        // already derived from the counter. This hook is the **clause**: a
        // card that wants to know 「受到[停留]时」 hears it here rather than
        // sniffing the grant site.
        On::Hook(&[HookKind::Abnormal], "", Some(stay_landed), on_stay_landed),
        // 「处于该状态时[无法移动]」 + [不可阻挡] 「无视已存在的[停留]」
        // (rules.txt 47/48/51) -- the move gate, **CEL-only**: the condition is
        // the whole answer, so the host evaluates it natively in pure reads
        // (`Gate::Move` at turn start, the view's `can_roll_here`). Holding
        // another status (`unstoppable`) is why the vocabulary exposes
        // per-seat status functions.
        On::Gate(&[GateKind::MoveGate], STAY_HOLDS_MOVE, None, refuse),
    ],
);

/// The condition under which [停留] closes the move gate. [停留] closes
/// neither the act nor the pay gate.
pub const STAY_HOLDS_MOVE: &str = "state(actor,'stay') > 0 && state(actor,'unstoppable') == 0";

/// The answer of a closed gate (only run if a gate is ever raised; pure reads
/// use the condition alone).
fn refuse(_owner: i32) -> card_sdk::Asked {
    trigger::set_cancelled();
    Ok(())
}

/// 规则书: the hook fires for an [停留] abnormal landing on anyone.
/// `trigger::abnormal_kind()` names the kind; `t.player_id` is the target.
fn stay_landed(_owner: i32) -> bool {
    trigger::abnormal_kind() == Some(AbKind::Stay)
}

/// The [停留] landing clause. The layer itself is already on the player (the
/// engine's generic state write); this is where a host-side effect would run.
/// Today the only host-side effect is the un-latch, which the shared engine
/// clause (`game_core` `stay_unlatch`) performs so it also covers `tick_state`
/// and StubRules matches. Logged as one 效果适用 entry when anything happens.
fn on_stay_landed(_owner: i32) -> card_sdk::Asked {
    // Intentionally empty beyond the reason log: the move gate is derived
    // from the counter, and un-latch is the count-changed clause below.
    Ok(())
}

/// [停留] count-changed clause (「每回合结束时移除一层」 reaching 0).
/// Dropping the last layer un-skips the turn's main move when [除外] is not
/// holding it either. The engine calls this shared clause from `give_stay`
/// (Phase B) so StubRules matches and `tick_state` behave the same; a
/// `CounterChanged` hook on the key is the Phase C shape once state writes
/// raise it.
pub fn unlatch_skip_move(stay_after: i32, exile_after: i32, unstoppable: i32) -> bool {
    // Returns whether the turn's main move is (now) free of the stay gate.
    stay_after <= 0 || unstoppable > 0 || exile_after <= 0
}

/// [停留]'s contribution to the generic move gate (`docs/STATUS-HOSTS.md`
/// §3.4). Kept here as the host's clause; `game_core`'s `status_gates` is the
/// engine-side derivation that reads it.
pub fn blocks_move(stay: i32, unstoppable: i32) -> bool {
    // 「处于该状态时[无法移动]」 vs [不可阻挡] 「无视已存在的[停留]」.
    stay > 0 && unstoppable <= 0
}

/// [停留] does **not** close the act or pay gates (only [晕眩] / [除外] do).
pub fn blocks_act(_stay: i32) -> bool {
    false
}

pub fn blocks_pay(_stay: i32) -> bool {
    false
}
// ------------------------------------------------------------- canonical API

/// Add `n` [停留] layers (negative removes; floors at 0). Wear-off is the
/// item's own `Tick::TurnEnd`. Un-latching the turn's move gate on the last
/// layer is [`unlatch_skip_move`] / the engine's `stay_unlatch`, not this
/// call site.
pub fn add(player_id: i32, n: i32) {
    card_sdk::ctx::give_stay(player_id, n);
}

/// [停留] layers on the player.
pub fn get(player_id: i32) -> i32 {
    card_sdk::ctx::stay_of(player_id)
}

/// Drop every [停留] layer.
pub fn clear(player_id: i32) {
    let n = get(player_id);
    if n > 0 {
        add(player_id, -n);
    }
}
