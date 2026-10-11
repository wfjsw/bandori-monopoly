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
//!   grant sites consult this host before adding stun / exile layers.
//!   TODO(规则书): today `give_stun` / `give_exile` do not refuse; Phase C
//!   adds the check in the hosts' grant verbs.
//! * **「[停留]，[晕眩]，[除外]在适当时机依旧掉层」** -- wear-off is
//!   unchanged (the items keep their ticks).
//! * **「可选择受到的[传送]，[强制移动]，[强制停下]效果是否生效」** --
//!   TODO(规则书): a separate prompt surface; not in this batch.

use card_sdk::CardDef;

/// The board-owned [不可阻挡] rule. One instance on `BOARD_OWNER`, `tile = -1`.
pub const STATUS_UNSTOPPABLE: CardDef = CardDef::new("status:unstoppable", &[]);

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