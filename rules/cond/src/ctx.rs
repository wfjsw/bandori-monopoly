//! Pure-data contexts. No dependency on `game-core` / `card-sdk` types: the
//! host (G2) fills these from `Trigger` + effect-link list + `World` in one
//! place. Choosing structs over a host-implemented trait keeps the hot
//! `Cond::eval` free of dynamic dispatch, lets a window scope borrow them
//! without generics, and makes the "built once per window" story literal --
//! a `WindowCtx` value *is* the amortised snapshot.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One link of the effect chain a window is answering (`effect::*` in the
/// guest imports, `chain.*` in GUARDS.md §4.2 -- same flat list).
///
/// All snapshots here derive `Serialize`/`Deserialize` with
/// `#[serde(default)]`: a host (or a test) may fill any subset of the fields
/// from JSON and the rest take [`Default`] -- the web-glue `ruleset_pre_eval`
/// seam is built on that.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChainLink {
    /// Trigger / chain kind wire value (see [`crate::kinds`]).
    pub kind: i64,
    /// Seat the link comes from (`-1` = none).
    pub from: i64,
    /// Seat the link hits (`-1` = none / not seat-scoped).
    pub hits: i64,
}

/// Snapshot of one seat's stats, indexed by seat id. Used by the `money(p)` /
/// `fire(p)` / `character_is(p, …)` family so a condition can talk about a
/// seat that is not the candidate's owner.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerSnap {
    pub money: i64,
    pub fire: i64,
    pub crystals: i64,
    pub hand: i64,
    pub pos: i64,
    /// 1 = out of the game.
    pub out: i64,
    pub stay: i64,
    pub stun: i64,
    pub exile: i64,
    /// 1 = 「不可持有手牌」.
    pub no_hand: i64,
    pub character: i64,
    pub band: i64,
    /// Count of tiles the seat owns.
    pub tiles: i64,
}

/// The tile the trigger is about. Field access is `tile.owner`, `tile.houses`, …
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TileSnap {
    pub id: i64,
    /// Owning seat (`-1` = unowned / bank).
    pub owner: i64,
    pub houses: i64,
    /// 1 = mortgaged.
    pub mortgaged: i64,
    pub price: i64,
}

/// The move a window is answering (`move.*`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MoveSnap {
    /// Face shown, if any. `None` binds CEL `null` (`move.roll != null`).
    pub roll: Option<i64>,
    /// [`crate::kinds::mv`] value; `None` when the window has no move.
    pub kind: Option<i64>,
    pub remaining: i64,
    /// `t.Move.Main` -- was this the turn's main move (`move.main`)? `false`
    /// when the window has no move.
    pub main: bool,
    /// `t.Move.Dir` (`move.dir`) -- 1 forward, -1 backward. `1` when the
    /// window has no move (the trigger default, matching the guest).
    pub dir: i64,
    /// `t.Move.Tags` (`move.tag(name)`) -- per-card counters on the move.
    pub tags: Vec<(String, i64)>,
}

impl Default for MoveSnap {
    fn default() -> Self {
        Self {
            roll: None,
            kind: None,
            remaining: 0,
            main: false,
            // Matches `Trigger::new`'s `move_dir: 1` -- forward when no move
            // caused the window (the guest's `trigger::move_dir()` reads 1).
            dir: 1,
            tags: Vec::new(),
        }
    }
}

/// Built **once per trigger / chain window** (the `declare_one` / `run_hook`
/// call) and reused by every candidate probe in that window. This is where
/// the savings of G1 live: 48 900 counteract probes share ~250 windows.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowCtx {
    /// Trigger / chain kind wire value of this window (see [`crate::kinds`]);
    /// `kind` in conditions (`trigger.kind` is an alias).
    pub kind: i64,
    /// Trigger actor seat (`-1` = none).
    pub actor: i64,
    /// Trigger target seat (`-1` = none).
    pub target: i64,
    pub tile: TileSnap,
    pub value: i64,
    pub step: i64,
    /// `by_card` owner seat (`-1` = none).
    pub by: i64,
    pub pay_is_rent: bool,
    pub mv: MoveSnap,
    /// Where a roll came from (`-1` = none). Binds `roll_source`.
    pub roll_source: i64,
    pub abnormal: bool,
    /// Flat effect-link list (same data the `effect::*` imports read).
    pub chain: Vec<ChainLink>,
    pub turn_player: i64,
    /// Once-per-turn key; compared against `slot('…')`.
    pub turn_key: i64,
    /// The trigger's card id hash (`trigger_card`), same encoding as
    /// [`CandidateCtx::card_id`]. `0` when the trigger carries no card.
    pub trigger_card: i64,
    /// Index = seat id.
    pub players: Vec<PlayerSnap>,
    /// `tile_named(name) -> id`, resolved at load. Empty is fine if no
    /// condition uses it.
    pub tile_ids: BTreeMap<String, i64>,
    /// Tile ids of each kind, for `is_circle(t)` / `is_ring(t)` /
    /// `is_live_house(t)` / `is_buyable(t)`.
    pub circle_tiles: Vec<i64>,
    pub ring_tiles: Vec<i64>,
    pub live_house_tiles: Vec<i64>,
    pub buyable_tiles: Vec<i64>,
}

impl WindowCtx {
    /// Seat count used by `neighbor(p, delta)`.
    pub fn seat_count(&self) -> i64 {
        self.players.len() as i64
    }

    pub fn player(&self, seat: i64) -> Option<&PlayerSnap> {
        if seat < 0 {
            return None;
        }
        self.players.get(seat as usize)
    }
}

/// Per-candidate overlay: the card + seat being probed. Built per (card, seat)
/// inside a window -- much cheaper than a window, so it is allowed to be a
/// fresh value each time.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CandidateCtx {
    /// Seat that owns the candidate (`player_id` arg of the host probe).
    pub owner: i64,
    /// Owner overlay. Same numbers as `window.players[owner]`, duplicated so
    /// `owner.money` binds without a lookup. The host fills both from one
    /// source; they must agree.
    pub owner_money: i64,
    pub owner_fire: i64,
    pub owner_crystals: i64,
    pub owner_hand: i64,
    pub owner_pos: i64,
    pub owner_out: i64,
    pub owner_stay: i64,
    pub owner_stun: i64,
    pub owner_exile: i64,
    pub owner_no_hand: i64,
    pub owner_character: i64,
    pub owner_band: i64,
    pub owner_tiles: i64,
    /// `card.*`
    pub card_id: i64,
    pub card_placed: bool,
    pub card_cp: i64,
    /// `slot(name)` lookup. Missing name evaluates to `0` (matches the
    /// "unset slot" guards such as `slot('asUsualTurn') != turn_key`).
    pub slots: BTreeMap<String, i64>,
    /// `tok('name')` lookup -- the owner's named token counters, the same
    /// data `ctx::tok(player_id, name)` reads. Missing name = `0`.
    pub tok_names: BTreeMap<String, i64>,
    /// Bands whose skill is blocked for this candidate (`blocked(band)`).
    pub blocked_bands: Vec<i64>,
}

impl CandidateCtx {
    pub fn slot(&self, name: &str) -> i64 {
        self.slots.get(name).copied().unwrap_or(0)
    }

    pub fn tok_named(&self, name: &str) -> i64 {
        self.tok_names.get(name).copied().unwrap_or(0)
    }

    pub fn blocked(&self, band: i64) -> bool {
        self.blocked_bands.contains(&band)
    }
}