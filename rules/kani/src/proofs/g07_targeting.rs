//! G07 pipeline and nearest-selection obligations
//! (`docs/rulebook/spec/abi/G07-targeting-and-distance.md` §10.4).
//!
//! Every harness is over the pure model in `spec::g07_targeting`, which mirrors
//! the production step named on each model item; a MUTANT harness per
//! obligation must fail. Card bodies stay PROTOCOL-LEVEL and are proved
//! elsewhere.

use kani::cover;

use crate::kassert;
use crate::spec::g07_targeting::{
    counts, exemption, nearest, nearest_with_prop, pipeline, prop_match, tile_counts, Instance,
    CANCELLED, COUNT, EARLY_CANCEL, GATE, IMMUNE, LANDED, OUT, REDIRECT, SEATS, UNTARGETABLE,
    WINDOW,
};

/// Symbolic pipeline inputs. `p`/`by` stay in the four-seat model and a
/// redirect destination is `None` or a seat.
fn any_inputs() -> (i32, i32, bool, Option<i32>, i32, bool) {
    let by: i32 = kani::any();
    let p: i32 = kani::any();
    kani::assume((0..SEATS as i32).contains(&by));
    kani::assume((0..SEATS as i32).contains(&p));
    let single: bool = kani::any();
    let r: u8 = kani::any();
    kani::assume(r < 5);
    let to = if r == 4 { None } else { Some(r as i32) };
    let turn: i32 = kani::any();
    kani::assume((0..SEATS as i32).contains(&turn));
    let out: bool = kani::any();
    (by, p, single, to, turn, out)
}

/// The stage set §7.1 requires, written independently of the implementation.
fn expected(
    by: i32,
    p: i32,
    single: bool,
    to: Option<i32>,
    turn: i32,
    out: bool,
    early_cancel: bool,
    gate: bool,
    untargetable: bool,
    window_cancel: bool,
    immune: bool,
) -> (u16, i32, bool) {
    let mut want = 0u16;
    let mut named = p;
    if out {
        want |= crate::spec::g07_targeting::OUT;
        return (want, named, false);
    }
    if early_cancel {
        want |= crate::spec::g07_targeting::EARLY_CANCEL;
        return (want, named, false);
    }
    if !gate {
        want |= GATE;
        return (want, named, false);
    }
    if single {
        if let Some(t) = to {
            want |= REDIRECT;
            if t != p && t != by && t >= 0 {
                named = t;
            }
        }
    }
    if untargetable {
        want |= UNTARGETABLE;
        return (want, named, false);
    }
    if counts(named, by, turn) {
        want |= COUNT;
    }
    want |= WINDOW;
    if window_cancel {
        want |= CANCELLED;
        return (want, named, false);
    }
    if named != by && immune {
        want |= IMMUNE;
        return (want, named, false);
    }
    want |= LANDED;
    (want, named, true)
}

/// The stage set is exactly §7.1's, the name is the post-redirect seat, and
/// only a landed call answers.
#[kani::proof]
pub fn pipeline_stages_iff() {
    let (by, p, single, to, turn, out) = any_inputs();
    let early_cancel: bool = kani::any();
    let gate: bool = kani::any();
    let untargetable: bool = kani::any();
    let window_cancel: bool = kani::any();
    let immune: bool = kani::any();

    let got = pipeline(
        by,
        p,
        single,
        out,
        early_cancel,
        gate,
        to,
        untargetable,
        turn,
        window_cancel,
        immune,
    );
    let (want, named, landed) = expected(
        by,
        p,
        single,
        to,
        turn,
        out,
        early_cancel,
        gate,
        untargetable,
        window_cancel,
        immune,
    );
    kassert!(got.stages == want, "the stage set is exactly §7.1's");
    kassert!(got.named == named, "the name is the post-redirect seat");
    kassert!(got.landed == landed, "only a landed call answers");

    // A self designation that gets past `Untargetable` runs the window; one
    // an earlier stage refuses never reaches it. Neither the count nor the
    // shield applies to a self name.
    kassert!(
        named != by
            || (got.stages & (OUT | EARLY_CANCEL | GATE | UNTARGETABLE)) != 0
            || got.stages & WINDOW != 0,
        "a self designation past Untargetable runs the window"
    );
    kassert!(
        named != by || got.stages & (COUNT | IMMUNE) == 0,
        "self excludes the count and the shield"
    );

    cover!(got.landed, "landed");
    cover!(out, "out");
    cover!(early_cancel, "early cancel");
    cover!(!gate, "gate");
    cover!(got.stages & REDIRECT != 0, "redirect visited");
    cover!(untargetable, "untargetable");
    cover!(window_cancel, "window cancel");
    cover!(immune && named != by, "immune");
    cover!(got.landed && named == by, "self landed");
}

