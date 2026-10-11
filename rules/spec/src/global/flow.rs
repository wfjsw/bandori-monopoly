//! G-FLOW -- phase order, turn order, the main move once per turn.
//!
//! Owned by unit 6. Append to [`CLAUSES`]; do not edit other areas.
//!
//! Note: the 回合流程 table rows 1-3 are corrupt in the book; use
//! **行动阶段** (`rulebook-doc.md:199+`) for the turn's action order.

use spec_kit::{Clause, Step};

/// Turn-flow clauses.
pub const CLAUSES: &[Clause] = &[G_FLOW_01];

/// The main move happens at most once per turn.
///
/// * **ID:** G-FLOW-01
/// * **Source:** `rulebook-doc.md:132`
/// * **Quote:** 「玩家在运营阶段可在非执行任何效果的时候进入移动阶段。移动阶段中需进行1次[主要移动]。基础[主要移动]为**投掷1d20**，在地图上顺时针移动投掷结果数量的格子，到达[移动终点]后需进行[结算]。[主要移动]和所需[结算]完成后进入结束阶段。」
/// * **Trigger:** an observed input that performs a main move (the engine's
///   `turn.main_moved` latch flips false -> true in this step).
/// * **Condition:** `pre.turn.main_moved` is already true (the main move
///   already happened this turn).
/// * **Effect:** the latch does not flip again inside the same turn -- a
///   second main move is refused / no-op'd.
/// * **Frame:** n/a (read of the turn latch).
/// * **Rulings & OPEN:** 「移动阶段中需进行1次」 is the "exactly one" that
///   makes a second attempt illegal. Card-driven extra moves are
///   「[强制移动]」 / 「[传送]」 and are *not* [主要移动] -- they are unit 4
///   and unit 6's shared edge. This example only guards the latch.
/// * **Executable:** [`g_flow_01`].
const G_FLOW_01: Clause = Clause::new(
    "G-FLOW-01",
    "rulebook-doc.md:132",
    "玩家在运营阶段可在非执行任何效果的时候进入移动阶段。移动阶段中需进行1次[主要移动]。",
    g_flow_01,
);

/// Executable form of [`G_FLOW_01`].
///
/// Checks the latch's *shape*: `main_moved` may go false -> true at most once
/// per turn, and never true -> false except at a turn boundary (which shows
/// up as a turn change in the same step).
pub fn g_flow_01(step: &Step<'_>) -> Result<(), String> {
    let pre_moved = step.pre.turn.main_moved;
    let post_moved = step.post.turn.main_moved;
    let pre_turn = step.pre.st.turn;
    let post_turn = step.post.st.turn;
    let turn_changed = pre_turn != post_turn;

    if pre_moved && !post_moved && !turn_changed {
        return Err(format!(
            "main_moved reset mid-turn (turn {pre_turn} -> {post_turn})"
        ));
    }
    // A second main move in one turn would need the latch to go true -> true
    // via a fresh roll. The engine refuses that; a step whose window contains
    // a second `roll` of the main mover after the latch is already true is a
    // finding. (A card's extra dice is not a main-move roll.)
    if pre_moved && post_moved && !turn_changed {
        for e in step.events {
            if e.r#type == "roll" && e.player_id == step.pre.turn.player_id as i32 {
                return Err(format!(
                    "second main-move roll in turn {pre_turn} (event {})",
                    e.id
                ));
            }
        }
    }
    Ok(())
}