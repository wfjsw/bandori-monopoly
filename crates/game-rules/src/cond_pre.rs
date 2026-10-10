//! G0/G2 of `docs/GUARDS.md`: the serialized **condition** per guarded entry,
//! compiled once at ruleset build and evaluated natively before the wasm guard
//! is instantiated.
//!
//! Distinct layers (user ruling 2026-10-07): **category / condition / guard**
//! each reject what only they can; nothing is rejected twice. The condition is
//! part of the decision, so every path that asks a guard goes through one
//! [`admits`] / [`admits_bool`] / [`admits_gate`] entry point.
//!
//! Wire form: [`CompiledPre::blob`] is `rules_cond::Cond::to_bytes(false)` (the
//! lean postcard AST). The host compiles from source (`compile`, the `cel`
//! parser); the browser glue loads the blob with `rules-cond`'s `runtime-only`
//! feature and never ships the parser.

pub use rules_cond::{
    CandidateCtx, ChainLink, Cond, MoveSnap, PlayerSnap, TileSnap, WindowCtx, WindowScope,
};
use rules_cond::view::{CondView, TileKind};

use crate::world::{CardWorld, Trigger};

// ---------------------------------------------------------------------------
// CondView over a SnapSrc (docs/GUARDS.md §4.2b)
// ---------------------------------------------------------------------------

/// A [`CondView`] over any [`SnapSrc`] plus the candidate being probed. This
/// is the host accessor implementation: adding a condition name = a [`CondView`]
/// method + a body here (and on [`rules_cond::SnapshotView`] / `LiveSnap`).
///
/// The methods call [`SnapSrc`] on `self.src` (the other type), never
/// `self.<same-name>` -- a CondView method that re-enters itself via method
/// resolution is an instant stack overflow (the previous attempt's bug).
pub struct SnapView<'a, S: SnapSrc> {
    pub src: &'a S,
    pub trigger: Trigger,
    /// The candidate being probed (`owner` seat). Named `owner_seat` so it
    /// never shadows [`CondView::owner`] under method resolution.
    pub owner_seat: i32,
    pub card: &'a str,
    pub placed: bool,
}

impl<'a, S: SnapSrc> SnapView<'a, S> {
    /// Window-only view (no candidate); candidate accessors answer defaults.
    pub fn window(src: &'a S) -> SnapView<'a, S> {
        SnapView {
            src,
            trigger: src.trigger(),
            owner_seat: -1,
            card: "",
            placed: false,
        }
    }

    pub fn candidate(
        src: &'a S,
        owner: i32,
        card: &'a str,
        placed: bool,
    ) -> SnapView<'a, S> {
        SnapView {
            src,
            trigger: src.trigger(),
            owner_seat: owner,
            card,
            placed,
        }
    }
}

