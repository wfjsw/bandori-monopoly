//! G-COUNTER -- [反击] windows, order, negation.
//!
//! Owned by unit 8. Append to [`CLAUSES`]; do not edit other areas.
//!
//! Terminology: "counteraction" for [反击] windows, "hook" for passive
//! listeners. Never "reaction".

use spec_kit::{Clause, Step};

/// Counteraction clauses.
pub const CLAUSES: &[Clause] = &[G_COUNTER_01];

/// A negated activation does not run its body.
///
/// * **ID:** G-COUNTER-01
/// * **Source:** `rulebook-doc.md:79`
/// * **Quote:** 「[反击]：带有“[反击]X：Y”效果在X发生时打出并触发效果Y，且结算优先于X（例：X可以是被其他玩家的卡[指定]时）。」
/// * **Trigger:** an observed input whose event window contains a `"card"`
///   activation marked `negated` (a counteraction cancelled it).
/// * **Condition:** the negated activation's card body would have moved
///   money / status / position (any of `pay` / `rent` / `gain` / `lose` /
///   `mortgage` / `redeem` / `move` children).
/// * **Effect:** no such child events hang under the negated activation
///   (`parent == that event's id`). The negation's own log line may.
/// * **Frame:** n/a (read of the event window).
/// * **Rulings & OPEN:** 「结算优先于X」 is why the negation can cancel X
///   outright: Y resolves first, and a negating Y leaves X's body unrun. The
///   counteraction *order* (「从行动顺序上在触发[反击]时点的玩家的下一位玩家开始」)
///   is unit 8's separate clause. A `negated` activation still flashes and
///   logs -- only the body is empty.
/// * **Executable:** [`g_counter_01`].
const G_COUNTER_01: Clause = Clause::new(
    "G-COUNTER-01",
    "rulebook-doc.md:79",
    "[反击]：带有“[反击]X：Y”效果在X发生时打出并触发效果Y，且结算优先于X",
    g_counter_01,
);

/// Executable form of [`G_COUNTER_01`].
pub fn g_counter_01(step: &Step<'_>) -> Result<(), String> {
    const BODY_TYPES: [&str; 8] = [
        "pay", "rent", "gain", "lose", "mortgage", "redeem", "move", "bankrupt",
    ];
    for e in step.events {
        if e.r#type != "card" || !e.negated {
            continue;
        }
        for c in step.events {
            if c.parent == e.id && BODY_TYPES.contains(&c.r#type.as_str()) {
                return Err(format!(
                    "negated activation {} ({}) has body event {} type {}",
                    e.id,
                    e.card,
                    c.id,
                    c.r#type
                ));
            }
        }
    }
    Ok(())
}