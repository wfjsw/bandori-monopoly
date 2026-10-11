//! Generated single-source guard IRs (see `docs/rulebook/spec/COND-SINGLE-SOURCE.md`).

pub mod cond_irs;

pub use cond_irs::{
    eval_id, eval_id_val, guard_id_const, guard_id_opt, GuardIr, ALL, BY_ID, GUARD_COUNT,
};