// ---- MUTANT: the removed `p == by` early return -------------------------

fn pipeline_early_return(
    by: i32,
    p: i32,
    single: bool,
    out: bool,
    gate: bool,
    to: Option<i32>,
    untargetable: bool,
    turn: i32,
    window_cancel: bool,
    immune: bool,
) -> u16 {
    let mut s = 0u16;
    if out || !gate {
        return s;
    }
    let mut named = p;
    if single {
        if let Some(t) = to {
            s |= REDIRECT;
            if t != p && t != by && t >= 0 {
                named = t;
            }
        }
    }
    if named == by {
        return s;
    }
    if untargetable {
        return s | UNTARGETABLE;
    }
    if counts(named, by, turn) {
        s |= COUNT;
    }
    s |= WINDOW;
    if window_cancel {
        return s | CANCELLED;
    }
    if immune {
        return s | IMMUNE;
    }
    s | LANDED
}

/// MUTANT: the old early return at `p == by` skips `Untargetable`, the window
/// and the retraction check.
#[kani::proof]
#[kani::should_panic]
pub fn pipeline_early_return_on_self_mutant() {
    let (by, p, single, to, turn, _out) = any_inputs();
    // No redirect move, so the name stays `p`.
    let mut named = p;
    if single {
        if let Some(t) = to {
            if t != p && t != by && t >= 0 {
                named = t;
            }
        }
    }
    let (want, _, _) = expected(
        by, p, single, to, turn, false, false, true, false, false, false,
    );
    let got = pipeline_early_return(by, p, single, false, true, to, false, turn, false, false);
    kassert!(named != by || got != 0, "the mutant must be exercised");
    kassert!(got == want, "the early return disagrees");
}

// ---- the count predicates ----------------------------------------------

#[kani::proof]
pub fn targeted_increment_iff() {
    let named: i32 = kani::any();
    let by: i32 = kani::any();
    let turn: i32 = kani::any();
    kassert!(
        counts(named, by, turn) == (named != by && named != turn),
        "exactly neither the actor nor the turn player"
    );
    cover!(counts(named, by, turn), "counts");
    cover!(named == by, "self");
    cover!(named == turn && named != by, "own turn");
}

/// MUTANT: the turn guard dropped.
#[kani::proof]
#[kani::should_panic]
pub fn targeted_increment_drops_turn_mutant() {
    let named: i32 = kani::any();
    let by: i32 = kani::any();
    let turn: i32 = kani::any();
    kassert!(
        counts(named, by, turn) == (named != by),
        "turn guard dropped"
    );
}

/// MUTANT: self counted.
#[kani::proof]
#[kani::should_panic]
pub fn targeted_increment_counts_self_mutant() {
    let named: i32 = kani::any();
    let by: i32 = kani::any();
    let turn: i32 = kani::any();
    kassert!(counts(named, by, turn) == (named != turn), "self counted");
}

#[kani::proof]
pub fn tile_increment_iff() {
    let owner: i32 = kani::any();
    let by: i32 = kani::any();
    let turn: i32 = kani::any();
    kassert!(
        tile_counts(owner, by, turn) == (owner >= 0 && owner != by && owner != turn),
        "exactly a live other owner off its own turn"
    );
    cover!(tile_counts(owner, by, turn), "counts");
    cover!(owner == by && owner >= 0, "own tile");
    cover!(owner < 0, "unowned");
}

/// MUTANT: the old "a tile never counts".
#[kani::proof]
#[kani::should_panic]
pub fn tile_increment_never_mutant() {
    let owner: i32 = kani::any();
    let by: i32 = kani::any();
    let turn: i32 = kani::any();
    kassert!(tile_counts(owner, by, turn) == false, "tiles never count");
}

