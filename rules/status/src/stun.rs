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

use bandori_std::kind::abnormal;
use card_sdk::abi::{ActionKind, FilterKind};
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
        // 「不可阻挡」 does **not** cancel this layer's move block (it only
        // 「无视」s the stay half).
        On::FilterFlag(&[FilterKind::ActGate, FilterKind::PayGate], STUNNED, None, refuse),
        On::FilterFlag(&[FilterKind::MoveGate], STUNNED, None, refuse),
    ],
)
    .board_role("status")
    // Play refusal (`cannot_play` → Act, unless the card carries
    // `exemption_prop`) and the turn-skip line (`play.rs` closed-act skip)
    // are **different frozen keys** on the same gate. `gate_msgs` is the play
    // path; `gate_skip_msgs` is the skip path. Host `gate_refusal_msg` reads
    // the play table; the turn-skip site should read the skip table.
    .gate_msgs(&[(FilterKind::ActGate.as_i32(), "err.play_stunned")])
    .gate_skip_msgs(&[(FilterKind::ActGate.as_i32(), "log.stunned_skip")])
    // 「（此卡可在眩晕时打出）」 cards name this prop; the engine skips the
    // act-gate check for them (`CardRules::card_act_exemption`).
    .exemption_prop(bandori_std::prop::PLAYABLE_STUNNED);

/// The condition under which [晕眩] closes the act, pay and move gates.
pub const STUNNED: &str = "state(actor,'stun') + state(actor,'stunStart') > 0";

fn refuse(_owner: i32, _v: i32) -> bool {
    false // deny
}

fn stun_landed(_owner: i32) -> bool {
    trigger::abnormal_kind() == Some(abnormal::Stun)
}

fn on_stun_landed(_owner: i32) -> card_sdk::Asked {
    // The layer is already on the player (generic state write). Gate updates
    // are derived; a card that wants 「受到[晕眩]时」 listens to this hook.
    Ok(())
}

// ------------------------------------------------------------- canonical API
// Verb bodies live in `bandori_std::status::stun`; re-exported here so
// `rules_status::stun::…` call sites keep working.

pub use bandori_std::status::stun::{
    add, add_start, blocks_act, blocks_move, blocks_on_board, blocks_pay, clear, get,
};

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