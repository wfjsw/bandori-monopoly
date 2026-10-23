//! Global rulebook clauses, one module per area.
//!
//! Units 1-8 own exactly one file each (`money.rs`, `cards.rs`, ...) and only
//! append to that file's `CLAUSES`. The dispatchers [`area`] and [`all`] pick
//! the slices up automatically, so `tests/global_soak.rs` never needs editing.
//!
//! ## Clause ID scheme
//!
//! * `G-<AREA>-NN` -- global clauses (this module). `<AREA>` is `MONEY`,
//!   `CARDS`, `STATUS`, `MOVE`, `SETTLE`, `FLOW`, `BANKRUPT`, `COUNTER`.
//! * `C-<GROUP>-<slug>-<clause>` -- card / skill contracts (unit-owned tests).
//! * `E-<slug>` -- event contracts.
//!
//! The doc entry (`docs/rulebook/spec/global/<area>.md`) and the code share
//! the same ID.

pub mod bankruptcy;
pub mod cards;
pub mod counteract;
pub mod flow;
pub mod money;
pub mod movement;
pub mod settle;
pub mod status;

use spec_kit::Clause;

/// The eight areas, in the order the soak tests name them.
pub const AREAS: [&str; 8] = [
    "money",
    "cards",
    "status",
    "movement",
    "settle",
    "flow",
    "bankruptcy",
    "counteract",
];

/// Clauses of one area, by name. Panics on an unknown name -- a soak test
/// with a typo must not silently check nothing.
pub fn area(name: &str) -> &'static [Clause] {
    match name {
        "money" => money::CLAUSES,
        "cards" => cards::CLAUSES,
        "status" => status::CLAUSES,
        "movement" => movement::CLAUSES,
        "settle" => settle::CLAUSES,
        "flow" => flow::CLAUSES,
        "bankruptcy" => bankruptcy::CLAUSES,
        "counteract" => counteract::CLAUSES,
        other => panic!("unknown global area {other:?}"),
    }
}

/// Every global clause, concatenated. Useful for a full-game soak.
pub fn all() -> Vec<&'static Clause> {
    AREAS.iter().flat_map(|a| area(a).iter()).collect()
}
