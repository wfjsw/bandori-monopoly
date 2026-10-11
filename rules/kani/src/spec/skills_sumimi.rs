//! Unit `skills_sumimi`: reference specs FROM the clause docs only.
//!
//! Sources: `data/characters.json` `text` (纯田真奈 / 三角初华（Sumimi）) and
//! `data/bands.json` `text` (Sumimi / 人气偶像组合), via
//! `docs/rulebook/spec/cards/sumimi.md`. Never derive a spec from engine
//! behaviour. OPEN terms stay OPEN.

use crate::world::Effect;

// ===========================================================================
// C-SUMIMI-mana-01 / C-SUMIMI-hina-01
// 「每次[经过]CiRCLE时获得1个[火罐]（初始1/2，上限2）」
// Shared with the card-unit fire specs (same quote shape).
// ===========================================================================

pub use crate::spec::sumimi::{
    pass_circle_fire_ok, pass_circle_fire_post, pass_circle_fire_post_mutant, FIRE_CAP_SUMIMI,
};

// ===========================================================================
// C-SUMIMI-band-02 -- rent bonus
// 「此卡上的每个奇迹水晶可使你在向其他人收费时让金额额外提高100资金。」
// ===========================================================================

pub use crate::spec::sumimi::{
    band_rent_bonus_amount, band_rent_bonus_amount_mutant, band_rent_bonus_log_ok,
};

// ===========================================================================
// C-SUMIMI-band-01 -- turn-end crystal
// 「若你在一回合内从其他玩家处获得过资金，你的回合结束时为此卡添加1个奇迹水晶；
//  否则，移除此卡上的所有奇迹水晶。」
// ===========================================================================

pub use crate::spec::sumimi::{band_crystal_clear_ok, band_crystal_clear_ok_mutant};

/// Add half: when the inbound-money latch is set, exactly one crystal is
/// added and the latch clears. The latch itself is a per-turn bookkeeping
/// flag (「在一回合内从其他玩家处获得过资金」) whose write goes through
/// `state::set` — see the unit table for the host packing note.
pub fn band_crystal_add_ok(
    effects: &[Effect],
    owner: i32,
    crystal_key: u16,
    state_key: u16,
) -> bool {
    // Expect AddCounter(+1) and SetState(latch → 0) in either relative order,
    // and nothing else.
    let mut add = false;
    let mut clear = false;
    for e in effects {
        match e {
            Effect::AddCounter { owner: o, key, n } => {
                if *o == owner && *key == crystal_key && *n == 1 && !add {
                    add = true;
                } else {
                    return false;
                }
            }
            Effect::SetState { player: p, key, n } => {
                if *p == owner && *key == state_key && *n == 0 && !clear {
                    clear = true;
                } else {
                    return false;
                }
            }
            _ => return false,
        }
    }
    add && clear
}

// ===========================================================================
// C-SUMIMI-band-03 -- exclusive play gate
// 「你可携带两个角色的专属卡牌，但仅有在你的角色卡为对应角色时才可打出。」
// ===========================================================================

/// The play of a character exclusive is allowed iff the user's character is
/// the card's owner. Carrying both is a deck/hand composition rule (not a
/// body effect).
pub fn band_exclusive_play_allowed(user_is_owner: bool) -> bool {
    user_is_owner
}

// ===========================================================================
// C-SUMIMI-mana-02 -- shop pre-pay + half window (determined part)
// 「经过"购物中心"，…时，在触发结算前可选择消耗一个火罐并向对应格子进行一次支付
//  （可向抵押格子正常支付其原本收费，若为无主格子则为消耗对应收费资金），之后，
//  直到你的下个回合结束，你的资金消耗减半。」
// OPEN-收费标价: the payment amount is the tile's 「原本收费」 / 「对应收费」 —
// the same open tag. Only the *structure* is specified: spend 1 pot, one pay
// of that open amount, then arm a half-consumption window lasting through the
// end of the user's **next** turn.
// ===========================================================================

/// The window arm: two `TurnEnd` layers so the half lasts through the end of
/// the owner's next turn (the arming turn's end takes the first).
pub const MANA_HALF_TURNEND_LAYERS: i32 = 2;

/// Effects the determined structure allows when the offer is accepted:
/// `SpendFire` is not in the `Effect` enum (the host returns 0/1) so the
/// observable frame is: one PayMoney/PayTo of the open amount, plus a SetState
/// arm of the half window with `MANA_HALF_TURNEND_LAYERS` turn-end layers.
/// The amount itself is OPEN-收费标价 and is not pinned here.
pub fn mana_shop_pay_structure_ok(
    effects: &[Effect],
    player: i32,
    half_key: u16,
    arm_value: i32,
) -> bool {
    let mut pay = false;
    let mut arm = false;
    for e in effects {
        match e {
            Effect::PayMoney { player: p, .. } | Effect::PayTo { from: p, .. } => {
                if *p == player && !pay {
                    pay = true;
                } else {
                    return false;
                }
            }
            Effect::SetState { player: p, key, n } => {
                if *p == player && *key == half_key && *n == arm_value && !arm {
                    arm = true;
                } else {
                    return false;
                }
            }
            Effect::GainFire { .. } | Effect::GiveExile { .. } | Effect::Teleport { .. } => {
                return false;
            }
            _ => {}
        }
    }
    pay && arm
}

/// Decline: no effects at all (the offer was refused).
pub fn mana_shop_pay_declined(effects: &[Effect]) -> bool {
    effects.is_empty()
}