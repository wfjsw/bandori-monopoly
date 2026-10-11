//! G-CARDS -- card zones, the hand limit, draw / discard / reshuffle.
//!
//! Owned by unit 2. Append to [`CLAUSES`]; do not edit other areas.

use spec_kit::{Clause, Step};

/// Card-zone clauses.
pub const CLAUSES: &[Clause] = &[G_CARDS_01];

/// An act must not leave a hand over its limit.
///
/// * **ID:** G-CARDS-01
/// * **Source:** `rulebook-doc.md:126`
/// * **Quote:** 「基础手卡上限为5张，任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张。」
/// * **Trigger:** every observed input, after the settle loop (the step's
///   `post` world is the rest state a player sees).
/// * **Condition:** a player's hand-limit counter is the engine's
///   `handLimit` state (default 5, raised / lowered by effects). A limit of
///   `NO_HAND_LIMIT` (`i32::MAX`) means uncapped. The player was **at or
///   under** the limit in `pre` -- an arranged over-limit hand is a
///   non-canonical setup and is not this act's fault. No prompt is open in
///   `post` (the step is at rest; a discard decision may be the prompt).
/// * **Effect:** the player's private hand length is at most the limit in
///   `post`.
/// * **Frame:** n/a (pure read).
/// * **Rulings & OPEN:** 「立刻」 means the discard happens inside the same
///   transition as the draw that overflowed -- so a check on the settled
///   `post` world is the right place. Mid-mulligan / mid-pick hands that a
///   prompt is still holding are not `post` of an *answered* act, so they are
///   not covered here; unit 2 owns the in-prompt variants.
/// * **Executable:** [`g_cards_01`].
const G_CARDS_01: Clause = Clause::new(
    "G-CARDS-01",
    "rulebook-doc.md:126",
    "基础手卡上限为5张，任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张。",
    g_cards_01,
);

/// Executable form of [`G_CARDS-01`].
pub fn g_cards_01(step: &Step<'_>) -> Result<(), String> {
    // A prompt means the step stopped mid-decision (the discard choice may be
    // the prompt). "At rest" is the post-prompt state this clause is about.
    if step.post_prompt_open {
        return Ok(());
    }
    for (i, p) in step.post.st.players.iter().enumerate() {
        let Some(limit) = p.hand_limit() else {
            continue;
        };
        if limit == game_core::state::key::NO_HAND_LIMIT {
            continue;
        }
        let limit = limit as usize;
        let pre = step.pre_hand_len(i);
        let post = step.post_hand_len(i);
        // Arrange writes (and a previous failing act) may already be over.
        // Only an act that *starts* legal and *ends* over is this clause's
        // finding.
        if pre <= limit && post > limit {
            return Err(format!(
                "player {i} went from {pre} to {post} cards, over handLimit {limit}"
            ));
        }
    }
    Ok(())
}