//! G09 harnesses on the pure model (`spec::abi_g09`).
//!
//! `docs/rulebook/spec/abi/G09-tile-build-pass.md` §9.4. Each harness pins the
//! clause exactly (no less, no more) and `cover!`s a point where the mutant
//! beside it disagrees, so a passing proof also shows the hazard reachable.
//! The bodies are stringified at the protocol level, so the model is the SDK
//! side of the contract.

use crate::spec::abi_g09 as m;

#[kani::proof]
pub fn price_iff() {
    let base: i64 = kani::any();
    let percent: i64 = kani::any();
    let cut: i64 = kani::any();
    kani::assume((0..=500).contains(&base));
    kani::assume((0..=200).contains(&percent));
    kani::assume((0..=2000).contains(&cut));

    let want = base * percent / 100 - cut;
    let want = if want < 0 { 0 } else { want };

    crate::kassert!(m::price_of(base, percent, cut) == want, "price: exact");
    crate::kassert!(m::price_of(base, percent, cut) >= 0, "price: floored");

    // The cut is taken after the discount, so only the un-clamped composition
    // can differ -- and it does exactly when the cut exceeds the discounted
    // figure.
    let discounted = base * percent / 100;
    kani::cover!(cut > discounted, "mutant: clamp before cut reachable");
    crate::kassert!(
        discounted - cut < 0 || m::price_of_mutant_clamp_early(base, percent, cut) == want,
        "clamp-early mutant agrees above the floor only"
    );
    if discounted - cut < 0 {
        crate::kassert!(
            m::price_of_mutant_clamp_early(base, percent, cut) != want,
            "clamp-early mutant disagrees below the floor"
        );
    }
    kani::cover!(discounted > 0 && cut > 0, "mutant: any cut reachable");
    crate::kassert!(
        m::price_of_mutant_no_floor(base, percent, cut) == want || cut > discounted,
        "no-floor mutant agrees while non-negative"
    );
}

#[kani::proof]
pub fn pipeline_commits_iff() {
    let gate: bool = kani::any();
    let before: bool = kani::any();
    let payable: bool = kani::any();
    let takes: bool = kani::any();

    crate::kassert!(
        m::pipeline_commits(gate, before, payable, takes) == (gate && before && payable && takes),
        "pipeline: every stage is necessary"
    );

    // Each stage alone is decisive: the corresponding mutant disagrees at the
    // point where it is the only false one.
    kani::cover!(!gate && before && payable && takes, "gate-only refusal");
    crate::kassert!(
        m::pipeline_commits(gate, before, payable, takes)
            == m::pipeline_commits_mutant_skip_gate(gate, before, payable, takes)
            || (!gate && before && payable && takes),
        "skip-gate mutant only differs on a gate-only refusal"
    );
    kani::cover!(gate && !before && payable && takes, "veto-only refusal");
    crate::kassert!(
        m::pipeline_commits(gate, before, payable, takes)
            == m::pipeline_commits_mutant_skip_veto(gate, before, payable, takes)
            || (gate && !before && payable && takes),
        "skip-veto mutant only differs on a veto-only refusal"
    );
}

#[kani::proof]
pub fn suppressed_iff() {
    let tag: i64 = kani::any();
    let tile: i64 = kani::any();
    let position: i64 = kani::any();
    let instance: i64 = kani::any();
    let is_base: bool = kani::any();
    kani::assume((0..60).contains(&tile));
    kani::assume((0..60).contains(&position));
    kani::assume((0..60).contains(&instance));
    kani::assume(tag == tile || tag == position || tag < 0);

    crate::kassert!(
        m::suppressed(tag, tile, position, instance, is_base)
            == (tag == tile && instance == tile && !is_base),
        "suppression: tile-keyed, base exempt"
    );

    // A swapped settle is the one place the tile and the slot differ. There a
    // position-keyed read hides the wrong instance: the tag names the tile and
    // the instance sits on that tile, but neither equals the slot.
    let settled_tile: i64 = kani::any();
    let settled_slot: i64 = kani::any();
    kani::assume((0..60).contains(&settled_tile) && (0..60).contains(&settled_slot));
    kani::assume(settled_tile != settled_slot);
    kani::cover!(true, "mutant: position-keyed read on a swapped settle");
    crate::kassert!(
        m::suppressed(
            settled_tile,
            settled_tile,
            settled_slot,
            settled_tile,
            false
        ),
        "swapped settle: the tile's instance is hidden"
    );
    crate::kassert!(
        !m::suppressed_mutant_position(
            settled_tile,
            settled_tile,
            settled_slot,
            settled_tile,
            false
        ),
        "position mutant misses the tile's instance on a swap"
    );

    // The `tile:` base always runs, tag or not.
    kani::cover!(true, "base immune");
    crate::kassert!(
        !m::suppressed(tile, tile, position, tile, true),
        "the base is never hidden"
    );
    crate::kassert!(
        m::suppressed_mutant_hides_base(tile, tile, position, tile, true),
        "hides-base mutant drops the base"
    );
}

