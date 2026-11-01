//! Pure movement-plan decisions shared by the engine and the Kani proofs.
//!
//! Nothing here touches a `World`: every function is a total function of its
//! arguments, so the proofs over this module are proofs over the engine's
//! decisions (`spec/abi/G04-move-plan.md` §4.10).

use serde::{Deserialize, Serialize};

/// Which slot a live plan frame was opened for.
///
/// A `MainMove` frame consumes the turn's main move; a `SideMove` frame is a
/// card / forced move that leaves the slot alone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanKind {
    #[default]
    MainMove,
    SideMove,
}

/// Every field a plan can shape.
///
/// The seed/restore match in the engine is exhaustive over this enum, so a new
/// plan field cannot be forgotten when a frame is built. Classification lives
/// in [`lifetime_of`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanField {
    Steps,
    Reverse,
    Parity,
    Kind,
    TeleportTo,
    Start,
    StartReason,
    StopAt,
    SettleTile,
    SettleRedirect,
    Resolve,
    PayFactor,
    RentFactor,
    Followers,
    ExtraSteps,
    NextSteps,
    MinimumRoll,
    Signed,
    Base,
    Dice,
    MoreSteps,
    NoBuy,
    CanBuild,
    BuildAnywhere,
    SettleAsAgent,
    Tags,
    BySkill,
    Forced,
    MainOverride,
    VoidWalk,
    Rolled,
    Reason,
    Roller,
    Cancelled,
    IsMainMove,
    From,
    To,
    Path,
    Total,
    Remaining,
    PassedPlayers,
    Stopped,
}

impl PlanField {
    /// Every field, for exhaustive iterations.
    pub const ALL: &'static [PlanField] = &[
        PlanField::Steps,
        PlanField::Reverse,
        PlanField::Parity,
        PlanField::Kind,
        PlanField::TeleportTo,
        PlanField::Start,
        PlanField::StartReason,
        PlanField::StopAt,
        PlanField::SettleTile,
        PlanField::SettleRedirect,
        PlanField::Resolve,
        PlanField::PayFactor,
        PlanField::RentFactor,
        PlanField::Followers,
        PlanField::ExtraSteps,
        PlanField::NextSteps,
        PlanField::MinimumRoll,
        PlanField::Signed,
        PlanField::Base,
        PlanField::Dice,
        PlanField::MoreSteps,
        PlanField::NoBuy,
        PlanField::CanBuild,
        PlanField::BuildAnywhere,
        PlanField::SettleAsAgent,
        PlanField::Tags,
        PlanField::BySkill,
        PlanField::Forced,
        PlanField::MainOverride,
        PlanField::VoidWalk,
        PlanField::Rolled,
        PlanField::Reason,
        PlanField::Roller,
        PlanField::Cancelled,
        PlanField::IsMainMove,
        PlanField::From,
        PlanField::To,
        PlanField::Path,
        PlanField::Total,
        PlanField::Remaining,
        PlanField::PassedPlayers,
        PlanField::Stopped,
    ];
}

/// How long a field's value is meant to last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanLifetime {
    /// Lasts the whole turn (the dice tables, the roll's sign).
    Turn,
    /// Survives a side move; consumed by the next main move (a staged
    /// teleport, a factor for the next main).
    NextMainMove,
    /// Belongs to one move; gone when the command that shaped it ends.
    Command,
}

/// The lifetime of `f`.
pub fn lifetime_of(f: PlanField) -> PlanLifetime {
    use PlanField::*;
    match f {
        MinimumRoll | Signed | Base | Dice => PlanLifetime::Turn,
        TeleportTo | Start | StartReason | PayFactor | RentFactor | Followers | BySkill => {
            PlanLifetime::NextMainMove
        }
        Steps | Reverse | Parity | Kind | StopAt | SettleTile | SettleRedirect | Resolve
        | ExtraSteps | NextSteps | MoreSteps | NoBuy | CanBuild | BuildAnywhere | SettleAsAgent
        | Tags | Forced | MainOverride | VoidWalk | Rolled | Reason | Roller | Cancelled
        | IsMainMove | From | To | Path | Total | Remaining | PassedPlayers | Stopped => {
            PlanLifetime::Command
        }
    }
}

