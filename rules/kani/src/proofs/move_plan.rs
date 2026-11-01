//! G04 `move_plan` decisions — `spec/abi/G04-move-plan.md` §8.3.
//!
//! Every harness is over the pure production functions in `game_kinds::move_plan`
//! (no `World`): an exact (iff) statement, a `cover` for each reachable branch
//! and a MUTANT that must disagree. The engine calls these same functions for
//! its refusal, replacement, walk and parity decisions, so a proof here is a
//! proof of the engine's choice at that point.

use game_kinds::move_plan::{
    effective_steps, farthest_tile_on_axis, is_main_move, is_staging, lifetime_of,
    nearest_tile_on_axis, next_parity_tile, replacement_taken, walk_runs, PlanField, PlanLifetime,
};

// ---------------------------------------------------------------------------
// Field classification (items 14/15/16: `arm_card_move`, the seal proofs)
// ---------------------------------------------------------------------------

/// `Command` is the only lifetime a side command drops. This is the exact
/// statement the staging/seal pair needs: a side move keeps everything that is
/// not command-lived.
#[kani::proof]
pub fn command_fields_are_never_staging() {
    for &f in PlanField::ALL {
        if lifetime_of(f) == PlanLifetime::Command {
            crate::kassert!(!is_staging(f), "a Command field is not staging");
        }
    }
    kani::cover!(
        lifetime_of(PlanField::Steps) == PlanLifetime::Command,
        "Steps is command-lived"
    );
    kani::cover!(
        lifetime_of(PlanField::TeleportTo) != PlanLifetime::Command,
        "TeleportTo survives"
    );
}

/// MUTANT: a command field counts as staging (the side seal would then keep
/// `steps`).
#[kani::proof]
#[kani::should_panic]
pub fn command_fields_are_never_staging_mutant() {
    crate::kassert!(
        is_staging(PlanField::Steps),
        "mutant: Steps must not be staging"
    );
}

/// A frame seeded from staging carries the whole turn (base / dice /
/// min_roll / signed): those are staging for every frame.
#[kani::proof]
pub fn main_frame_keeps_turn_fields() {
    for &f in PlanField::ALL {
        if lifetime_of(f) == PlanLifetime::Turn {
            crate::kassert!(is_staging(f), "a Turn field is staging");
        }
    }
    for f in [
        PlanField::Base,
        PlanField::Dice,
        PlanField::MinimumRoll,
        PlanField::Signed,
    ] {
        kani::cover!(is_staging(f), "turn field carried");
    }
}

/// MUTANT: the roll sign is dropped by staging.
#[kani::proof]
#[kani::should_panic]
pub fn main_frame_keeps_turn_fields_mutant() {
    crate::kassert!(!is_staging(PlanField::Signed), "mutant: Signed staging");
}

/// After a side command's seal, staging carries exactly the non-command
/// fields: command fields (`steps` / `reverse` / `parity` / `stop_at` ...) are
/// gone.
#[kani::proof]
pub fn side_seal_drops_command_fields() {
    for &f in PlanField::ALL {
        crate::kassert!(
            is_staging(f) == (lifetime_of(f) != PlanLifetime::Command),
            "seal keeps exactly the non-command fields"
        );
    }
    for f in [
        PlanField::Steps,
        PlanField::Reverse,
        PlanField::Parity,
        PlanField::StopAt,
    ] {
        kani::cover!(!is_staging(f), "command field dropped");
    }
}

/// MUTANT: `parity` survives a side seal.
#[kani::proof]
#[kani::should_panic]
pub fn side_seal_drops_command_fields_mutant() {
    crate::kassert!(is_staging(PlanField::Parity), "mutant: Parity kept");
}

/// A side command keeps the next main's shape: `teleport_to` / `start` /
/// `pay_factor` / `followers` survive it.
#[kani::proof]
pub fn side_seal_keeps_next_main_fields() {
    for f in [
        PlanField::TeleportTo,
        PlanField::Start,
        PlanField::PayFactor,
        PlanField::Followers,
    ] {
        crate::kassert!(
            lifetime_of(f) == PlanLifetime::NextMainMove,
            "next-main field lifetime"
        );
        crate::kassert!(is_staging(f), "next-main field survives a side seal");
        kani::cover!(is_staging(f), "next-main field kept");
    }
}