#[kani::proof]
pub fn fall_iff() {
    let before: u32 = kani::any();
    let after: u32 = kani::any();
    kani::assume(before <= 5 && after <= 5);

    crate::kassert!(
        m::fall_raised(before, after) == (after < before),
        "fall: strict"
    );
    kani::cover!(before == after, "mutant: unchanged count");
    crate::kassert!(
        m::fall_raised(before, after) == m::fall_raised_mutant_le(before, after) || before == after,
        "<= mutant differs only on an unchanged count"
    );
}

#[kani::proof]
pub fn swap_involution() {
    let a: i64 = kani::any();
    let b: i64 = kani::any();
    let x: i64 = kani::any();
    kani::assume((0..60).contains(&a) && (0..60).contains(&b) && (0..60).contains(&x));
    kani::assume(a != b);
    let swap = Some(m::Swap {
        slot: a,
        partner: b,
    });

    crate::kassert!(m::tile_at(swap, a) == b, "swap: slot displays partner");
    crate::kassert!(m::tile_at(swap, b) == a, "swap: partner displays slot");
    crate::kassert!(m::position_of_tile(swap, b) == a, "inverse: position of b");
    crate::kassert!(m::position_of_tile(swap, a) == b, "inverse: position of a");
    crate::kassert!(
        m::tile_at(swap, m::position_of_tile(swap, a)) == a,
        "round trip a"
    );
    crate::kassert!(
        m::tile_at(swap, m::position_of_tile(swap, b)) == b,
        "round trip b"
    );
    if x != a && x != b {
        crate::kassert!(m::tile_at(swap, x) == x, "swap: unrelated is identity");
        crate::kassert!(
            m::position_of_tile(swap, x) == x,
            "inverse: unrelated is identity"
        );
    } else {
        kani::cover!(x == a || x == b, "swap: pair reachable");
    }

    crate::kassert!(m::tile_at(None, a) == a, "no record: identity");
    crate::kassert!(
        m::position_of_tile(None, a) == a,
        "no record: inverse identity"
    );
}

#[kani::proof]
pub fn path_is_every_step() {
    let start: i64 = kani::any();
    let steps: usize = kani::any();
    kani::assume((0..60).contains(&start));
    kani::assume(steps <= m::MAX_STEPS);

    let (path, n) = m::travel(start, steps);
    crate::kassert!(n == steps, "path: one entry per step");
    for (i, tile) in path.iter().enumerate().take(steps) {
        crate::kassert!(
            *tile == start + i as i64 + 1,
            "path: entry is the traversed tile"
        );
    }

    let (mutant, mn) = m::travel_mutant_endpoint_only(start, steps);
    crate::kassert!(mn == steps, "mutant: same length");
    kani::cover!(steps >= 2, "mutant: a multi-step walk");
    if steps >= 2 {
        crate::kassert!(mutant[0] != path[0], "endpoint-only drops the first step");
        crate::kassert!(
            mutant[steps - 1] == path[steps - 1],
            "endpoint-only keeps the endpoint"
        );
    }
}

#[kani::proof]
pub fn start_never_travels() {
    let a: i64 = kani::any();
    let b: i64 = kani::any();
    let start: i64 = kani::any();
    kani::assume((0..60).contains(&a) && (0..60).contains(&b) && (0..60).contains(&start));
    kani::assume(a != b);
    let swap = Some(m::Swap {
        slot: a,
        partner: b,
    });

    crate::kassert!(
        m::start_never_travels(swap, start),
        "start stays at its position"
    );
    crate::kassert!(m::start_never_travels(None, start), "start with no record");

    // A card that just moved onto the start tile's slot is exactly the case
    // the direct read gets wrong.
    kani::cover!(start == a || start == b, "mutant: start swapped");
    if start == a || start == b {
        crate::kassert!(
            !m::start_never_travels_mutant(swap, start),
            "direct read loses the start onto the partner"
        );
    }
}
