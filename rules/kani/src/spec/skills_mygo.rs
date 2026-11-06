//! Unit `skills_mygo`: reference specs FROM the clause docs only.
//!
//! Sources: `docs/rulebook/spec/cards/mygo.md` (Character skill / Band skill
//! sections), `the roster` (band `MyGO!!!!!`), `the roster`
//! (skill 「迷途之星」). Never derived from engine behaviour.

use crate::world::Effect;

// ===========================================================================
// C-MYGO-band-02
// 「若移动掷骰出目为16及以上，为此卡添加一个[奇迹水晶]（上限1）。」
// Hook: `skill:MyGO!!!!!:迷途之星` On::Action(DiceRollAfter),
// guard `"actor == owner && move.roll >= 16"`.
// Effect: the band card gains exactly one [奇迹水晶], capped at 1.
// Frame: only the band-card crystal count moves.
// ===========================================================================

/// Post crystals after a roll-gain: min(condition + 1, 1) when condition < 1, else condition.
/// The cap (上限1) applies to this roll gain; (3)'s over-cap crystals are a
/// different clause.
pub fn band_02_crystals_after(condition: i32) -> i32 {
    if condition < 1 {
        1
    } else {
        condition
    }
}

/// Exact effect log of the roll-gain body: one AddCounter on the band card
/// whose `n` is the host's clamped delta (`now - was`) for
/// `add_crystals(1, 1)`. When `condition < 1` the gain lands (+1); at/above the
/// roll cap the host clamps the post value to `max=1`, so the logged delta is
/// `min(condition + 1, 1) - condition` (0 at the cap, negative if over-cap crystals are
/// present — bookkeeping of the clamp, not a second gain).
pub fn band_02_log_ok(effects: &[Effect], owner: i32, condition: i32) -> bool {
    let want_n = spec_clamped_delta(condition, 1, 1);
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter {
                owner: o,
                n,
                ..
            } if o == owner && n == want_n
        )
}

/// Host `arm_add_counter_self` delta for `add(n, max)`: `clamp(was+n, 0, max) - was`.
fn spec_clamped_delta(was: i32, n: i32, max: i32) -> i32 {
    let mut now = was + n;
    if now < 0 {
        now = 0;
    }
    if max > 0 && now > max {
        now = max;
    }
    now - was
}

/// MUTANT: demands n == 2 (a double-gain card).
pub fn band_02_log_ok_mutant(effects: &[Effect], owner: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter {
                owner: o,
                n,
                ..
            } if o == owner && n == 2
        )
}

// ===========================================================================
// C-MYGO-band-05
// 「你的回合中，可移除此卡的两个[奇迹水晶]以抽一张卡。」
// Press half: spend 2 crystals, draw exactly 1 card.
// Effect: AddCounter(-2) then Draw 1.
// Frame: only band crystals / hand-draw zones move.
// OPEN-multi-activation: the engine may merge (2)'s move half and (3)'s draw
// half into one press; this clause checks the draw half's numbers.
// ===========================================================================

pub fn band_05_log_ok(effects: &[Effect], owner: i32) -> bool {
    if effects.len() != 2 {
        return false;
    }
    matches!(effects[0], Effect::AddCounter { owner: o, n, .. } if o == owner && n == -2)
        && matches!(effects[1], Effect::Draw { player: p, n } if p == owner && n == 1)
}

/// MUTANT: draws 2 cards.
pub fn band_05_log_ok_mutant(effects: &[Effect], owner: i32) -> bool {
    if effects.len() != 2 {
        return false;
    }
    matches!(effects[0], Effect::AddCounter { owner: o, n, .. } if o == owner && n == -2)
        && matches!(effects[1], Effect::Draw { player: p, n } if p == owner && n == 2)
}

// ===========================================================================
// C-MYGO-band-04
// 「每次你的卡在未生效的情况下进入弃牌堆时，为此卡添加一个可超出上限的[奇迹水晶]
//  （最多超出2个）」
// Hook: Discarded (mine). Effect: +1 crystal that may exceed the roll cap of 1,
// at most 2 over (held total ceiling 3). Body `skill.mygo.over` is audit only.
// ===========================================================================

/// Post crystals after an ineffective-discard gain. The +1 may exceed the roll
/// cap of 1; the held total never exceeds 3 (「最多超出2个」). The gain is
/// refused once `condition >= 3`.
pub fn band_04_crystals_after(condition: i32) -> i32 {
    if condition >= 3 {
        condition
    } else {
        condition + 1
    }
}

/// Log when the gain fires: one AddCounter(+1). `state::set(OVER, …)` is a
/// SetState bookkeeping entry the body also pushes — tolerate it.
pub fn band_04_log_ok(effects: &[Effect], owner: i32) -> bool {
    let mut add = 0;
    for e in effects {
        match e {
            Effect::AddCounter { owner: o, n, .. } if *o == owner && *n == 1 => add += 1,
            Effect::AddCounter { .. } => return false,
            Effect::SetState { .. } => {}
            _ => return false,
        }
    }
    add == 1
}

/// MUTANT: no effect when the gain should fire.
pub fn band_04_log_ok_mutant(effects: &[Effect], _owner: i32) -> bool {
    effects.is_empty()
}

