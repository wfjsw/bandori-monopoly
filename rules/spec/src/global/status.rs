//! G-STATUS -- [停留] / [晕眩] / [除外] layers and what they forbid.
//!
//! Owned by unit 3. Append to [`CLAUSES`]; do not edit other areas.

use spec_kit::{Clause, Step};

/// Status clauses.
pub const CLAUSES: &[Clause] = &[G_STATUS_01];

/// Stay / stun / exile layers are never negative.
///
/// * **ID:** G-STATUS-01
/// * **Source:** `rulebook-doc.md:96-98`
/// * **Quote:** 「[停留]：处于该状态时[无法移动]。重复受到该效果时可叠加，玩家的每回合结束时移除一层。」
///   (and the parallel stacking sentences for [晕眩] / [除外])
/// * **Trigger:** every observed input, on the settled `post` world.
/// * **Condition:** the engine's `stay` / `stun` / `stunStart` / `exile`
///   counters.
/// * **Effect:** every such counter is `>= 0`.
/// * **Frame:** n/a (pure read).
/// * **Rulings & OPEN:** "可叠加" + "移除一层" makes the counters layer counts;
///   a layer count that went negative would mean a decay ran one too many
///   times. `stunStart` is the [晕眩] variant that expires at turn start;
///   same non-negativity. What each status *forbids* is unit 3's work.
/// * **Executable:** [`g_status_01`].
const G_STATUS_01: Clause = Clause::new(
    "G-STATUS-01",
    "rulebook-doc.md:96",
    "[停留]：处于该状态时[无法移动]。重复受到该效果时可叠加，玩家的每回合结束时移除一层。",
    g_status_01,
);

/// Executable form of [`G_STATUS_01`].
pub fn g_status_01(step: &Step<'_>) -> Result<(), String> {
    const KEYS: [&str; 4] = [
        game_core::state::key::STAY,
        game_core::state::key::STUN,
        game_core::state::key::STUN_START,
        game_core::state::key::EXILE,
    ];
    for (i, p) in step.post.st.players.iter().enumerate() {
        for k in KEYS {
            let v = p.state_get(k);
            if v < 0 {
                return Err(format!("player {i} {k}={v} is negative"));
            }
        }
    }
    Ok(())
}