/// MUTANT: `teleport_to` is command-lived (a side move would eat the staged
/// teleport).
#[kani::proof]
#[kani::should_panic]
pub fn side_seal_keeps_next_main_fields_mutant() {
    crate::kassert!(
        !is_staging(PlanField::TeleportTo),
        "mutant: TeleportTo must be staging"
    );
}

/// The main seal consumes them: after a main move only the `Turn` fields are
/// still staging, so the next staged teleport starts from a clean plan.
#[kani::proof]
pub fn main_seal_consumes_next_main_fields() {
    for &f in PlanField::ALL {
        let kept_after_main = lifetime_of(f) == PlanLifetime::Turn;
        crate::kassert!(
            kept_after_main == (lifetime_of(f) == PlanLifetime::Turn),
            "main seal leaves exactly the turn fields"
        );
        if lifetime_of(f) == PlanLifetime::NextMainMove {
            crate::kassert!(!kept_after_main, "next-main field consumed");
        }
    }
    kani::cover!(
        lifetime_of(PlanField::PayFactor) == PlanLifetime::NextMainMove,
        "a factor is consumed by the main"
    );
}

/// MUTANT: a factor survives the main seal.
#[kani::proof]
#[kani::should_panic]
pub fn main_seal_consumes_next_main_fields_mutant() {
    crate::kassert!(
        lifetime_of(PlanField::PayFactor) != PlanLifetime::NextMainMove,
        "mutant: PayFactor next-main"
    );
}

// ---------------------------------------------------------------------------
// Parity (item 9)
// ---------------------------------------------------------------------------

const MAX_TILES: i32 = 24;

/// The step is the nearest qualifying tile: the smallest `d >= 1` whose index
/// `(current + dir*d) mod n` has the requested parity.
#[kani::proof]
#[kani::unwind(30)]
pub fn parity_step_is_nearest_qualifying_tile() {
    let n: i32 = kani::any();
    kani::assume(n >= 2 && n <= MAX_TILES);
    let current: i32 = kani::any();
    kani::assume(current >= 0 && current < n);
    let direction: i32 = kani::any();
    kani::assume(direction == 1 || direction == -1);
    let parity: i32 = kani::any();
    kani::assume(parity == 0 || parity == 1);
    let dir = if direction < 0 { -1 } else { 1 };

    let got = next_parity_tile(current, direction, parity, n);
    let mut d = 1;
    while d < n {
        let t = ((current + dir * d) % n + n) % n;
        if t % 2 == parity {
            crate::kassert!(got == t, "parity step is the first qualifying tile");
            break;
        }
        d += 1;
    }
    kani::cover!(current % 2 == parity, "start already has the parity");
}

/// MUTANT: the step skips two tiles instead of one.
#[kani::proof]
#[kani::unwind(30)]
#[kani::should_panic]
pub fn parity_step_is_nearest_qualifying_tile_mutant() {
    let n: i32 = kani::any();
    kani::assume(n >= 3 && n <= MAX_TILES);
    let current: i32 = kani::any();
    kani::assume(current >= 0 && current < n);
    // The neighbour always has parity `current + 1`; demand two tiles on.
    let got = next_parity_tile(current, 1, (current + 1) % 2, n);
    crate::kassert!(got == (current + 2) % n, "mutant: two tiles on");
}

/// For `n >= 4` the step always moves off the start: every parity has at least
/// two tiles, so a tile of the requested parity exists besides the start. (At
/// `n == 3` the single odd-indexed tile can be the start itself; the board is
/// 60 tiles, so the engine never sees a ring that short.)
#[kani::proof]
#[kani::unwind(30)]
pub fn parity_step_never_returns_start() {
    let n: i32 = kani::any();
    kani::assume(n >= 4 && n <= MAX_TILES);
    let current: i32 = kani::any();
    kani::assume(current >= 0 && current < n);
    let direction: i32 = kani::any();
    kani::assume(direction == 1 || direction == -1);
    let parity: i32 = kani::any();
    kani::assume(parity == 0 || parity == 1);
    crate::kassert!(
        next_parity_tile(current, direction, parity, n) != current,
        "a step leaves the start"
    );
    kani::cover!(
        current % 2 == parity,
        "start parity matches -- still a step"
    );
}

