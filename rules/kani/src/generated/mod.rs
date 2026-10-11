//! Generated single-source tables (see `docs/rulebook/spec/COND-SINGLE-SOURCE.md`).
//!
//! * `cond_irs` — lowered CEL guards + `guard_id!`
//! * `card_ids` — card/skill/event/tile/mark id → index + `card_id!`

pub mod card_ids;
pub mod cond_irs;

pub use card_ids::{card_id_const, card_id_opt, CARD_COUNT, IDS};
pub use cond_irs::{
    eval_id, eval_id_val, guard_id_const, guard_id_opt, GuardIr, ALL, BY_ID, GUARD_COUNT,
};