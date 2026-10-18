//! Semantics-preserving library stubs for CBMC cost (class-E deep dive).
//!
//! These stub **std/alloc internals only** — never a card body, skill body,
//! event body, host arm, or spec. Each replacement is justified below as
//! equivalent for every property a clause proof asserts.
//!
//! Enable with `cargo kani -Z stubbing` and
//! `#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]` on the harness
//! (or `KANI_STUBS=1` via `tools/kani/run.sh`, which injects the set).
//!
//! ## What the stubs replace and why it is sound
//!
//! | original | replacement | why equivalent |
//! |---|---|---|
//! | `core::slice::memchr::memchr` | 128-step `.get` scan (zero loop unwind) | pure search: same `Option<usize>` for every input; only the *cost* drops. Used by `str::split('\\n')` inside `fmt` `PadAdapter` and by panic-message assembly. |
//! | `core::slice::index::slice_index_fail` | `kani::assert(false)` + diverge | the original formats "index out of bounds: …" then panics. A reachable OOB is a loud fail either way; we only drop the fmt/memchr of the message. `assert!(false)` is fail-closed. |
//! | `std::alloc::handle_alloc_error` | `kani::assert(false)` + diverge | the original formats the layout then aborts. Reachable OOM is a loud fail either way. |
//! | `alloc::raw_vec::capacity_overflow` | `kani::assert(false)` + diverge | same: the original panics with a static message. |
//! | `alloc::raw_vec::handle_error` | `kani::assert(false)` + diverge | central RawVec error reporter (`capacity_overflow` / `handle_alloc_error`). Reachable grow failure is a loud fail either way. |
//!
//! Not stubbed (would change observed behaviour):
//! * `core::fmt::write` / `Display` — `format!("key:{p}")` in production bodies
//!   must produce the same key bytes for `state::get`/`set` round-trips;
//! * any `rules/**` body or `rules/kani` host arm;
//! * `kani::assert` / proof `assert!` / `kassert!` themselves.
//!
//! `#[kani::should_panic]` mutants keep working: the set does **not** stub
//! `panic_fmt`, so a `panic!` still reports as the expected panic. Loud
//! library failures use `kani::assert(false)` (failed property), which is
//! fail-closed for iff harnesses and unused by mutants.

#![cfg(kani)]

/// Zero-loop `memchr`: 128 `.get` steps, then a loud cap.
///
/// Same result as `core::slice::memchr::memchr` for haystacks up to 128 bytes
/// (every `format!` key / panic snippet in this corpus). Longer haystacks are
/// a loud FAIL — never a silent wrong index.
pub fn memchr_stub(needle: u8, haystack: &[u8]) -> Option<usize> {
    // 8×16 unrolled steps via macro; no `while` for CBMC to unwind.
    macro_rules! step {
        ($i:expr) => {
            if haystack.get($i) == Some(&needle) {
                return Some($i);
            }
        };
    }
    macro_rules! steps16 {
        ($b:expr) => {
            step!($b + 0);
            step!($b + 1);
            step!($b + 2);
            step!($b + 3);
            step!($b + 4);
            step!($b + 5);
            step!($b + 6);
            step!($b + 7);
            step!($b + 8);
            step!($b + 9);
            step!($b + 10);
            step!($b + 11);
            step!($b + 12);
            step!($b + 13);
            step!($b + 14);
            step!($b + 15);
        };
    }
    steps16!(0);
    steps16!(16);
    steps16!(32);
    steps16!(48);
    steps16!(64);
    steps16!(80);
    steps16!(96);
    steps16!(112);
    if haystack.len() > 128 {
        kani::assert(false, "kani_stubs::memchr: haystack longer than 128");
    }
    None
}

/// OOB index: real path formats + panics; we fail loud and stop.
pub fn slice_index_fail_stub(_start: usize, _end: usize, _len: usize) -> ! {
    kani::assert(false, "kani_stubs: slice index out of bounds");
    loop {
        kani::assume(false);
    }
}

/// Alloc OOM: real path formats the layout + aborts; we fail loud and stop.
pub fn handle_alloc_error_stub(_layout: core::alloc::Layout) -> ! {
    kani::assert(false, "kani_stubs: allocation failed");
    loop {
        kani::assume(false);
    }
}

/// RawVec capacity overflow: real path panics with a static message.
pub fn capacity_overflow_stub() -> ! {
    kani::assert(false, "kani_stubs: capacity overflow");
    loop {
        kani::assume(false);
    }
}

/// RawVec central error reporter.
pub fn handle_error_stub(_e: *const u8) -> ! {
    kani::assert(false, "kani_stubs: raw_vec handle_error");
    loop {
        kani::assume(false);
    }
}

kani::stub_set!(pub LIB_COST,
    stub(core::slice::memchr::memchr, crate::kani_stubs::memchr_stub),
    // Private helpers: `memchr` is `#[inline]` and CBMC profiles show
    // `memchr_naive` / `memchr_aligned::runtime` as their own unwind loops.
    stub(core::slice::memchr::memchr_naive, crate::kani_stubs::memchr_stub),
    stub(core::slice::memchr::memchr_aligned, crate::kani_stubs::memchr_stub),
    stub(core::slice::index::slice_index_fail, crate::kani_stubs::slice_index_fail_stub),
    stub(std::alloc::handle_alloc_error, crate::kani_stubs::handle_alloc_error_stub),
    stub(alloc::raw_vec::capacity_overflow, crate::kani_stubs::capacity_overflow_stub),
);