// ===========================================================================
// G13 — `skill:高松灯:诗超绊` (2), `On::Action(SettleBefore).id("2")`
//
// Book:
// > （2）当其他玩家的[移动终点]位于您前后5格内时可以选择使用4个火罐，在该玩家
// > [触发结算]前强制将该玩家[传送]至自己所在的格子并[触发结算]，然后使其投掷
// > 1d20，若出目小于等于6则获得一层[停留]并在自己的下回合结束时[触发结算]；
// > 在此技能影响下的支付价格变为原本的一半。
//
// The (2) arm is «another player's landing». The body is only called after the
// `before_settle_applies` guard admits (`mover != owner`, once per move, within
// 5). A borrower passes `invoke_paid` and the body must not ask or spend.
//
// Effect frame of a completed press, in body order: the once-per-move latch,
// the half-price latch, the teleport; on a low follow-up roll the [停留] layer
// and the owed-settle latch follow. Narration is not the frame.
// ===========================================================================

/// Exact effect log of the press up to the teleport.
pub fn tomori_pull_effects_ok(
    effects: &[Effect],
    mover: i32,
    destination: i32,
    used_key: u16,
    half_key: u16,
) -> bool {
    effects.len() == 3
        && matches!(effects[0], Effect::SetState { player: p, key: k, n: 1 } if p == mover && k == used_key)
        && matches!(effects[1], Effect::SetState { player: p, key: k, n: 1 } if p == mover && k == half_key)
        && matches!(effects[2], Effect::Teleport { player: p, tile: t } if p == mover && t == destination)
}

/// Exact effect log including the (2) follow-up on a low roll.
pub fn tomori_follow_up_effects_ok(
    effects: &[Effect],
    mover: i32,
    destination: i32,
    used_key: u16,
    half_key: u16,
    owed_key: u16,
) -> bool {
    effects.len() == 5
        && matches!(effects[4], Effect::SetState { player: p, key: k, n: 1 } if p == mover && k == owed_key)
        && matches!(effects[3], Effect::GiveStay { player: p, n: 1 } if p == mover)
        && tomori_pull_effects_ok(&effects[..3], mover, destination, used_key, half_key)
}

/// 「若出目小于等于6」.
pub fn tomori_follow_up(face: i32) -> bool {
    face <= 6
}

/// [停留] layers after the press (a follow-up adds exactly one).
pub fn tomori_stay_after(stay: i32, followed: bool) -> i32 {
    if followed {
        stay + 1
    } else {
        stay
    }
}

/// The owed-settle latch after the press.
pub fn tomori_owed_after(followed: bool) -> i32 {
    followed as i32
}

/// `at_next_turn_end` calls after the press.
pub fn tomori_schedules_after(followed: bool) -> u32 {
    followed as u32
}

/// Exact effect log of `owed_settle`: drop the owed latch and a borrowed
/// bend. With a square it asks for the settle (a protocol stub with no
/// effect-log entry); the half latch was armed by the forced settle and stays
/// armed for the deferred `SettleAfter` body. With no square it is dropped
/// here instead.
pub fn tomori_owed_effects_ok(
    effects: &[Effect],
    player: i32,
    owed_key: u16,
    borrowed_key: u16,
    half_key: u16,
    settles: bool,
) -> bool {
    let want = if settles { 2 } else { 3 };
    effects.len() == want
        && matches!(effects[0], Effect::SetState { player: p, key: k, n: 0 } if p == player && k == owed_key)
        && matches!(effects[1], Effect::SetState { player: p, key: k, n: 0 } if p == player && k == borrowed_key)
        && (settles
            || matches!(effects[2], Effect::SetState { player: p, key: k, n: 0 } if p == player && k == half_key))
}

/// Exact effect log of the `SettleAfter` body that follows a settle: disarm
/// the half price and the borrowed bend.
pub fn tomori_owed_clear_effects_ok(
    effects: &[Effect],
    player: i32,
    borrowed_key: u16,
    half_key: u16,
) -> bool {
    effects.len() == 2
        && matches!(effects[0], Effect::SetState { player: p, key: k, n: 0 } if p == player && k == half_key)
        && matches!(effects[1], Effect::SetState { player: p, key: k, n: 0 } if p == player && k == borrowed_key)
}

/// MUTANT: the follow-up is tied to `cost == 4` instead of the mover arm — it
/// predicts a roll even when the destination is unset (`pos < 0`, early out).
pub fn tomori_follow_up_mutant_cost_four(cost: i32) -> bool {
    cost == 4
}

/// MUTANT: the roll-off threshold is `< 6` instead of `<= 6`.
pub fn tomori_follow_up_mutant_below_six(face: i32) -> bool {
    face < 6
}

/// MUTANT: the half price is already disarmed by `owed_settle` itself (the
/// R6 shape: clearing before the settle pays), so the deferred body's write is
/// not the one the settle rides on.
pub fn tomori_owed_effects_mutant_half_disarmed_early(effects: &[Effect], half_key: u16) -> bool {
    effects
        .iter()
        .any(|e| matches!(e, Effect::SetState { key: k, n: 0, .. } if *k == half_key))
}
