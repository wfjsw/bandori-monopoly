//! 火罐 -- a capped **技能指示物**, *not* a status (see `docs/STATUS-HOSTS.md`
//! taxonomy). Left as a thin named wrapper; the `markers` worktree owns the
//! charge/Marker layer end-to-end.
//!
//! 规则书（`data/rules.txt` 126）:
//! > · 火罐：技能指示物，在任意时刻，您持有的火罐数量不能超过持有上限
//!
//! ## Clause-by-clause
//!
//! * **「技能指示物」** -- 火罐 is a named counter on the player, capped by
//!   the character skill's 「持有上限」 (`StateVar.max`, written by
//!   `status::fire::add_max` / the skill body).
//! * **「在任意时刻，您持有的火罐数量不能超过持有上限」** -- gain is partial
//!   (at most up to the cap); a cap change pulls the value down.
//! * **Gain** logs one 效果适用 entry (`log.gain_charge`): which effect, why,
//!   how many landed, the new count and cap.
//! * **Spend** opens the `markerSpend` [反击] window **before** the pots move
//!   (user ruling 2026-10-07); a cancelled window spends nothing. After a
//!   successful spend the engine raises `fireSpent` (「每当你消耗火罐时」).
//!
//! 火罐 does not set any player gate -- it is a resource, not a status
//! marker. The cap is the only bound.

use card_sdk::CardDef;

/// The board-owned 火罐 rule. One instance on `BOARD_OWNER`, `tile = -1`.
/// No hooks of its own yet: the verbs live in [`crate::charge_api`] and the
/// `markerSpend` / `fireSpent` windows are generic engine points. A future
/// 「获得[火罐]时」 clause would hang off `markerGain`.
pub const STATUS_FIRE: CardDef = CardDef::new("status:fire", &[])
    .board_role("status");

/// Gain `n`, at most up to the cap; returns how many landed. This is the
/// host's clause (「不能超过持有上限」). The engine verb `gain_charge` is the
/// same shape and logs.
pub fn gain_clamped(have: i32, cap: i32, n: i32) -> i32 {
    if n <= 0 || cap <= 0 {
        return 0;
    }
    n.clamp(0, (cap - have).max(0))
}

/// A cap change pulls the value down under the new ceiling
/// (「不能超过持有上限」, applied 「在任意时刻」).
pub fn pull_under_cap(have: i32, cap: i32) -> i32 {
    have.min(cap.max(0))
}
// ------------------------------------------------------------- canonical API

/// The state key this host owns (frozen webui wire).
pub const KEY: &str = "charge";

/// 火罐 held.
pub fn get(player_id: i32) -> i32 {
    ruleset_core::ctx::charge(player_id)
}

/// The mandated cap (0 = none declared).
pub fn max(player_id: i32) -> i32 {
    ruleset_core::ctx::charge_max(player_id)
}

/// Gain `n` pots, at most up to the cap; 0 if nothing. Logs the gain.
pub fn gain(
    player_id: i32,
    n: i32,
    why: &card_sdk::Msg,
) -> Result<i32, card_sdk::Prompt> {
    ruleset_core::ctx::gain_charge(player_id, n, why)
}

/// Spend `n` pots. `markerSpend` opens **before** the pots move; a
/// cancelled window spends nothing.
pub fn spend(
    player_id: i32,
    n: i32,
    why: &card_sdk::Msg,
) -> Result<bool, card_sdk::Prompt> {
    ruleset_core::ctx::spend_charge(player_id, n, why)
}

/// Move the cap by `n`; pulls the value down under the new ceiling.
pub fn add_max(player_id: i32, n: i32) -> i32 {
    ruleset_core::ctx::add_charge_max(player_id, n)
}
