//! Unit `skills_ag`: Afterglow character + band skill specs FROM the sheet.
//!
//! Sources: the roster character row `text` (band == "Afterglow"),
//! the roster band row `text` (Afterglow), clause doc
//! `docs/rulebook/spec/cards/ag.md` § Character skills / Band skill.

use crate::world::Effect;

// ===========================================================================
// C-AG-firepot-01 (shared (1) of all five)
// 「（1）每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）」
// OPEN-每三回合: whose turns? The sheet's (1) is the band-wide rest counter;
// this spec pins the tick arithmetic the text determines: after three ticks
// with no (2) press, +1 [火罐]; the counter resets. 「初始1，上限1」 is the
// pot cap (rulebook-doc.md 专有名词 2).
// ===========================================================================

/// Next rest-counter value after one idle turn-end tick.
pub fn firepot_01_next_rest(pre_rest: i32) -> i32 {
    let n = pre_rest + 1;
    if n >= 3 {
        0
    } else {
        n
    }
}

/// Does this tick grant a pot? (pre_rest + 1 >= 3)
pub fn firepot_01_grants(pre_rest: i32) -> bool {
    pre_rest + 1 >= 3
}

/// Exact log of a granting tick: +1 fire.
pub fn firepot_01_grant_log_ok(effects: &[Effect], owner: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainFire { player: p, n: 1 } if p == owner
        )
}

/// A non-granting tick produces no clause effects. The body clears the rest
/// counter via `state::set` which pushes `SetState` bookkeeping — ignore it.
pub fn firepot_01_idle_log_ok(effects: &[Effect]) -> bool {
    effects
        .iter()
        .all(|e| matches!(e, Effect::SetState { .. }))
}

/// MUTANT: grants at rest 0 (the card does not — every-other-turn is wrong).
pub fn firepot_01_grants_mutant(pre_rest: i32) -> bool {
    pre_rest + 1 >= 1
}

// ===========================================================================
// C-AG-moca-02 (青叶摩卡 我行我素 (2))
// 「当他人使用角色技能或打出手牌时，你可以消耗一个[火罐]传送到其所在格子
//  （不触发结算且该传送本身不引发其他被动效果（如玩家重叠），该效果对每个
//  玩家的被动技能和卡存在2次使用cd）」
// Determined half: spend 1 [火罐] and [传送] to the actor's tile, no settle.
// OPEN-2次cd: cooldown arithmetic not invented.
// ===========================================================================

/// Accepted offer: fire −1 and teleport to the actor's tile. The CD counter
/// and REST_TURNS reset are keyed state writes (not Effects).
pub fn moca_02_log_ok(effects: &[Effect], owner: i32, to_tile: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::Teleport {
                player: p,
                tile
            } if p == owner && tile == to_tile
        )
}

/// Declined / gated offer: no effects.
pub fn moca_02_noop_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// Post: owner is on `to_tile`, fire fell by 1.
pub fn moca_02_post_ok(post_pos: i32, post_fire: i32, to_tile: i32, pre_fire: i32) -> bool {
    post_pos == to_tile && post_fire == pre_fire - 1
}

/// MUTANT: teleport to the wrong tile.
pub fn moca_02_log_ok_mutant(effects: &[Effect], owner: i32, to_tile: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::Teleport {
                player: p,
                tile
            } if p == owner && tile == to_tile + 1
        )
}

// ===========================================================================
// C-AG-ran-02 (美竹兰 叛逆的红挑染 (2))
// 「投掷移动步数前可使用一个[火罐]选择本次移动向后，并且向后移动[经过]CiRCLE时
//  不获得CiRCLE奖励。」
// Determined half under Kani: spend 1 [火罐]. The reverse / NO_REWARD flags are
// plan + prop writes (`set_reverse` / `set_prop` are host protocol stubs) —
// see the unit table.
// ===========================================================================

pub fn ran_02_spend_log_empty(effects: &[Effect]) -> bool {
    // `spend_charge` mutates `World::fire` and does not push an Effect.
    effects.is_empty()
}

pub fn ran_02_fire_ok(post_fire: i32, pre_fire: i32) -> bool {
    post_fire == pre_fire - 1
}

// ===========================================================================
// C-AG-band-01 / 02 (Afterglow 商店街的宠儿)
// 「你购买商店街的或价值小于等于1200的格子时自动免费在上面加盖一栋房子」
// The filter half is determined: shop-street (group-10 buyable) OR price ≤ 1200.
// The free-house half is `card_build` (PROTOCOL stub, returns 0 under Kani).
// ===========================================================================

/// Determined filter: a 商店街-colour deed, or a tile whose purchase price
/// ≤ 1200.
pub fn band_01_applies(is_shop_street: bool, price: i32) -> bool {
    is_shop_street || price <= 1200
}

/// MUTANT: price ≤ 1000 (too tight).
pub fn band_01_applies_mutant(is_shop_street: bool, price: i32) -> bool {
    is_shop_street || price <= 1000
}