impl<S: SnapSrc> CondView for SnapView<'_, S> {
    // -- window / trigger ---------------------------------------------------
    fn kind(&self) -> i64 {
        self.trigger.kind as i64
    }
    fn actor(&self) -> i64 {
        self.trigger.player_id as i64
    }
    fn target(&self) -> i64 {
        self.trigger.target as i64
    }
    fn tile_id(&self) -> i64 {
        self.trigger.tile as i64
    }
    fn tile_owner(&self) -> i64 {
        if self.trigger.tile >= 0 {
            self.src.tile_owner(self.trigger.tile) as i64
        } else {
            -1
        }
    }
    fn tile_houses(&self) -> i64 {
        if self.trigger.tile >= 0 {
            self.src.houses_of(self.trigger.tile) as i64
        } else {
            0
        }
    }
    fn tile_mortgaged(&self) -> i64 {
        if self.trigger.tile >= 0 {
            self.src.mortgaged_of(self.trigger.tile) as i64
        } else {
            0
        }
    }
    fn tile_price(&self) -> i64 {
        if self.trigger.tile >= 0 {
            self.src.tile_price(self.trigger.tile) as i64
        } else {
            0
        }
    }
    fn value(&self) -> i64 {
        self.trigger.value as i64
    }
    fn step(&self) -> i64 {
        self.trigger.step as i64
    }
    fn by(&self) -> i64 {
        self.trigger.by_card.map(|b| b as i64).unwrap_or(-1)
    }
    fn pay_is_rent(&self) -> bool {
        self.trigger.pay_is_rent
    }
    fn move_roll(&self) -> i64 {
        // Negative face is the pre-cast sentinel; reads as `null`.
        self.trigger.move_roll.filter(|&r| r >= 0).map(|r| r as i64).unwrap_or(-1)
    }
    fn move_kind(&self) -> i64 {
        self.trigger.move_kind.map(|k| k as i64).unwrap_or(-1)
    }
    fn move_remaining(&self) -> i64 {
        self.trigger.move_remaining as i64
    }
    fn move_main(&self) -> bool {
        self.trigger.move_main
    }
    fn roll_source(&self) -> i64 {
        self.trigger.roll_source as i64
    }
    fn abnormal(&self) -> bool {
        matches!(self.trigger.kind, crate::TriggerKind::Abnormal)
    }
    fn turn_player(&self) -> i64 {
        self.src.turn_player() as i64
    }
    fn turn_key(&self) -> i64 {
        self.src.turn_key() as i64
    }
    fn chain_count(&self) -> i64 {
        self.trigger.effects.len() as i64
    }
    fn chain_kinds(&self) -> Vec<i64> {
        self.trigger
            .effects
            .iter()
            .map(|e| crate::TriggerKind::from_str(e.kind) as i64)
            .collect()
    }
    fn chain_hits(&self) -> Vec<i64> {
        self.trigger.effects.iter().map(|e| e.target as i64).collect()
    }

    // -- candidate / owner --------------------------------------------------
    fn owner(&self) -> i64 {
        self.owner_seat as i64
    }
    fn owner_money(&self) -> i64 {
        if self.owner_seat >= 0 { self.src.money(self.owner_seat) as i64 } else { 0 }
    }
    fn owner_fire(&self) -> i64 {
        if self.owner_seat >= 0 { self.src.fire(self.owner_seat) as i64 } else { 0 }
    }
    fn owner_crystals(&self) -> i64 {
        if self.owner_seat >= 0 { self.src.band_crystals(self.owner_seat) as i64 } else { 0 }
    }
    fn owner_hand(&self) -> i64 {
        if self.owner_seat >= 0 { self.src.hand_size(self.owner_seat) as i64 } else { 0 }
    }
    fn owner_pos(&self) -> i64 {
        if self.owner_seat >= 0 { self.src.player_pos(self.owner_seat) as i64 } else { 0 }
    }
    fn owner_out(&self) -> i64 {
        if self.owner_seat >= 0 { self.src.player_out(self.owner_seat) as i64 } else { 0 }
    }
    fn owner_stay(&self) -> i64 {
        if self.owner_seat >= 0 { self.src.stay_of(self.owner_seat) as i64 } else { 0 }
    }
    fn owner_stun(&self) -> i64 {
        if self.owner_seat >= 0 { self.src.stun_of(self.owner_seat) as i64 } else { 0 }
    }
    fn owner_exile(&self) -> i64 {
        if self.owner_seat >= 0 {
            self.src.state_get(self.owner_seat, game_core::state::key::EXILE) as i64
        } else {
            0
        }
    }
    fn owner_no_hand(&self) -> i64 {
        if self.owner_seat >= 0 {
            self.src.state_get(self.owner_seat, game_core::state::key::NO_HAND) as i64
        } else {
            0
        }
    }
    fn owner_character(&self) -> i64 {
        if self.owner_seat < 0 {
            return 0;
        }
        id_of(&self.src.character_skill_id(self.owner_seat).unwrap_or_default())
    }
    fn owner_band(&self) -> i64 {
        if self.owner_seat < 0 {
            return 0;
        }
        id_of(&self.src.band_skill_id(self.owner_seat).unwrap_or_default())
    }
    fn owner_tiles(&self) -> i64 {
        if self.owner_seat >= 0 { self.src.owned_count(self.owner_seat) as i64 } else { 0 }
    }
    fn card_id(&self) -> i64 {
        id_of(self.card)
    }
    fn card_placed(&self) -> bool {
        self.placed
    }
    fn card_cp(&self) -> i64 {
        if self.owner_seat >= 0 {
            self.src.card_crystals(self.owner_seat, self.card) as i64
        } else {
            0
        }
    }
    fn slot(&self, name: &str) -> i64 {
        if self.owner_seat >= 0 {
            self.src.state_get(self.owner_seat, name) as i64
        } else {
            0
        }
    }
    fn tok(&self, _kind: i64) -> i64 {
        // The live world keeps tokens in per-card state; `fill_candidate`
        // leaves `toks` empty and the eager path answers 0. Match that.
        0
    }
    fn blocked(&self, band: i64) -> bool {
        // Mirror of `fill_candidate`'s empty `blocked_bands`: the host fills
        // the blocked list from the skill dispatch, not the raw world.
        let _ = band;
        false
    }

    // -- player table -------------------------------------------------------
    fn money(&self, seat: i64) -> i64 {
        self.src.money(seat as i32) as i64
    }
    fn fire(&self, seat: i64) -> i64 {
        self.src.fire(seat as i32) as i64
    }
    fn crystals(&self, seat: i64) -> i64 {
        self.src.band_crystals(seat as i32) as i64
    }
    fn hand(&self, seat: i64) -> i64 {
        self.src.hand_size(seat as i32) as i64
    }
    fn pos(&self, seat: i64) -> i64 {
        self.src.player_pos(seat as i32) as i64
    }
    fn out(&self, seat: i64) -> i64 {
        self.src.player_out(seat as i32) as i64
    }
    fn stay(&self, seat: i64) -> i64 {
        self.src.stay_of(seat as i32) as i64
    }
    fn stun(&self, seat: i64) -> i64 {
        self.src.stun_of(seat as i32) as i64
    }
    fn exile(&self, seat: i64) -> i64 {
        self.src.state_get(seat as i32, game_core::state::key::EXILE) as i64
    }
    fn no_hand(&self, seat: i64) -> i64 {
        self.src.state_get(seat as i32, game_core::state::key::NO_HAND) as i64
    }
    fn character(&self, seat: i64) -> i64 {
        id_of(&self.src.character_skill_id(seat as i32).unwrap_or_default())
    }
    fn band(&self, seat: i64) -> i64 {
        id_of(&self.src.band_skill_id(seat as i32).unwrap_or_default())
    }
    fn tiles(&self, seat: i64) -> i64 {
        self.src.owned_count(seat as i32) as i64
    }
    fn seat_count(&self) -> i64 {
        self.src.player_count() as i64
    }

    // -- board --------------------------------------------------------------
    fn tile_named(&self, name: &str) -> i64 {
        self.src.tile_named(name) as i64
    }
    fn is_circle(&self, tile: i64) -> bool {
        self.src.is_circle(tile as i32)
    }
    fn is_ring(&self, tile: i64) -> bool {
        self.src.is_ring(tile as i32)
    }
    fn is_live_house(&self, tile: i64) -> bool {
        self.src.is_live_house(tile as i32)
    }
    fn is_buyable(&self, tile: i64) -> bool {
        self.src.is_buyable(tile as i32)
    }

    // -- eager function tables ---------------------------------------------
    fn slot_table(&self) -> Vec<(String, i64)> {
        // Same keys `fill_candidate_extras` registers.
        SLOT_NAMES
            .iter()
            .map(|n| {
                let v = if self.owner_seat >= 0 {
                    self.src.state_get(self.owner_seat, n) as i64
                } else {
                    0
                };
                (n.to_string(), v)
            })
            .collect()
    }
    fn tok_table(&self) -> Vec<(i64, i64)> {
        Vec::new()
    }
    fn blocked_bands(&self) -> Vec<i64> {
        Vec::new()
    }
    fn tile_id_table(&self) -> Vec<(String, i64)> {
        collect_tile_ids(self.src)
            .into_iter()
            .map(|(k, v)| (k, v))
            .collect()
    }
    fn tile_kind_list(&self, kind: TileKind) -> Vec<i64> {
        let count = self.src.tile_count();
        let mut out = Vec::new();
        for tile in 0..count {
            let hit = match kind {
                TileKind::Circle => self.src.is_circle(tile),
                TileKind::Ring => self.src.is_ring(tile),
                TileKind::LiveHouse => self.src.is_live_house(tile),
                TileKind::Buyable => self.src.is_buyable(tile),
            };
            if hit {
                out.push(tile as i64);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Snapshot sources (docs/GUARDS.md §4.2)
// ---------------------------------------------------------------------------

/// The reads [`fill_window`] / [`fill_candidate`] need. Blanket-implemented for
/// every [`CardWorld`]; a live-world adapter (see
/// [`crate::wasm_rules::LiveSnap`]) implements it over `(&World, &GameData,
/// &Trigger)` so the counteract pre-filter builds a window context **without**
/// cloning the world. Nothing here mutates.
pub trait SnapSrc {
    fn trigger(&self) -> Trigger;
    fn tile_named(&self, name: &str) -> i32;
    /// Board tile count (ids are `0..tile_count`).
    fn tile_count(&self) -> i32;
    /// Board name of `tile` (the `tile_named` spelling, line breaks already
    /// stripped). Empty when the id is out of range. `fill_window` registers
    /// every name so a condition's `tile_named('…')` resolves.
    fn tile_name(&self, tile: i32) -> String;
    /// Tile-kind predicates backing the CEL `is_circle(t)` family.
    fn is_circle(&self, tile: i32) -> bool;
    fn is_ring(&self, tile: i32) -> bool;
    fn is_live_house(&self, tile: i32) -> bool;
    fn is_buyable(&self, tile: i32) -> bool;
    fn player_count(&self) -> i32;
    fn money(&self, player_id: i32) -> i32;
    fn fire(&self, player_id: i32) -> i32;
    fn band_crystals(&self, player_id: i32) -> i32;
    fn hand_size(&self, player_id: i32) -> i32;
    fn player_pos(&self, player_id: i32) -> i32;
    fn player_out(&self, player_id: i32) -> i32;
    fn stay_of(&self, player_id: i32) -> i32;
    fn stun_of(&self, player_id: i32) -> i32;
    fn state_get(&self, player_id: i32, key: &str) -> i32;
    fn character_skill_id(&self, player_id: i32) -> Option<String>;
    fn band_skill_id(&self, player_id: i32) -> Option<String>;
    fn owned_count(&self, player_id: i32) -> i32;
    fn card_crystals(&self, player_id: i32, card: &str) -> i32;
    fn tile_owner(&self, tile: i32) -> i32;
    fn houses_of(&self, tile: i32) -> i32;
    fn mortgaged_of(&self, tile: i32) -> i32;
    fn tile_price(&self, tile: i32) -> i32;
    fn turn_player(&self) -> i32;
    fn turn_key(&self) -> i32;
}

impl<W: CardWorld> SnapSrc for W {
    #[inline]
    fn trigger(&self) -> Trigger {
        CardWorld::trigger(self)
    }
    #[inline]
    fn tile_named(&self, name: &str) -> i32 {
        CardWorld::tile_named(self, name)
    }
    #[inline]
    fn tile_count(&self) -> i32 {
        CardWorld::tile_count(self)
    }
    #[inline]
    fn tile_name(&self, tile: i32) -> String {
        CardWorld::tile_name(self, tile)
    }
    #[inline]
    fn is_circle(&self, tile: i32) -> bool {
        CardWorld::is_circle(self, tile) != 0
    }
    #[inline]
    fn is_ring(&self, tile: i32) -> bool {
        CardWorld::is_ring(self, tile) != 0
    }
    #[inline]
    fn is_live_house(&self, tile: i32) -> bool {
        CardWorld::is_live_house(self, tile) != 0
    }
    #[inline]
    fn is_buyable(&self, tile: i32) -> bool {
        CardWorld::is_buyable(self, tile) != 0
    }
    #[inline]
    fn player_count(&self) -> i32 {
        CardWorld::player_count(self)
    }
    #[inline]
    fn money(&self, player_id: i32) -> i32 {
        CardWorld::money(self, player_id)
    }
    #[inline]
    fn fire(&self, player_id: i32) -> i32 {
        CardWorld::fire(self, player_id)
    }
    #[inline]
    fn band_crystals(&self, player_id: i32) -> i32 {
        CardWorld::band_crystals(self, player_id)
    }
    #[inline]
    fn hand_size(&self, player_id: i32) -> i32 {
        CardWorld::hand_size(self, player_id)
    }
    #[inline]
    fn player_pos(&self, player_id: i32) -> i32 {
        CardWorld::player_pos(self, player_id)
    }
    #[inline]
    fn player_out(&self, player_id: i32) -> i32 {
        CardWorld::player_out(self, player_id)
    }
    #[inline]
    fn stay_of(&self, player_id: i32) -> i32 {
        CardWorld::stay_of(self, player_id)
    }
    #[inline]
    fn stun_of(&self, player_id: i32) -> i32 {
        CardWorld::stun_of(self, player_id)
    }
    #[inline]
    fn state_get(&self, player_id: i32, key: &str) -> i32 {
        CardWorld::state_get(self, player_id, key)
    }
    #[inline]
    fn character_skill_id(&self, player_id: i32) -> Option<String> {
        CardWorld::character_skill_id(self, player_id)
    }
    #[inline]
    fn band_skill_id(&self, player_id: i32) -> Option<String> {
        CardWorld::band_skill_id(self, player_id)
    }
    #[inline]
    fn owned_count(&self, player_id: i32) -> i32 {
        CardWorld::owned_count(self, player_id)
    }
    #[inline]
    fn card_crystals(&self, player_id: i32, card: &str) -> i32 {
        CardWorld::card_crystals(self, player_id, card)
    }
    #[inline]
    fn tile_owner(&self, tile: i32) -> i32 {
        CardWorld::tile_owner(self, tile)
    }
    #[inline]
    fn houses_of(&self, tile: i32) -> i32 {
        CardWorld::houses_of(self, tile)
    }
    #[inline]
    fn mortgaged_of(&self, tile: i32) -> i32 {
        CardWorld::mortgaged_of(self, tile)
    }
    #[inline]
    fn tile_price(&self, tile: i32) -> i32 {
        CardWorld::tile_price(self, tile)
    }
    #[inline]
    fn turn_player(&self) -> i32 {
        CardWorld::turn_player(self)
    }
    #[inline]
    fn turn_key(&self) -> i32 {
        CardWorld::turn_key(self)
    }
}

/// Does this condition's **verdict** depend on hand contents? A ring's only
/// world change is declarations removing cards from hands (`build_round`), so
/// a verdict that reads no hand field is stable for the whole window and can be
/// memoised across laps. `owner_hand` is the candidate overlay's hand size;
/// `hand(p)` is the window's per-seat hand-size table.
pub fn cond_reads_hand(cond: &Cond) -> bool {
    cond.used_vars().iter().any(|v| v == "owner_hand")
        || cond.used_fns().iter().any(|f| f == "hand")
}

/// Does evaluating this condition read the window's hand table (`hand(p)`)?
/// Only these need a **fresh** [`WindowScope`] after a declaration -- the
/// candidate overlay (`owner_hand`) is rebuilt per probe from the live world,
/// so a stale scope still answers it correctly.
pub fn cond_reads_window_hand(cond: &Cond) -> bool {
    cond.used_fns().iter().any(|f| f == "hand")
}

/// One guarded entry's compiled condition plus its lean wire bytes.
#[derive(Clone, Debug)]
pub struct CompiledPre {
    /// Authoring source (tooling / `precheck`); `None` when loaded from bytes.
    pub src: Option<String>,
    /// `Cond::to_bytes(false)` -- what the runtime-only path loads.
    pub blob: Vec<u8>,
    pub cond: Cond,
}

impl CompiledPre {
    /// Host path: parse + lint + rewrite once (docs/GUARDS.md §4.3). Fail-closed:
    /// a parse / unknown-var / float error is a build error, never "treat as
    /// true" -- the clauses the condition carries are gone from the guard.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn compile(src: &str) -> Result<Self, rules_cond::CondError> {
        let cond = rules_cond::compile(src)?;
        let blob = cond.to_bytes(false);
        Ok(Self {
            src: Some(src.to_string()),
            blob,
            cond,
        })
    }

    /// Runtime-only path (browser glue): decode a lean blob and evaluate. No
    /// parser, no `compile` feature.
    pub fn from_bytes(blob: &[u8]) -> Result<Self, rules_cond::CondError> {
        let cond = Cond::from_bytes(blob)?;
        Ok(Self {
            src: None,
            blob: blob.to_vec(),
            cond,
        })
    }

    pub fn eval(&self, win: &WindowCtx, cand: &CandidateCtx) -> bool {
        self.cond.eval(win, cand)
    }
}

// ---------------------------------------------------------------- precompiled blob

/// Envelope format version of [`PrecompiledConds`]. First byte on the wire
/// (postcard is not self-describing); bump on any layout change so older
/// blobs fail loudly instead of decoding into garbage.
pub const PRECOMPILED_CONDS_VERSION: u8 = 1;

/// One guarded entry's lean compiled condition, keyed the way
/// [`crate::RulesetBuilder::precompiled`] wants it.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PrecompiledCond {
    /// Card id (`CardInfo.id`), e.g. `AG:回家的路上绕个道`.
    pub card: String,
    /// Index into that card's `on` list.
    pub entry: i32,
    /// `rules_cond::Cond::to_bytes(false)` -- the runtime-only wire form.
    pub blob: Vec<u8>,
}

/// The shipped precompiled-condition blob (`docs/GUARDS.md` §8.2): every
/// guarded entry's [`CompiledPre`], postcard in a versioned envelope.
///
/// `tools/build-ruleset.mjs` writes this next to the ruleset index as
/// `conds-<sha256>.bin`; the browser glue feeds it to
/// [`crate::RulesetBuilder::precompiled`] because its `rules-cond` build is
/// `runtime-only` (no CEL parser, §8.3). Native hosts compile the same
/// sources themselves and use this only to check agreement.
///
/// Entries are kept sorted by `(card, entry)` so the encoding -- and the
/// content hash -- is deterministic.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PrecompiledConds {
    pub version: u8,
    pub entries: Vec<PrecompiledCond>,
}