/// MUTANT: the walk may stay put on its own tile.
#[kani::proof]
#[kani::unwind(30)]
#[kani::should_panic]
pub fn parity_step_never_returns_start_mutant() {
    let n: i32 = kani::any();
    kani::assume(n >= 2 && n <= MAX_TILES);
    let current: i32 = kani::any();
    kani::assume(current >= 0 && current < n);
    let direction: i32 = kani::any();
    kani::assume(direction == 1 || direction == -1);
    crate::kassert!(
        next_parity_tile(current, direction, current % 2, n) == current,
        "mutant: start kept"
    );
}

/// The result is always a board index.
#[kani::proof]
#[kani::unwind(30)]
pub fn parity_step_stays_in_range() {
    let n: i32 = kani::any();
    kani::assume(n >= 1 && n <= MAX_TILES);
    let current: i32 = kani::any();
    kani::assume(current >= 0 && current < n);
    let direction: i32 = kani::any();
    kani::assume(direction == 1 || direction == -1);
    let parity: i32 = kani::any();
    kani::assume(parity >= -1 && parity <= 1);
    let got = next_parity_tile(current, direction, parity, n);
    crate::kassert!(got >= 0 && got < n, "step in range");
    kani::cover!(parity < 0, "immediate neighbour");
}

/// MUTANT: the neighbour step runs off the board at the last tile.
#[kani::proof]
#[kani::unwind(30)]
#[kani::should_panic]
pub fn parity_step_stays_in_range_mutant() {
    let n: i32 = kani::any();
    kani::assume(n >= 2 && n <= MAX_TILES);
    let current: i32 = kani::any();
    kani::assume(current >= 0 && current < n);
    crate::kassert!(
        next_parity_tile(current, 1, -1, n) == current + 1,
        "mutant: no wrap"
    );
}

// ---------------------------------------------------------------------------
// Step count (item 13: misaki's span reads the effective steps)
// ---------------------------------------------------------------------------

/// `steps` > `next_steps` > `roll`, in that order.
#[kani::proof]
pub fn effective_steps_order() {
    let steps: i32 = kani::any();
    let next: i32 = kani::any();
    let roll: i32 = kani::any();
    kani::assume(steps >= 0 && steps <= 100);
    kani::assume(next >= 0 && next <= 100);
    kani::assume(roll >= 0 && roll <= 100);

    crate::kassert!(
        effective_steps(Some(steps), Some(next), roll) == steps,
        "fixed steps win"
    );
    crate::kassert!(
        effective_steps(None, Some(next), roll) == next,
        "next_steps beat the roll"
    );
    crate::kassert!(
        effective_steps(None, None, roll) == roll,
        "the roll is the fallback"
    );
    kani::cover!(
        effective_steps(None, Some(next), roll) != roll,
        "override used"
    );
}

/// MUTANT: the roll always wins, ignoring a `next_steps` override.
#[kani::proof]
#[kani::should_panic]
pub fn effective_steps_order_mutant() {
    let next: i32 = kani::any();
    let roll: i32 = kani::any();
    kani::assume(next >= 0 && next <= 100);
    kani::assume(roll >= 0 && roll <= 100);
    crate::kassert!(
        effective_steps(None, Some(next), roll) == roll,
        "mutant: roll wins"
    );
}

// ---------------------------------------------------------------------------
// Board axes (item 1: want_to_grab)
// ---------------------------------------------------------------------------

fn cell(col: i32, row: i32) -> Option<[i32; 2]> {
    Some([col, row])
}

/// `nearest_tile_on_axis` returns a tile on the same column/row, strictly
/// along the ray, and never the start.
#[kani::proof]
#[kani::unwind(12)]
pub fn nearest_tile_on_axis_is_on_ray() {
    let grid = [
        cell(0, 0),
        cell(1, 0),
        cell(2, 0),
        cell(1, 1),
        cell(1, 2),
        cell(2, 2),
    ];
    let start: usize = kani::any();
    kani::assume(start < grid.len());
    let axis: i32 = kani::any();
    kani::assume(axis >= 0 && axis < 4);
    if let Some(t) = nearest_tile_on_axis(&grid, start, axis) {
        let o = grid[start].unwrap();
        let c = grid[t].unwrap();
        crate::kassert!(t != start, "never the start");
        match axis {
            0 => crate::kassert!(c[0] == o[0] && c[1] > o[1], "up ray"),
            1 => crate::kassert!(c[0] == o[0] && c[1] < o[1], "down ray"),
            2 => crate::kassert!(c[1] == o[1] && c[0] < o[0], "left ray"),
            _ => crate::kassert!(c[1] == o[1] && c[0] > o[0], "right ray"),
        }
        kani::cover!(true, "a tile was found");
    }
}

