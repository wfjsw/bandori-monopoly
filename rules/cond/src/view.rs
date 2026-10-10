//! The read-only query surface a condition evaluates against.
//!
//! One vocabulary: a condition and a guard read the world through the same
//! names (the card SDK's `ctx::*` read-only host queries, flattened). The
//! host implements [`CondView`] for its live snapshot (`game-rules`'s
//! `LiveSnap` / `SnapView`, `rules-native`'s mirror); [`crate::ctx::WindowCtx`]
//! + [`crate::ctx::CandidateCtx`] implement it for the eager snapshot the
//! tests and the precompiled-conds runtime use.
//!
//! Accessors are **lazy** in principle: [`crate::vocab`]'s getters call the
//! view on demand. The evaluator still **eagerly** copies the used names into
//! the CEL context (see `eval.rs`) because a CEL `Context` wants owned
//! `'static` values -- a live trait-object indirection per probe measured
//! slower than the copy (GUARDS.md §4.2b).

/// Which tile-kind set a condition's `is_*` family asks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TileKind {
    Circle,
    Ring,
    LiveHouse,
    Buyable,
}

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
    /// `t.Move.Dir` (`move.dir`) -- 1 forward, -1 backward; `1` when the
    /// window has no move (matches the guest's `trigger::move_dir()`).
    fn move_dir(&self) -> i64;
    /// `move.tag(name)` -- the move's per-card tag counter
    /// (`t.Move.Tags`, `trigger::move_tag(name)`); missing name = 0.
    fn move_tag_named(&self, name: &str) -> i64;
    /// Eager table for `move.tag(name)` (window-scope, not per-candidate).
    fn move_tag_table(&self) -> Vec<(String, i64)>;
    fn roll_source(&self) -> i64;
    fn abnormal(&self) -> bool;
    fn turn_player(&self) -> i64;
    fn turn_key(&self) -> i64;
    /// Effect-chain length (`effect.count` / `chain.count`).
    fn chain_count(&self) -> i64;
    /// Chain link kinds, in order.
    fn chain_kinds(&self) -> Vec<i64>;
    /// Chain link `hits` seats, in order.
    fn chain_hits(&self) -> Vec<i64>;
    /// The trigger's card id hash (`trigger_card`), same encoding as
    /// [`CondView::card_id`] -- comparable to `card.id` and to a card-id
    /// literal (`id_of("…")`). `0` when the trigger carries no card.
    fn trigger_card(&self) -> i64;
    /// The trigger's counter / message name hash (`counter_name`), same
    /// encoding as [`crate::id_of`] -- `Trigger.name` on a `CounterChanged`
    /// hook (the counter name) or an `On::Message` entry (the message name).
    /// `0` when empty. `counter_is('cp')` is the string spelling.
    fn counter_name(&self) -> i64;

    // -- candidate / owner --------------------------------------------------
    /// The candidate's owner seat (`owner` / `owner.id`).
    fn owner(&self) -> i64;
    /// Owner overlay (`owner.money` etc.). Same numbers as `money(owner())`
    /// in a well-filled snapshot; the overlay is the per-probe answer.
    fn owner_money(&self) -> i64;
    fn owner_fire(&self) -> i64;
    fn owner_crystals(&self) -> i64;
    fn owner_hand(&self) -> i64;
    fn owner_pos(&self) -> i64;
    fn owner_out(&self) -> i64;
    fn owner_stay(&self) -> i64;
    fn owner_stun(&self) -> i64;
    fn owner_exile(&self) -> i64;
    fn owner_no_hand(&self) -> i64;
    fn owner_character(&self) -> i64;
    fn owner_band(&self) -> i64;
    fn owner_tiles(&self) -> i64;
    /// The candidate card's id hash (`card.id`).
    fn card_id(&self) -> i64;
    /// Is the candidate's running instance in play (`card.placed`)?
    fn card_placed(&self) -> bool;
    /// Named counter `name` on the candidate instance (`card.counter('name')`).
    /// `"cp"` → `FieldCard::cp`, `"crystals"` → `FieldCard::crystals`, else
    /// `FieldCard::counters[name]`. Missing = 0. (The wire names of
    /// `card-sdk`'s `counter` module; kept as a plain `&str` here so
    /// `rules-cond` stays free of the SDK.)
    fn card_counter(&self, name: &str) -> i64;
    /// Per-card latch (`slot(name)`); missing = 0.
    fn slot(&self, name: &str) -> i64;
    /// Per-card token counter (`tok('name')`, the guest's `ctx::tok(owner,
    /// name)`); missing = 0.
    fn tok_named(&self, name: &str) -> i64;
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
    /// Character **name** hash (`character_is(p, "名")` compares this to
    /// `id_of("名")` -- the same name `ctx::character_is` matches).
    fn character(&self, seat: i64) -> i64;
    /// Band **name** hash (`band_is(p, "Band")` compares this to
    /// `id_of("Band")` -- the same name `ctx::in_band` matches).
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

    // -- eager function tables (docs/GUARDS.md §4.2b: why eager) ------------
    /// Every `slot(name)` latch the host knows. Baked into `_<var>` for the
    /// eager function closures; a live view may answer from the world.
    fn slot_table(&self) -> Vec<(String, i64)>;
    /// Every `tok('name')` counter (non-zero ones; missing = 0).
    fn tok_named_table(&self) -> Vec<(String, i64)>;
    /// Every `card.counter('name')` counter on the candidate instance
    /// (non-zero ones; missing = 0). The two on-card wire names (`"cp"` /
    /// `"crystals"`) are always present when non-zero.
    fn card_counter_table(&self) -> Vec<(String, i64)>;
    /// Bands blocked for this candidate.
    fn blocked_bands(&self) -> Vec<i64>;
    /// Every `tile_named` spelling the host registers.
    fn tile_id_table(&self) -> Vec<(String, i64)>;
    /// Tile ids of one kind (the `is_*` family's backing list).
    fn tile_kind_list(&self, kind: TileKind) -> Vec<i64>;
}