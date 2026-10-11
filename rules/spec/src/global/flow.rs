//! G-FLOW -- phase order, turn order, the main move once per turn.
//!
//! Owned by unit 6. Append to [`CLAUSES`]; do not edit other areas.
//!
//! Note: the 回合流程 table rows 1-3 are corrupt in the book; use
//! **行动阶段** (`rulebook-doc.md:199+`) for the turn's action order.

use spec_kit::{Clause, Step, StepAct};

/// Turn-flow clauses.
pub const CLAUSES: &[Clause] = &[G_FLOW_01];

/// The main move happens at most once per turn.
///
/// * **ID:** G-FLOW-01
/// * **Source:** `rulebook-doc.md:132`
/// * **Quote:** 「玩家在运营阶段可在非执行任何效果的时候进入移动阶段。移动阶段中需进行1次[主要移动]。基础[主要移动]为**投掷1d20**，在地图上顺时针移动投掷结果数量的格子，到达[移动终点]后需进行[结算]。[主要移动]和所需[结算]完成后进入结束阶段。」
/// * **Trigger:** an observed input that is a `roll` command (the player
///   pressing the main-move button), or any input that touches the
///   `turn.main_moved` latch.
/// * **Condition:** `pre.turn.main_moved` is already true in the same turn
///   (the main move already happened).
/// * **Effect:** a second `roll` command does not move the player (the engine
///   must refuse it / no-op it). The latch may clear at a turn boundary --
///   including an extra turn for the same seat -- which is not a violation.
/// * **Frame:** n/a (read of the turn latch and `pos`).
/// * **Rulings & OPEN:** 「移动阶段中需进行1次」 is the "exactly one" that
///   makes a second attempt illegal. Card-driven dice (a card's 「投掷3d10」)
///   are **not** a `roll` command and are out of scope -- they are not
///   [主要移动]. Card-driven extra moves are 「[强制移动]」 / 「[传送]」 and
///   are unit 4 and unit 6's shared edge.
/// * **Executable:** [`g_flow_01`].
const G_FLOW_01: Clause = Clause::new(
    "G-FLOW-01",
    "rulebook-doc.md:132",
    "玩家在运营阶段可在非执行任何效果的时候进入移动阶段。移动阶段中需进行1次[主要移动]。",
    g_flow_01,
);

/// Executable form of [`G-FLOW-01`].
pub fn g_flow_01(step: &Step<'_>) -> Result<(), String> {
    let pre_moved = step.pre.turn.main_moved;
    let pre_turn = step.pre.st.turn;

    // A second [主要移动] attempt is the player pressing `roll` again while
    // the latch is already set. The engine must refuse it: `pos` does not
    // move. (A card's own dice is not a `roll` command.) An extra turn for
    // the same seat legitimately clears `main_moved` inside one settle -- that
    // is a turn boundary, not a second main move -- so the latch alone is not
    // a violation.
    let is_roll_cmd = matches!(step.act, StepAct::Act { msg, .. } if msg.act == "roll");
    if is_roll_cmd && pre_moved {
        let seat = step.pre.turn.player_id;
        let pos_before = step
            .pre
            .st
            .players
            .get(seat)
            .map(|p| p.pos)
            .unwrap_or(-1);
        let pos_after = step
            .post
            .st
            .players
            .get(seat)
            .map(|p| p.pos)
            .unwrap_or(-1);
        if pos_before != pos_after {
            return Err(format!(
                "second [主要移动] moved player {seat} {pos_before} -> {pos_after} in turn {pre_turn}"
            ));
        }
    }
    Ok(())
}