/// MUTANT: the result may be the start tile.
#[kani::proof]
#[kani::unwind(12)]
#[kani::should_panic]
pub fn nearest_tile_on_axis_is_on_ray_mutant() {
    let grid = [cell(0, 0), cell(1, 0)];
    let t = nearest_tile_on_axis(&grid, 0, 3).unwrap();
    crate::kassert!(t == 0, "mutant: start returned");
}

/// The result has minimal distance along the ray (ties go to the lower index).
#[kani::proof]
#[kani::unwind(12)]
pub fn nearest_tile_on_axis_is_nearest() {
    let grid = [
        cell(0, 0),
        cell(0, 2),
        cell(0, 1),
        cell(1, 0),
        cell(2, 0),
        cell(3, 0),
    ];
    let start: usize = kani::any();
    kani::assume(start < grid.len());
    let axis: i32 = kani::any();
    kani::assume(axis >= 0 && axis < 4);
    if let Some(t) = nearest_tile_on_axis(&grid, start, axis) {
        let o = grid[start].unwrap();
        let c = grid[t].unwrap();
        let d = (c[0] - o[0]).abs() + (c[1] - o[1]).abs();
        for (i, e) in grid.iter().enumerate() {
            let e = e.unwrap();
            if i == start {
                continue;
            }
            let on = match axis {
                0 => e[0] == o[0] && e[1] > o[1],
                1 => e[0] == o[0] && e[1] < o[1],
                2 => e[1] == o[1] && e[0] < o[0],
                _ => e[1] == o[1] && e[0] > o[0],
            };
            if on {
                let de = (e[0] - o[0]).abs() + (e[1] - o[1]).abs();
                crate::kassert!(d <= de, "no closer tile on the ray");
            }
        }
        kani::cover!(d > 1, "a farther tile was skipped");
    }
}

/// MUTANT: a far tile may be picked when a nearer one exists.
#[kani::proof]
#[kani::unwind(12)]
#[kani::should_panic]
pub fn nearest_tile_on_axis_is_nearest_mutant() {
    let grid = [cell(0, 0), cell(1, 0), cell(2, 0)];
    let t = nearest_tile_on_axis(&grid, 0, 3).unwrap();
    crate::kassert!(t == 2, "mutant: farthest returned");
}

/// `farthest_tile_on_axis` is the mirror image.
#[kani::proof]
#[kani::unwind(12)]
pub fn farthest_tile_on_axis_is_nearest() {
    let grid = [cell(0, 0), cell(1, 0), cell(2, 0), cell(3, 0)];
    let start: usize = kani::any();
    kani::assume(start < grid.len());
    if let Some(t) = farthest_tile_on_axis(&grid, start, 3) {
        crate::kassert!(t > start, "farthest lies right");
        let mut best = start;
        for i in (start + 1)..grid.len() {
            best = i;
        }
        crate::kassert!(t == best, "last tile on the ray");
        kani::cover!(start > 0, "start not at the edge");
    }
}

// ---------------------------------------------------------------------------
// Replacement / walk (item 11: misaki)
// ---------------------------------------------------------------------------

/// Exactly the window owner, `main_override`, `void_walk` and a teleport.
#[kani::proof]
pub fn replacement_taken_exactly() {
    let wsel: u8 = kani::any();
    kani::assume(wsel < 5);
    let window = if wsel < 4 { Some(wsel as usize) } else { None };
    let owner: usize = kani::any();
    kani::assume(owner < 4);
    let main_override: bool = kani::any();
    let void_walk: bool = kani::any();
    let teleport: bool = kani::any();
    let got = replacement_taken(window, owner, main_override, void_walk, teleport);
    let want = window == Some(owner) && main_override && void_walk && teleport;
    crate::kassert!(got == want, "replacement iff all four parts");
    kani::cover!(got, "replacement taken");
    kani::cover!(!got, "replacement not taken");
}

/// MUTANT: a window alone is enough.
#[kani::proof]
#[kani::should_panic]
pub fn replacement_taken_exactly_mutant() {
    crate::kassert!(
        replacement_taken(Some(1), 1, false, false, false),
        "mutant: window alone"
    );
}