/// `Turn` or `NextMainMove` -- a staging field, not one move's shape.
pub fn is_staging(f: PlanField) -> bool {
    lifetime_of(f) != PlanLifetime::Command
}

/// A `run_planned_move` of the turn player is the main move unless it is forced
/// without the 「视为主要移动」 override.
pub fn is_main_move(is_turn_player: bool, forced: bool, main_override: bool) -> bool {
    is_turn_player && (!forced || main_override)
}

/// The innermost open roll window belongs to `owner`, and the call replaces the
/// pending walk instead of starting a second move.
pub fn replacement_taken(
    window: Option<usize>,
    owner: usize,
    main_override: bool,
    void_walk: bool,
    teleport: bool,
) -> bool {
    window == Some(owner) && main_override && void_walk && teleport
}

/// The walk still runs: no teleport target, not cancelled, not voided.
pub fn walk_runs(teleport: bool, cancelled: bool, void_walk: bool) -> bool {
    !teleport && !cancelled && !void_walk
}

/// The length a move takes: a fixed count, else the stored `next_steps`
/// override, else the roll.
pub fn effective_steps(steps: Option<i32>, override_steps: Option<i32>, roll: i32) -> i32 {
    steps.unwrap_or_else(|| override_steps.unwrap_or(roll))
}

/// The next tile walked from `current` in `direction`.
///
/// `parity < 0` is the immediate neighbour; otherwise it is the nearest tile
/// whose index parity matches (`0` even, `1` odd), skipping the tiles between.
pub fn next_parity_tile(current: i32, direction: i32, parity: i32, tile_count: i32) -> i32 {
    if tile_count <= 0 {
        return current;
    }
    let dir = if direction < 0 { -1 } else { 1 };
    let mut next = current;
    for _ in 0..tile_count {
        next = ((next + dir) % tile_count + tile_count) % tile_count;
        if parity < 0 || next % 2 == parity {
            return next;
        }
    }
    next
}

/// Whether `cell` lies strictly along `axis` from `origin`.
///
/// `board.json` stores `grid_cell` as `[column, row]` with row `0` at the top,
/// so 上 (`axis::UP`) decreases the row.
fn on_axis(origin: [i32; 2], cell: [i32; 2], axis: i32) -> bool {
    match axis {
        0 => cell[0] == origin[0] && cell[1] < origin[1],
        1 => cell[0] == origin[0] && cell[1] > origin[1],
        2 => cell[1] == origin[1] && cell[0] < origin[0],
        3 => cell[1] == origin[1] && cell[0] > origin[0],
        _ => false,
    }
}

fn distance(origin: [i32; 2], cell: [i32; 2]) -> i32 {
    (cell[0] - origin[0]).abs() + (cell[1] - origin[1]).abs()
}

/// The closest tile strictly along `axis` from `tile`, or `None`.
pub fn nearest_tile_on_axis(
    grid_cells: &[Option<[i32; 2]>],
    tile: usize,
    axis: i32,
) -> Option<usize> {
    let origin = grid_cells.get(tile).copied().flatten()?;
    grid_cells
        .iter()
        .enumerate()
        .filter_map(|(i, c)| Some((i, (*c)?)))
        .filter(|(i, c)| *i != tile && on_axis(origin, *c, axis))
        .min_by_key(|(i, c)| (distance(origin, *c), *i))
        .map(|(i, _)| i)
}

/// The farthest tile strictly along `axis` from `tile`, or `None`.
pub fn farthest_tile_on_axis(
    grid_cells: &[Option<[i32; 2]>],
    tile: usize,
    axis: i32,
) -> Option<usize> {
    let origin = grid_cells.get(tile).copied().flatten()?;
    grid_cells
        .iter()
        .enumerate()
        .filter_map(|(i, c)| Some((i, (*c)?)))
        .filter(|(i, c)| *i != tile && on_axis(origin, *c, axis))
        .max_by_key(|(i, c)| (distance(origin, *c), *i))
        .map(|(i, _)| i)
}
