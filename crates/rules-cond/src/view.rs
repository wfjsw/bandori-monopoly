//! The read-only query surface a condition evaluates against.
//!
//! One vocabulary: a condition and a guard read the world through the same
//! names (the card SDK's `ctx::*` read-only host queries, flattened). The
//! host implements [`CondView`] for its live snapshot (`game-rules`'s
//! `LiveSnap` / `SnapSrc`, `rules-native`'s mirror); [`crate::ctx::WindowCtx`]
//! + [`crate::ctx::CandidateCtx`] implement it for the eager snapshot the
//! tests and the precompiled-conds runtime use.
//!
//! Accessors are **lazy**: [`crate::vocab`]'s function closures call the view
//! on demand, so an unevaluated branch costs nothing. The host may still
//! pre-fill a snapshot (the counteract pre-scan's ~250 windows × ~200 probes
//! amortise it); the view is the contract either way.

/// Read-only queries a condition name may make. Int-only: every accessor
/// returns `i64` (or `bool`, bound as a CEL bool).
///
/// Seat ids are `0..player_count`; `-1` means "none". Tile ids are board
/// indices (`-1` = no tile). `slot` / `tok` are the candidate's latches.
pub trait CondView {
    // -- window / trigger ---------------------------------------------------
    /// Trigger kind wire value (`kind` / `trigger.kind`).
    fn kind(&self) -> i64;
    /// Trigger actor seat (`actor`), `-1` = none.
    fn actor(&self) -> i64;
    /// Trigger target seat (`target`), `-1` = none.
    fn target(&self) -> i64;
    /// The trigger's tile (`tile.id`), `-1` = none.
    fn tile_id(&self) -> i64;
    fn tile_owner(&self) -> i64;
    fn tile_houses(&self) -> i64;
    fn tile_mortgaged(&self) -> i64;
    fn tile_price(&self) -> i64;
    /// The amount involved (`value`).
    fn value(&self) -> i64;
    fn step(&self) -> i64;
    /// `by_card` owner seat (`by`), `-1` = none.
    fn by(&self) -> i64;
    fn pay_is_rent(&self) -> bool;
    /// Move face (`move.roll`), `-1` = none (binds CEL `null`).
    fn move_roll(&self) -> i64;
    /// Move kind (`move.kind`), `-1` = none (binds CEL `null`).
    fn move_kind(&self) -> i64;
    fn move_remaining(&self) -> i64;
    /// `t.Move.Main` (`move.main`).
    fn move_main(&self) -> bool;
    fn roll_source(&self) -> i64;
    fn abnormal(&self) -> bool;
    fn turn_player(&self) -> i64;
    fn turn_key(&self) -> i64;
    /// Effect-chain length (`chain.count`).
    fn chain_count(&self) -> i64;
    /// Chain link kinds, in order.
    fn chain_kinds(&self) -> Vec<i64>;
    /// Chain link `hits` seats, in order.
    fn chain_hits(&self) -> Vec<i64>;

    // -- candidate / owner --------------------------------------------------
    /// The candidate's owner seat (`owner`).
    fn owner(&self) -> i64;
    /// The candidate card's id hash (`card.id`).
    fn card_id(&self) -> i64;
    /// Is the candidate's running instance in play (`card.placed`)?
    fn card_placed(&self) -> bool;
    /// Crystals on the candidate instance (`card.cp`).
    fn card_cp(&self) -> i64;
    /// Per-card latch (`slot(name)`); missing = 0.
    fn slot(&self, name: &str) -> i64;
    /// Per-card token (`tok(kind)`); missing = 0.
    fn tok(&self, kind: i64) -> i64;
    /// Is `band` blocked for this candidate (`blocked(band)`)?
    fn blocked(&self, band: i64) -> bool;

    // -- player table (any seat) -------------------------------------------
    fn money(&self, seat: i64) -> i64;
    fn fire(&self, seat: i64) -> i64;
    fn crystals(&self, seat: i64) -> i64;
    fn hand(&self, seat: i64) -> i64;
    fn pos(&self, seat: i64) -> i64;
    fn out(&self, seat: i64) -> i64;
    fn stay(&self, seat: i64) -> i64;
    fn stun(&self, seat: i64) -> i64;
    fn exile(&self, seat: i64) -> i64;
    fn no_hand(&self, seat: i64) -> i64;
    /// Character id hash (`character_is(p, …)`).
    fn character(&self, seat: i64) -> i64;
    /// Band id hash (`band_is(p, …)`).
    fn band(&self, seat: i64) -> i64;
    /// Owned-tile count.
    fn tiles(&self, seat: i64) -> i64;
    /// Seat count (for `neighbor`).
    fn seat_count(&self) -> i64;

    // -- board --------------------------------------------------------------
    /// `tile_named(name) -> id`; `-1` when unknown (the guest's convention).
    fn tile_named(&self, name: &str) -> i64;
    /// Tile-kind predicates (the guest's `is_circle` family).
    fn is_circle(&self, tile: i64) -> bool;
    fn is_ring(&self, tile: i64) -> bool;
    fn is_live_house(&self, tile: i64) -> bool;
    fn is_buyable(&self, tile: i64) -> bool;
}