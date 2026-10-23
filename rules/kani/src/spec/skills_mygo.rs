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

/// Post crystals after a roll-gain: min(pre + 1, 1) when pre < 1, else pre.
/// The cap (上限1) applies to this roll gain; (3)'s over-cap crystals are a
/// different clause.
pub fn band_02_crystals_after(pre: i32) -> i32 {
    if pre < 1 {
        1
    } else {
        pre
    }
}

/// Exact effect log of the roll-gain body: one AddCounter on the band card
/// whose `n` is the host's clamped delta (`now - was`) for
/// `add_crystals(1, 1)`. When `pre < 1` the gain lands (+1); at/above the
/// roll cap the host clamps the post value to `max=1`, so the logged delta is
/// `min(pre + 1, 1) - pre` (0 at the cap, negative if over-cap crystals are
/// present — bookkeeping of the clamp, not a second gain).
pub fn band_02_log_ok(effects: &[Effect], owner: i32, pre: i32) -> bool {
    let want_n = spec_clamped_delta(pre, 1, 1);
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
/// refused once `pre >= 3`.
pub fn band_04_crystals_after(pre: i32) -> i32 {
    if pre >= 3 {
        pre
    } else {
        pre + 1
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
