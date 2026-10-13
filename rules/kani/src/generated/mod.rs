//! Generated single-source tables (see `docs/rulebook/spec/COND-SINGLE-SOURCE.md`).
//!
//! * `cond_irs` — lowered CEL guards + `guard_id!` (build.rs → `$OUT_DIR`)
//! * `card_ids` — card/skill/event/tile/mark id → index + `card_id!`
//! * `body_dispatch_fn.rs` — per-card Kani body trampolines (build.rs →
//!   `$OUT_DIR`, `include!`d by `src/invoke.rs`, never committed)

pub mod card_ids;
/// Lowered CEL guards + `guard_id!`. Generated at build time by `build.rs`
/// (`rules_cond::gen_ir::generate`) into `$OUT_DIR`; never committed.
#[allow(dead_code)]
pub mod cond_irs {
    include!(concat!(env!("OUT_DIR"), "/cond_irs.rs"));
}

pub use card_ids::{card_id_const, card_id_opt, CARD_COUNT, IDS};
pub use cond_irs::{
    eval_id, eval_id_val, guard_id_const, guard_id_opt, GuardIr, ALL, BY_ID, GUARD_COUNT,
};

/// Compile-time guard string -> const id. Unknown strings are a **compile
/// error** (const-eval panic in `cond_irs::const_id`).
///
/// ```ignore
/// let id = guard_id!("actor == owner && value > 0");
/// ```
#[macro_export]
macro_rules! guard_id {
    ($s:expr) => {{
        const ID: usize = $crate::generated::cond_irs::const_id($s);
        ID
    }};
}
