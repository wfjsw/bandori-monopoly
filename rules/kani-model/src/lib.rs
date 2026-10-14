//! Kani symbolic host model: world state + `Op` dispatch.
//!
//! Ordinary Rust crate (not an `extern` import) so `card-sdk`'s
//! `kani_host::call` is a **direct** call into [`dispatch`] under
//! `cfg(all(kani, feature = "kani-model"))` — no `BACKEND` fn-ptr
//! (that expanded to an 874-target IF-chain at every `ctx::*` site).
//!
//! Host-only. Never enabled for wasm/native production builds.

pub mod board;
pub mod host;
pub mod intern;
pub mod native;
pub mod world;

pub use host::dispatch;
pub use kani_ops::{Args, Op};