impl PrecompiledConds {
    /// Collect every compiled condition of a built set, in canonical order.
    pub fn collect(cards: &[crate::CardInfo], pre: &[Vec<Option<CompiledPre>>]) -> Self {
        let mut entries = Vec::new();
        for (c, row) in cards.iter().zip(pre) {
            for (ei, p) in row.iter().enumerate() {
                if let Some(p) = p {
                    entries.push(PrecompiledCond {
                        card: c.id.clone(),
                        entry: ei as i32,
                        blob: p.blob.clone(),
                    });
                }
            }
        }
        entries.sort_by(|a, b| a.card.cmp(&b.card).then(a.entry.cmp(&b.entry)));
        Self {
            version: PRECOMPILED_CONDS_VERSION,
            entries,
        }
    }

    /// Postcard envelope. Deterministic (sorted entries).
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("PrecompiledConds is plain data")
    }

    /// Decode the envelope; rejects a wrong [`PRECOMPILED_CONDS_VERSION`] --
    /// no migration (same policy as `Cond::from_bytes`).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::RuleError> {
        let bad = |m: String| crate::RuleError::Load(format!("precompiled conds blob: {m}"));
        let v: Self = postcard::from_bytes(bytes).map_err(|e| bad(e.to_string()))?;
        if v.version != PRECOMPILED_CONDS_VERSION {
            return Err(bad(format!(
                "wire version {}, this build reads {}",
                v.version, PRECOMPILED_CONDS_VERSION
            )));
        }
        Ok(v)
    }

    /// sha256 of the canonical envelope, hex. What `conds-<sha>.bin` is named
    /// after and what the index lists.
    pub fn sha256(&self) -> String {
        crate::host::hex_sha256(&self.to_bytes())
    }

    /// The ruleset-identity contribution (docs/GUARDS.md §8.2): one
    /// `card + entry + blob-hash` line per entry, sorted. Mixed into
    /// [`crate::Ruleset::sha256`] so record stamps / bundle ids change with
    /// the compiled conditions, not only with the module bytes.
    pub fn identity_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .entries
            .iter()
            .map(|e| {
                format!(
                    "{}\u{1f}{}\u{1f}{}",
                    e.card,
                    e.entry,
                    crate::host::hex_sha256(&e.blob)
                )
            })
            .collect();
        lines.sort();
        lines
    }
}

