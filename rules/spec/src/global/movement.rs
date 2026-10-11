//! G-MOVE -- [路径] / [经过] / [传送] / the main move.
//!
//! Owned by unit 4. Append to [`CLAUSES`]; do not edit other areas.

use spec_kit::{Clause, Step};

/// Movement clauses.
pub const CLAUSES: &[Clause] = &[G_MOVE_01];

/// A player's position is always a legal board index.
///
/// * **ID:** G-MOVE-01
/// * **Source:** `rulebook-doc.md:90`
/// * **Quote:** 「[路径]：[移动起点]与[移动终点]之间的路径所包含的格子，[路径]不包括[移动起点]但包括[移动终点]，如果两点相同则包括」
/// * **Trigger:** every observed input, on the settled `post` world.
/// * **Condition:** the board has `n` tiles (indices `0..n`).
/// * **Effect:** every player's `pos` is in `0..n`.
/// * **Frame:** n/a (pure read).
/// * **Rulings & OPEN:** the quote fixes what a [路径] *contains*; a full
///   path check needs the move's start / end / direction out of the event
///   stream and is unit 4's work. This example is the standing index
///   invariant every path statement presupposes. 「如果两点相同则包括」
///   (start == end includes the tile) is OPEN for the zero-step case -- see
///   the area doc.
/// * **Executable:** [`g_move_01`].
const G_MOVE_01: Clause = Clause::new(
    "G-MOVE-01",
    "rulebook-doc.md:90",
    "[路径]：[移动起点]与[移动终点]之间的路径所包含的格子，[路径]不包括[移动起点]但包括[移动终点]，如果两点相同则包括",
    g_move_01,
);

/// Executable form of [`G_MOVE_01`].
pub fn g_move_01(step: &Step<'_>) -> Result<(), String> {
    let n = step.post.st.owners.len() as i32;
    if n <= 0 {
        return Err("board has no tiles".into());
    }
    for (i, p) in step.post.st.players.iter().enumerate() {
        if p.pos < 0 || p.pos >= n {
            return Err(format!("player {i} pos {} outside 0..{n}", p.pos));
        }
    }
    Ok(())
}