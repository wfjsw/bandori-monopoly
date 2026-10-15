//! Unit `skills_ras`: reference specs FROM the clause docs only.
//!
//! Sources: the roster character row `text` (band = RAISE A SUILEN) and
//! the roster band row `text` (`RAISE A SUILEN` / `UNSTOPPABLE`), quoted in
//! `docs/rulebook/spec/cards/ras.md`. Never derived from engine behaviour.

use crate::world::Effect;

// ===========================================================================
// C-RAS-rio-01 / C-RAS-masking-01 (fire-pot pass gains)
// 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」 -- 和奏瑞依
// 「（1）每次[经过]"白雪学园"或"银河拉面馆"时获得1个[火罐]（初始0，上限2）」 -- 佐藤益木
//
// The bodies call `ctx::gain_charge(player, 1, …)`. The printed cap is enforced
// by the engine (`game-core::engine::ops::gain_charge` clamps to `charge_max`).
// Under Kani `Op::gain_charge` does not clamp and `state_set` ignores the
// MIN/MAX field columns `fire_pot` writes — see the unit table
// (BLOCKED-HOST). Spec here is the **request**: +1 [火罐].
// ===========================================================================

/// The pass-gain request: +1 [火罐].
pub fn fire_pass_gain() -> i32 {
    1
}

/// Exact log for a pass gain of +1.
pub fn fire_pass_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainFire { player: p, n } if p == player && n == 1
        )
}

/// Mutant: the request is +2.
pub fn fire_pass_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainFire { player: p, n } if p == player && n == 2
        )
}