/// Measurement counters (same shape as [`crate::bot_cost`]). Compiled out
/// unless the `bot-cost` feature is on; G3/G4 use these to measure how much the
/// conditions skip. Always-on twin counters are cheap `AtomicU64` bumps and
/// live in [`guard_cost`] so a test can read them without the feature.
pub mod guard_cost {
    use std::sync::atomic::AtomicU64;

    /// Admits calls that evaluated a condition.
    pub static COND_EVALS: AtomicU64 = AtomicU64::new(0);
    /// Condition rejected: the wasm guard was skipped (a `can_counteract`
    /// probe / hook guard / play gate never instantiated).
    pub static SKIPPED_BY_CONDITION: AtomicU64 = AtomicU64::new(0);
    /// Condition passed (or absent) and the guard was asked.
    pub static GUARD_ASKED: AtomicU64 = AtomicU64::new(0);
    /// ns spent in `Cond::eval`.
    pub static COND_EVAL_NS: AtomicU64 = AtomicU64::new(0);

    pub fn reset() {
        use std::sync::atomic::Ordering::Relaxed;
        COND_EVALS.store(0, Relaxed);
        SKIPPED_BY_CONDITION.store(0, Relaxed);
        GUARD_ASKED.store(0, Relaxed);
        COND_EVAL_NS.store(0, Relaxed);
    }
}

