//! `#[kani::proof]` harnesses. One module per representative clause.
//!
//! Every harness:
//! 1. builds a symbolic pre-state (`kani::any` + `assume`ed ranges);
//! 2. evaluates the **single-source** guard IR (no `assume` of the host
//!    condition) and only runs the body when the guard admits;
//! 3. asserts post == spec(pre) and the effect log is exactly the clause's
//!    effects (no less, no more);
//! 4. includes a `#[kani::should_panic]` MUTANT that must FAIL.

pub mod cond_ops;
pub mod money_shave;
pub mod teleport;
pub mod prompt;
pub mod wrong_guard;