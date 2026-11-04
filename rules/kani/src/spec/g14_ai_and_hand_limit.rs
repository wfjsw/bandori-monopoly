//! Unit `g14`: reference specs FROM the clause docs only.
//!
//! Sources:
//! * `docs/rulebook/cards.json` `Mor:（NNM）稍微努力了一下` (1)
//!   「抽x张卡（可超过上限），回合结束后将手牌弃置到五张」
//! * `docs/rulebook/cards.json` `Mor:夏日合宿` (rules 1253) -- the static
//!   `aiNever` prop.
//!
//! Design: `docs/rulebook/spec/abi/G14-ai-and-hand-limit.md`.
//!
//! Never derived from engine behaviour. The clip 「可超过上限」 is the import's
//! **flag**, so the spec is over the host effect log: an exempt draw logs
//! [`Effect::DrawOver`], a plain one [`Effect::Draw`].

use crate::world::Effect;

// ===========================================================================
// C-MOR-nnm-02 (1) -- 「抽x张卡（可超过上限）」
// ===========================================================================

/// Exactly `x` exempt single-card imports for `player` and nothing else.
/// Narration (`ctx::log`) is not in the frame; `before_turn_end` schedules a
/// callback and logs no effect.
pub fn nnm_02_draw_over_iff(effects: &[Effect], player: i32, x: i32) -> bool {
    x >= 0
        && effects.len() == x as usize
        && effects
            .iter()
            .all(|e| matches!(e, Effect::DrawOver { player: p, n: 1 } if *p == player))
}

/// MUTANT: the same draw shape without the exemption flag.
pub fn nnm_02_draw_over_iff_mutant_plain(effects: &[Effect], player: i32, x: i32) -> bool {
    x >= 0
        && effects.len() == x as usize
        && effects
            .iter()
            .all(|e| matches!(e, Effect::Draw { player: p, n: 1 } if *p == player))
}

/// A plain `ctx::draw` body still logs the unexempt [`Effect::Draw`].
pub fn plain_draw_stays_plain(effects: &[Effect], player: i32, x: i32) -> bool {
    x >= 0
        && effects.len() == x as usize
        && effects
            .iter()
            .all(|e| matches!(e, Effect::Draw { player: p, n: 1 } if *p == player))
}

// ===========================================================================
// C-MOR-夏日合宿 -- the static prop `aiNever`
// ===========================================================================

/// The def declares `(AI_NEVER, 1)`.
pub fn summer_ai_never_declared(props: &[(&str, i32)]) -> bool {
    props
        .iter()
        .any(|(k, v)| *k == card_sdk::abi::prop::AI_NEVER && *v == 1)
}

/// MUTANT: a def with no such prop (or a zero value) fails.
pub fn summer_ai_never_declared_mutant(props: &[(&str, i32)]) -> bool {
    props.iter().all(|(k, _)| *k != card_sdk::abi::prop::AI_NEVER)
}