/// G3 (docs/GUARDS.md §5.1): the migration audit. On every probe the host
/// evaluates the card's pre-migration `legacy_*` guard and
/// `kind ∧ pre(ctx) ∧ guard(ctx)` and **panics** with card id + trigger dump on
/// any mismatch (equivalence, both directions). Compiled out unless the
/// `guard-audit` feature is on; once a card is clean its `legacy_*` copy is
/// deleted and the check is a no-op for it.
///
/// `legacy == None` means the entry has no legacy copy (`has_legacy == false`)
/// and the check is skipped. `trigger` is a one-line dump of the window.
#[cfg(feature = "guard-audit")]
pub fn legacy_audit(
    card: &str,
    entry: i32,
    legacy: Option<bool>,
    migrated: bool,
    trigger: &str,
) {
    let Some(legacy) = legacy else {
        return;
    };
    if legacy != migrated {
        panic!(
            "GUARDS G3 audit mismatch: card {card:?} entry {entry}\n\
             trigger: {trigger}\n\
             legacy(ctx) = {legacy}, category ∧ pre(ctx) ∧ guard(ctx) = {migrated}\n\
             The two must be equivalent (docs/GUARDS.md §5.1). Either the \
             condition drops a clause the legacy guard kept, or the residual \
             guard / condition rejects what the legacy guard admitted."
        );
    }
}

