//! Unit `sumimi`: reference specs FROM the clause docs only.
//!
//! Sources: `docs/rulebook/spec/cards/sumimi.md` + `docs/rulebook/cards.json`.
//! Never derive a spec from engine behaviour. OPEN terms stay OPEN: only the
//! determined part of a quote is specified here.

use crate::world::Effect;

// ===========================================================================
// C-SUMIMI-idolband -- Sumimi:兼顾偶像与乐队
// 「当你本回合未进行过赎回操作时可打出，将你的资金重设为3000。
//  （若你的资金为3000以上则无效果）」
// ===========================================================================

/// The reset target named by the quote.
pub const IDOLBAND_TARGET: i32 = 3000;

/// C-SUMIMI-idolband-02 / -03 / -frame.
///
/// When `pre_money < 3000` the body must set money to exactly 3000 (a set,
/// not a gain of 3000) via one credit of `3000 - pre_money`.
/// When `pre_money >= 3000` the parenthetical is the no-effect case: the
/// money path is empty and the amount is untouched.
///
/// Frame: `money:P{user}` only (C-SUMIMI-idolband-frame). Card zones are the
/// play's own bookkeeping and are not in this clause's money frame.
pub fn idolband_money_ok(effects: &[Effect], player: i32, pre_money: i32) -> bool {
    if pre_money >= IDOLBAND_TARGET {
        // C-SUMIMI-idolband-03: 「若你的资金为3000以上则无效果」
        effects.is_empty()
    } else {
        // C-SUMIMI-idolband-02: 「将你的资金重设为3000」 -- set, so the credit
        // is exactly the gap.
        effects.len() == 1
            && matches!(
                effects[0],
                Effect::GainMoney { player: p, n } if p == player && n == IDOLBAND_TARGET - pre_money
            )
    }
}

/// Post money the clause demands.
pub fn idolband_post_money(pre_money: i32) -> i32 {
    if pre_money >= IDOLBAND_TARGET {
        pre_money
    } else {
        IDOLBAND_TARGET
    }
}

/// Mutant: the reference wants 2999 (off-by-one). Used to show non-vacuity.
pub fn idolband_post_money_mutant(pre_money: i32) -> i32 {
    if pre_money >= IDOLBAND_TARGET {
        pre_money
    } else {
        IDOLBAND_TARGET - 1
    }
}

// ===========================================================================
// C-SUMIMI-mana-01 / C-SUMIMI-hina-01 -- pass CiRCLE → +1 [火罐]
// 「每次[经过]CiRCLE时获得1个[火罐]（初始1/2，上限2）」
// Global (rulebook-doc.md:240): 「您持有的火罐数量不能超过持有上限」.
// ===========================================================================

/// The two Sumimi character fire caps named by the sheets.
pub const FIRE_CAP_SUMIMI: i32 = 2;

/// C-SUMIMI-mana-01 / -hina-01 effect under the global hold-cap rule.
///
/// When `pre_fire < cap` the pass gains exactly one pot.
/// When `pre_fire >= cap` the global 「不能超过持有上限」 forbids the hold,
/// so the gain is a no-op. The (初始N，上限2) parenthetical is the character's
/// fire-pot parameters (「初始」 written at game start; 「上限」 the hold cap).
pub fn pass_circle_fire_ok(effects: &[Effect], player: i32, pre_fire: i32, cap: i32) -> bool {
    if pre_fire < cap {
        effects.len() == 1
            && matches!(effects[0], Effect::GainFire { player: p, n } if p == player && n == 1)
    } else {
        effects.is_empty()
    }
}

/// Post fire the clause demands.
pub fn pass_circle_fire_post(pre_fire: i32, cap: i32) -> i32 {
    if pre_fire < cap {
        pre_fire + 1
    } else {
        pre_fire
    }
}

/// Mutant: wants +2 instead of +1 (or a gain past the cap).
pub fn pass_circle_fire_post_mutant(pre_fire: i32, cap: i32) -> i32 {
    if pre_fire < cap {
        pre_fire + 2
    } else {
        pre_fire + 1
    }
}

// ===========================================================================
// C-SUMIMI-childhood-02 -- after the re-based move, +1 [火罐]
// 「…并在移动后获得一个火罐。」
// ===========================================================================

/// After-move charge: +1 pot, fire-pot path only (C-SUMIMI-childhood-frame).
/// Same global hold-cap reading as [`pass_circle_fire_ok`].
pub fn childhood_fire_ok(effects: &[Effect], player: i32, pre_fire: i32, cap: i32) -> bool {
    pass_circle_fire_ok(effects, player, pre_fire, cap)
}

pub fn childhood_fire_post(pre_fire: i32, cap: i32) -> i32 {
    pass_circle_fire_post(pre_fire, cap)
}

// ===========================================================================
// C-SUMIMI-twodonuts-01 -- 「（1）获得[除外]直至你原本所在格子被其他玩家经过。」
// ===========================================================================

