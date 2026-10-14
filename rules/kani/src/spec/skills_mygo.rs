//! Unit `skills_mygo`: reference specs FROM the clause docs only.
//!
//! Sources: `docs/rulebook/spec/cards/mygo.md` (Character skill / Band skill
//! sections), `the roster` (band `MyGO!!!!!`), `the roster`
//! (skill 「迷途之星」). Never derived from engine behaviour.

use crate::world::Effect;

// ===========================================================================
// C-MYGO-band-02
// 「若移动掷骰出目为16及以上，为此卡添加一个[奇迹水晶]（上限1）。」
// Hook: `skill:MyGO!!!!!:迷途之星` On::Hook(RollAfter),
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

/// Exact effect log of the roll-gain body: one AddCounter(+1) on the band card.
/// `add_crystals(1, 1)` is `add_counter(CRYSTALS, 1, max=1)`.
pub fn band_02_log_ok(effects: &[Effect], owner: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter {
                owner: o,
                n,
                ..
            } if o == owner && n == 1
        )
}

/// MUTANT: demands n == 2.
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
// Hook: Discarded (mine). Effect: +1 crystal that may exceed the roll cap,
// at most 2 over (so max 3 with the cap of 1).
// KNOWN black-box LESS: the over-cap crystal does not land when the band card
// is already at the roll cap of 1. Spec from the book.
// ===========================================================================

/// Post crystals after an ineffective-discard gain, given pre and the current
/// over-cap count `over` (0..=2). The gain is refused once `over == 2`.
pub fn band_04_crystals_after(pre: i32, over: i32) -> i32 {
    let room = 1 + over.min(2);
    if pre >= room {
        pre
    } else {
        pre + 1
    }
}

/// Exact log when the gain fires: one AddCounter(+1).
pub fn band_04_log_ok(effects: &[Effect], owner: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter {
                owner: o,
                n,
                ..
            } if o == owner && n == 1
        )
}

/// MUTANT: no effect when the gain should fire.
pub fn band_04_log_ok_mutant(effects: &[Effect], _owner: i32) -> bool {
    effects.is_empty()
}