/// No-op when the `guard-audit` feature is off (the production shape).
#[cfg(not(feature = "guard-audit"))]
#[inline]
pub fn legacy_audit(
    _card: &str,
    _entry: i32,
    _legacy: Option<bool>,
    _migrated: bool,
    _trigger: &str,
) {
}

#[inline]
fn bump(stat: &std::sync::atomic::AtomicU64) {
    stat.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Evaluate the condition half of [`admits`]. `true` when the entry has no
/// condition or its condition accepts the (window, candidate) pair.
///
/// `scope` may be `None` only when `pre` is `None` -- a condition needs a
/// window scope to evaluate against. That shape lets a caller skip building
/// the (expensive) CEL scope entirely for unconditioned entries.
#[inline]
pub fn condition_allows(
    pre: Option<&CompiledPre>,
    scope: Option<&WindowScope>,
    cand: &CandidateCtx,
) -> bool {
    let Some(pre) = pre else {
        return true;
    };
    let scope = scope.expect("a condition evaluation needs a window scope");
    bump(&guard_cost::COND_EVALS);
    #[cfg(feature = "bot-cost")]
    let t0 = std::time::Instant::now();
    let ok = scope.eval(&pre.cond, cand);
    #[cfg(feature = "bot-cost")]
    guard_cost::COND_EVAL_NS
        .fetch_add(t0.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    if !ok {
        bump(&guard_cost::SKIPPED_BY_CONDITION);
    }
    ok
}

/// The one shared gate: `admits = cond(window, candidate) && guard(...)`.
/// Every path that asks a guard goes through this -- counteraction offers,
/// hook dispatch, play gates / skill gates, the view's `playable` flags, the
/// bot paths, `rules-native`. No caller may call a guard directly: with
/// clauses removed from the guards (G4), skipping the condition would admit
/// what it should reject.
///
/// `guard` returns `Ok(true)` = the residual guard admits. A rejecting
/// condition short-circuits before the wasm guard is ever instantiated.
#[inline]
pub fn admits<E, F: FnOnce() -> Result<bool, E>>(
    pre: Option<&CompiledPre>,
    scope: Option<&WindowScope>,
    cand: &CandidateCtx,
    guard: F,
) -> Result<bool, E> {
    if !condition_allows(pre, scope, cand) {
        return Ok(false);
    }
    bump(&guard_cost::GUARD_ASKED);
    let ok = guard()?;
    Ok(ok)
}

/// [`admits`] for the `On::Play` gate shape (`Ok(None)` = playable,
/// `Ok(Some(why))` = blocked). A rejecting condition **is** a block -- the
/// entry is part of the decision -- and the gate is skipped. `blocked`
/// supplies the reason the caller shows (so the UI/i18n stays in the caller's
/// hands).
#[inline]
pub fn admits_gate<T, E, F: FnOnce() -> Result<Option<T>, E>>(
    pre: Option<&CompiledPre>,
    scope: Option<&WindowScope>,
    cand: &CandidateCtx,
    blocked: impl FnOnce() -> T,
    gate: F,
) -> Result<Option<T>, E> {
    if !condition_allows(pre, scope, cand) {
        bump(&guard_cost::GUARD_ASKED);
        return Ok(Some(blocked()));
    }
    bump(&guard_cost::GUARD_ASKED);
    let why = gate()?;
    Ok(why)
}

/// `admits` for a pure "does this entry apply" question with no residual
/// guard at all (`pre` only). Used when the guard was deleted (`None`) in G4.
#[inline]
pub fn admits_pre(
    pre: Option<&CompiledPre>,
    scope: Option<&WindowScope>,
    cand: &CandidateCtx,
) -> bool {
    if !condition_allows(pre, scope, cand) {
        return false;
    }
    bump(&guard_cost::GUARD_ASKED);
    true
}

// ---------------------------------------------------------------------------
// Window / candidate snapshots (docs/GUARDS.md §4.2)
// ---------------------------------------------------------------------------

/// Stable int id for a string identity (character / band / card id). Not the
/// data-table index -- a name-stable hash so a condition's `character_is(p, …)`
/// literal survives a data reshuffle. Collisions are astronomically unlikely
/// at 64 bits and the schema is int-only.
pub fn id_of(name: &str) -> i64 {
    // FNV-1a 64.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h as i64
}

/// Build the window snapshot once per trigger / chain window and reuse it
/// across every candidate probe in that window ([`WindowScope`]). This is
/// where the G1 savings live: ~48 900 counteract probes share ~250 windows.
pub fn fill_window<S: SnapSrc>(world: &S) -> WindowCtx {
    let t = world.trigger();
    let tile_ids = collect_tile_ids(world);
    let mut circle_tiles = Vec::new();
    let mut ring_tiles = Vec::new();
    let mut live_house_tiles = Vec::new();
    let mut buyable_tiles = Vec::new();
    for tile in 0..world.tile_count() {
        if world.is_circle(tile) {
            circle_tiles.push(tile as i64);
        }
        if world.is_ring(tile) {
            ring_tiles.push(tile as i64);
        }
        if world.is_live_house(tile) {
            live_house_tiles.push(tile as i64);
        }
        if world.is_buyable(tile) {
            buyable_tiles.push(tile as i64);
        }
    }

    let players: Vec<PlayerSnap> = (0..world.player_count())
        .map(|p| PlayerSnap {
            money: world.money(p) as i64,
            fire: world.fire(p) as i64,
            crystals: world.band_crystals(p) as i64,
            hand: world.hand_size(p) as i64,
            pos: world.player_pos(p) as i64,
            out: world.player_out(p) as i64,
            stay: world.stay_of(p) as i64,
            stun: world.stun_of(p) as i64,
            exile: world.state_get(p, game_core::state::key::EXILE) as i64,
            no_hand: world.state_get(p, game_core::state::key::NO_HAND) as i64,
            character: id_of(&world.character_skill_id(p).unwrap_or_default()),
            band: id_of(&world.band_skill_id(p).unwrap_or_default()),
            tiles: world.owned_count(p) as i64,
        })
        .collect();

    let chain: Vec<ChainLink> = t
        .effects
        .iter()
        .map(|e| ChainLink {
            kind: crate::TriggerKind::from_str(e.kind) as i64,
            from: e.from as i64,
            hits: e.target as i64,
        })
        .collect();

    let mv = MoveSnap {
        // Match the guest's `trigger::move_roll()` (`card-sdk/ctx.rs`): a
        // negative face is the pre-cast sentinel (`value = -1` on the `roll`
        // pre-trigger) and reads as `null`, so `move.roll != null` means "a
        // real face exists" exactly as the guards wrote it.
        roll: t.move_roll.filter(|&r| r >= 0).map(|r| r as i64),
        kind: t.move_kind.map(|k| k as i64),
        remaining: t.move_remaining as i64,
        // `t.Move.Main` (`trigger::move_is_main()` in the guest).
        main: t.move_main,
    };

    let tile = TileSnap {
        id: t.tile as i64,
        owner: if t.tile >= 0 { world.tile_owner(t.tile) as i64 } else { -1 },
        houses: if t.tile >= 0 { world.houses_of(t.tile) as i64 } else { 0 },
        mortgaged: if t.tile >= 0 { world.mortgaged_of(t.tile) as i64 } else { 0 },
        price: if t.tile >= 0 { world.tile_price(t.tile) as i64 } else { 0 },
    };

    WindowCtx {
        kind: t.kind as i64,
        actor: t.player_id as i64,
        target: t.target as i64,
        tile,
        value: t.value as i64,
        step: t.step as i64,
        by: t.by_card.map(|b| b as i64).unwrap_or(-1),
        pay_is_rent: t.pay_is_rent,
        mv,
        roll_source: t.roll_source as i64,
        // `abnormal` is the AbnormalGate window, not "this link was negated".
        // There is no `t.Abnormal` bool on `Trigger`; the closest signal is the
        // trigger kind itself.
        abnormal: matches!(t.kind, crate::TriggerKind::Abnormal),
        chain,
        turn_player: world.turn_player() as i64,
        turn_key: world.turn_key() as i64,
        players,
        tile_ids,
        circle_tiles,
        ring_tiles,
        live_house_tiles,
        buyable_tiles,
    }
}

/// Play-gate window (docs/GUARDS.md §4.4 item 3): `cant_play` / skill gates have
/// **no** `Trigger` to source a window from -- same schema with `kind` absent
/// (`TriggerKind::None`). Everything else is ambient world state, actor = the
/// player being asked. Built once per ask; reused across the cards of one view.
pub fn fill_window_ambient<S: SnapSrc>(world: &S, player_id: i32) -> WindowCtx {
    let tile_ids = collect_tile_ids(world);
    let mut circle_tiles = Vec::new();
    let mut ring_tiles = Vec::new();
    let mut live_house_tiles = Vec::new();
    let mut buyable_tiles = Vec::new();
    for tile in 0..world.tile_count() {
        if world.is_circle(tile) {
            circle_tiles.push(tile as i64);
        }
        if world.is_ring(tile) {
            ring_tiles.push(tile as i64);
        }
        if world.is_live_house(tile) {
            live_house_tiles.push(tile as i64);
        }
        if world.is_buyable(tile) {
            buyable_tiles.push(tile as i64);
        }
    }
    let players: Vec<PlayerSnap> = (0..world.player_count())
        .map(|p| PlayerSnap {
            money: world.money(p) as i64,
            fire: world.fire(p) as i64,
            crystals: world.band_crystals(p) as i64,
            hand: world.hand_size(p) as i64,
            pos: world.player_pos(p) as i64,
            out: world.player_out(p) as i64,
            stay: world.stay_of(p) as i64,
            stun: world.stun_of(p) as i64,
            exile: world.state_get(p, game_core::state::key::EXILE) as i64,
            no_hand: world.state_get(p, game_core::state::key::NO_HAND) as i64,
            character: id_of(&world.character_skill_id(p).unwrap_or_default()),
            band: id_of(&world.band_skill_id(p).unwrap_or_default()),
            tiles: world.owned_count(p) as i64,
        })
        .collect();
    WindowCtx {
        kind: 0,
        actor: player_id as i64,
        target: -1,
        tile: TileSnap {
            id: world.player_pos(player_id) as i64,
            owner: {
                let t = world.player_pos(player_id);
                if t >= 0 { world.tile_owner(t) as i64 } else { -1 }
            },
            houses: 0,
            mortgaged: 0,
            price: 0,
        },
        value: 0,
        step: 0,
        by: -1,
        pay_is_rent: false,
        mv: MoveSnap::default(),
        roll_source: 0,
        abnormal: false,
        chain: Vec::new(),
        turn_player: world.turn_player() as i64,
        turn_key: world.turn_key() as i64,
        players,
        tile_ids,
        circle_tiles,
        ring_tiles,
        live_house_tiles,
        buyable_tiles,
    }
}

/// Every board name -> id, so `tile_named('Bandori车站')` / `tile_named('CiRCLE')`
/// resolve to the same id the guest's `ctx::tile_named` returns. Unknown names
/// stay out of the map; the CEL `tile_named` answers `-1` for those (matching
/// the guest), never tile 0.
pub fn collect_tile_ids<S: SnapSrc>(world: &S) -> std::collections::BTreeMap<String, i64> {
    let mut tile_ids = std::collections::BTreeMap::new();
    for tile in 0..world.tile_count() {
        let name = world.tile_name(tile);
        if !name.is_empty() {
            tile_ids.insert(name, tile as i64);
        }
    }
    // Legacy §2.1 sample aliases (`tile_named("circle")` etc.): resolve through
    // the same `tile_named` lookup so a kind-spelled name still works if a
    // board ever carries one.
    for name in ["circle", "shop", "ring", "agent", "liveHouse"] {
        let id = world.tile_named(name);
        if id >= 0 {
            tile_ids.insert(name.to_string(), id as i64);
        }
    }
    tile_ids
}

/// Per-candidate overlay: the card + seat being probed. Built per (card, seat)
/// inside a window -- much cheaper than a window, so a fresh value is fine.
pub fn fill_candidate<S: SnapSrc>(
    world: &S,
    owner: i32,
    card: &str,
    placed: bool,
) -> CandidateCtx {
    let p = owner;
    let mut cand = CandidateCtx {
        owner: owner as i64,
        owner_money: world.money(p) as i64,
        owner_fire: world.fire(p) as i64,
        owner_crystals: world.band_crystals(p) as i64,
        owner_hand: world.hand_size(p) as i64,
        owner_pos: world.player_pos(p) as i64,
        owner_out: world.player_out(p) as i64,
        owner_stay: world.stay_of(p) as i64,
        owner_stun: world.stun_of(p) as i64,
        owner_exile: world.state_get(p, game_core::state::key::EXILE) as i64,
        owner_no_hand: world.state_get(p, game_core::state::key::NO_HAND) as i64,
        owner_character: id_of(&world.character_skill_id(p).unwrap_or_default()),
        owner_band: id_of(&world.band_skill_id(p).unwrap_or_default()),
        owner_tiles: world.owned_count(p) as i64,
        card_id: id_of(card),
        card_placed: placed,
        card_cp: world.card_crystals(p, card) as i64,
        slots: Default::default(),
        toks: Default::default(),
        blocked_bands: Vec::new(),
    };
    fill_candidate_extras(world, owner, &mut cand, true);
    cand
}

/// Slot names any condition reads (`slot('…')`). Each is a bare player-state
/// key -- the same one `ctx::slot(player_id, name)` reads via
/// `CardWorld::slot`, **not** a `slot:`-prefixed alias (that mismatch made
/// `slot('lastWalk') > 0` read 0 and silently close a counteraction window).
/// Extend this list when a new `slot('…')` shows up in a `pre`.
pub const SLOT_NAMES: &[&str] = &[
    "asUsualTurn",
    "lastWalk",
    // `追逐梦想的步伐` pass tag (`ctx::set_slot(player_id, SLOT_TAG, 1)`).
    "lock_dream_tag",
    // `甜甜圈爱好者` half-price window (`state::set(player_id, HALF, 1)`).
    "skill.manaDonut.half",
    // `（沙绫）总有一天要给这片天空命名` pass tag (`ctx::set_slot(player_id, SLOT_PASSED, 1)`).
    "saaya_sky_passed",
    // `one_of_us` partner seat+1 (`ctx::set_slot(owner, "one_of_us_partner", partner+1)`).
    "one_of_us_partner",
    // `骰子已经掷下` turn-key lock (`ctx::set_slot(player_id, NO_COUNTERACT, turn_key)`).
    "diceCastActive",
    // `Change the world` once-per-turn tag (`ctx::set_slot(player_id, SLOT_TURN, turn_key)`).
    "change_world_turn",
    // `here_the_world` held-cards count (`ctx::set_slot(player_id, SLOT_HELD_LEN, n)`).
    "here_held_len",
    // `（灯）诗超绊` due-payment latch (`state::set(player_id, DUE, ...)`).
    "skill.tomoriPoem.due",
    // `（莉莉）坚定决心` passed-this-turn latch (`state::set(player_id, PASSED, ...)`).
    "skill.rimiResolve.passed",
    // `（鸫）素日日常` owed-money latch (`state::set(player_id, OWED, ...)`).
    "skill.tsugumiPlain.owed",
];

/// Fill the per-candidate slot / token tables a condition may read
/// (`slot('asUsualTurn')`, `tok(kind)`). Only worth the copy when the
/// condition actually names them -- the caller can pass `false` otherwise.
pub fn fill_candidate_extras<S: SnapSrc>(
    world: &S,
    owner: i32,
    cand: &mut CandidateCtx,
    with_slots: bool,
) {
    if with_slots {
        // The schema's `slot(name)` is per-card latch state keyed by name.
        // The host stores latches as bare player-state keys -- the same key
        // `ctx::slot(player_id, name)` reads through `CardWorld::slot`.
        for name in SLOT_NAMES {
            let v = world.state_get(owner, name);
            cand.slots.insert(name.to_string(), v as i64);
        }
    }
}

/// A window scope helper: build once, evaluate every candidate against it.
pub fn window_scope(win: &WindowCtx) -> WindowScope {
    WindowScope::new(win)
}