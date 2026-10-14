//! Generic host-import dispatch for Kani verification (no game vocabulary).
//!
//! Under `cfg(kani)` the `extern "C-unwind"` `bandori_*` imports cannot be
//! linked to host shims (Kani treats them as foreign C calls; see
//! kani#2423). Instead `super::sys` becomes ordinary Rust that funnels every
//! import through [`call`]. The verifier crate (`rules-kani`) installs a
//! backend with [`install`] at proof start.
//!
//! Keep this module generic: opcodes and arg shapes only.

#![allow(missing_docs)]

//! Generic host-import dispatch for Kani verification (no game vocabulary).
//!
//! `Op` / `Args` live in the leaf crate [`kani_ops`] so the symbolic model
//! crate can depend on them without a cycle. This module keeps the
//! `BACKEND` fn-ptr install path for native conformance tests.

#![allow(missing_docs)]

pub use kani_ops::{Args, Op};

/// Host backend: one entry point, six i32 slots in, i64 out.
pub type Backend = fn(Op, Args) -> i64;

static mut BACKEND: Option<Backend> = None;

/// Install the symbolic host. Call once at the top of every proof.
pub fn install(f: Backend) {
    unsafe { BACKEND = Some(f); }
}

/// Dispatch one import to the installed backend.
pub fn call(op: Op, a: Args) -> i64 {
    let f = unsafe { BACKEND };
    let f = f.expect("rules-kani host backend not installed");
    f(op, a)
}

/// `call` truncated to i32 (most imports).
pub fn call_i32(op: Op, a: Args) -> i32 {
    call(op, a) as i32
}

/// `call` as unit.
pub fn call_unit(op: Op, a: Args) {
    let _ = call(op, a);
}
