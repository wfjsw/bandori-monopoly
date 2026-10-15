//! Harness kit: macros + helpers that enforce the standard clause-proof shape.
//!
//! Per clause an author writes **four** proofs (see `docs/rulebook/spec/KANI-FOUNDATION.md`):
//!
//! 1. **iff** — post == spec(pre) AND effect log == exactly the specified
//!    effects AND frame unchanged;
//! 2. **no-op** — when the trigger/condition is false, the log is empty and
//!    the frame is unchanged;
//! 3. **covers** — `kani::cover!` on each branch;
//! 4. **mutant** — a harness whose spec is deliberately wrong, so the iff
//!    proof is non-vacuous (prefer a reachable-disagreement companion; keep
//!    `#[kani::should_panic]` only when it actually works).
//!
//! ## Per-harness host-op subset (TRIAGE-LOG `op-subset`)
//!
//! A card body only reaches a handful of `ruleset_*` ops. Installing the
//! full ~300-op [`crate::host::dispatch`] match makes CBMC build that match
//! at every `ctx::*` site — the dominant SSA cost on band proofs. Declare
//! the subset instead:
//!
//! ```ignore
//! crate::host_subset!(same_dream_dispatch,
//!     arm_add_band_crystals, arm_count_held, arm_add_held,
//!     arm_log, arm_gain, arm_player_count, arm_hand_count,
//! );
//! // in the harness:
//! crate::host::install_backend(same_dream_dispatch);
//! ```
//!
//! Every op outside the list hits [`crate::host::outside_subset`] — a
//! **loud FAIL** (`kani::assert(false)`), never `assume(false)`. A body that
//! reaches an undeclared op therefore fails the proof; it cannot silently
//! pass. Extend the list when a prompt / resonance path is live.
//!
//! ## Sparse world ([`sparse_play_world`])
//!
//! Only the seats the clause reads become symbolic; the rest stay at
//! `World::new()` defaults (concrete — free for SAT). `player_count()`
//! semantics stay TUNING §7 (the `cfg(kani)` facade returns the constant 4;
//! `player_out(p)` is true for `p >= n_players`).
//!
//! ## Standard proof shape
//!
//! ```ignore
//! let id = guard_id!("actor == owner && value > 0");
//! let w = symbolic_world();
//! let ctx = ir_fill::fill_from_world(&w, owner, card, placed, tile, &ov);
//! let admitted = guard!(id, &ctx);
//! if admitted {
//!     // run the unmodified card body
//!     // assert post == spec(pre) ∧ effect log exact ∧ frame unchanged
//! } else {
//!     // assert the world is untouched (no-op)
//! }
//! ```
//!
//! The guard id is a **const** (`guard_id!`), so CBMC never walks a string
//! table. See [`crate::shim`] for `guard!` / `guard_id!` / `guard_assumed!`.

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

/// Emit a mutant harness. Prefer a reachable-disagreement companion
/// (`*_mutant_is_detected`) over `#[kani::should_panic]` when the tooling
/// quirk bites (see COND-SINGLE-SOURCE.md §10).
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

/// `assert!` without Rust's fmt machinery (a static message still pulls
/// `panic_fmt` → `PadAdapter` → `memchr` — 1193 unrolls on moca_02_mutant).
/// Under Kani this is `kani::assert` (a CBMC property, no fmt).
#[macro_export]
macro_rules! kassert {
    ($c:expr) => {
        $crate::kassert!($c, "assertion failed")
    };
    ($c:expr, $msg:expr) => {{
        #[cfg(kani)]
        {
            kani::assert($c, $msg);
        }
        #[cfg(not(kani))]
        {
            assert!($c, $msg);
        }
    }};
}

