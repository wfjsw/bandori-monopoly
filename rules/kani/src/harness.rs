//! Harness kit: macros + helpers that enforce the standard clause-proof shape.
//!
//! Per clause an author writes **four** proofs (see `docs/rulebook/spec/KANI-FOUNDATION.md`):
//!
//! 1. **iff** — post == spec(pre) AND effect log == exactly the specified
//!    effects AND frame unchanged;
//! 2. **no-op** — when the trigger/condition is false, the log is empty and
//!    the frame is unchanged;
//! 3. **covers** — `kani::cover!` on each branch;
//! 4. **mutant** — `#[kani::should_panic]` variant whose spec is deliberately
//!    wrong, so the iff proof is non-vacuous.
//!
//! [`clause_proof!`] emits the harness function skeleton; the author fills
//! the body. [`unit_mod!`] pre-creates the empty per-unit modules so 20
//! parallel authors never edit shared `mod.rs` files.

/// Emit a named proof harness with the standard attributes.
///
/// ```ignore
/// clause_proof! {
///     /// C-PP-aya-haruka-01 iff
///     #[kani::unwind(32)]
///     pub fn aya_haruka_01_iff() {
///         // body
///     }
/// }
/// ```
#[macro_export]
macro_rules! clause_proof {
    (
        $(#[$attr:meta])*
        $vis:vis fn $name:ident() $body:block
    ) => {
        $(#[$attr])*
        #[kani::proof]
        $vis fn $name() $body
    };
}

/// Emit a `#[kani::should_panic]` mutant harness.
#[macro_export]
macro_rules! clause_mutant {
    (
        $(#[$attr:meta])*
        $vis:vis fn $name:ident() $body:block
    ) => {
        $(#[$attr])*
        #[kani::proof]
        #[kani::should_panic]
        $vis fn $name() $body
    };
}

/// Declare a unit module (spec + proofs) as an empty stub.
///
/// Used once at crate root so authors only touch `spec/<unit>.rs` and
/// `proofs/<unit>.rs`.
#[macro_export]
macro_rules! unit_mod {
    ($($u:ident),* $(,)?) => {
        $(
            pub mod $u;
        )*
    };
}

/// Frame assertion: every listed snapshot field is unchanged.
///
/// ```ignore
/// assert_frame!(pre, post, money, pos, fire);
/// ```
#[macro_export]
macro_rules! assert_frame {
    ($pre:expr, $post:expr, $($field:ident),+ $(,)?) => {
        $(
            assert!($pre.$field == $post.$field, concat!("frame: ", stringify!($field)));
        )+
    };
}

/// Cover both sides of a boolean, with labels.
#[macro_export]
macro_rules! cover_both {
    ($cond:expr, $yes:expr, $no:expr) => {{
        kani::cover!($cond, $yes);
        kani::cover!(!$cond, $no);
    }};
}

/// Build a symbolic player index in `0..n`.
#[cfg(kani)]
pub fn any_player(n: i32) -> i32 {
    let p: i32 = kani::any();
    kani::assume(p >= 0 && p < n);
    p
}

/// Build a symbolic i32 in `lo..=hi`.
#[cfg(kani)]
pub fn any_in(lo: i32, hi: i32) -> i32 {
    let x: i32 = kani::any();
    kani::assume(x >= lo && x <= hi);
    x
}

/// Native stubs so the module typechecks outside `cargo kani`.
#[cfg(not(kani))]
pub fn any_player(_n: i32) -> i32 {
    0
}
#[cfg(not(kani))]
pub fn any_in(lo: i32, _hi: i32) -> i32 {
    lo
}