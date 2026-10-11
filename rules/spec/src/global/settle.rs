//! G-SETTLE -- 基础[结算]: tile effects, rent, buy, build, mortgage.
//!
//! Owned by unit 5. Append to [`CLAUSES`]; do not edit other areas.

use spec_kit::{Clause, Step};

/// Settlement clauses.
pub const CLAUSES: &[Clause] = &[G_SETTLE_01];

/// A mortgaged deed's settle has no effect (no rent leg).
///
/// * **ID:** G-SETTLE-01
/// * **Source:** `rulebook-doc.md:158`
/// * **Quote:** 「如果格子地契已抵押则无效果。」 (the owner-landed branch; the
///   rent branch at `rulebook-doc.md:160` is the complement: 「如果格子地契未抵押则**[支付]拥有格子的玩家格子地契所标记的现等级地租。**」)
/// * **Trigger:** an observed input whose event window contains a `rent` /
///   `pay` leg attributed to a landed settle on tile `T`.
/// * **Condition:** `T` is a player-owned buyable tile that is mortgaged in
///   `pre`.
/// * **Effect:** no `rent` event for `T` in this window.
/// * **Frame:** n/a (read of the event window).
/// * **Rulings & OPEN:** the force-buy branch of a mortgaged tile
///   (「可选择[支付]...两倍，从该玩家处强行购买」) is a `pay` that is *not*
///   rent and is out of scope here -- unit 5 owns it. An event's tile is the
///   `to` field on the money line when present; a `rent` with no tile is not
///   a landed settle and is ignored.
/// * **Executable:** [`g_settle_01`].
const G_SETTLE_01: Clause = Clause::new(
    "G-SETTLE-01",
    "rulebook-doc.md:158",
    "如果格子地契已抵押则无效果。",
    g_settle_01,
);

/// Executable form of [`G_SETTLE_01`].
pub fn g_settle_01(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "rent" {
            continue;
        }
        let tile = e.to;
        if tile < 0 {
            continue;
        }
        let t = tile as usize;
        if step.pre.st.mortgaged.get(t).copied().unwrap_or(false) {
            return Err(format!(
                "rent paid on mortgaged tile {t} (value {}, owner {})",
                e.value,
                e.other
            ));
        }
    }
    Ok(())
}