/// MUTANT: an own tile counted.
#[kani::proof]
#[kani::should_panic]
pub fn tile_increment_counts_own_tile_mutant() {
    let owner: i32 = kani::any();
    let by: i32 = kani::any();
    let turn: i32 = kani::any();
    kassert!(
        tile_counts(owner, by, turn) == (owner >= 0 && owner != turn),
        "own tile counted"
    );
}

// ---- the nearest selection ---------------------------------------------

fn any_board() -> ([bool; SEATS], [i32; SEATS]) {
    let present: [bool; SEATS] = kani::any();
    let mut measure: [i32; SEATS] = kani::any();
    for m in measure.iter_mut() {
        kani::assume((0..=8).contains(m));
    }
    (present, measure)
}

/// A symbolic instance value: absent, or one of seats 0..4.
fn any_values() -> [Option<i32>; SEATS] {
    let mut v = [None; SEATS];
    for slot in v.iter_mut() {
        let tag: u8 = kani::any();
        kani::assume(tag < 5);
        *slot = if tag == 4 { None } else { Some(tag as i32) };
    }
    v
}

/// Four symbolic instances (bond or not, absent or a small value).
fn any_instances() -> [Instance; SEATS] {
    let mut v = [Instance {
        bond: false,
        grantor: None,
    }; SEATS];
    let values = any_values();
    for (k, slot) in v.iter_mut().enumerate() {
        *slot = Instance {
            bond: kani::any(),
            grantor: values[k],
        };
    }
    v
}

#[kani::proof]
pub fn nearest_iff() {
    let by: usize = kani::any();
    kani::assume(by < SEATS);
    let (present, measure) = any_board();
    let shortest: bool = kani::any();
    let got = nearest(by, &present, &measure, shortest);

    // Independent characterisation: the valid set and its minimum.
    let mut best = i32::MAX;
    for s in 0..SEATS {
        let valid = s != by && present[s] && (shortest || measure[s] != 0);
        if valid && measure[s] < best {
            best = measure[s];
        }
    }
    let mut want = 0u32;
    for s in 0..SEATS {
        let valid = s != by && present[s] && (shortest || measure[s] != 0);
        if valid && measure[s] == best {
            want |= 1 << s;
        }
    }
    kassert!(got == want, "every valid seat at the minimum, seat order");
    cover!(got.count_ones() > 1, "a tie");
    cover!(got == 0, "empty");
    cover!(got.count_ones() == 1, "singleton");
}

/// MUTANT: only the first seat of the tied set.
#[kani::proof]
#[kani::should_panic]
pub fn nearest_first_only_mutant() {
    let present = [false, true, true, false];
    let measure = [0, 3, 3, 0];
    let got = nearest(0, &present, &measure, true);
    let first = 1u32 << got.trailing_zeros();
    kassert!(got == first, "first only");
}

/// MUTANT: every present seat, not just the minimum.
#[kani::proof]
#[kani::should_panic]
pub fn nearest_all_candidates_mutant() {
    let present = [false, true, true, false];
    let measure = [0, 3, 7, 0];
    let got = nearest(0, &present, &measure, true);
    kassert!(got == 0b1110, "all candidates");
}

#[kani::proof]
pub fn nearest_with_prop_iff() {
    let by: usize = kani::any();
    kani::assume(by < SEATS);
    let (present, measure) = any_board();
    let values = any_values();
    let shortest: bool = kani::any();
    let raw = nearest(by, &present, &measure, shortest);
    let got = nearest_with_prop(by, &present, &measure, &values, shortest);

    let mut want = raw;
    for s in 0..SEATS {
        if s != by
            && present[s]
            && raw & (1 << s) == 0
            && prop_match(core::slice::from_ref(&values[s]), by as i32)
        {
            want |= 1 << s;
        }
    }
    kassert!(got == want, "exactly the raw set plus the matches");
    kassert!(got & raw == raw, "the raw set is kept");
    cover!(got != raw, "a match appended");
}

