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
//! * **「重复受到该效果时可叠加」** -- layer add.
//! * **「玩家的每回合结束时移除一层」** -- `Tick::TurnEnd` on the item.
//! * **`stunStart`** -- the layer that only starts counting next turn
//!   (`Tick::TurnStart`). `default_expiry` stamps it; `tick_state(TurnStart)`
//!   folds it into `stun`. The view shows `stun + stunStart` as 眩晕
//!   (`webui/src/scenes/board/Players.tsx`).
//!
//! A stunned player's whole turn is skipped (`play.rs`: log `log.stunned_skip`,
//! `end_turn`) -- the engine reads the act / move gates, not `stunned()`.

use card_sdk::abi::{AbKind, HookKind};
use card_sdk::ctx::trigger;
use card_sdk::{CardDef, On};

/// The board-owned [晕眩] rule. One instance on `BOARD_OWNER`, `tile = -1`.
pub const STATUS_STUN: CardDef = CardDef::new(
    "status:stun",
    &[
        // 「受到[晕眩]时」 -- the landing clause, same shape as `status:stay`.
        On::Hook(&[HookKind::Abnormal], "", Some(stun_landed), on_stun_landed),
    ],
);

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