/// The [手] body grants [除外]. The expiry clause (「直至…经过」) is a
/// multi-act window and is not part of this grant's effect log.
/// The quote does not pin a layer count (`n >= 1` is the grant); the body
/// also arms a field carrier and slots — those are the attachment plumbing
/// and are not part of clause (1)'s status frame beyond the grant itself.
pub fn twodonuts_exile_grant_in(effects: &[Effect], player: i32) -> bool {
    let mut n_grants = 0;
    for e in effects {
        if matches!(e, Effect::GiveExile { player: p, n, .. } if *p == player && *n >= 1) {
            n_grants += 1;
        }
    }
    n_grants == 1
}

/// No second exile grant and no money/pos side effects from clause (1).
pub fn twodonuts_exile_ok(effects: &[Effect], player: i32) -> bool {
    twodonuts_exile_grant_in(effects, player)
        && !effects.iter().any(|e| {
            matches!(e,
                Effect::GainMoney { .. }
                | Effect::PayMoney { .. }
                | Effect::PayTo { .. }
                | Effect::Teleport { .. }
                | Effect::GainFire { .. }
            )
        })
}

// ===========================================================================
// C-SUMIMI-twoinone-01b -- fire becomes the other character's 初始火罐数
// 「将自己的角色卡替换为sumimi的另一名角色及其初始火罐数」
// 纯田真奈 initial 1 / max 2; 三角初华（Sumimi） initial 2 / max 2.
// ===========================================================================

pub const MANA_INITIAL_FIRE: i32 = 1;
pub const HINA_INITIAL_FIRE: i32 = 2;
pub const TWOINONE_FIRE_MAX: i32 = 2;

/// From which character, to which character's initial fire.
/// `from_is_hina`: the user is 三角初华（Sumimi） before the swap.
pub fn twoinone_fire_target(from_is_hina: bool) -> i32 {
    if from_is_hina {
        MANA_INITIAL_FIRE
    } else {
        HINA_INITIAL_FIRE
    }
}

/// C-SUMIMI-twoinone-01b fire half: the user's fire becomes the other
/// character's 初始火罐数. At least one SetState on the fire key to `to`;
/// no GainFire / token writes that would double-count pots.
pub fn twoinone_fire_effects_ok(effects: &[Effect], player: i32, fire_key: u16, to: i32) -> bool {
    let mut saw_fire = false;
    for e in effects {
        match e {
            Effect::SetState { player: p, key, n } => {
                if *p == player && *key == fire_key {
                    if saw_fire || *n != to {
                        return false;
                    }
                    saw_fire = true;
                }
            }
            Effect::GainFire { .. } | Effect::AddToken { .. } | Effect::SetToken { .. } => {
                return false;
            }
            _ => {}
        }
    }
    saw_fire
}

// ===========================================================================
// C-SUMIMI-band-02 -- rent bonus +100 per crystal
// 「此卡上的每个奇迹水晶可使你在向其他人收费时让金额额外提高100资金。」
// ===========================================================================

/// Post pay-amount the clause demands: `pre + crystals * 100`.
pub fn band_rent_bonus_amount(pre_amount: i32, crystals: i32) -> i32 {
    pre_amount + crystals * 100
}

/// ABI v75 pure Filter: the PayMul body RETURNS the raised amount and emits
/// **no** Effect (narration is separate; the host writes `SetPayAmount` when
/// it folds the return). SPEC CHANGE (Action→Filter stage move): the old
/// Action body wrote `set_pay_amount` itself and this pinned exactly that
/// effect; the pure body's no-less/no-more is the return value plus an empty
/// effect log. When `crystals <= 0` the return is the unchanged amount.
pub fn band_rent_bonus_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// Mutant: +100 total instead of +100 per crystal.
pub fn band_rent_bonus_amount_mutant(pre_amount: i32, _crystals: i32) -> i32 {
    pre_amount + 100
}

// ===========================================================================
// C-SUMIMI-band-01 (clear half) -- 「否则，移除此卡上的所有奇迹水晶。」
// ===========================================================================

/// When the turn's inbound-money latch is clear, every crystal on this skill
/// card is removed (`add_crystals(-n)`).
pub fn band_crystal_clear_ok(
    effects: &[Effect],
    owner: i32,
    crystal_key: u16,
    pre_crystals: i32,
) -> bool {
    if pre_crystals <= 0 {
        effects.is_empty()
    } else {
        effects.len() == 1
            && matches!(
                effects[0],
                Effect::AddCounter { owner: o, key, n }
                    if o == owner && key == crystal_key && n == -pre_crystals
            )
    }
}

/// Mutant: removes one crystal instead of all.
pub fn band_crystal_clear_ok_mutant(
    effects: &[Effect],
    owner: i32,
    crystal_key: u16,
    pre_crystals: i32,
) -> bool {
    if pre_crystals <= 0 {
        true
    } else {
        effects.len() == 1
            && matches!(
                effects[0],
                Effect::AddCounter { owner: o, key, n }
                    if o == owner && key == crystal_key && n == -1
            )
    }
}