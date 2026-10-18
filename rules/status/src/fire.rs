//! 火罐 -- the capped **skill counter** (「技能指示物」), *not* a status marker.
//!
//! 规则书（`data/rules.txt` 126）:
//! > · 火罐：技能指示物，在任意时刻，您持有的火罐数量不能超过持有上限
//!
//! Taxonomy (see `docs/STATUS-HOSTS.md`): 留/晕眩/除外/不可阻挡 are 状态
//! (「处于该状态时…」 gates + ticks). 火罐 is a **指示物** -- a capped
//! spendable resource, kin of CP点 / 奇迹水晶 -- held on the generic `charge`
//! state key (value + `max` = cap) and spent through the generic
//! `HostRequest::Marker` [反击] window. It sets **no** player gate and has no
//! tick. The `status:fire` pseudo-card is a placeholder with no entries
//! (`status_rule_ids` binds it; nothing listens).
//!
//! ## Clause-by-clause (the resource API owns these)
//!
//! * **「技能指示物」** -- a named counter on the player, capped by the
//!   character skill's 「持有上限」 (`StateVar.max`, written by
//!   [`add_max`] / the skill body).
//! * **「在任意时刻，您持有的火罐数量不能超过持有上限」** -- gain is partial
//!   (at most up to the cap, [`gain_clamped`]); a cap change pulls the value
//!   down under the new ceiling ([`pull_under_cap`]).
//! * **Gain** logs one 效果适用 entry (`log.gain_charge`): which effect, why,
//!   how many landed, the new count and cap. Overflow is refused with
//!   `log.gain_charge_capped`.
//! * **Spend** opens the `markerSpend` [反击] window **before** the pots move
//!   (user ruling 2026-10-07); a cancelled window spends nothing. After a
//!   successful spend the engine raises `fireSpent` (「每当你消耗火罐时」).
//!
//! TODO(ABI): the value-moving primitive and the `markerSpend` window are
//! still the engine service (`gain_charge` / `spend_charge` /
//! `add_charge_max` + `HostRequest::Marker`). Genericize to
//! `marker::{gain_capped,spend}` and delete the game-named imports
//! (`docs/STATUS-HOSTS.md` §3.7). Until then the engine service is the mover;
//! this module is the **policy** the service mirrors.

use card_sdk::CardDef;

/// The board-owned placeholder for 火罐. One instance on `BOARD_OWNER`,
/// `tile = -1`. No hooks, no gates: 火罐 is a resource, not a status. Kept so
/// `status_rule_ids` / `CARDS` stay complete; dropping it is a cheap rename
/// later (nothing keys off `status:fire`).
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
// ------------------------------------------------------------- resource API

/// The state key this host owns (frozen webui wire). The generic name is
/// `charge`; the view still presents `fire`.
pub const KEY: &str = "charge";

/// 火罐 held.
pub fn get(player_id: i32) -> i32 {
    ruleset_core::ctx::charge(player_id)
}

/// The mandated cap (0 = none declared).
pub fn max(player_id: i32) -> i32 {
    ruleset_core::ctx::charge_max(player_id)
}

/// Gain `n` pots, at most up to the cap; 0 if nothing. Logs the gain
/// (`log.gain_charge`) and the overflow refusal (`log.gain_charge_capped`).
/// Policy is [`gain_clamped`]; the engine service moves the value and logs
/// (`TODO(ABI)`: `marker::gain_capped`). Passes `n` through so the service
/// also narrates a fully-capped refusal.
pub fn gain(
    player_id: i32,
    n: i32,
    why: &card_sdk::Msg,
) -> Result<i32, card_sdk::Prompt> {
    ruleset_core::ctx::gain_charge(player_id, n, why)
}

/// Spend `n` pots. `markerSpend` opens **before** the pots move; a
/// cancelled window spends nothing. Engine service (`TODO(ABI)`:
/// `marker::spend`).
pub fn spend(
    player_id: i32,
    n: i32,
    why: &card_sdk::Msg,
) -> Result<bool, card_sdk::Prompt> {
    ruleset_core::ctx::spend_charge(player_id, n, why)
}

/// Move the cap by `n`; pulls the value down under the new ceiling
/// ([`pull_under_cap`]). Engine service (`TODO(ABI)`: `marker::set_max`).
pub fn add_max(player_id: i32, n: i32) -> i32 {
    ruleset_core::ctx::add_charge_max(player_id, n)
}