/// MUTANT: the matching extras are dropped.
#[kani::proof]
#[kani::should_panic]
pub fn nearest_with_prop_drops_matches_mutant() {
    let present = [false, true, true, false];
    let measure = [0, 3, 0, 0];
    let values = [None, None, Some(0), None];
    let got = nearest_with_prop(0, &present, &measure, &values, true);
    // A mutant returning only the raw set (P1) loses the co-located match P2.
    kassert!(got == 0b0010, "matches dropped");
}

/// MUTANT: a farther non-matching seat admitted.
#[kani::proof]
#[kani::should_panic]
pub fn nearest_with_prop_admits_farther_mutant() {
    // P1 is nearest and does not match (the raw set); P2 is farther and does
    // match (a legitimate extra); P3 is farther still and does not match, so
    // only a mutant that appends every farther seat adds it.
    let present = [false, true, true, true];
    let measure = [0, 3, 5, 7];
    let values = [None, None, Some(0), None];
    let got = nearest_with_prop(0, &present, &measure, &values, true);
    // The real set is 0b0110; the mutant's is every farther seat, 0b1110.
    kassert!(got == 0b1110, "only the farther match is admitted");
}

// ---- the property read and its gate ------------------------------------

#[kani::proof]
pub fn prop_match_iff() {
    let values = any_values();
    let by: i32 = kani::any();
    kassert!(
        prop_match(&values, by) == values.iter().any(|v| *v == Some(by)),
        "presence of the value"
    );
    cover!(prop_match(&values, by), "match");
    cover!(values.contains(&Some(0)) && by == 0, "seat zero is a value");
}

/// MUTANT: `unwrap_or(0)` -- an absent key reads as the seat-0 value.
#[kani::proof]
#[kani::should_panic]
pub fn prop_match_unwrap_or_zero_mutant() {
    // No instance carries the key; P1 carries another seat's value.
    let values = [None, Some(1), None, None];
    let by = 0;
    let got = values.iter().any(|v| v.unwrap_or(0) == by);
    // The real read is false; the mutant's absent P0/P2/P3 all read seat 0.
    kassert!(
        got == prop_match(&values, by),
        "absent keys admitted as seat 0"
    );
}

#[kani::proof]
pub fn exemption_iff() {
    let grantor: i32 = kani::any();
    let instances = any_instances();
    let got = exemption(grantor, &instances);
    kassert!(
        got == instances
            .iter()
            .any(|i| i.bond && i.grantor == Some(grantor)),
        "a bond instance carries the grantor"
    );
    cover!(got, "exempt");
}

/// MUTANT: the card-id gate dropped -- every instance's absent key reads `0`
/// and exempts seat 0.
#[kani::proof]
#[kani::should_panic]
pub fn exemption_without_the_card_gate_mutant() {
    // An unrelated placed card that nevertheless carries the grantor: only the
    // card-id gate (`bond`) excludes it.
    let instances = [
        Instance {
            bond: false,
            grantor: Some(0),
        },
        Instance {
            bond: false,
            grantor: None,
        },
        Instance {
            bond: false,
            grantor: None,
        },
        Instance {
            bond: false,
            grantor: None,
        },
    ];
    let got = instances.iter().any(|i| prop_match(&[i.grantor], 0));
    kassert!(got == exemption(0, &instances), "no card-id gate");
}

/// The waiver is on the instance: with it removed the match is gone. (The
/// instance unit is the field entry; a removed entry no longer exists.)
#[kani::proof]
pub fn exemption_dies_with_instance() {
    let grantor: i32 = kani::any();
    let mut instances = any_instances();
    // Force one live bond instance at slot 0.
    instances[0] = Instance {
        bond: true,
        grantor: Some(grantor),
    };
    kassert!(exemption(grantor, &instances), "the bond exempts");
    // Removing it (slot replaced by an empty, bond-less one) drops the match
    // unless another bond instance also carries the grantor.
    let before = exemption(grantor, &instances);
    instances[0] = Instance {
        bond: false,
        grantor: None,
    };
    let after = exemption(grantor, &instances);
    kassert!(before || after, "the forced instance matched");
    kassert!(
        after
            == instances[1..]
                .iter()
                .any(|i| i.bond && i.grantor == Some(grantor)),
        "only the remaining instances matter"
    );
    cover!(!after, "no other instance");
}
