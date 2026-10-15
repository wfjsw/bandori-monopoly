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
use rules_cond::view::CondView;

/// Name-stable hash for string identities (persona / group / card id).
/// Re-exported from [`rules_cond::id_of`] -- one hash, one spelling.
pub use rules_cond::id_of;

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
        self.trigger.kind.as_i32() as i64
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
        self.trigger.pay.map(|p| p.is_rent).unwrap_or(false)
    }
    fn move_roll(&self) -> i64 {
        // Negative face is the pre-cast sentinel; reads as `null`.
        self.trigger
            .mv
            .as_ref()
            .and_then(|m| m.roll)
            .filter(|&r| r >= 0)
            .map(|r| r as i64)
            .unwrap_or(-1)
    }
    fn move_kind(&self) -> i64 {
        self.trigger.mv.as_ref().map(|m| m.kind as i64).unwrap_or(-1)
    }
    fn move_remaining(&self) -> i64 {
        self.trigger.mv.as_ref().map(|m| m.remaining).unwrap_or(0) as i64
    }
    fn move_main(&self) -> bool {
        self.trigger.main
    }
    fn move_dir(&self) -> i64 {
        self.trigger
            .mv
            .as_ref()
            .map(|m| m.dir.as_i32() as i64)
            .unwrap_or(1)
    }
    fn move_tag_named(&self, name: &str) -> i64 {
        self.trigger
            .mv
            .as_ref()
            .and_then(|m| m.tags.iter().find(|(k, _)| k == name))
            .map(|(_, v)| *v as i64)
            .unwrap_or(0)
    }
    fn move_tag_table(&self) -> Vec<(String, i64)> {
        self.trigger
            .mv
            .as_ref()
            .map(|m| m.tags.iter().map(|(k, v)| (k.clone(), *v as i64)).collect())
            .unwrap_or_default()
    }
    fn roll_source(&self) -> i64 {
        self.trigger.roll_source.as_i32() as i64
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
            .map(|e| crate::TriggerKind::from_str(e.kind).as_i32() as i64)
            .collect()
    }
    fn chain_hits(&self) -> Vec<i64> {
        self.trigger.effects.iter().map(|e| e.target as i64).collect()
    }
    fn trigger_card(&self) -> i64 {
        match self.trigger.card.as_deref() {
            Some(card) if !card.is_empty() => id_of(card),
            _ => 0,
        }
    }
    fn counter_name(&self) -> i64 {
        match self.trigger.name.as_deref() {
            Some(name) if !name.is_empty() => id_of(name),
            _ => 0,
        }
    }

    // -- candidate / owner --------------------------------------------------
    fn owner(&self) -> i64 {
        self.owner_seat as i64
    }
    fn owner_money(&self) -> i64 {
        if self.owner_seat >= 0 { self.src.money(self.owner_seat) as i64 } else { 0 }
    }
    fn owner_charge(&self) -> i64 {
        if self.owner_seat >= 0 { self.src.charge(self.owner_seat) as i64 } else { 0 }
    }
    fn owner_group_pool(&self, name: &str) -> i64 {
        if self.owner_seat >= 0 { self.src.group_pool(self.owner_seat, name) as i64 } else { 0 }
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
    fn owner_persona(&self) -> i64 {
        if self.owner_seat < 0 {
            return 0;
        }
        id_of(&self.src.persona_name(self.owner_seat).unwrap_or_default())
    }
    fn owner_group(&self) -> i64 {
        if self.owner_seat < 0 {
            return 0;
        }
        id_of(&self.src.group_name(self.owner_seat).unwrap_or_default())
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
    fn card_counter(&self, name: &str) -> i64 {
        if self.owner_seat >= 0 {
            self.src.card_counter(self.owner_seat, self.card, name) as i64
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
    fn tok_named(&self, name: &str) -> i64 {
        // Same data the guest's `ctx::tok(owner, name)` reads
        // (`CardWorld::tok`).
        if self.owner_seat >= 0 {
            self.src.tok(self.owner_seat, name) as i64
        } else {
            0
        }
    }
    fn blocked(&self, group: i64) -> bool {
        // Mirror of `fill_candidate`'s empty `blocked_groups`: the host fills
        // the blocked list from the skill dispatch, not the raw world.
        let _ = group;
        false
    }

    // -- player table -------------------------------------------------------
    fn money(&self, seat: i64) -> i64 {
        self.src.money(seat as i32) as i64
    }
    fn charge(&self, seat: i64) -> i64 {
        self.src.charge(seat as i32) as i64
    }
    fn group_pool(&self, seat: i64, name: &str) -> i64 {
        self.src.group_pool(seat as i32, name) as i64
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
    fn state(&self, seat: i64, key: &str) -> i64 {
        self.src.state_get(seat as i32, key) as i64
    }
    fn state_table(&self) -> Vec<(i64, String, i64)> {
        let mut rows = Vec::new();
        for s in 0..self.src.player_count() {
            for (k, v) in self.src.state_entries(s) {
                rows.push((s as i64, k, v as i64));
            }
        }
        rows
    }
    fn persona(&self, seat: i64) -> i64 {
        id_of(&self.src.persona_name(seat as i32).unwrap_or_default())
    }
    fn group(&self, seat: i64) -> i64 {
        id_of(&self.src.group_name(seat as i32).unwrap_or_default())
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
    fn tile_is(&self, tile: i64, kind: &str) -> bool {
        self.src.tile_is(tile as i32, kind)
    }
    fn tile_attr(&self, tile: i64, attr: &str) -> i64 {
        self.src.tile_attr(tile as i32, attr) as i64
    }
    fn tile_in(&self, player: i64, tile: i64, class: &str) -> bool {
        self.src.tile_in(player as i32, tile as i32, class)
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
    fn tok_named_table(&self) -> Vec<(String, i64)> {
        if self.owner_seat < 0 {
            return Vec::new();
        }
        self.src
            .tok_names(self.owner_seat, "")
            .into_iter()
            .map(|n| {
                let v = self.src.tok(self.owner_seat, &n) as i64;
                (n, v)
            })
            .collect()
    }
    fn card_counter_table(&self) -> Vec<(String, i64)> {
        if self.owner_seat < 0 {
            return Vec::new();
        }
        self.src
            .card_counter_names(self.owner_seat, self.card)
            .into_iter()
            .map(|n| {
                let v = self.src.card_counter(self.owner_seat, self.card, &n) as i64;
                (n, v)
            })
            .collect()
    }
    fn blocked_groups(&self) -> Vec<i64> {
        Vec::new()
    }
    fn tile_id_table(&self) -> Vec<(String, i64)> {
        collect_tile_ids(self.src)
            .into_iter()
            .map(|(k, v)| (k, v))
            .collect()
    }
    fn tile_kind_list(&self, kind: &str) -> Vec<i64> {
        let count = self.src.tile_count();
        let mut out = Vec::new();
        for tile in 0..count {
            if self.src.tile_is(tile, kind) {
                out.push(tile as i64);
            }
        }
        out
    }
    fn tile_kind_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        let count = self.src.tile_count();
        for tile in 0..count {
            let k = self.src.tile_kind_of(tile);
            if !k.is_empty() && !names.iter().any(|n| n == &k) {
                names.push(k);
            }
        }
        names
    }

    // -- turn plan / counters ----------------------------------------------
    fn plan_fixed_roll(&self) -> i64 {
        self.src.fixed_roll() as i64
    }
    fn gains_this_turn(&self, seat: i64) -> i64 {
        if seat < 0 {
            return 0;
        }
        self.src.gains_this_turn(seat as i32) as i64
    }
    fn targeted_count(&self, seat: i64) -> i64 {
        if seat < 0 {
            return 0;
        }
        self.src.targeted_count(seat as i32) as i64
    }

    // -- candidate instance -------------------------------------------------
    fn card_tile(&self) -> i64 {
        if self.owner_seat < 0 || self.card.is_empty() {
            return -2;
        }
        self.src.card_tile(self.owner_seat, self.card) as i64
    }

    // -- board geometry ----------------------------------------------------
    fn tile_count(&self) -> i64 {
        self.src.tile_count() as i64
    }
    fn dist(&self, a: i64, b: i64) -> i64 {
        let n = self.src.tile_count() as i64;
        if n <= 0 {
            return 0;
        }
        let d = ((b - a) % n + n) % n;
        d.min(n - d)
    }
    fn players_on(&self, tile: i64, except: i64) -> i64 {
        // `ctx::players_on` counts present (`!out && exile == 0`) standers.
        let count = self.src.tile_count() as i64; // unused; seats drive the scan
        let _ = count;
        (0..self.src.player_count() as i64)
            .filter(|&s| {
                s != except
                    && self.out(s) == 0
                    && self.exile(s) == 0
                    && self.pos(s) == tile
            })
            .count() as i64
    }
    fn next_dist(&self, p: i64, dir: i64) -> i64 {
        let n = self.src.tile_count() as i64;
        let pos = self.pos(p);
        if n <= 0 || pos < 0 {
            return -1;
        }
        let mut best = i64::MAX;
        for o in 0..self.src.player_count() as i64 {
            if o == p || self.out(o) != 0 {
                continue;
            }
            let q = self.pos(o);
            if q < 0 {
                continue;
            }
            let fwd = if dir >= 0 {
                ((q - pos) % n + n) % n
            } else {
                ((pos - q) % n + n) % n
            };
            if fwd > 0 && fwd < best {
                best = fwd;
            }
        }
        if best == i64::MAX {
            -1
        } else {
            best
        }
    }
    fn others_within(&self, p: i64, radius: i64) -> i64 {
        let pos = self.pos(p);
        if pos < 0 {
            return 0;
        }
        (0..self.src.player_count() as i64)
            .filter(|&o| {
                if o == p || self.out(o) != 0 {
                    return false;
                }
                let d = self.dist(pos, self.pos(o));
                d > 0 && d <= radius
            })
            .count() as i64
    }
    fn owned_within(&self, p: i64, radius: i64) -> i64 {
        let pos = self.pos(p);
        if pos < 0 {
            return 0;
        }
        (0..self.src.tile_count() as i64)
            .filter(|&t| {
                self.src.tile_owner(t as i32) as i64 == p && self.dist(pos, t) <= radius
            })
            .count() as i64
    }
    fn on_path(&self, me: i64, them: i64) -> i64 {
        let roll = self.move_roll();
        if roll < 0 {
            return 0;
        }
        let steps = roll.abs();
        let n = self.src.tile_count() as i64;
        let pos = self.pos(them);
        if n <= 0 || pos < 0 {
            return 0;
        }
        (1..=steps)
            .filter(|i| {
                let t = ((pos + i) % n + n) % n;
                self.src.tile_owner(t as i32) as i64 == me
            })
            .count() as i64
    }
    fn between(&self, p: i64) -> i64 {
        let roll = self.move_roll();
        if roll < 0 {
            return 0;
        }
        let roll = roll.abs();
        let start = self.pos(p);
        let n = self.src.tile_count() as i64;
        if n <= 0 || start < 0 {
            return 0;
        }
        let backward = self.move_dir() < 0;
        (0..self.src.player_count() as i64)
            .filter(|&o| {
                if o == p || self.out(o) != 0 {
                    return false;
                }
                let q = self.pos(o);
                if q < 0 {
                    return false;
                }
                let fwd = ((q - start) % n + n) % n;
                let d = if backward { (n - fwd) % n } else { fwd };
                (1..=roll).contains(&d)
            })
            .count() as i64
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
    /// Data kind wire spelling for `tile` (ruleset data; empty = unknown).
    fn tile_kind_of(&self, tile: i32) -> String {
        let _ = tile;
        String::new()
    }
    /// ABI v56 generic tile queries (the host `tile_is` / `tile_attr` /
    /// `tile_in`). Defaults fall back safely for fixture worlds.
    fn tile_is(&self, tile: i32, kind: &str) -> bool {
        let _ = (tile, kind);
        false
    }
    fn tile_attr(&self, tile: i32, attr: &str) -> i32 {
        // Fixture fallback only -- `"house"` is `TileData.house` (build cost),
        // not the standing house count (`houses_of`).
        match attr {
            "price" => self.tile_price(tile),
            _ => {
                let _ = tile;
                0
            }
        }
    }
    fn tile_in(&self, player_id: i32, tile: i32, class: &str) -> bool {
        let _ = (player_id, tile, class);
        false
    }
    /// Fast path for `fill_tile_generic`: push every true `(player, tile,
    /// class)` triple for `tile` in one pass and return `true`, or return
    /// `false` to fall back to per-probe [`Self::tile_in`] (the default; a
    /// world says `true` only when it can answer exactly).
    fn tile_class_rows(
        &self,
        tile: i32,
        classes: &[String],
        out: &mut Vec<(i64, i64, String)>,
    ) -> bool {
        let _ = (tile, classes, out);
        false
    }
    fn player_count(&self) -> i32;
    fn money(&self, player_id: i32) -> i32;
    fn charge(&self, player_id: i32) -> i32;
    fn group_pool(&self, player_id: i32, name: &str) -> i32;
    fn group_pool_total(&self, player_id: i32) -> i32 {
        let _ = player_id;
        0
    }
    fn counter_names_at(&self, _uid: i32) -> Vec<String> {
        Vec::new()
    }
    fn hand_size(&self, player_id: i32) -> i32;
    fn player_pos(&self, player_id: i32) -> i32;
    fn player_out(&self, player_id: i32) -> i32;
    fn stay_of(&self, player_id: i32) -> i32;
    fn stun_of(&self, player_id: i32) -> i32;
    fn state_get(&self, player_id: i32, key: &str) -> i32;
    /// The seat's player-state `(key, value)` pairs (`MatchPlayer.state`),
    /// for the baked `state(p, 'key')` table. Live worlds walk the real map;
    /// the `CardWorld` blanket probes the well-known status keys via
    /// [`Self::state_get`] and omits zeros (so a test world that never sets
    /// state answers empty). Missing keys are simply absent -- `state(p, k)`
    /// answers 0 for them.
    fn state_entries(&self, player_id: i32) -> Vec<(String, i32)>;
    fn persona_skill_id(&self, player_id: i32) -> Option<String>;
    fn group_skill_id(&self, player_id: i32) -> Option<String>;
    /// The player's persona **name** -- the spelling `ctx::persona_is`
    /// matches. `persona_is(p, "名")` compares `id_of(name)` to this.
    fn persona_name(&self, player_id: i32) -> Option<String>;
    /// The player's group **name** -- the spelling `ctx::in_group` matches.
    /// `group_is(p, "Group")` compares `id_of(name)` to this.
    fn group_name(&self, player_id: i32) -> Option<String>;
    /// Named token counter (`ctx::tok`); 0 when absent.
    fn tok(&self, player_id: i32, name: &str) -> i32;
    /// Non-zero named token counters (`ctx::tok_names`); `tok('name')` missing = 0.
    fn tok_names(&self, player_id: i32, prefix: &str) -> Vec<String>;
    fn owned_count(&self, player_id: i32) -> i32;
    fn card_pool(&self, player_id: i32, card: &str, name: &str) -> i32;
    /// Named counter `name` on the instance of `card` at `player_id`
    /// (ruleset counter keys such as `"crystals"`, or any
    /// `FieldCard::counters` key). Missing = 0.
    fn card_counter(&self, player_id: i32, card: &str, name: &str) -> i32 {
        let _ = (player_id, card, name);
        0
    }
    /// Non-zero named counters on that instance (`card.counter('name')`;
    /// missing = 0). The two on-card wire names are always listed when
    /// non-zero. A host that tracks extra `FieldCard::counters` keys
    /// overrides this.
    fn card_counter_names(&self, player_id: i32, card: &str) -> Vec<String> {
        let mut out = Vec::new();
        for n in ["cp", "crystals"] {
            if self.card_counter(player_id, card, n) != 0 {
                out.push(n.to_string());
            }
        }
        out
    }
    fn tile_owner(&self, tile: i32) -> i32;
    fn houses_of(&self, tile: i32) -> i32;
    fn mortgaged_of(&self, tile: i32) -> i32;
    fn tile_price(&self, tile: i32) -> i32;
    fn turn_player(&self) -> i32;
    fn turn_key(&self) -> i32;
    /// This turn's fixed main-move roll (`ctx::fixed_roll()`); `-1` = unset.
    fn fixed_roll(&self) -> i32;
    /// Money-ins for `player_id` this turn (`ctx::gains_this_turn`).
    fn gains_this_turn(&self, player_id: i32) -> i32;
    /// Hostile targetings of `player_id` this turn (`ctx::targeted_count`).
    fn targeted_count(&self, player_id: i32) -> i32;
    /// Where the `card` instance on `player_id`'s field sits
    /// (`ctx::self_tile()`'s raw host value): `-2` = no such instance,
    /// `-1` = with its owner.
    fn card_tile(&self, player_id: i32, card: &str) -> i32;
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
    fn tile_kind_of(&self, tile: i32) -> String {
        CardWorld::tile_kind_of(self, tile)
    }
    #[inline]
    fn tile_is(&self, tile: i32, kind: &str) -> bool {
        CardWorld::tile_is(self, tile, kind)
    }
    #[inline]
    fn tile_attr(&self, tile: i32, attr: &str) -> i32 {
        CardWorld::tile_attr(self, tile, attr)
    }
    #[inline]
    fn tile_in(&self, player_id: i32, tile: i32, class: &str) -> bool {
        CardWorld::tile_in(self, player_id, tile, class)
    }
    #[inline]
    fn tile_class_rows(&self, tile: i32, classes: &[String], out: &mut Vec<(i64, i64, String)>) -> bool {
        CardWorld::tile_class_rows(self, tile, classes, out)
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
    fn charge(&self, player_id: i32) -> i32 {
        CardWorld::charge(self, player_id)
    }
    #[inline]
    fn group_pool(&self, player_id: i32, name: &str) -> i32 {
        CardWorld::group_pool(self, player_id, name)
    }
    fn group_pool_total(&self, player_id: i32) -> i32 {
        CardWorld::group_pool_total(self, player_id)
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
    fn state_entries(&self, player_id: i32) -> Vec<(String, i32)> {
        // No map to walk on a bare `CardWorld` -- probe the well-known status
        // keys via `state_get` and omit zeros. A test world that never sets
        // state answers empty; `state(p, k)` still answers 0 for anything
        // missing. Live worlds override this with the real `MatchPlayer.state`.
        const KNOWN: &[&str] = &[
            game_core::state::key::STAY,
            game_core::state::key::STUN,
            game_core::state::key::STUN_START,
            game_core::state::key::EXILE,
            game_core::state::key::EXILE_TO,
            game_core::state::key::EXILE_MAIN,
            game_core::state::key::CHARGE,
            game_core::state::key::NO_HAND,
            game_core::state::key::UNSTOPPABLE,
            game_core::state::key::HAND_LIMIT,
        ];
        KNOWN
            .iter()
            .filter_map(|k| {
                let v = CardWorld::state_get(self, player_id, k);
                if v != 0 {
                    Some((k.to_string(), v))
                } else {
                    None
                }
            })
            .collect()
    }
    #[inline]
    fn persona_skill_id(&self, player_id: i32) -> Option<String> {
        CardWorld::persona_skill_id(self, player_id)
    }
    #[inline]
    fn group_skill_id(&self, player_id: i32) -> Option<String> {
        CardWorld::group_skill_id(self, player_id)
    }
    #[inline]
    fn persona_name(&self, player_id: i32) -> Option<String> {
        CardWorld::persona_name(self, player_id)
    }
    #[inline]
    fn group_name(&self, player_id: i32) -> Option<String> {
        CardWorld::group_name(self, player_id)
    }
    #[inline]
    fn tok(&self, player_id: i32, name: &str) -> i32 {
        CardWorld::tok(self, player_id, name)
    }
    #[inline]
    fn tok_names(&self, player_id: i32, prefix: &str) -> Vec<String> {
        CardWorld::tok_names(self, player_id, prefix)
    }
    #[inline]
    fn owned_count(&self, player_id: i32) -> i32 {
        CardWorld::owned_count(self, player_id)
    }
    #[inline]
    fn card_pool(&self, player_id: i32, card: &str, name: &str) -> i32 {
        CardWorld::card_pool(self, player_id, card, name)
    }
    #[inline]
    fn card_counter(&self, player_id: i32, card: &str, name: &str) -> i32 {
        CardWorld::card_counter(self, player_id, card, name)
    }
    #[inline]
    fn card_counter_names(&self, player_id: i32, card: &str) -> Vec<String> {
        CardWorld::field_instances(self, player_id)
            .into_iter()
            .find(|(_, id)| id == card)
            .map(|(uid, _)| CardWorld::counter_names_at(self, uid))
            .unwrap_or_default()
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
    #[inline]
    fn fixed_roll(&self) -> i32 {
        CardWorld::fixed_roll(self)
    }
    #[inline]
    fn gains_this_turn(&self, player_id: i32) -> i32 {
        CardWorld::gains_this_turn(self, player_id)
    }
    #[inline]
    fn targeted_count(&self, player_id: i32) -> i32 {
        CardWorld::targeted_count(self, player_id)
    }
    #[inline]
    fn card_tile(&self, player_id: i32, card: &str) -> i32 {
        // First field instance of this card id, same name-based lookup as
        // `card_pool`. `tile_at` answers `-2` when the uid is gone.
        CardWorld::field_instances(self, player_id)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-2, |(uid, _)| CardWorld::tile_at(self, uid))
    }
}

/// Does this condition's **verdict** depend on hand contents? A counteraction
/// window's only world change is declarations removing cards from hands
/// (`build_round`), so
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

/// Build the window snapshot once per trigger / chain window and reuse it
/// across every candidate probe in that window ([`WindowScope`]). This is
/// where the G1 savings live: ~48 900 counteract probes share ~250 windows.
pub fn fill_window<S: SnapSrc>(world: &S) -> WindowCtx {
    let t = world.trigger();
    let tile_ids = collect_tile_ids(world);
    let mut tile_kinds = Vec::new();
    let mut tile_attrs = std::collections::BTreeMap::new();
    let mut tile_classes = Vec::new();
    for tile in 0..world.tile_count() {
        // Generic tile queries (kind / attr / class tables).
        fill_tile_generic(world, tile, &mut tile_kinds, &mut tile_attrs, &mut tile_classes);
    }

    let players: Vec<PlayerSnap> = (0..world.player_count())
        .map(|p| PlayerSnap {
            money: world.money(p) as i64,
            charge: world.charge(p) as i64,
            group_pool: world.group_pool_total(p) as i64,
            hand: world.hand_size(p) as i64,
            pos: world.player_pos(p) as i64,
            out: world.player_out(p) as i64,
            stay: world.stay_of(p) as i64,
            stun: world.stun_of(p) as i64,
            exile: world.state_get(p, game_core::state::key::EXILE) as i64,
            no_hand: world.state_get(p, game_core::state::key::NO_HAND) as i64,
            state: world
                .state_entries(p)
                .into_iter()
                .map(|(k, v)| (k, v as i64))
                .collect(),
            persona: id_of(&world.persona_name(p).unwrap_or_default()),
            group: id_of(&world.group_name(p).unwrap_or_default()),
            tiles: world.owned_count(p) as i64,
            gains: world.gains_this_turn(p) as i64,
            targeted: world.targeted_count(p) as i64,
        })
        .collect();

    let chain: Vec<ChainLink> = t
        .effects
        .iter()
        .map(|e| ChainLink {
            kind: crate::TriggerKind::from_str(e.kind).as_i32() as i64,
            from: e.from as i64,
            hits: e.target as i64,
        })
        .collect();

    let mv = MoveSnap {
        // Match the guest's `trigger::move_roll()` (`card-sdk/ctx.rs`): a
        // negative face is the pre-cast sentinel (`value = -1` on the `roll`
        // pre-trigger) and reads as `null`, so `move.roll != null` means "a
        // real face exists" exactly as the guards wrote it.
        roll: t
            .mv
            .as_ref()
            .and_then(|m| m.roll)
            .filter(|&r| r >= 0)
            .map(|r| r as i64),
        kind: t.mv.as_ref().map(|m| m.kind as i64),
        remaining: t.mv.as_ref().map(|m| m.remaining).unwrap_or(0) as i64,
        // `t.Move.Main` (`trigger::move_is_main()` in the guest) lives on the
        // trigger itself, not the move payload.
        main: t.main,
        // `t.Move.Dir` (`trigger::move_dir()` in the guest) -- 1 forward,
        // -1 backward; the trigger default is 1 when no move caused it.
        dir: t.mv.as_ref().map(|m| m.dir.as_i32() as i64).unwrap_or(1),
        tags: t
            .mv
            .as_ref()
            .map(|m| m.tags.iter().map(|(k, v)| (k.clone(), *v as i64)).collect())
            .unwrap_or_default(),
    };

    let tile = TileSnap {
        id: t.tile as i64,
        owner: if t.tile >= 0 { world.tile_owner(t.tile) as i64 } else { -1 },
        houses: if t.tile >= 0 { world.houses_of(t.tile) as i64 } else { 0 },
        mortgaged: if t.tile >= 0 { world.mortgaged_of(t.tile) as i64 } else { 0 },
        price: if t.tile >= 0 { world.tile_price(t.tile) as i64 } else { 0 },
    };

    WindowCtx {
        kind: t.kind.as_i32() as i64,
        actor: t.player_id as i64,
        target: t.target as i64,
        tile,
        value: t.value as i64,
        step: t.step as i64,
        by: t.by_card.map(|b| b as i64).unwrap_or(-1),
        pay_is_rent: t.pay.map(|p| p.is_rent).unwrap_or(false),
        mv,
        roll_source: t.roll_source.as_i32() as i64,
        // `abnormal` is the AbnormalGate window, not "this link was negated".
        // There is no `t.Abnormal` bool on `Trigger`; the closest signal is the
        // trigger kind itself.
        abnormal: matches!(t.kind, crate::TriggerKind::Abnormal),
        chain,
        turn_player: world.turn_player() as i64,
        turn_key: world.turn_key() as i64,
        trigger_card: match t.card.as_deref() {
            Some(card) if !card.is_empty() => id_of(card),
            _ => 0,
        },
        name: t.name.clone().unwrap_or_default(),
        players,
        tile_ids,
        // Field names come from `rules_cond::ctx::WindowCtx` (out of scope).
        tile_kinds,
        tile_attrs,
        tile_classes,
        tile_count: world.tile_count() as i64,
        tile_owners: (0..world.tile_count())
            .map(|t| world.tile_owner(t) as i64)
            .collect(),
        plan_fixed_roll: (world.fixed_roll() >= 0).then(|| world.fixed_roll() as i64),
    }
}

/// Play-gate window (docs/GUARDS.md §4.4 item 3): `cant_play` / skill gates have
/// **no** `Trigger` to source a window from -- same schema with `kind` absent
/// (`TriggerKind::None`). Everything else is ambient world state, actor = the
/// player being asked. Built once per ask; reused across the cards of one view.
pub fn fill_window_ambient<S: SnapSrc>(world: &S, player_id: i32) -> WindowCtx {
    let tile_ids = collect_tile_ids(world);
    let mut tile_kinds = Vec::new();
    let mut tile_attrs = std::collections::BTreeMap::new();
    let mut tile_classes = Vec::new();
    for tile in 0..world.tile_count() {
        fill_tile_generic(world, tile, &mut tile_kinds, &mut tile_attrs, &mut tile_classes);
    }
    let players: Vec<PlayerSnap> = (0..world.player_count())
        .map(|p| PlayerSnap {
            money: world.money(p) as i64,
            charge: world.charge(p) as i64,
            group_pool: world.group_pool_total(p) as i64,
            hand: world.hand_size(p) as i64,
            pos: world.player_pos(p) as i64,
            out: world.player_out(p) as i64,
            stay: world.stay_of(p) as i64,
            stun: world.stun_of(p) as i64,
            exile: world.state_get(p, game_core::state::key::EXILE) as i64,
            no_hand: world.state_get(p, game_core::state::key::NO_HAND) as i64,
            state: world
                .state_entries(p)
                .into_iter()
                .map(|(k, v)| (k, v as i64))
                .collect(),
            persona: id_of(&world.persona_name(p).unwrap_or_default()),
            group: id_of(&world.group_name(p).unwrap_or_default()),
            tiles: world.owned_count(p) as i64,
            gains: world.gains_this_turn(p) as i64,
            targeted: world.targeted_count(p) as i64,
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
        // No trigger: no card. `trigger_card == card.id` is false here.
        trigger_card: 0,
        // No trigger: no counter / message name either.
        name: String::new(),
        players,
        tile_ids,
        // Field names come from `rules_cond::ctx::WindowCtx` (out of scope).
        tile_kinds,
        tile_attrs,
        tile_classes,
        tile_count: world.tile_count() as i64,
        tile_owners: (0..world.tile_count())
            .map(|t| world.tile_owner(t) as i64)
            .collect(),
        plan_fixed_roll: (world.fixed_roll() >= 0).then(|| world.fixed_roll() as i64),
    }
}

/// ABI v56: fill one tile's kind / attr / class rows for the window snapshot.
fn fill_tile_generic<S: SnapSrc>(
    world: &S,
    tile: i32,
    tile_kinds: &mut Vec<String>,
    tile_attrs: &mut std::collections::BTreeMap<(i64, String), i64>,
    tile_classes: &mut Vec<(i64, i64, String)>,
) {
    // Kind: ruleset-supplied wire spelling (no engine kind table).
    tile_kinds.push(world.tile_kind_of(tile));

    // Attrs: the seven documented fields.
    for attr in ["group", "tier", "price", "house", "area", "rent", "index"] {
        tile_attrs.insert((tile as i64, attr.to_string()), world.tile_attr(tile, attr) as i64);
    }

    // Classes: the ruleset vocabulary plus `color:<g>` for groups present on
    // the board. `player = -1` plus each live seat.
    let classes = class_names();
    if world.tile_class_rows(tile, &classes, tile_classes) {
        return;
    }
    let players: Vec<i32> = std::iter::once(-1)
        .chain(0..world.player_count())
        .collect();
    for &p in &players {
        for c in &classes {
            if world.tile_in(p, tile, c.as_str()) {
                tile_classes.push((p as i64, tile as i64, c.clone()));
            }
        }
    }
}

/// The class vocabulary `tile_in` is probed over (built once).
fn class_names() -> Vec<String> {
    // Ruleset-registered class names, plus the generic colour classes.
    let mut classes: Vec<String> = crate::tile_class::class_names();
    for g in -2..=32 {
        classes.push(format!("color:{g}"));
    }
    classes
}

/// Every board name -> id, so `tile_named('the game车站')` / `tile_named('the start square')`
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
    // Legacy §2.1 sample aliases (ruleset tile-name bindings): resolve through
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
        owner_charge: world.charge(p) as i64,
        // "crystals" is the ruleset's named-counter key for the group pool.
        owner_group_pool: world.group_pool(p, "crystals") as i64,
        owner_hand: world.hand_size(p) as i64,
        owner_pos: world.player_pos(p) as i64,
        owner_out: world.player_out(p) as i64,
        owner_stay: world.stay_of(p) as i64,
        owner_stun: world.stun_of(p) as i64,
        owner_exile: world.state_get(p, game_core::state::key::EXILE) as i64,
        owner_no_hand: world.state_get(p, game_core::state::key::NO_HAND) as i64,
        owner_persona: id_of(&world.persona_name(p).unwrap_or_default()),
        owner_group: id_of(&world.group_name(p).unwrap_or_default()),
        owner_tiles: world.owned_count(p) as i64,
        card_id: id_of(card),
        card_placed: placed,
        card_counters: world
            .card_counter_names(p, card)
            .into_iter()
            .map(|n| {
                let v = world.card_counter(p, card, &n) as i64;
                (n, v)
            })
            .collect(),
        card_tile: if placed {
            let t = world.card_tile(p, card);
            (t >= -1).then_some(t as i64)
        } else {
            None
        },
        slots: Default::default(),
        // Non-zero named counters only: `tok('name')` missing = 0, so the
        // zero entries are exactly the absent ones. Same data
        // `ctx::tok_names` / `ctx::tok` read.
        tok_names: world
            .tok_names(p, "")
            .into_iter()
            .map(|n| {
                let v = world.tok(p, &n) as i64;
                (n, v)
            })
            .collect(),
        blocked_groups: Vec::new(),
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
    // `no_road` designated rent-receiver seat (`ctx::set_slot(player_id, "noRoad.who", who)`).
    "noRoad.who",
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
    // Standard player-state keys used by skill body-top checks.
    "skillState",
    "charge",
    "skill.aveMujica.inTwo",
    "skill.ras.taken",
    "skill.roselia.first",
    "skill.sumimi.got",
    "skill.asahiAim.first",
    "skill.kasumiGroup.start",
    "skill.kanonLost.silent",
    "skill.soyoClear.armed",
    "skill.soyoClear.penalty",
    "skill.tamade.builtLast",
    "skill.mumeiStream.state",
    "skill.mutsumiActor.state",
    "skill.kaedeSupport.state",
    "skill.sakikoLife.state",
    "skill.uikaImprisoned.state",
    "skill.kokoroPractice.waived",
    "skill.misakiOther.charge",
    "skill.uikaIdol.charge",
    "skill.takiCrychic.armed",
    // `soyo_crychic` teleport one-shot flag (`state::set(player_id, ARMED, 1)`).
    "skill.soyoCrychic.armed",
    "skill.tamade.builtThis",
    // `kaoru_thief` mark-turn latch (`ctx::set_slot(player_id, TURN, turn_key)`).
    "kaoru_thief_turn",
    // `umiri_card` user seat+1 (`ctx::set_slot(holder, USER_KEY, user + 1)`).
    "umiri_user",
    // `dream_ahead` overflow counter (`ctx::inc_slot(player_id, SLOT_X, 1)`).
    "dream_ahead_x",
    // `detour` next-roll 1d6 arm (`state::set(player_id, NEXT_ROLL, 1)`); the
    // `On::RollPlan` condition is `slot('detour_next_roll') != 0`.
    "detour_next_roll",
    // `sayo_thorns` / `sayo_play` once-per-turn lock (`state::set(player_id, USED, turn_key)`).
    "skill.sayoThorns.used",
    // `mumei_streamer` flip count (`state::set(player_id, FLIPPED, n)`).
    "skill.mumei.flipped",
    // `hina_lottery` borrowed-skill index (`state::set(player_id, BORROWED, k)`).
    "skill.hinaLottery.borrowed",
    // `arisa_bonsai` passed-station latch (`ctx::set_slot(player_id, …, 1)`).
    "skill.arisa.passedStation",
    // `uika_imprisoned` 1d6 bonus count (`state::set(player_id, BONUS, n)`).
    "skill.uikaImprisoned.bonus",
    // `star_beat` banked build layers (`ctx::inc_slot(player_id, "star_beat.layers", ±1)`).
    "star_beat.layers",
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

/// [`window_scope`] for a snapshot built just for it (no copy of the tables).
pub fn window_scope_owned(win: WindowCtx) -> WindowScope {
    WindowScope::from_owned(win)
}