//! `status:stun` -- the [晕眩] **owner**.
//!
//! 规则书（`data/rules.txt` 49）:
//! > · [晕眩]：处于该状态时[无法移动]，无法使用[主动]技能和手卡的[手]效果，无法收付款。
//! >   重复受到该效果时可叠加，玩家的每回合结束时移除一层。
//!
//! ## Clause-by-clause
//!
//! * **「处于该状态时[无法移动]」** -- generic **move gate** while layers > 0.
//! * **「无法使用[主动]技能和手卡的[手]效果」** -- generic **act gate**.
//! * **「无法收付款」** -- generic **pay gate** (also the payee filter in
//!   `play.rs` 「stunned payee」: a [晕眩]d player cannot be paid).
//! * **「重复受到该效果时可叠加」** -- layer add. (Unlike [除外], stacking is
//!   legal; do not copy exile's refuse-on-restack.)
//! * **「玩家的每回合结束时移除一层」** -- wear-off is the item's own
//!   `Tick::TurnEnd`, **declared by this host's grant verb**
//!   (`ctx::state::set_expires`). `default_expiry` in `game-core` is the
//!   safety net for raw writes; `tick_state` is key-agnostic.
//! * **`stunStart`** -- the layer that only starts counting next turn
//!   (`Tick::TurnStart`), declared the same way by [`add_start`].
//!   `tick_state(TurnStart)` folds it into `stun`. The view shows
//!   `stun + stunStart` as 眩晕 (`webui/src/scenes/board/Players.tsx`).
//! * **[不可阻挡] 「晕眩效果也不会获得新的层数」** (rules.txt 51, STA-01) --
//!   [`add`] / [`add_start`] refuse while `unstoppable > 0`, *before* the
//!   engine service (so no abnormal [反击] window opens for a refused grant).
//!   Existing layers still wear off (「在适当时机依旧掉层」).
//!
//! A stunned player's whole turn is skipped (`play.rs`: log `log.stunned_skip`,
//! `end_turn`) -- the engine reads the act / move gates, not `stunned()`.

use card_sdk::abi::{AbKind, ActionKind, FilterKind};
use card_sdk::ctx::trigger;
use card_sdk::{CardDef, On};

/// The state key this host owns (frozen webui wire).
pub const KEY: &str = "stun";
/// The layer that only starts counting next turn (`Tick::TurnStart`).
pub const KEY_START: &str = "stunStart";

/// The board-owned [晕眩] rule. One instance on `BOARD_OWNER`, `tile = -1`.
pub const STATUS_STUN: CardDef = CardDef::new(
    "status:stun",
    &[
        // 「受到[晕眩]时」 -- the landing clause, same shape as `status:stay`.
        On::Action(&[ActionKind::Abnormal], "", "", Some(stun_landed), on_stun_landed),
        // rules.txt 49: 「处于该状态时[无法移动]，无法使用[主动]技能和手卡的[手]
        // 效果，无法收付款」 -- three gates, **CEL-only**. `stunned` counts the
        // pending `stunStart` layer too (it already holds the gate).
        On::FilterFlag(
            &[FilterKind::ActGate, FilterKind::PayGate, FilterKind::MoveGate],
            STUNNED,
            None,
            refuse,
        ),
    ],
)
    .board_role("status");

/// The condition under which [晕眩] closes the act, pay and move gates.
pub const STUNNED: &str = "state(actor,'stun') + state(actor,'stunStart') > 0";

fn refuse(_owner: i32, _v: i32) -> bool {
    false // deny
}

fn stun_landed(_owner: i32) -> bool {
    trigger::abnormal_kind() == Some(AbKind::Stun)
}

fn on_stun_landed(_owner: i32) -> card_sdk::Asked {
    // The layer is already on the player (generic state write). Gate updates
    // are derived; a card that wants 「受到[晕眩]时」 listens to this hook.
    Ok(())
}

/// [晕眩]'s contribution to the generic move gate.
pub fn blocks_move(stun: i32) -> bool {
    stun > 0
}

/// 「无法使用[主动]技能和手卡的[手]效果」 -- act gate.
pub fn blocks_act(stun: i32) -> bool {
    stun > 0
}

/// 「无法收付款」 -- pay gate (and the payee filter).
pub fn blocks_pay(stun: i32) -> bool {
    stun > 0
}

/// [晕眩] does not close the on-board gate (the piece still stands on a tile).
pub fn blocks_on_board(_stun: i32) -> bool {
    false
}
// ------------------------------------------------------------- canonical API

/// Add `n` [晕眩] layers (floors at 0). Wear-off `Tick::TurnEnd`, declared
/// here. [不可阻挡] 「晕眩效果也不会获得新的层数」: a positive grant while
/// `unstoppable > 0` is refused before the engine service, so no abnormal
/// [反击] window opens (STA-01). Negative `n` always runs (strip layers).
pub fn add(player_id: i32, n: i32) {
    if n > 0 && crate::unstoppable::refuses_layers(crate::unstoppable::get(player_id)) {
        return;
    }
    card_sdk::ctx::give_stun(player_id, n);
    if n != 0 {
        card_sdk::ctx::state::set_expires(player_id, KEY, card_sdk::ctx::state::TURN_END);
    }
}

/// Add `n` `stunStart` layers -- the ones that only start counting next turn
/// (`Tick::TurnStart`). Same [不可阻挡] refusal as [`add`]. No abnormal
/// window today (raw state write; the engine service opens one only for
/// `stun`).
pub fn add_start(player_id: i32, n: i32) {
    if n > 0 && crate::unstoppable::refuses_layers(crate::unstoppable::get(player_id)) {
        return;
    }
    if n == 0 {
        return;
    }
    card_sdk::ctx::state::add(player_id, KEY_START, n);
    card_sdk::ctx::state::set_expires(player_id, KEY_START, card_sdk::ctx::state::TURN_START);
}

/// [晕眩] layers (`stun` + `stunStart`, what the view shows as 眩晕).
pub fn get(player_id: i32) -> i32 {
    card_sdk::ctx::stun_of(player_id)
}

/// Drop every [晕眩] layer (both counters).
pub fn clear(player_id: i32) {
    use card_sdk::ctx;
    let n = ctx::state::get(player_id, KEY);
    if n > 0 {
        ctx::state::add(player_id, KEY, -n);
    }
    let s = ctx::state::get(player_id, KEY_START);
    if s > 0 {
        ctx::state::add(player_id, KEY_START, -s);
    }
}

// Kani-only direct-call exports (ppcost). Production builds omit these.

#[cfg(kani)]
#[inline(always)]
pub fn on_stun_landed_kani(player_id: i32) -> card_sdk::Asked {
    on_stun_landed(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn refuse_kani(owner: i32, v: i32) -> bool {
    refuse(owner, v)
}
