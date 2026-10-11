//! Unit `mygo`: reference specs FROM the clause docs only.
//!
//! Source of truth: `docs/rulebook/spec/cards/mygo.md` and the verbatim
//! `docs/rulebook/cards.json` text. Never derived from engine behaviour.
//!
//! Clause ids are `C-MYGO-<slug>-<n>`. Each pure function is one clause's
//! Trigger / Condition / Effect / Frame as a predicate over
//! `(pre, effect-log, post)`.

use crate::world::{Effect, Snapshot, MAX_PLAYERS};

// ===========================================================================
// C-MYGO-rinpuu-01
// 「使自己获得一层[停留]」
// Card: `MyGO:轮符雨` On::Play.
// Trigger: a play of this card. Condition: none.
// Effect: the user gains exactly one layer of [停留].
// Frame: only `stay:P{user}` moves (the effect log is exactly one GiveStay).
// ===========================================================================

/// Exact effect log of the play body: one `GiveStay { player, n: 1 }`.
/// `ctx::before_turn_end` is a scheduling op (protocol stub) and logs nothing;
/// `ctx::log` is narration, not in the frame.
pub fn rinpuu_01_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GiveStay {
                player: p,
                n
            } if p == player && n == 1
        )
}

/// Post: user's stay rose by exactly 1; every other snapshot field unchanged.
pub fn rinpuu_01_post_ok(pre: &Snapshot, post: &Snapshot, player: i32) -> bool {
    let p = player as usize;
    post.stay[p] == pre.stay[p].saturating_add(1)
        && post.money == pre.money
        && post.pos == pre.pos
        && post.fire == pre.fire
        && post.hand == pre.hand
        && post.discard == pre.discard
        && post.pay_amount == pre.pay_amount
        && (0..MAX_PLAYERS).all(|i| i == p || post.stay[i] == pre.stay[i])
}

/// MUTANT: demands two stay layers instead of one. A correct card must FAIL
/// this spec (`*_mutant_is_detected` shows the disagreement is reachable).
pub fn rinpuu_01_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GiveStay {
                player: p,
                n
            } if p == player && n == 2
        )
}