//! G07 designation-pipeline and nearest-selection models
//! (`docs/rulebook/spec/abi/G07-targeting-and-distance.md` §4, §7, §10.4).
//!
//! Card bodies stay PROTOCOL-LEVEL. What is modelled here is the decision each
//! host step reads:
//!
//! * [`pipeline`] — the one stage sequence of §7.1: out check, per-pair
//!   cancel, `OnBoard` gate, `redirect` (single only), `Untargetable`, the
//!   between-turns count, the `target_window`, the post-window cancel check
//!   and `ImmuneAll` (other sources only). Production is
//!   `WasmRules::target_player` / `target_tile` (`wasm_rules.rs`); the stage
//!   constants are its outcome-only bits.
//! * [`counts`] — §7.3's increment predicate for a player name.
//! * [`tile_counts`] — the same for a tile name (owner live and neither the
//!   actor nor the current turn).
//! * [`nearest`] — §4's minimal-metric selection over present, non-`by` seats.
//! * [`nearest_with_prop`] — the raw set plus every present seat carrying the
//!   value, distance-then-seat appended.
//! * [`prop_match`] — the presence read `props.get(key) == Some(&by)`, so the
//!   value `0` is a match and an absent key is not.
//! * [`exemption`] — the card-id-gated `bandori_std::nearest::exemption` scan
//!   over a seat's instances.

/// The seat left the game / is not on the board.
pub const OUT: u16 = 1;
/// A designation already cancelled earlier in the same play.
pub const EARLY_CANCEL: u16 = 1 << 1;
/// The `OnBoard` gate.
pub const GATE: u16 = 1 << 2;
/// The `redirect` hook (single-target only).
pub const REDIRECT: u16 = 1 << 3;
/// The `Untargetable` fold.
pub const UNTARGETABLE: u16 = 1 << 4;
/// The between-turns count increment.
pub const COUNT: u16 = 1 << 5;
/// The `target_window` (`targeted` → `effect` → `target`).
pub const WINDOW: u16 = 1 << 6;
/// `ImmuneAll` (other sources only).
pub const IMMUNE: u16 = 1 << 7;
/// The designation was cancelled inside the window.
pub const CANCELLED: u16 = 1 << 8;
/// The designation landed.
pub const LANDED: u16 = 1 << 9;

/// What a designating call reached.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Stages {
    /// The stages visited, as the bits above.
    pub stages: u16,
    /// The seat that was finally named (the redirect destination once moved).
    pub named: i32,
    /// `LANDED` (the call answers the seat) or not.
    pub landed: bool,
}

/// §7.1's sequence. `single` is the single-target flag (`redirect` only), and
/// `redirect_to` is the redirect hook's answer (`None` when the fold did not
/// apply). A self designation (`p == by`) still runs `UNTARGETABLE` and
/// `WINDOW`, but neither `COUNT` nor `IMMUNE`.
pub fn pipeline(
    by: i32,
    p: i32,
    single: bool,
    out: bool,
    early_cancel: bool,
    gate: bool,
    redirect_to: Option<i32>,
    untargetable: bool,
    turn: i32,
    window_cancel: bool,
    immune: bool,
) -> Stages {
    let mut s = Stages {
        stages: 0,
        named: p,
        landed: false,
    };
    if out {
        s.stages |= OUT;
        return s;
    }
    if early_cancel {
        s.stages |= EARLY_CANCEL;
        return s;
    }
    if !gate {
        s.stages |= GATE;
        return s;
    }
    let mut p = p;
    if single {
        if let Some(to) = redirect_to {
            s.stages |= REDIRECT;
            if to != p && to != by && to >= 0 {
                p = to;
                s.named = to;
            }
        }
    }
    if untargetable {
        s.stages |= UNTARGETABLE;
        return s;
    }
    if counts(p, by, turn) {
        s.stages |= COUNT;
    }
    s.stages |= WINDOW;
    if window_cancel {
        s.stages |= CANCELLED;
        return s;
    }
    if p != by && immune {
        s.stages |= IMMUNE;
        return s;
    }
    s.stages |= LANDED;
    s.landed = true;
    s
}

/// §7.3, player names: neither the actor nor the current turn player.
pub fn counts(named: i32, by: i32, turn: i32) -> bool {
    named != by && named != turn
}

/// §7.3, tile names: a live owner that is neither the actor nor the current
/// turn player.
pub fn tile_counts(owner: i32, by: i32, turn: i32) -> bool {
    owner >= 0 && owner != by && owner != turn
}

/// Seats the models run over (the §4 four-seat model).
pub const SEATS: usize = 4;

/// §4's selection: every present non-`by` seat at the minimal metric distance.
/// `measure[i]` is `i`'s distance from `by` (`Forward` skips a zero).
pub fn nearest(by: usize, present: &[bool], measure: &[i32], shortest: bool) -> u32 {
    let mut best = i32::MAX;
    let mut mask = 0u32;
    for s in 0..SEATS {
        if s == by || !present[s] {
            continue;
        }
        let d = measure[s];
        if !shortest && d == 0 {
            continue;
        }
        if d < best {
            best = d;
            mask = 1 << s;
        } else if d == best {
            mask |= 1 << s;
        }
    }
    mask
}

/// §4's `nearest_with_prop`: the raw set, then every present seat outside it
/// that carries the value in `values[i]`, distance-then-seat ordered. The raw
/// set stays first.
pub fn nearest_with_prop(
    by: usize,
    present: &[bool],
    measure: &[i32],
    values: &[Option<i32>],
    shortest: bool,
) -> u32 {
    // The answer is a set; the distance-then-seat order of the appended seats
    // is pinned by the integration matrix (`g07_targeting.rs` §10.2), the mask
    // here proves the membership.
    let raw = nearest(by, present, measure, shortest);
    let mut out = raw;
    for s in 0..SEATS {
        if s == by || !present[s] || raw & (1 << s) != 0 {
            continue;
        }
        if prop_match(core::slice::from_ref(&values[s]), by as i32) {
            out |= 1 << s;
        }
    }
    out
}

/// The presence read on one instance list: true iff some instance carries
/// `by`.
pub fn prop_match(values: &[Option<i32>], by: i32) -> bool {
    values.iter().any(|v| *v == Some(by))
}

/// A seat's instances: `card` says it is a bond, `grantor` is the value.
#[derive(Clone, Copy)]
pub struct Instance {
    pub bond: bool,
    pub grantor: Option<i32>,
}

/// `bandori_std::nearest::exemption`: a live instance of the bond card carries
/// the grantor, keyed on the card id so an unrelated instance without the key
/// (which reads back `0`) is not a seat-0 grant.
pub fn exemption(grantor: i32, instances: &[Instance]) -> bool {
    instances
        .iter()
        .any(|i| i.bond && prop_match(&[i.grantor], grantor))
}
