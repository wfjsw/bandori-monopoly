//! G09 pure model: build price, tile-effect suppression, house fall, the
//! build pipeline, the swap layer and the move path.
//!
//! `docs/rulebook/spec/abi/G09-tile-build-pass.md` §5.7 / §9.4. Every function
//! here is a restatement of one clause of the spec; the mutants beside it
//! disagree with the clause where the hazard is, so a proof that only pins the
//! model would still catch the real defect if the body drifted.

/// Growable move path length modelled by the harness.
pub const MAX_STEPS: usize = 12;

// ---------------------------------------------------------------- price (§5.2)

/// The effective build price: `max(0, base * percent / 100 - cut)`. Rounding
/// is the engine's once-per-multiply floor division, applied before the cut so
/// a cut can bring the figure down but never negative.
pub fn price_of(base: i64, percent: i64, cut: i64) -> i64 {
    let priced = base * percent / 100 - cut;
    if priced < 0 {
        0
    } else {
        priced
    }
}

/// Mutant: clamp at 0 *before* the cut, so a cut that exceeds the discounted
/// figure leaves the un-cut remainder negative.
pub fn price_of_mutant_clamp_early(base: i64, percent: i64, cut: i64) -> i64 {
    let priced = base * percent / 100;
    (if priced < 0 { 0 } else { priced }) - cut
}

/// Mutant: no 0 floor at all, so the cut can make the price negative.
pub fn price_of_mutant_no_floor(base: i64, percent: i64, cut: i64) -> i64 {
    base * percent / 100 - cut
}

// ----------------------------------------------------- pipeline commit (§5.3)

/// A house commits iff every stage admits: the gate (buildable, ownership when
/// the card asks for it), `buildBefore` standing, the chosen charge payable,
/// and a house still fitting on the tile. The charge only sizes the price; it
/// never replaces a stage.
pub fn pipeline_commits(gate: bool, before_stands: bool, payable: bool, takes_house: bool) -> bool {
    gate && before_stands && payable && takes_house
}

/// Mutant: the gate is skipped (only the veto and the money matter).
pub fn pipeline_commits_mutant_skip_gate(
    _gate: bool,
    before_stands: bool,
    payable: bool,
    takes_house: bool,
) -> bool {
    before_stands && payable && takes_house
}

/// Mutant: the `buildBefore` veto is skipped (gate and money suffice).
pub fn pipeline_commits_mutant_skip_veto(
    gate: bool,
    _before_stands: bool,
    payable: bool,
    takes_house: bool,
) -> bool {
    gate && payable && takes_house
}

// ---------------------------------------------------- suppression (§4.3, §9.2)

/// The tag hides an instance exactly when the tag was armed with the
/// **governing tile** and the instance's own tile is that same tile, and the
/// instance is not a `tile:` base (the base always runs). The settle's slot is
/// deliberately not consulted: the extras travel with the tile.
pub fn suppressed(
    tag_value: i64,
    trigger_tile: i64,
    _trigger_position: i64,
    instance_tile: i64,
    is_base: bool,
) -> bool {
    tag_value == trigger_tile && instance_tile == trigger_tile && !is_base
}

/// Mutant: compares the tag against the settle's slot instead of the tile.
pub fn suppressed_mutant_position(
    tag_value: i64,
    _trigger_tile: i64,
    trigger_position: i64,
    instance_tile: i64,
    is_base: bool,
) -> bool {
    tag_value == trigger_position && instance_tile == trigger_position && !is_base
}

/// Mutant: also hides the `tile:` base.
pub fn suppressed_mutant_hides_base(
    tag_value: i64,
    trigger_tile: i64,
    _trigger_position: i64,
    instance_tile: i64,
    _is_base: bool,
) -> bool {
    tag_value == trigger_tile && instance_tile == trigger_tile
}

// ---------------------------------------------------------- house fall (§5.4)

/// A `houseRemoved` is raised exactly on a strict fall, once per write.
pub fn fall_raised(before: u32, after: u32) -> bool {
    after < before
}

/// Mutant: fires on an unchanged count too (`<=`).
pub fn fall_raised_mutant_le(before: u32, after: u32) -> bool {
    after <= before
}

// --------------------------------------------------------------- swap (§5.1)

/// One swap record: the instance at `slot` names `partner`.
#[derive(Copy, Clone, Debug)]
pub struct Swap {
    pub slot: i64,
    pub partner: i64,
}

/// The tile **displayed** at position `p`: the partner of a swap when `p` is
/// either end of it, `p` itself otherwise.
pub fn tile_at(swap: Option<Swap>, p: i64) -> i64 {
    match swap {
        Some(s) if p == s.slot => s.partner,
        Some(s) if p == s.partner => s.slot,
        _ => p,
    }
}

/// The board position whose display is `t`: the other end of the swap when `t`
/// is one, `t` itself otherwise. `tile_at ∘ position_of_tile` and
/// `position_of_tile ∘ tile_at` are the identity on the pair.
pub fn position_of_tile(swap: Option<Swap>, t: i64) -> i64 {
    tile_at(swap, t)
}

// ---------------------------------------------------------------- path (§5.5)

/// The traversed tiles of a `steps` walk from `start`, in order: every step's
/// landing tile, not just the endpoint.
pub fn travel(start: i64, steps: usize) -> ([i64; MAX_STEPS], usize) {
    let mut path = [0i64; MAX_STEPS];
    let n = if steps > MAX_STEPS { MAX_STEPS } else { steps };
    for (i, slot) in path.iter_mut().enumerate().take(n) {
        *slot = start + i as i64 + 1;
    }
    (path, n)
}

/// Mutant: keeps only the endpoint, padding the rest.
pub fn travel_mutant_endpoint_only(start: i64, steps: usize) -> ([i64; MAX_STEPS], usize) {
    let mut path = [0i64; MAX_STEPS];
    let n = if steps > MAX_STEPS { MAX_STEPS } else { steps };
    if n > 0 {
        path[n - 1] = start + n as i64;
    }
    (path, n)
}

/// Does `tile_at` still report the start at the position a card on the start
/// tile reads? The start prop is keyed by display, and the swap never moves
/// the *position* that displays it.
pub fn start_never_travels(swap: Option<Swap>, start: i64) -> bool {
    let p = position_of_tile(swap, start);
    tile_at(swap, p) == start
}

/// Mutant: reads the start prop off the position directly, ignoring the swap.
pub fn start_never_travels_mutant(swap: Option<Swap>, start: i64) -> bool {
    tile_at(swap, start) == start
}