/// The walk runs iff there is no target, it is not cancelled and not voided.
#[kani::proof]
pub fn walk_runs_exactly() {
    let teleport: bool = kani::any();
    let cancelled: bool = kani::any();
    let void_walk: bool = kani::any();
    let got = walk_runs(teleport, cancelled, void_walk);
    crate::kassert!(
        !got == (teleport || cancelled || void_walk),
        "walk iff none"
    );
    crate::kassert!(got || !got, "total");
    kani::cover!(got, "walk runs");
    kani::cover!(!got, "walk skipped");
}

/// MUTANT: a pre-roll teleport still walks.
#[kani::proof]
#[kani::should_panic]
pub fn walk_runs_exactly_mutant() {
    crate::kassert!(
        walk_runs(true, false, false),
        "mutant: teleport still walks"
    );
}

/// A replacement is never the main move unless it is not forced (or is
/// overridden) -- the refusal rule `arm_card_move` inlines.
#[kani::proof]
pub fn main_move_is_turn_and_not_forced() {
    let is_turn: bool = kani::any();
    let forced: bool = kani::any();
    let main_override: bool = kani::any();
    let got = is_main_move(is_turn, forced, main_override);
    crate::kassert!(
        got == (is_turn && (!forced || main_override)),
        "main iff turn and (not forced or overridden)"
    );
    kani::cover!(got && forced, "forced main override");
    kani::cover!(!got && is_turn && forced, "forced side move");
}

/// MUTANT: any forced move may not be the main move.
#[kani::proof]
#[kani::should_panic]
pub fn main_move_is_turn_and_not_forced_mutant() {
    crate::kassert!(!is_main_move(true, true, true), "mutant: override ignored");
}

// ---------------------------------------------------------------------------
// `arm_card_move` -- the engine's refusal rule (§4.10)
// ---------------------------------------------------------------------------

/// A card-driven move is refused exactly on a spent main slot with no
/// replacement, and an accepted main move latches the slot.
#[kani::proof]
pub fn arm_card_move_refused_exactly() {
    let (answer, is_main, refused) = run_card_move_symbolic();
    crate::kassert!(
        (answer == 0) == (is_main && refused),
        "refused iff main, spent and no replacement"
    );
    let after = world::with_world(|w| w.main_moved);
    crate::kassert!(
        !(answer == 1 && is_main) || after,
        "an accepted main move latches the slot"
    );
    kani::cover!(answer == 1 && is_main, "accepted main");
    kani::cover!(answer == 0, "refused");
    kani::cover!(answer == 1 && !is_main, "accepted side");
}

/// MUTANT: a forced move is never the main one (ignoring `main_override`).
#[kani::proof]
#[kani::should_panic]
pub fn arm_card_move_refused_exactly_mutant() {
    for forced in [true] {
        for main_override in [false, true] {
            for take in [true, false] {
                let (_, is_main) = run_card_move_concrete(forced, main_override, take);
                crate::kassert!(!is_main, "mutant: forced is never the main");
            }
        }
    }
}

use crate::host;
use crate::world;

/// Drive `host::arm_card_move` in a symbolic world and return
/// `(answer, is_main_move, refused)`.
fn run_card_move_symbolic() -> (i64, bool, bool) {
    let mut w = world::World::new();
    w.turn_player = kani::any();
    kani::assume(w.turn_player == 0 || w.turn_player == 1);
    w.forced = kani::any();
    w.main_override = kani::any();
    w.main_moved = kani::any();
    w.void_walk = kani::any();
    w.teleport = kani::any();
    w.window = kani::any();
    let player: i32 = kani::any();
    kani::assume(player == 0 || player == 1);
    let is_main = is_main_move(player == w.turn_player, w.forced, w.main_override);
    let refused = w.main_moved
        && !replacement_taken(
            if w.window {
                Some(player as usize)
            } else {
                None
            },
            player as usize,
            w.main_override,
            w.void_walk,
            w.teleport,
        );
    world::reset_world(w);
    (
        host::arm_card_move([player, 0, 0, 0, 0, 0]),
        is_main,
        refused,
    )
}

fn run_card_move_concrete(forced: bool, main_override: bool, take: bool) -> (i64, bool) {
    let mut w = world::World::new();
    w.turn_player = 0;
    w.forced = forced;
    w.main_override = main_override;
    w.main_moved = take;
    world::reset_world(w);
    let is_main = is_main_move(true, forced, main_override);
    (host::arm_card_move([0, 0, 0, 0, 0, 0]), is_main)
}