/// Declare a **narrow** per-harness host dispatcher covering only the
/// [`crate::host::arm_*`] handlers the card body can reach.
///
/// Every other `Op` goes to [`crate::host::outside_subset`] — a loud FAIL,
/// never `assume(false)`. The generated `match` is a handful of arms, so
/// CBMC constant-folds it per `ctx::*` call site instead of building the
/// full ~300-op `dispatch` (TRIAGE-LOG `op-subset`).
///
/// ```ignore
/// crate::host_subset!(my_dispatch {
///     log | effect => arm_log,
///     gain | gain_typed | gain_fixed => arm_gain,
///     count_held | count_held_name => arm_count_held,
///     add_held => arm_add_held,
///     add_group_crystals => arm_add_band_crystals,
///     player_count => arm_player_count,
///     hand_count => arm_hand_count,
/// });
/// crate::host::install_backend(my_dispatch);
/// ```
///
/// List **every** op the unmodified body reaches (including prompt /
/// resonance paths that a harness currently assumes away — if isolation
/// ever breaks, the undeclared op FAILS the proof rather than passing
/// vacuously).
#[macro_export]
macro_rules! host_subset {
    (
        $fname:ident {
            $( $($op:ident)|+ => $arm:ident ),+ $(,)?
        }
    ) => {
        fn $fname(
            op: card_sdk::kani_host::Op,
            a: card_sdk::kani_host::Args,
        ) -> i64 {
            use card_sdk::kani_host::Op;
            match op {
                $( $(Op::$op)|+ => $crate::host::$arm(a), )+
                other => $crate::host::outside_subset(other),
            }
        }
    };
}

/// Per-harness sparse symbolic world: only `used_seats` seats get symbolic
/// money / fans; the rest stay at [`crate::world::World::new`] defaults
/// (concrete — SAT-free). Piles / tiles / marks / events stay empty.
///
/// `player_count()` semantics are unchanged (TUNING §7): the `cfg(kani)`
/// facade returns the constant 4 after asserting `n_players ∈ 2..=4`, and
/// `player_out(p)` is true for `p >= n_players`. A clause whose body reads
/// the *value* of `player_count()` is n=4-only and must pin `n_players == 4`
/// (or take three fixed-n proofs).
#[cfg(kani)]
pub struct SparsePlay {
    /// Seats `0..used_seats` get symbolic money/fans (bounded).
    pub used_seats: usize,
    /// Concrete `n_players` (2, 3, or 4). Default 2 for seat-count-
    /// independent clauses.
    pub n_players: i32,
    pub money_lo: i32,
    pub money_hi: i32,
    pub fans_lo: i32,
    pub fans_hi: i32,
    /// Isolation: `hand_resonance[p] = 0` for every seat (default).
    pub resonance_in_hand: bool,
}

#[cfg(kani)]
impl Default for SparsePlay {
    fn default() -> Self {
        Self {
            used_seats: 2,
            n_players: 2,
            money_lo: 0,
            money_hi: 10_000,
            fans_lo: 0,
            fans_hi: 20,
            resonance_in_hand: false,
        }
    }
}

/// Build the sparse play world **in place** (no `reset_world` full-struct
/// copy). Call [`crate::host::install_backend`] (or `host::install`) first.
#[cfg(kani)]
pub fn sparse_play_world(cfg: SparsePlay) {
    let up = crate::intern::intern(crate::world::TOK_PP_POS);
    let down = crate::intern::intern(crate::world::TOK_PP_NEG);
    crate::world::with_world_mut(|w| {
        w.n_players = cfg.n_players;
        let used = cfg.used_seats.min(crate::world::MAX_PLAYERS);
        for p in 0..used {
            w.money[p] = kani::any();
            kani::assume(w.money[p] >= cfg.money_lo && w.money[p] <= cfg.money_hi);
            w.fans_pos[p] = kani::any();
            kani::assume(w.fans_pos[p] >= cfg.fans_lo && w.fans_pos[p] <= cfg.fans_hi);
            w.fans_neg[p] = kani::any();
            kani::assume(w.fans_neg[p] >= cfg.fans_lo && w.fans_neg[p] <= cfg.fans_hi);
            w.hand_resonance[p] = if cfg.resonance_in_hand {
                let n: i32 = kani::any();
                kani::assume(n >= 0 && n <= 2);
                n
            } else {
                0
            };
            w.key_set(p as i32, up, w.fans_pos[p]);
            w.key_set(p as i32, down, w.fans_neg[p]);
        }
        // Seats `used..MAX_PLAYERS` stay at `World::new()` defaults:
        // money=0, fans=0, hand_resonance=0 — concrete, not free variables.
        w.symbolic_answers = true;
    });
}

/// Native stub so the module typechecks outside `cargo kani`.
#[cfg(not(kani))]
pub fn sparse_play_world(_cfg: ()) {}
