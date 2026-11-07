//! General symbolic world model for Kani proofs.
//!
//! Fixed-capacity so CBMC unwinds every loop. Game vocabulary lives here
//! (this crate is game-specific), not in card-sdk.
//!
//! Design notes
//! ------------
//! * **Players** — 4 slots; proofs `assume` `n_players ∈ 2..=4` and only
//!   touch `0..n_players`.
//! * **Named counters / tokens / state** — generic key→i32 maps keyed by
//!   interned [`crate::intern`] ids (u16). The pilot's `fans_pos` /
//!   `fans_neg` views mirror the `ppFansUp` / `ppFansDown` state-dict keys;
//!   `hand_resonance` remains a view so existing proofs compile.
//! * **Piles** — hand / draw / discard / field are bounded arrays of interned
//!   card ids (`u16`), with uid / crystals / props per field instance.
//! * **Tiles** — 60 board slots from `data/board.json` ([`board`]), each with
//!   owner / level / mortgage.
//! * **RNG** — symbolic dice: `kani::any` within the die range.
//! * **Prompts** — answered symbolically in-host (`ask_*` → `kani::any`
//!   bounded by the offered option count).
//! * **Effect log** — every gain/lose/pay/move/teleport/draw/discard/token/
//!   status/mark change is a typed record. Narration is a separate counter
//!   (the book's frame is about game effects).

use crate::board;
use crate::intern::{self, NameId};

// ---------------------------------------------------------------------------
// Bounds
// ---------------------------------------------------------------------------

pub const MAX_PLAYERS: usize = 4;
/// Keep at 24: `proofs/money_shave.rs` (owned by another agent) types
/// `let logbuf: [Effect; 24] = …`. Do not change without coordinating.
pub const MAX_LOG: usize = 24;
pub const MAX_NARRATION: usize = 8;
pub const MAX_ANSWERS: usize = 8;
/// Bounded pile / field arrays (interned card ids) and the pilot bitsets.
pub const MAX_CARDS: usize = 8;
/// Generic named counters (tokens, state keys, slots).
///
/// **Direct-index by `NameId.0`** (`0..SLOTS`, 64), not a linear scan.
/// The old `MAX_KEYS=8` walk was the pp-unit unwind cliff
/// (`NameMap::get_opt` 144 + `NameMap::set` 80 unrolls in
/// `same_dream_01_band_plus_3_iff`). Intern ids are `u16` in `0..32`
/// (`intern::MAX_INTERN`); `NameId::INVALID` and any out-of-range id read
/// as missing and are a silent no-op on `set` (never alias).
///
/// `MAX_KEYS` is kept as the **documented per-map key budget** for
/// conformance diffs (the engine's `NameMap` is also 8). The model holds
/// up to `SLOTS` distinct keys; a proof that needs more than
/// `MAX_KEYS` live keys is still allowed (strictly more room than master).
pub const MAX_KEYS: usize = 8;
/// Field card instances (uid + card id + props).
pub const MAX_FIELD: usize = 8;
/// Open invocation frames (the nested guest cannot run under Kani, so the host
/// records the call facts instead -- `G13` §5.5). `MAX_NESTING` is 8.
pub const MAX_INVOKE: usize = 8;

/// One recorded invocation frame: `(card, entry, by, by_instance, instance,
/// paid, depth)`.
pub type InvokeRecord = (NameId, NameId, i32, i32, i32, bool, i32);
/// Tile marks.
pub const MAX_MARKS: usize = 16;
/// Active event ids.
pub const MAX_EVENTS: usize = 8;
/// Deferred `event` records (`event_defer`).
pub const MAX_DEFERRED: usize = 2;
/// World snapshot slots (`world_snapshot`).
pub const MAX_SNAPSHOTS: usize = 4;
/// Owners allowed on one snapshot key.
pub const MAX_SNAP_OWNERS: usize = 2;

// ---------------------------------------------------------------------------
// Interned vocabulary used by existing proofs (stable names)
// ---------------------------------------------------------------------------

pub const CARD_AYA: &str = "PP:[丸山彩]憧憬的前方";
pub const CARD_RESONANCE: &str = "PP:[衍生]共鸣";
pub const CARD_OTHER: &str = "PP:[标题偶像]";
pub const CARD_EXTRA: &str = "PP:[强韧之花]";

/// Map a well-known pilot card name to a small id (0..4) for the bitset
/// fields kept for compatibility. Unknown names map to `2` (`CARD_OTHER`).
pub fn card_id_str(id: u8) -> &'static str {
    match id {
        0 => CARD_AYA,
        1 => CARD_RESONANCE,
        2 => CARD_OTHER,
        3 => CARD_EXTRA,
        _ => CARD_OTHER,
    }
}

pub fn card_slot_of(name: &str) -> u8 {
    card_slot_of_bytes(name.as_bytes())
}

/// Byte-level slot lookup (no UTF-8 validation).
/// Zero-loop compare: `while i < e.len()` over a 29-byte CJK id was an
/// unwind cliff (the host `memcmp` family).
pub fn card_slot_of_bytes(b: &[u8]) -> u8 {
    fn eq(a: &[u8], lit: &str) -> bool {
        let e = lit.as_bytes();
        if a.len() != e.len() || a.len() > 64 {
            return false;
        }
        let mut ok = true;
        macro_rules! step {
            ($i:expr) => {
                // `.get`, not `[i]`: no slice_index_fail panic path.
                match (a.get($i), e.get($i)) {
                    (Some(x), Some(y)) if x != y => ok = false,
                    _ => {}
                }
            };
        }
        step!(0);
        step!(1);
        step!(2);
        step!(3);
        step!(4);
        step!(5);
        step!(6);
        step!(7);
        step!(8);
        step!(9);
        step!(10);
        step!(11);
        step!(12);
        step!(13);
        step!(14);
        step!(15);
        step!(16);
        step!(17);
        step!(18);
        step!(19);
        step!(20);
        step!(21);
        step!(22);
        step!(23);
        step!(24);
        step!(25);
        step!(26);
        step!(27);
        step!(28);
        step!(29);
        step!(30);
        step!(31);
        step!(32);
        step!(33);
        step!(34);
        step!(35);
        step!(36);
        step!(37);
        step!(38);
        step!(39);
        step!(40);
        step!(41);
        step!(42);
        step!(43);
        step!(44);
        step!(45);
        step!(46);
        step!(47);
        step!(48);
        step!(49);
        step!(50);
        step!(51);
        step!(52);
        step!(53);
        step!(54);
        step!(55);
        step!(56);
        step!(57);
        step!(58);
        step!(59);
        step!(60);
        step!(61);
        step!(62);
        step!(63);
        ok
    }
    if eq(b, CARD_AYA) {
        0
    } else if eq(b, CARD_RESONANCE) {
        1
    } else if eq(b, CARD_OTHER) {
        2
    } else if eq(b, CARD_EXTRA) {
        3
    } else {
        2
    }
}

/// Per-player state-dict keys (`skill_groups::fans::{UP,DOWN}`). Owned by no
/// card; grants/flips go through `fans::` helpers. Same keys the CEL
/// `slot('ppFansUp')` / `slot('ppFansDown')` read.
pub const TOK_PP_POS: &str = "ppFansUp";
pub const TOK_PP_NEG: &str = "ppFansDown";
/// Band skill that listens `On::Action(FansFlip)` for clause （2） overflow.
pub const TOK_PP_BAND: &str = "skill:Pastel✽Palettes:与偶像一起";

/// Cached `NameId`s for the two fan tokens. Interned once per proof (the
/// same table `host::install` pre-fills); every later `sync_views` is a `u16`
/// compare, not a hash.
fn fan_tok_ids() -> (NameId, NameId) {
    static mut IDS: Option<(NameId, NameId)> = None;
    unsafe {
        if IDS.is_none() {
            IDS = Some((intern::intern(TOK_PP_POS), intern::intern(TOK_PP_NEG)));
        }
        IDS.unwrap()
    }
}

/// Cached `NameId` for `RENT_HOUSES` (see [`World::rent_houses`]).
fn rent_houses_id() -> NameId {
    static mut ID: Option<NameId> = None;
    unsafe {
        if ID.is_none() {
            ID = Some(intern::intern(RENT_HOUSES));
        }
        ID.unwrap()
    }
}

/// Cached `NameId` for `PROP_LEAVE_DEST` (see [`World::leave_dest`]).
fn leave_dest_id() -> NameId {
    static mut ID: Option<NameId> = None;
    unsafe {
        if ID.is_none() {
            ID = Some(intern::intern(PROP_LEAVE_DEST));
        }
        ID.unwrap()
    }
}

/// `game_core::state::prop::RENT_HOUSES` — house-count override for rent.
pub const RENT_HOUSES: &str = "rentHouses";

/// `game_core::state::prop::LEAVE_DEST` — the destination an instance chose
/// for its own leave (design G08 §7.1 step 2 / I4).
pub const PROP_LEAVE_DEST: &str = "leaveDest";

pub const TILE_SHOP_NAME: &str = "江户川乐器店";
/// Named on-card counters (`game_core::state::counter`).
pub const COUNTER_CRYSTALS: &str = "crystals";
pub const COUNTER_CP: &str = "cp";
/// The pilot used `TILE_SHOP = 3` against a fake 8-tile board; keep that
/// constant for `teleport.rs` / `spec.rs` compatibility and also expose the
/// real board index.
pub const TILE_SHOP: i32 = 3;
/// Real-engine 0-based index of 「江户川乐器店」 (`board.json` index 31 →
/// position 30). Matches `game_core` `tile_named` = `Vec::position`.
pub const TILE_SHOP_BOARD: i32 = 30;
pub const TILE_COUNT: i32 = 60;

// ---------------------------------------------------------------------------
// Trigger payload (move / buy / deal) — mirrors `game_rules::world::Trigger*
// `map_or` sentinels in `hostfns.rs`:
//   no move  → kind=-1, roll=-1, dir=1, from=-1, remaining=0, total=0, resolve=0
//   no buy   → kind=0, seller=-1, price=0, deal_owner=-1, deal_houses=0,
//              deal_mortgaged=0
// ---------------------------------------------------------------------------

/// The move that caused a trigger. `present == false` is "no move" and the
/// getter ops answer the documented sentinels above (see `hostfns`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MoveSnap {
    /// `MoveKind`: 0 walk / 1 teleport. Stored as i32 so `World` stays `Copy`.
    pub kind: i32,
    /// Does the move settle where it lands?
    pub settles: bool,
    /// Direction: 1 forward / -1 backward (guest + CEL encoding).
    pub dir: i32,
    /// 移动起点 (origin tile, 0-based).
    pub from: i32,
    /// Steps the move has left to walk.
    pub remaining: i32,
    /// Path length.
    pub total: i32,
    pub roll: i32,
    /// Per-card counters on the move (a [火罐] roll is card-owned state).
    pub tags: NameMap,
    /// Was this settle initiated by a card (`settle_tile_at`)?
    pub settle_redirect: bool,
    /// The walk was rolled (`move.rolled`); a teleport or a card-driven hop is
    /// not.
    pub rolled: bool,
    /// The steps the move actually took (`move.effective`): the fixed count,
    /// else `next_steps`, else the roll.
    pub effective: i32,
    /// An abnormal landed somewhere in this walk's span
    /// (`trigger::move_abnormal`, `MoveCtx.abnormal`).
    pub abnormal: bool,
}

impl MoveSnap {
    pub const NONE: MoveSnap = MoveSnap {
        kind: -1,
        settles: false,
        dir: 1,
        from: -1,
        remaining: 0,
        total: 0,
        roll: -1,
        tags: NameMap::new(),
        settle_redirect: false,
        rolled: false,
        effective: 0,
        abnormal: false,
    };

    /// A move payload is present when kind is a real `MoveKind` (0/1).
    /// `hostfns` uses `trigger().mv.map_or(sentinel, …)` — `NONE` is the
    /// no-move case.
    pub fn present(&self) -> bool {
        self.kind >= 0
    }
}

/// The purchase a buy trigger is about (`docs/PURCHASE.md`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BuySnap {
    /// `BuyKind`: 0 land / 1 agent / 2 card / 3 force / 4 acquire / 5 auction.
    pub kind: i32,
    /// Payee (guest encoding: `-1` = bank, else seat).
    pub seller: i32,
    /// Price the buyer would be charged (post BuyAdd/Mul/Set).
    pub price: i32,
    /// Ownership after the deal. `-1` = keep the default (the buyer).
    pub deal_owner: i32,
    /// Houses after the deal.
    pub deal_houses: i32,
    /// Mortgage after the deal.
    pub deal_mortgaged: bool,
}

impl BuySnap {
    /// "No buy" — matches every `trigger().buy.map_or(default, …)` in hostfns.
    pub const NONE: BuySnap = BuySnap {
        kind: 0,
        seller: -1,
        price: 0,
        deal_owner: -1,
        deal_houses: 0,
        deal_mortgaged: false,
    };
}

/// `RollSource` guest codes (`hostfns::trig_roll_source` / `rules.rs`).
pub const ROLL_UNATTRIBUTED: i32 = 0;
pub const ROLL_CHARGE: i32 = 1;
pub const ROLL_CARD: i32 = 2;
pub const ROLL_SKILL: i32 = 3;

// ---------------------------------------------------------------------------
// Effect log
// ---------------------------------------------------------------------------

/// A requested game-state change. Narration (`ctx::log`) is recorded
/// separately -- the book's frame is about effects, not log lines.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    SetPayAmount {
        to: i32,
    },
    Teleport {
        player: i32,
        tile: i32,
    },
    GainFire {
        player: i32,
        n: i32,
    },
    GiveStay {
        player: i32,
        n: i32,
    },
    GiveStun {
        player: i32,
        n: i32,
    },
    GiveExile {
        player: i32,
        n: i32,
        to: i32,
    },
    /// `give_extra_turn` — queue a second turn (scheduling is PROTOCOL).
    GiveExtraTurn {
        player: i32,
    },
    PlaceCard {
        player: i32,
        dest: i32,
        card: u8,
    },
    TakeFromPile {
        player: i32,
        pile: i32,
        card: u8,
    },
    AddToHand {
        player: i32,
        card: u8,
    },
    DiscardFromHand {
        player: i32,
        card: u8,
    },
    Draw {
        player: i32,
        n: i32,
    },
    /// `ctx::draw_over_hand_limit` -- `ruleset_draw` with
    /// [`card_sdk::abi::draw_flag::OVER_HAND_LIMIT`]. The model has no hand
    /// limit, so the flag is only observable here.
    DrawOver {
        player: i32,
        n: i32,
    },
    /// `inspection_begin` -- `n` cards turned from `owner`'s deck.
    InspectionHold {
        looker: i32,
        owner: i32,
        n: i32,
    },
    /// `inspection_end` -- `n` cards returned to `owner`'s deck.
    InspectionReturn {
        owner: i32,
        n: i32,
    },
    /// `reveal_pile` / `reveal_card` -- a publication, no pile write.
    Reveal {
        player: i32,
        pile: i32,
    },
    GainMoney {
        player: i32,
        n: i32,
    },
    PayMoney {
        player: i32,
        n: i32,
    },
    PayTo {
        from: i32,
        to: i32,
        n: i32,
    },
    AddToken {
        player: i32,
        key: u16,
        n: i32,
    },
    SetToken {
        player: i32,
        key: u16,
        n: i32,
    },
    AddCounter {
        owner: i32,
        key: u16,
        n: i32,
    },
    SetState {
        player: i32,
        key: u16,
        n: i32,
    },
    SetOwner {
        tile: i32,
        player: i32,
    },
    SetHouses {
        tile: i32,
        n: i32,
    },
    SetMortgaged {
        tile: i32,
        on: bool,
    },
    PlaceMark {
        tile: i32,
        key: NameId,
    },
    EventExpire {
        key: NameId,
    },
    UnplaceCard {
        uid: i32,
    },
    /// `set_tile_at` / `set_card_tile` moved a field instance to a tile
    /// (`-1` = back with its owner).
    SetCardTile {
        uid: i32,
        tile: i32,
    },
    MoveUnits {
        from: i32,
        to: i32,
        n: i32,
    },
    /// One committed zone transition (`zone_leave`); the order of these
    /// records is the cascade order (attachments, children, leaver).
    ZoneLeave {
        uid: i32,
        from: i32,
        to: i32,
        cause: i32,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Narration {
    pub player: i32,
    pub key: u16,
}

/// Prompt request kinds (mirrors `card_sdk::abi::PromptKind` discriminants).
pub const PROMPT_CARD: i32 = 4;
pub const PROMPT_YESNO: i32 = 1;

// ---------------------------------------------------------------------------
// Generic named map (tokens / state / counters)
// ---------------------------------------------------------------------------

/// One `(key, value)` slot. Kept for any external pin of the old linear-map
/// shape; `NameMap` itself is direct-index (see below) and no longer stores
/// `NamedVal`s.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub struct NamedVal {
    pub key: NameId,
    pub value: i32,
}

impl NamedVal {
    pub const EMPTY: NamedVal = NamedVal {
        key: NameId::INVALID,
        value: 0,
    };
}

/// A map from interned name → i32, **direct-indexed by `NameId.0`**.
///
/// CBMC cost: O(1) (one compare + one array index). The old 8-slot linear
/// scan unrolled 8× per `get`/`set` and was the pp-unit unwind cliff
/// (144 + 80 unrolls in `same_dream_01`).
///
/// Presence is a bitset of one `u64` word per 64 slots; a single `u64` word
/// overflows at slot 64 in the debug build.
/// `NameId::INVALID` / out-of-range ids never alias a live key: `get_opt`
/// is `None`, `set` is a documented no-op.
/// Extra (key, value) slots for any remaining ids outside both the intern
/// bucket range and the derived-column tags.
const MAX_EXTRA: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NameMap {
    vals: [i32; crate::intern::SLOTS],
    /// Bit `i` of word `i / 64` set = key `NameId(i)` is present (even if
    /// `vals[i] == 0`). One word per 64 slots -- a single `u64` overflows at
    /// slot 64 in the debug build.
    present: [u64; (crate::intern::SLOTS + 63) / 64],
    /// Linear fallback for ids outside the intern range (derived columns on
    /// field cards / tiles). Player state columns live on [`World`], not here:
    /// a per-map column array would have to be paid by all 60 tile maps.
    extra: [(NameId, i32); MAX_EXTRA],
    extra_n: u8,
}

impl NameMap {
    pub const fn new() -> Self {
        Self {
            vals: [0; crate::intern::SLOTS],
            present: [0; (crate::intern::SLOTS + 63) / 64],
            extra: [(NameId::INVALID, 0); MAX_EXTRA],
            extra_n: 0,
        }
    }

    pub fn get(&self, key: NameId) -> i32 {
        self.get_opt(key).unwrap_or(0)
    }

    /// `Some(value)` when `key` is present (even if the value is 0).
    /// O(1): one range check + one bit test. No loop.
    pub fn get_opt(&self, key: NameId) -> Option<i32> {
        let i = key.0 as usize;
        if i < crate::intern::SLOTS {
            if ((self.present[i / 64] >> (i % 64)) & 1) == 1 {
                return Some(self.vals[i]);
            }
            return None;
        }
        // Past the presence word — tiny linear table (≤ MAX_EXTRA).
        let mut e = 0usize;
        while e < MAX_EXTRA {
            if e < self.extra_n as usize && self.extra[e].0 == key {
                return Some(self.extra[e].1);
            }
            e += 1;
        }
        None
    }

    /// Set `key` to `value`, inserting if needed. Returns the value.
    /// O(1). Out-of-range / `INVALID` ids are a no-op (never alias).
    pub fn set(&mut self, key: NameId, value: i32) -> i32 {
        let i = key.0 as usize;
        if i < crate::intern::SLOTS {
            self.vals[i] = value;
            self.present[i / 64] |= 1u64 << (i % 64);
            return value;
        }
        if !key.is_valid() {
            return value;
        }
        let mut e = 0usize;
        while e < MAX_EXTRA {
            if e < self.extra_n as usize && self.extra[e].0 == key {
                self.extra[e].1 = value;
                return value;
            }
            e += 1;
        }
        if (self.extra_n as usize) < MAX_EXTRA {
            self.extra[self.extra_n as usize] = (key, value);
            self.extra_n += 1;
        } else {
            // Documented cut: overflow overwrites extra[0] (same shape as the
            // old NameMap full-table rule). Never aliases a different key.
            self.extra[0] = (key, value);
        }
        value
    }

    pub fn add(&mut self, key: NameId, delta: i32) -> i32 {
        let v = self.get(key).saturating_add(delta);
        self.set(key, v)
    }

    /// Visit every present `(key, value)` in index order.
    ///
    /// Bound is `MAX_INTERN` (32) — used only by `ir_fill`'s copy path,
    /// not the hot `get`/`set`. `f` sees at most `MAX_KEYS` live keys in
    /// practice (the engine budget); extra slots are model headroom.
    pub fn for_each(&self, mut f: impl FnMut(NameId, i32)) {
        // Walk every bucket. The bound is `SLOTS` (64 under Kani, 4096 in a
        // native test build), so a native id lands wherever the hash puts it.
        let mut i = 0usize;
        while i < crate::intern::SLOTS {
            if ((self.present[i / 64] >> (i % 64)) & 1) == 1 {
                f(NameId(i as u16), self.vals[i]);
            }
            i += 1;
        }
        let mut e = 0usize;
        while e < MAX_EXTRA {
            if e < self.extra_n as usize {
                let (k, v) = self.extra[e];
                f(k, v);
            }
            e += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Field card instance
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FieldCard {
    pub uid: i32,
    pub card: u16,
    /// The **field holder** (whose field vector the row sits in; `-1` = the
    /// board field).
    pub player: i32,
    /// The instance's real `owner` (`FieldCard.owner`); moves with
    /// `set_instance_owner` without moving the row.
    pub owner: i32,
    pub tile: i32,
    pub face_down: bool,
    pub immune: bool,
    /// 「不受除拥有者以外的效果影响」 (`immuneExceptOwner`), causer-aware
    /// flavour read through `hostfns::is_immune_from_at`.
    pub immune_except_owner: bool,
    pub crystals: i32,
    pub props: NameMap,
    /// The card whose printed surface this instance runs (`u16::MAX` = its
    /// own). Dies with the instance, like the row.
    pub effective_card: u16,
    /// The instance this one sits on (zone `Attached`), `-1` when not
    /// attached. Holds exactly while the row is attached (G08 I6).
    pub on: i32,
    /// The instance that made this one (`-1` = none). A child dies with its
    /// creator (G08 I7).
    pub created_by: i32,
    /// Did this card take effect? Cleared by `set_ineffective_at` and read by
    /// a Graveyard arrival (G08 I8).
    pub effective: bool,
    /// Did the running body mint this row (`ctx::is_own_placement`)? A row the
    /// harness planted as pre-state is not the body's own placement.
    pub mine: bool,
}

impl FieldCard {
    pub const EMPTY: FieldCard = FieldCard {
        uid: 0,
        card: u16::MAX,
        player: -1,
        owner: -1,
        tile: -1,
        face_down: false,
        immune: false,
        immune_except_owner: false,
        crystals: 0,
        props: NameMap::new(),
        effective_card: u16::MAX,
        on: -1,
        created_by: -1,
        effective: true,
        mine: false,
    };
}

/// `game_kinds::Zone` wire codes (the model stores the wire form).
pub const ZONE_NOWHERE: i32 = 0;
pub const ZONE_DECK: i32 = 1;
pub const ZONE_HAND: i32 = 2;
pub const ZONE_FIELD: i32 = 3;
pub const ZONE_GRAVEYARD: i32 = 4;
pub const ZONE_BANISHED: i32 = 5;
pub const ZONE_ATTACHED: i32 = 9;

/// `game_kinds::ZoneChangeCause` codes the writer itself produces.
pub const CAUSE_HOST_LEFT: i32 = 8;
pub const CAUSE_CREATOR_LEFT: i32 = 9;
pub const CAUSE_EFFECT: i32 = 3;

/// A guest `Dest` wire code → the zone it lands in (mirrors
/// `game_core::engine::zone::dest_zone`).
pub fn dest_zone(dest: i32) -> i32 {
    match dest {
        1 => ZONE_HAND,
        2 => ZONE_FIELD,
        3 => ZONE_BANISHED,
        4 | 5 | 6 => ZONE_DECK,
        _ => ZONE_GRAVEYARD,
    }
}

/// Is `zone` a played zone (`Field` / `Attached`)?
pub fn is_played(zone: i32) -> bool {
    zone == ZONE_FIELD || zone == ZONE_ATTACHED
}

// ---------------------------------------------------------------------------
// Tile runtime state
// ---------------------------------------------------------------------------

/// A deferred event record (design G10 §6.2). `event` is an interned card id
/// (`u16::MAX` = none).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DeferredEvent {
    pub event: i32,
    pub player: i32,
    pub tile: i32,
    /// The draw this record parked; two draws of one card differ by it.
    pub ordinal: i32,
    pub due_owner: i32,
    pub deferrer_uid: i32,
    pub after_turn_key: i32,
}

impl DeferredEvent {
    pub const fn empty() -> Self {
        Self {
            event: 0,
            player: -1,
            tile: -1,
            ordinal: 0,
            due_owner: -1,
            deferrer_uid: -1,
            after_turn_key: 0,
        }
    }
}

/// The six families a snapshot checkpoints (design G10 §6.5): positions,
/// keyed state, money, owners, houses, mortgage flags.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SnapData {
    pub pos: [i32; MAX_PLAYERS],
    pub money: [i32; MAX_PLAYERS],
    pub state: [NameMap; MAX_PLAYERS],
    pub owners: [i32; 60],
    pub houses: [i32; 60],
    pub mortgaged: [bool; 60],
}

impl SnapData {
    pub fn empty() -> Self {
        Self {
            pos: [-1; MAX_PLAYERS],
            money: [0; MAX_PLAYERS],
            state: [NameMap::new(); MAX_PLAYERS],
            owners: [-1; 60],
            houses: [0; 60],
            mortgaged: [false; 60],
        }
    }
}

/// One allocated snapshot key with its owners.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SnapshotEntry {
    pub used: bool,
    pub key: i32,
    pub owners: [i32; MAX_SNAP_OWNERS],
    pub owners_n: u8,
    pub data: SnapData,
}

impl SnapshotEntry {
    pub const fn empty() -> Self {
        Self {
            used: false,
            key: 0,
            owners: [-1; MAX_SNAP_OWNERS],
            owners_n: 0,
            data: SnapData {
                pos: [-1; MAX_PLAYERS],
                money: [0; MAX_PLAYERS],
                state: [NameMap::new(); MAX_PLAYERS],
                owners: [-1; 60],
                houses: [0; 60],
                mortgaged: [false; 60],
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TileState {
    pub owner: i32,
    pub houses: i32,
    pub mortgaged: bool,
    pub props: NameMap,
}

impl TileState {
    pub const fn new() -> Self {
        Self {
            owner: -1,
            houses: 0,
            mortgaged: false,
            props: NameMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Mark
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mark {
    pub tile: i32,
    pub key: NameId,
    pub src_card: u16,
    pub instance: i32,
    pub n: i32,
}

impl Mark {
    pub const EMPTY: Mark = Mark {
        tile: -1,
        key: NameId::INVALID,
        src_card: u16::MAX,
        instance: 0,
        n: 0,
    };
}

// ---------------------------------------------------------------------------
// Roll transactions (v109 model)
// ---------------------------------------------------------------------------

/// One cast in the model.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RollCastM {
    pub value: i32,
    pub source: i32,
}

impl RollCastM {
    pub const EMPTY: RollCastM = RollCastM {
        value: 0,
        source: 0,
    };
}

/// A model roll transaction (bounded: 4 casts, two live transactions).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RollTx {
    pub id: i32,
    pub roller: i32,
    pub purpose: i32,
    pub source: i32,
    pub has_result: bool,
    pub open: bool,
    pub settling: bool,
    pub casts: [RollCastM; 4],
    pub n: u8,
    pub chosen: u8,
    pub unused: i32,
    pub final_face: i32,
}

impl RollTx {
    pub const EMPTY: RollTx = RollTx {
        id: -1,
        roller: 0,
        purpose: 0,
        source: 0,
        has_result: false,
        open: false,
        settling: false,
        casts: [RollCastM::EMPTY; 4],
        n: 0,
        chosen: 0,
        unused: -1,
        final_face: -1,
    };
}

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct World {
    // ---- players ----
    pub n_players: i32,
    pub money: [i32; MAX_PLAYERS],
    pub out: [bool; MAX_PLAYERS],
    pub pos: [i32; MAX_PLAYERS],
    pub charge: [i32; MAX_PLAYERS],
    pub charge_max: [i32; MAX_PLAYERS],
    pub stay: [i32; MAX_PLAYERS],
    /// [眩晕] layers (`key::STUN`) — distinct from [`Self::stay`].
    pub stun: [i32; MAX_PLAYERS],
    /// [除外] layers (`key::EXILE`) + destination (`key::EXILE_TO`).
    pub exile: [i32; MAX_PLAYERS],
    pub exile_to: [i32; MAX_PLAYERS],
    /// Extra turns queued by `give_extra_turn` (scheduling is PROTOCOL).
    pub extra_turns: [bool; MAX_PLAYERS],
    /// `ctx::n(k,v)` doubling slot. `-1` = no doubling (engine `None`).
    pub play_doubled: i32,
    pub persona_id: [u16; MAX_PLAYERS],
    pub group_id: [u16; MAX_PLAYERS],
    /// The `persona_skill` answer (0 = the seat has no persona skill). A
    /// harness sets it to the `skill:<character>:<name>` id it binds.
    pub persona_skill: u16,
    /// Generic per-player named counters (tokens, held, state).
    pub player_keys: [NameMap; MAX_PLAYERS],
    /// MIN / MAX / EXPIRES / FLAGS columns of `player_keys`, direct-indexed
    /// `[player][column][base key]`. Off `NameMap` on purpose: the 60 tile
    /// maps and 8 field maps would otherwise each carry four column arrays.
    pub player_cols: [[[i32; crate::intern::SLOTS]; 4]; MAX_PLAYERS],
    pub player_cols_present: [[[u64; (crate::intern::SLOTS + 63) / 64]; 4]; MAX_PLAYERS],
    /// Band crystals / skills (kept explicit; also mirrored in player_keys).
    pub group_pool: [i32; MAX_PLAYERS],

    // ---- compatibility views for the pilot proofs ----
    /// `ppFansUp` / `ppFansDown` counts (also in `player_keys`).
    pub fans_pos: [i32; MAX_PLAYERS],
    pub fans_neg: [i32; MAX_PLAYERS],
    /// Copies of `PP:[衍生]共鸣` in hand (the only hand identity the pilot needs).
    pub hand_resonance: [i32; MAX_PLAYERS],

    // ---- piles ----
    /// Pilot bitset (presence) over the first 4 card slots — the shape
    /// `proofs/money_shave.rs` / `prompt.rs` / `spec` expect.
    pub discard: [bool; MAX_CARDS],
    pub hand: [bool; MAX_CARDS],
    /// Interned piles (u16 card ids; `u16::MAX` = empty slot).
    pub hand_pile: [u16; MAX_CARDS],
    pub hand_n: u8,
    pub deck_pile: [u16; MAX_CARDS],
    pub deck_n: u8,
    pub discard_pile: [u16; MAX_CARDS],
    pub discard_n: u8,
    /// The open inspection frame (v114): cards turned over and not released,
    /// front-first. One frame is modelled (`inspection_frame_n` is 0 or 1).
    pub inspection_pile: [u16; MAX_CARDS],
    pub inspection_n: u8,
    /// Whose deck the open frame holds (index), or -1 for none. The refill
    /// gate reads it.
    pub inspection_deck_owner: i32,
    pub inspection_public: bool,
    pub inspection_revealed: [bool; MAX_CARDS],
    pub inspection_frame_n: u8,

    // ---- field cards ----
    pub field: [FieldCard; MAX_FIELD],
    pub field_n: u8,
    pub next_uid: i32,
    /// Pending `place_card` destination (`ctx::set_dest`).
    pub dest: i32,
    /// The seat a pending `set_transfer_to_dest` aims the run's arrival at
    /// (`-1` = its own owner).
    pub dest_to: i32,
    /// The instance the **run** is for (`Run::current_uid`): what `set_dest` /
    /// `self_uid` answer. An inner `place_card` does not move it (the engine's
    /// `apply_dest` files `current_uid`), so it differs from `last_uid` after a
    /// body places.
    pub bound_uid: i32,
    /// The model's "running instance" slot for **self** writes: the harness's
    /// offered instance, advanced by `place_card` (`ctx::add_counter_self` and
    /// friends follow the latest placement).
    pub last_uid: i32,
    /// A card claimed the move at the pre-dice pass.
    pub plan_claimed: bool,
    /// Open invocation frames, innermost last; `invoke_n` counts the pushes
    /// (so it is also the nesting depth). The nested guest does not run, so
    /// `arm_invoke_entry` records instead (`G13` §5.5).
    pub invoke_n: u32,
    pub invoke_stack: [InvokeRecord; MAX_INVOKE],
    /// The last `arm_invoke_entry` fact for proof readers:
    /// `(card, entry, paid, by_instance)`.
    pub last_invoke: (NameId, NameId, bool, i32),
    /// Crystals of the running instance when it has no `field[]` slot
    /// (events: heat_head phase, circle_rebuild's 5 miracle crystals).
    pub self_crystals: i32,

    // ---- tiles ----
    pub tiles: [TileState; 60],
    /// Per tile: the acquisition sequence of its current owner (`-1` = nobody
    /// / already released) and the next one to hand out (G08 I13).
    pub owned_seq: [i32; 60],
    pub next_owned_seq: i32,
    /// `prop::FIRST_TILE` cache per seat, rebuilt on every ownership write
    /// that can change the answer (`-1` = none).
    pub first_tile: [i32; MAX_PLAYERS],

    // ---- zone trigger payload (`Trigger.zone` / `play_outcome` / `out_cause`)
    /// The last zone transition, as a raised `ZoneChange` would carry it.
    pub trig_zone_present: bool,
    pub trig_zone_from: i32,
    pub trig_zone_to: i32,
    pub trig_zone_cause: i32,
    pub trig_zone_holder: i32,
    pub trig_zone_wasted: bool,
    pub trig_zone_counted: bool,
    /// `Trigger.play_outcome` wire code, `-1` = none.
    pub trig_play_outcome: i32,
    /// `Trigger.out_cause` wire code, `-1` = none.
    pub trig_out_cause: i32,
    /// The `effective` value the last Graveyard landing left on the row after
    /// the transition cleared it (`true` once a wasted entry lands). The row
    /// itself dies, so this is how the read-then-clear is observable.
    pub last_leave_effective_after: bool,

    // ---- marks / events ----
    pub marks: [Mark; MAX_MARKS],
    pub marks_n: u8,
    pub events: [u16; MAX_EVENTS],
    pub events_n: u8,
    /// Parked `event` records (design G10 §6.2).
    pub deferred: [DeferredEvent; MAX_DEFERRED],
    pub deferred_n: u8,
    /// The last `draw_event_as` attribution `(player, tile)`.
    pub last_event_draw: (i32, i32),
    /// World snapshot store; keys run from 1 (`0` = none) like the engine.
    pub snapshots: [SnapshotEntry; MAX_SNAPSHOTS],
    pub next_snapshot_key: i32,
    /// Pilot teleport world: whether 「江户川乐器店」 is present.
    pub tile_shop_present: bool,

    // ---- turn / phase ----
    pub turn_player: i32,
    pub round_no: i32,
    pub turn_key: i32,

    // ---- trigger context (a pending [支付] / [消耗] / …) ----
    pub trig_player: i32,
    pub trig_value: i32,
    /// Last value written by `trig_set_pay_amount`.
    pub pay_amount: i32,
    /// Trigger kind wire **spelling** (`TriggerKind::spelling`). `""` = none.
    /// The `trig_kind` hostcall writes these bytes; the IR `kind` scalar stores
    /// `id_of` of the same spelling (`VarType::Str`).
    pub trig_kind: &'static str,
    pub trig_target: i32,
    pub trig_tile: i32,
    /// The `event` draw's ordinal (copies of the card still in the deck).
    pub trig_ordinal: i32,
    pub trig_cancelled: bool,
    pub trig_negate: bool,
    /// `Trigger.abnormal_blocked` — did the announced abnormal hold the move
    /// (`trigger::abnormal_blocked`)?
    pub trig_abnormal_blocked: bool,
    /// 真奈 join (CH-05): the seat a joiner's self-targeted effects are
    /// redirected to; `-1` outside a join (hostfns `join_redirect_target`).
    pub trig_join_redirect: i32,
    /// When non-zero, `roll_dice` returns this value instead of a symbolic
    /// face (harness control for dice-clause proofs; 0 = symbolic).
    pub forced_roll: i32,
    /// Effect-roll calls (`arm_roll` non-move branch) — the roll itself is not
    /// an `Effect`, so proofs that owe exactly one roll count it here.
    pub roll_calls: u32,
    /// `at_turn_end` / `at_next_turn_end` / `before_turn_end` calls, recorded
    /// by `arm_schedule_turn_end` (`(player, mode)` of the last).
    pub schedule_turn_end_n: u32,
    pub last_schedule: (i32, i32),
    /// `settle_tile_at` calls, recorded by `arm_settle_tile_at` (the settle
    /// itself is a protocol stub with no effect-log entry).
    pub settle_calls: u32,
    pub last_settle: (i32, i32),
    /// A joiner used the redirect (hostfns `set_join_used`).
    pub trig_join_used: bool,
    /// `join::open` ran on this pass (the model has no join round beyond the
    /// flag; design G06 §6.8).
    pub trig_join_opened: bool,
    /// `Trigger::main` — fired as part of the turn's [主要移动]?
    /// (`hostfns::trig_move_main` reads this, not `mv.main`.)
    pub trig_main: bool,
    /// `Trigger::roll_source` guest code (`ROLL_*`).
    pub trig_roll_source: i32,
    /// `Trigger::pay.is_rent` — pay-only fact (`hostfns::trig_pay_is_rent`).
    pub trig_pay_is_rent: bool,
    /// `Trigger.by_card_instance` (`-1` = board raise).
    pub trig_by_card_instance: i32,
    /// `SettleFacts::bits()` (0 = none).
    pub trig_settle_facts: i32,
    /// `SettleFacts::paid_value` (0 = none).
    pub trig_settle_paid_value: i32,
    /// The seat a landed redirect moved a live target from (`-1` = none).
    pub trig_redirected_from: i32,
    /// `TriggerPay.kind` wire code, `-1` = no payment (`pay_kind` pre).
    pub trig_pay_kind: i32,
    /// `TriggerPay.shape == Fixed` (`pay_is_fixed` pre).
    pub trig_pay_is_fixed: bool,
    /// `TriggerPay.total_stage` (`pay_total_stage` pre); `true` is neutral.
    pub trig_pay_total_stage: bool,
    /// `TriggerPay.split_total` (`pay_split_total` pre); `false` is neutral.
    pub trig_pay_split_total: bool,
    /// `Trigger.charge_skill` (`charge_is_skill` pre).
    pub trig_charge_skill: bool,
    /// `Trigger.settle_tile` — the innermost open settle, `-1` = none.
    pub trig_settle_tile: i32,
    /// `Trigger.outcome` as `SettleOutcome::as_i32`, `-1` = none.
    pub trig_settle_outcome: i32,
    /// `pay_amount` was written since the last fold (`spendBefore` reads it).
    pub pay_amount_set: bool,
    /// Pay factors per scope, milli (1000 = x1); `pay_factor` composes the
    /// live ones.
    pub next_move_pay_factor: i32,
    pub plan_pay_factor: i32,
    pub next_settle_pay_factor: i32,
    pub settle_pay_factor: i32,
    /// A `thisMove` scope is open.
    pub move_open: bool,
    /// Number of open settles (`thisSettle` is open while non-zero).
    pub settle_open: u32,
    /// `paid_in_settle_by` — per-seat array, index = seat.
    pub paid_in_settle_by: [i32; MAX_PLAYERS],
    /// A symbolic `rent_quote` figure; `-1` = no rent charge.
    pub rent_quote: i32,
    /// The turn's main move is already taken (`run_planned_move` latch).
    pub main_moved: bool,
    /// The move gate holds this turn (`skip_main_move` / `clear_move_skip`),
    /// i.e. `World.st.skip_move`. A [停留] turn answers `3` from `cant_move`.
    pub skip_move: bool,
    /// The running activation (`turn.activation_ordinal`; 0 outside a
    /// routine). Stamped from the single routine entry before the snapshot
    /// (`trigger::activation_ordinal`).
    pub turn_activation_ordinal: i32,
    /// The activation that consumed this turn's main move
    /// (`turn.move_activation_ordinal`; 0 = none).
    pub move_activation_ordinal: i32,
    /// A landed abnormal during this turn's main move, latched at the walk's
    /// end (`turn.move_abnormal`, read by `trigger::move_abnormal`).
    pub turn_move_abnormal: bool,
    /// 「打出此卡的回合…」: the turn's dice-immunity flag
    /// (`turn.dice_immune`).
    pub dice_immune: bool,
    /// The next move is forced (`plan::set_forced`).
    pub forced: bool,
    /// A forced move counts as the main move anyway (`set_main_override`).
    pub main_override: bool,
    /// A replacement skips the rolled walk (`plan::void_walk`).
    pub void_walk: bool,
    /// The plan has a teleport destination (`plan::set_teleport_to`).
    pub teleport: bool,
    /// The call runs inside a roll window owned by the mover.
    pub window: bool,
    /// Fixed plan steps (`plan::set_steps`), `None` = "the roll".
    pub steps: Option<i32>,
    /// Walk parity (`plan::set_parity`); `-1` = no parity, `0` even, `1` odd.
    pub parity: i32,
    /// Move payload (`Trigger::mv`); `MoveSnap::NONE` = no move.
    pub move_snap: MoveSnap,
    /// Buy payload (`Trigger::buy`); `BuySnap::NONE` = no buy.
    pub buy_snap: BuySnap,
    /// `Trigger::card` name id (0 = none) — `ir_fill` `trigger_card`.
    pub trig_card: i32,
    /// `Trigger::name` (counter / message name id, 0 = none) — `counter_name`.
    pub counter_name: i32,
    /// Move-plan fixed roll (`-1` = null) — `ir_fill` `plan_fixed_roll`.
    pub plan_fixed_roll: i32,
    /// Open roll transactions, depth 2 (design G03 §5.9).
    pub rolls: [RollTx; 2],
    pub roll_depth: u8,
    pub next_roll_id: i32,
    /// `(roller, value)` per cast this turn, bounded.
    pub roll_hist: [(i32, i32); 8],
    pub roll_hist_n: u8,
    /// The roll transaction of the pending trigger (`Trigger.roll`).
    pub trig_roll: Option<RollTx>,
    /// Armed 「是否触发结算」 answers per seat (0 = unanswered, 1 = settle,
    /// 2 = skip).
    pub move_answer: [i32; MAX_PLAYERS],
    pub turn_answer: [i32; MAX_PLAYERS],
    /// Bands currently blocked from skills (`ir_fill` `BlockedBands`).
    /// Non-zero entries are band name ids.
    pub blocked_bands: [i32; 4],
    pub blocked_bands_n: u8,

    // ---- cross-card message context (`ctx::send` / `On::Message`) ----
    /// Interned message name id (`mark:cp`'s `"place"` / …). 0 = none.
    pub msg_name: u16,
    /// `Message.seat` — the holder / subject seat of the payload.
    pub msg_seat: i32,
    pub msg_arg0: i32,
    pub msg_arg1: i32,
    pub msg_arg2: i32,
    pub msg_tile: i32,
    /// Reply the handler set (`ctx::message::reply`).
    pub msg_reply: i32,
    /// Sender's instance uid / seat (`ctx::message::sender_*`).
    pub msg_sender_uid: i32,
    pub msg_sender_seat: i32,

    // ---- turn trackers (ir_fill `Gains` / `Targeted` / `Between`) ----
    /// Money gained this turn per seat (`hostfns::gains_this_turn`).
    pub gains_this_turn: [i32; MAX_PLAYERS],
    /// How many times this seat was targeted this turn (`targeted_count`).
    pub targeted_count: [i32; MAX_PLAYERS],
    /// `between(p, a, b)` seat-exclusion helper used by some guards; the
    /// model stores the last "between witness" seat per probe (`-1` = none).
    /// Documented gap: no turn-scoped between-set; proofs that need the full
    /// predicate should set this from their own fixture.
    pub between_seat: [i32; MAX_PLAYERS],

    // ---- prompt state ----
    pub opt_n: i32,
    pub next_answer: usize,
    pub answers: [i32; MAX_ANSWERS],
    /// When true, `ask` returns a fresh symbolic answer in range (Kani only).
    pub symbolic_answers: bool,

    // ---- effect / narration logs ----
    pub log: [Effect; MAX_LOG],
    pub log_n: usize,
    pub narration: [Narration; MAX_NARRATION],
    pub narration_n: usize,

    /// Set when the host hits an unmodeled op (see `host::dispatch`).
    pub unmodeled_op: i32,
}

fn player_idx(player: i32) -> usize {
    if player >= 0 && (player as usize) < MAX_PLAYERS {
        player as usize
    } else {
        0
    }
}

/// Derived column tag (`NameId.0 >> 12`) → column slot. The engine's state
/// columns are `key + 0x1000` (flags) / `+0x2000` (expires) / `+0x4000` (min)
/// / `+0x8000` (max).
fn column_of(tag: usize) -> Option<usize> {
    match tag {
        0x1 => Some(0),
        0x2 => Some(1),
        0x4 => Some(2),
        0x8 => Some(3),
        _ => None,
    }
}

impl World {
    pub const fn new() -> Self {
        Self {
            n_players: 2,
            money: [0; MAX_PLAYERS],
            out: [false; MAX_PLAYERS],
            pos: [0; MAX_PLAYERS],
            charge: [0; MAX_PLAYERS],
            charge_max: [3; MAX_PLAYERS],
            stay: [0; MAX_PLAYERS],
            stun: [0; MAX_PLAYERS],
            exile: [0; MAX_PLAYERS],
            exile_to: [-1; MAX_PLAYERS],
            extra_turns: [false; MAX_PLAYERS],
            play_doubled: -1,
            persona_id: [u16::MAX; MAX_PLAYERS],
            group_id: [u16::MAX; MAX_PLAYERS],
            persona_skill: 0,
            player_keys: [NameMap::new(); MAX_PLAYERS],
            player_cols: [[[0; crate::intern::SLOTS]; 4]; MAX_PLAYERS],
            player_cols_present: [[[0; (crate::intern::SLOTS + 63) / 64]; 4]; MAX_PLAYERS],
            group_pool: [0; MAX_PLAYERS],
            fans_pos: [0; MAX_PLAYERS],
            fans_neg: [0; MAX_PLAYERS],
            hand_resonance: [0; MAX_PLAYERS],
            discard: [false; MAX_CARDS],
            hand: [false; MAX_CARDS],
            hand_pile: [u16::MAX; MAX_CARDS],
            hand_n: 0,
            deck_pile: [u16::MAX; MAX_CARDS],
            deck_n: 0,
            discard_pile: [u16::MAX; MAX_CARDS],
            discard_n: 0,
            inspection_pile: [u16::MAX; MAX_CARDS],
            inspection_n: 0,
            inspection_deck_owner: -1,
            inspection_public: false,
            inspection_revealed: [false; MAX_CARDS],
            inspection_frame_n: 0,
            field: [FieldCard::EMPTY; MAX_FIELD],
            field_n: 0,
            next_uid: 1,
            dest: 0,
            dest_to: -1,
            bound_uid: 1,
            last_uid: 1,
            plan_claimed: false,
            invoke_n: 0,
            invoke_stack: [(NameId::INVALID, NameId::INVALID, -1, -1, -1, false, 0); MAX_INVOKE],
            last_invoke: (NameId::INVALID, NameId::INVALID, false, -1),
            self_crystals: 0,
            tiles: [TileState::new(); 60],
            owned_seq: [-1; 60],
            next_owned_seq: 0,
            first_tile: [-1; MAX_PLAYERS],
            trig_zone_present: false,
            trig_zone_from: ZONE_NOWHERE,
            trig_zone_to: ZONE_NOWHERE,
            trig_zone_cause: 0,
            trig_zone_holder: -1,
            trig_zone_wasted: false,
            trig_zone_counted: false,
            trig_play_outcome: -1,
            trig_out_cause: -1,
            last_leave_effective_after: true,
            marks: [Mark::EMPTY; MAX_MARKS],
            marks_n: 0,
            events: [u16::MAX; MAX_EVENTS],
            events_n: 0,
            deferred: [DeferredEvent::empty(); MAX_DEFERRED],
            deferred_n: 0,
            last_event_draw: (-1, -1),
            snapshots: [SnapshotEntry::empty(); MAX_SNAPSHOTS],
            next_snapshot_key: 1,
            tile_shop_present: true,
            turn_player: 0,
            round_no: 1,
            turn_key: 0,
            trig_player: 0,
            trig_value: 0,
            pay_amount: 0,
            trig_kind: "",
            trig_target: -1,
            trig_tile: -1,
            trig_ordinal: 0,
            trig_cancelled: false,
            trig_negate: false,
            trig_abnormal_blocked: false,
            trig_join_redirect: -1,
            forced_roll: 0,
            roll_calls: 0,
            schedule_turn_end_n: 0,
            last_schedule: (-1, 0),
            settle_calls: 0,
            last_settle: (-1, -1),
            trig_join_used: false,
            trig_join_opened: false,
            trig_main: false,
            trig_roll_source: ROLL_UNATTRIBUTED,
            trig_pay_is_rent: false,
            trig_by_card_instance: -1,
            trig_settle_facts: 0,
            trig_settle_paid_value: 0,
            trig_redirected_from: -1,
            trig_pay_kind: -1,
            trig_pay_is_fixed: false,
            trig_pay_total_stage: true,
            trig_pay_split_total: false,
            trig_charge_skill: false,
            trig_settle_tile: -1,
            trig_settle_outcome: -1,
            pay_amount_set: false,
            next_move_pay_factor: 1000,
            plan_pay_factor: 1000,
            next_settle_pay_factor: 1000,
            settle_pay_factor: 1000,
            move_open: false,
            settle_open: 0,
            paid_in_settle_by: [0; MAX_PLAYERS],
            rent_quote: -1,
            main_moved: false,
            skip_move: false,
            turn_activation_ordinal: 0,
            move_activation_ordinal: 0,
            turn_move_abnormal: false,
            dice_immune: false,
            forced: false,
            main_override: false,
            void_walk: false,
            teleport: false,
            window: false,
            steps: None,
            parity: -1,
            move_snap: MoveSnap::NONE,
            buy_snap: BuySnap::NONE,
            trig_card: 0,
            counter_name: 0,
            plan_fixed_roll: -1,
            rolls: [RollTx::EMPTY; 2],
            roll_depth: 0,
            next_roll_id: 1,
            roll_hist: [(0, 0); 8],
            roll_hist_n: 0,
            trig_roll: None,
            move_answer: [0; MAX_PLAYERS],
            turn_answer: [0; MAX_PLAYERS],
            blocked_bands: [0; 4],
            blocked_bands_n: 0,
            msg_name: 0,
            msg_seat: -1,
            msg_arg0: 0,
            msg_arg1: 0,
            msg_arg2: 0,
            msg_tile: -1,
            msg_reply: 0,
            msg_sender_uid: 0,
            msg_sender_seat: 0,
            gains_this_turn: [0; MAX_PLAYERS],
            targeted_count: [0; MAX_PLAYERS],
            between_seat: [-1; MAX_PLAYERS],
            opt_n: 0,
            next_answer: 0,
            answers: [0; MAX_ANSWERS],
            symbolic_answers: false,
            log: [Effect::SetPayAmount { to: 0 }; MAX_LOG],
            log_n: 0,
            narration: [Narration { player: 0, key: 0 }; MAX_NARRATION],
            narration_n: 0,
            unmodeled_op: -1,
        }
    }

    pub fn push(&mut self, e: Effect) {
        if self.log_n < MAX_LOG {
            self.log[self.log_n] = e;
            self.log_n += 1;
        }
    }

    pub fn push_narration(&mut self, player: i32, key: u16) {
        if self.narration_n < MAX_NARRATION {
            self.narration[self.narration_n] = Narration { player, key };
            self.narration_n += 1;
        }
    }

    pub fn effect_slice(&self) -> &[Effect] {
        &self.log[..self.log_n]
    }

    /// Living players in `0..n_players` that are not `out`.
    pub fn living(&self, n: i32) -> impl Iterator<Item = i32> + '_ {
        let n = n.clamp(1, MAX_PLAYERS as i32);
        (0..n).filter(move |&p| !self.out[p as usize])
    }

    pub fn push_answer(&mut self, a: i32) {
        if self.next_answer < MAX_ANSWERS {
            self.answers[self.next_answer] = a;
            self.next_answer += 1;
        }
    }

    // ---- generic named accessors (tokens / state / counters) ----

    pub fn key_get(&self, player: i32, key: NameId) -> i32 {
        if player < 0 || player as usize >= MAX_PLAYERS {
            return 0;
        }
        self.player_keys[player as usize].get(key)
    }

    pub fn key_set(&mut self, player: i32, key: NameId, v: i32) {
        if player < 0 || player as usize >= MAX_PLAYERS {
            return;
        }
        let v = if self.key_never_negative(player, key) {
            v.max(0)
        } else {
            v
        };
        self.player_keys[player as usize].set(key, v);
        self.sync_fan_key(player, key);
    }

    pub fn key_add(&mut self, player: i32, key: NameId, delta: i32) -> i32 {
        if player < 0 || player as usize >= MAX_PLAYERS {
            return 0;
        }
        let mut v = self.player_keys[player as usize].add(key, delta);
        if self.key_never_negative(player, key) && v < 0 {
            v = 0;
            self.player_keys[player as usize].set(key, 0);
        }
        self.sync_fan_key(player, key);
        v
    }

    /// [`state_flag::NEVER_NEGATIVE`] (bit 0) on `key`, as declared by
    /// `state_set` field `FLAGS`. Matches the engine's floor in
    /// `MatchPlayer::state_set`.
    pub fn key_never_negative(&self, player: i32, key: NameId) -> bool {
        self.key_flags(player, key) & 1 != 0
    }

    /// Flag byte declared for `key` (0 when none). Stored at `key + 0x1000`,
    /// beside the MIN/MAX/EXPIRES derived columns.
    pub fn key_flags(&self, player: i32, key: NameId) -> u8 {
        self.col_get(player, key, 0x1000) as u8
    }

    /// Declare [`state_flag`] bits for `key` (`state_set` field `FLAGS`).
    pub fn key_set_flags(&mut self, player: i32, key: NameId, flags: u8) {
        self.col_set(player, key, 0x1000, flags as i32);
    }

    /// One derived column cell (`tag` = 0x1000 flags / 0x2000 expires /
    /// 0x4000 min / 0x8000 max), direct-indexed.
    fn col_get(&self, player: i32, key: NameId, tag: u16) -> i32 {
        let Some(c) = column_of(tag as usize >> 12) else {
            return 0;
        };
        self.player_cols[player_idx(player)][c][(key.0 as usize) & 0xfff]
    }

    fn col_set(&mut self, player: i32, key: NameId, tag: u16, v: i32) {
        let Some(c) = column_of(tag as usize >> 12) else {
            return;
        };
        let p = player_idx(player);
        let base = (key.0 as usize) & 0xfff;
        self.player_cols[p][c][base] = v;
        self.player_cols_present[p][c][base / 64] |= 1u64 << (base % 64);
    }

    /// A present column cell addressed by the **derived** id, for `ir_fill`'s
    /// named probe (`Option` = present).
    pub fn col_get_opt(&self, player: i32, key: NameId) -> Option<i32> {
        let i = key.0 as usize;
        if i < 0x1000 {
            return None;
        }
        let c = column_of(i >> 12)?;
        let base = i & 0xfff;
        if base >= crate::intern::SLOTS {
            return None;
        }
        let p = player_idx(player);
        if ((self.player_cols_present[p][c][base / 64] >> (base % 64)) & 1) == 1 {
            return Some(self.player_cols[p][c][base]);
        }
        None
    }

    /// Visit every present column cell of `player` as `base + tag` ids
    /// (`ir_fill` copies them with the map).
    pub fn for_each_col(&self, player: i32, mut f: impl FnMut(NameId, i32)) {
        let p = player_idx(player);
        for (c, tag) in [0x1000u16, 0x2000, 0x4000, 0x8000].into_iter().enumerate() {
            let mut i = 0usize;
            while i < crate::intern::SLOTS {
                if ((self.player_cols_present[p][c][i / 64] >> (i % 64)) & 1) == 1 {
                    f(
                        NameId((i as u16).wrapping_add(tag)),
                        self.player_cols[p][c][i],
                    );
                }
                i += 1;
            }
        }
    }

    /// Mirror a fan-key write into the pilot `fans_pos` / `fans_neg` views.
    /// `state_get` / `state_add` / `state_set` all land here via `key_*`.
    fn sync_fan_key(&mut self, player: i32, key: NameId) {
        let (pos_id, neg_id) = fan_tok_ids();
        if key == pos_id || key == neg_id {
            self.sync_views_player(player);
        }
    }

    /// Keep `fans_pos` / `fans_neg` / `hand_resonance` in sync with the
    /// generic map when those well-known keys are written.

    // ---- state MIN / MAX / EXPIRES columns (hostfns `state_set` field 1/2/3) ----
    /// Declared MAX for `key` (0 = none). Stored beside the value.
    pub fn key_max(&self, player: i32, key: NameId) -> i32 {
        self.col_get(player, key, 0x8000)
    }
    pub fn key_min(&self, player: i32, key: NameId) -> i32 {
        self.col_get(player, key, 0x4000)
    }
    /// `Tick` code: 0 none / 1 TurnStart / 2 TurnEnd (hostfns field 3).
    pub fn key_expires(&self, player: i32, key: NameId) -> i32 {
        self.col_get(player, key, 0x2000)
    }
    pub fn key_set_max(&mut self, player: i32, key: NameId, v: i32) {
        self.col_set(player, key, 0x8000, v);
    }
    pub fn key_set_min(&mut self, player: i32, key: NameId, v: i32) {
        self.col_set(player, key, 0x4000, v);
    }
    pub fn key_set_expires(&mut self, player: i32, key: NameId, v: i32) {
        self.col_set(player, key, 0x2000, v);
    }

    // ---- field-instance counters (uid-addressed) ----
    pub fn field_crystals(&self, uid: i32) -> i32 {
        if let Some(f) = self.field_slot(uid) {
            return f.crystals;
        }
        self.self_crystals
    }
    pub fn set_field_crystals(&mut self, uid: i32, v: i32) {
        if let Some(f) = self.field_slot_mut(uid) {
            f.crystals = v;
        } else {
            // Event / non-field instance: the counter lives on the running
            // self, not in `field[]`. (heat_head phase, circle_rebuild's 5
            // miracle crystals, …). Reset by `World::new` / `reset_world`.
            self.self_crystals = v;
        }
    }
    pub fn field_cp(&self, uid: i32) -> i32 {
        self.field_slot(uid)
            .map(|f| f.props.get(NameId(1)))
            .unwrap_or(0)
    }
    pub fn field_counter(&self, uid: i32, key: NameId) -> i32 {
        self.field_slot(uid).map(|f| f.props.get(key)).unwrap_or(0)
    }
    pub fn set_field_counter(&mut self, uid: i32, key: NameId, v: i32) {
        if let Some(f) = self.field_slot_mut(uid) {
            f.props.set(key, v);
        }
    }
    fn field_slot(&self, uid: i32) -> Option<&FieldCard> {
        let mut i = 0usize;
        while i < self.field_n as usize {
            if self.field[i].uid == uid {
                return Some(&self.field[i]);
            }
            i += 1;
        }
        None
    }
    fn field_slot_mut(&mut self, uid: i32) -> Option<&mut FieldCard> {
        let mut i = 0usize;
        while i < self.field_n as usize {
            if self.field[i].uid == uid {
                return Some(&mut self.field[i]);
            }
            i += 1;
        }
        None
    }

    /// Swap-remove the field instance `uid`; returns its card id when present.
    /// Used by the host's unplace / deferred-dest (`set_dest`) commit.
    pub fn remove_field(&mut self, uid: i32) -> Option<u16> {
        let mut i = 0usize;
        while i < self.field_n as usize {
            if self.field[i].uid == uid {
                let card = self.field[i].card;
                let last = (self.field_n as usize) - 1;
                self.field[i] = self.field[last];
                self.field[last] = FieldCard::EMPTY;
                self.field_n -= 1;
                return Some(card);
            }
            i += 1;
        }
        None
    }

    /// Allocate a fresh uid and push a plain field row for `owner` on `tile`.
    /// Answers the uid; the running-instance slot becomes it (`last_uid`).
    /// The row is unattached, has no creator and is effective.
    pub fn make_row(&mut self, card: u16, owner: i32, holder: i32, tile: i32) -> i32 {
        let uid = self.next_uid;
        self.next_uid += 1;
        self.last_uid = uid;
        let fi = self.field_n as usize;
        if fi < MAX_FIELD {
            self.field[fi] = FieldCard {
                uid,
                card,
                player: holder,
                owner,
                tile,
                face_down: false,
                immune: false,
                immune_except_owner: false,
                crystals: 0,
                props: NameMap::new(),
                effective_card: u16::MAX,
                on: -1,
                created_by: -1,
                effective: true,
                mine: true,
            };
            self.field_n += 1;
        }
        uid
    }

    /// Plant a field instance as a harness pre-state -- the same row
    /// `arm_place_card` writes: a fresh uid, the running-instance slot and a
    /// `FieldCard` on the owner's tile. Returns the uid; `last_uid` becomes it,
    /// which is also the instance the offer body runs on.
    pub fn place_instance(&mut self, player: i32, card: &str) -> i32 {
        let uid = self.next_uid;
        self.next_uid += 1;
        self.last_uid = uid;
        self.bound_uid = uid;
        let tile = self.pos[player_idx(player)];
        let fi = self.field_n as usize;
        if fi < MAX_FIELD {
            self.field[fi] = FieldCard {
                uid,
                card: intern::intern(card).0,
                player,
                owner: player,
                tile,
                face_down: false,
                immune: false,
                immune_except_owner: false,
                crystals: 0,
                props: NameMap::new(),
                effective_card: u16::MAX,
                on: -1,
                created_by: -1,
                effective: true,
                mine: false,
            };
            self.field_n += 1;
        }
        uid
    }

    /// Refresh `fans_pos` / `fans_neg` for **one** seat from `player_keys`.
    ///
    /// The old `sync_views` walked all 4 seats on every `add_held` /
    /// `set_held_name` (pp setup cliff). Call sites that know the seat
    /// use this; [`Self::sync_views`] remains for a full refresh.
    pub fn sync_views_player(&mut self, player: i32) {
        let (pos_id, neg_id) = fan_tok_ids();
        let i = player_idx(player);
        self.fans_pos[i] = self.player_keys[i].get(pos_id);
        self.fans_neg[i] = self.player_keys[i].get(neg_id);
    }

    pub fn sync_views(&mut self) {
        let (pos_id, neg_id) = fan_tok_ids();
        for p in 0..MAX_PLAYERS {
            self.fans_pos[p] = self.player_keys[p].get(pos_id);
            self.fans_neg[p] = self.player_keys[p].get(neg_id);
        }
    }

    // ---- piles (free functions; methods would double-borrow) ----

    // ---- field-card / tile helpers (mirror `game_core::engine::ops`) ----

    /// Field instance at `uid`, or `None`. Mirrors `field_by_uid`.
    pub fn field_by_uid(&self, uid: i32) -> Option<&FieldCard> {
        let mut i = 0usize;
        let mut found: Option<&FieldCard> = None;
        while i < MAX_FIELD {
            // `card != u16::MAX` marks a live slot: an `EMPTY` slot also has
            // `uid == 0`, and a board row has `player == -1`.
            if self.field[i].uid == uid && self.field[i].card != u16::MAX {
                found = Some(&self.field[i]);
            }
            i += 1;
        }
        found
    }

    pub fn field_by_uid_mut(&mut self, uid: i32) -> Option<&mut FieldCard> {
        let mut i = 0usize;
        let mut idx = None;
        while i < MAX_FIELD {
            if self.field[i].uid == uid && self.field[i].card != u16::MAX {
                idx = Some(i);
            }
            i += 1;
        }
        idx.map(|i| &mut self.field[i])
    }

    // ------------------------------------------------- zone writer (G08)

    /// The zone of an instance: `Attached` while it sits on a host, `Field`
    /// while its row is on a field, `Nowhere` when dead (mirrors
    /// `World::zone_of`).
    pub fn zone_of(&self, uid: i32) -> i32 {
        let Some(f) = self.field_by_uid(uid) else {
            return ZONE_NOWHERE;
        };
        if f.on >= 0 {
            ZONE_ATTACHED
        } else {
            ZONE_FIELD
        }
    }

    /// The instance `uid` sits on, `-1` when it is not attached.
    pub fn attachment_host(&self, uid: i32) -> i32 {
        self.field_by_uid(uid).map_or(-1, |f| f.on)
    }

    pub fn holder_of(&self, uid: i32) -> i32 {
        self.field_by_uid(uid).map_or(-1, |f| f.player)
    }

    /// The direct attachments of `host`, in filing (creation) order, into
    /// `out`; answers the count.
    pub fn attached_into(&self, host: i32, out: &mut [i32; MAX_FIELD]) -> usize {
        let mut n = 0usize;
        let mut i = 0usize;
        while i < MAX_FIELD && n < MAX_FIELD {
            if self.field[i].card != u16::MAX && self.field[i].on == host {
                out[n] = self.field[i].uid;
                n += 1;
            }
            i += 1;
        }
        n
    }

    /// The direct created children of `creator`, in creation order, into
    /// `out`; answers the count.
    pub fn created_into(&self, creator: i32, out: &mut [i32; MAX_FIELD]) -> usize {
        let mut n = 0usize;
        let mut i = 0usize;
        while i < MAX_FIELD && n < MAX_FIELD {
            if self.field[i].card != u16::MAX && self.field[i].created_by == creator {
                out[n] = self.field[i].uid;
                n += 1;
            }
            i += 1;
        }
        n
    }

    /// The row's own leave destination (`prop::LEAVE_DEST`), else the
    /// Graveyard (mirrors `PlayCtx::leave_dest`).
    pub fn leave_dest(&self, uid: i32) -> i32 {
        let key = leave_dest_id();
        let dest = self
            .field_by_uid(uid)
            .and_then(|f| f.props.get_opt(key))
            .unwrap_or(0);
        dest_zone(dest)
    }

    /// Would filing `uid` onto `host` make a cycle? (mirrors
    /// `World::attach_cycles`; the depth guard is the row bound).
    pub fn attach_cycles(&self, uid: i32, host: i32) -> bool {
        if uid == host {
            return true;
        }
        let mut cursor = host;
        let mut guard = 0usize;
        while cursor >= 0 && guard <= MAX_FIELD {
            if cursor == uid {
                return true;
            }
            cursor = self.attachment_host(cursor);
            guard += 1;
        }
        false
    }

    /// Resolve an attach of the live row `uid` onto `host`: answers the real
    /// `from` and the landing holder, `None` when refused (dead row, dead or
    /// non-played host, cycle). Mirrors `World::resolve_zone_transition`.
    pub fn zone_prepare_attach(&self, uid: i32, host: i32) -> Option<(i32, i32)> {
        let row = self.field_by_uid(uid)?;
        let from = self.zone_of(uid);
        if !is_played(from) {
            return None;
        }
        if host < 0 || !is_played(self.zone_of(host)) {
            return None;
        }
        if self.attach_cycles(uid, host) {
            return None;
        }
        Some((from, self.holder_of(host)))
    }

    /// Take the row `uid` out of play into `destination` (`-1` = its own
    /// leave destination). Cascades attachments (cause `HostLeft`) then
    /// created children (cause `CreatorLeft`) depth-first, then files the
    /// leaver. Answers the landing holder, `-1` when it was not in play.
    ///
    /// Mirrors `PlayCtx::zone_leave` / `zone_commit` / `World::drop_instance_edges`
    /// / `World::apply_zone_row` without the hook raise.
    pub fn zone_leave(&mut self, uid: i32, destination: i32, to_seat: i32, cause: i32) -> i32 {
        let from = self.zone_of(uid);
        if self.field_by_uid(uid).is_none() {
            return -1;
        }
        self.zone_leave_from(uid, from, destination, to_seat, cause)
    }

    /// [`Self::zone_leave`] with the `from` the caller already knows. A cascade
    /// clears the edge before it recurses (as `drop_instance_edges` does), and
    /// the transition it built keeps the row's real pair -- so the recursed
    /// leaf must be told its `from` (`Attached` / `Field`), not re-read it.
    fn zone_leave_from(
        &mut self,
        uid: i32,
        from: i32,
        destination: i32,
        to_seat: i32,
        cause: i32,
    ) -> i32 {
        let Some(row) = self.field_by_uid(uid).copied() else {
            return -1;
        };
        if !is_played(from) {
            return -1;
        }
        let mut to = if destination < 0 {
            ZONE_GRAVEYARD
        } else {
            dest_zone(destination)
        };
        // The card's own rewrite wins, except into `Attached`. The engine
        // clears the prop at the window start and re-reads it after the
        // `zoneLeft` body, so only a write made there counts; the model has no
        // hooks, and a seeded prop stands for exactly that write.
        let rewrite = self.leave_dest(uid);
        if rewrite != to && to != ZONE_ATTACHED {
            to = rewrite;
        }
        // A non-pile landing (Field) or an unnamed seat falls back to the
        // owner, exactly as `zone_commit`'s post-rewrite holder fix-up does.
        let holder = if to == ZONE_FIELD || to_seat < 0 {
            row.owner
        } else {
            to_seat
        };
        let wasted = !row.effective;

        // Attachments first, then created children, then the leaver (I5).
        // The edge is cleared before the cascade, as the engine does; the
        // record keeps the row's real `from`, so the recursed leaf is told it.
        let mut edges = [0i32; MAX_FIELD];
        let atts = self.attached_into(uid, &mut edges);
        let mut i = 0usize;
        while i < atts {
            let a = edges[i];
            if let Some(f) = self.field_by_uid_mut(a) {
                f.on = -1;
            }
            let d = self.leave_dest(a);
            self.zone_leave_from(a, ZONE_ATTACHED, d, -1, CAUSE_HOST_LEFT);
            i += 1;
        }
        let kids = self.created_into(uid, &mut edges);
        let mut i = 0usize;
        while i < kids {
            let c = edges[i];
            if let Some(f) = self.field_by_uid_mut(c) {
                f.created_by = -1;
            }
            self.zone_leave_from(c, ZONE_FIELD, 0, -1, CAUSE_CREATOR_LEFT);
            i += 1;
        }

        self.trig_zone_present = true;
        self.trig_zone_from = from;
        self.trig_zone_to = to;
        self.trig_zone_cause = cause;
        self.trig_zone_holder = holder;
        self.trig_zone_wasted = wasted;
        self.trig_zone_counted = false;
        self.push(Effect::ZoneLeave {
            uid,
            from,
            to,
            cause,
        });

        if to == ZONE_ATTACHED {
            // Only a prepared attach reaches here; the host's tile is taken.
            let Some((host, tile)) = self
                .field_by_uid(row.on)
                .map(|h| (h.uid, h.tile))
                .filter(|_| row.on >= 0)
            else {
                return -1;
            };
            let h = self.holder_of(host);
            self.remove_field(uid);
            let mut moved = row;
            moved.on = host;
            moved.tile = tile;
            self.push_field_row(moved, h);
            return h;
        }
        let holder_now = row.player;
        self.remove_field(uid);
        if to == ZONE_FIELD {
            let mut moved = row;
            moved.on = -1;
            self.push_field_row(moved, holder_now);
            return holder_now;
        }
        self.file_row(row, to, holder);
        if to == ZONE_GRAVEYARD {
            // The entry read the bit and the landing cleared it.
            self.last_leave_effective_after = true;
        }
        holder.max(0)
    }

    /// Put a row back on a field, keyed by holder (`-1` = the board field).
    fn push_field_row(&mut self, row: FieldCard, holder: i32) {
        let fi = self.field_n as usize;
        if fi < MAX_FIELD {
            self.field[fi] = row;
            let _ = holder;
            self.field_n += 1;
        }
    }

    /// File a removed row into a pile; the row dies. A Graveyard landing reads
    /// and clears the `effective` bit (I8) and records the post-state.
    pub fn file_row(&mut self, row: FieldCard, to: i32, _holder: i32) {
        match to {
            ZONE_GRAVEYARD => {
                let mut row = row;
                // Read-then-clear: the entry's `wasted` bit was taken before
                // the row moved, and the bit dies with it (I8).
                row.effective = true;
                pile_push(&mut self.discard_pile, &mut self.discard_n, row.card);
            }
            ZONE_HAND => pile_push(&mut self.hand_pile, &mut self.hand_n, row.card),
            ZONE_DECK => pile_push(&mut self.deck_pile, &mut self.deck_n, row.card),
            _ => {}
        }
    }

    /// `set_tile_owner` -- write the deed row and its acquisition sequence,
    /// then rebuild the previous owner's `firstTile` slot (I13).
    pub fn set_tile_owner(&mut self, tile: i32, owner: i32) {
        let Ok(t) = usize::try_from(tile) else {
            return;
        };
        if t >= 60 {
            return;
        }
        let cur = self.tiles[t].owner;
        if cur == owner {
            return;
        }
        self.tiles[t].owner = owner;
        self.owned_seq[t] = if owner >= 0 {
            let s = self.next_owned_seq;
            self.next_owned_seq = s.saturating_add(1);
            s
        } else {
            -1
        };
        if cur >= 0 {
            let first = self.earliest_owned_tile(cur);
            let p = player_idx(cur);
            self.first_tile[p] = first;
        }
        if owner >= 0 {
            let first = self.earliest_owned_tile(owner);
            let p = player_idx(owner);
            self.first_tile[p] = first;
        }
    }

    /// The player's earliest-owned tile by acquisition sequence, `-1` if none.
    pub fn earliest_owned_tile(&self, player_id: i32) -> i32 {
        let mut best_seq = i32::MAX;
        let mut best_tile = -1;
        let mut t = 0usize;
        while t < 60 {
            if self.tiles[t].owner == player_id && self.owned_seq[t] >= 0 {
                if self.owned_seq[t] < best_seq {
                    best_seq = self.owned_seq[t];
                    best_tile = t as i32;
                }
            }
            t += 1;
        }
        best_tile
    }

    /// Every **unattached** live row held by `player` (`-1` = board field),
    /// in filing order, into `out`; answers the count (I6).
    pub fn field_instances_into(&self, player: i32, out: &mut [i32; MAX_FIELD]) -> usize {
        let mut n = 0usize;
        let mut i = 0usize;
        while i < MAX_FIELD && n < MAX_FIELD {
            let f = &self.field[i];
            if f.card != u16::MAX && f.player == player && f.on < 0 {
                out[n] = f.uid;
                n += 1;
            }
            i += 1;
        }
        n
    }

    /// Every unattached live board rule on `tile` (I6).
    pub fn tile_rule_instances_into(&self, tile: i32, out: &mut [i32; MAX_FIELD]) -> usize {
        let mut n = 0usize;
        let mut i = 0usize;
        while i < MAX_FIELD && n < MAX_FIELD {
            let f = &self.field[i];
            if f.card != u16::MAX && f.player == -1 && f.tile == tile && f.on < 0 {
                out[n] = f.uid;
                n += 1;
            }
            i += 1;
        }
        n
    }

    /// `hostfns::tile_at` — where the instance at `uid` sits, or -1.
    pub fn tile_at(&self, uid: i32) -> i32 {
        self.field_by_uid(uid).map_or(-1, |f| f.tile)
    }

    /// `hostfns::set_tile_at` — move instance to `tile` (-1 = with owner).
    /// Returns whether the instance exists.
    pub fn set_tile_at(&mut self, uid: i32, tile: i32) -> bool {
        match self.field_by_uid_mut(uid) {
            Some(f) => {
                f.tile = tile;
                true
            }
            None => false,
        }
    }

    /// `WorldOps::rent_houses` — the house count a rent lookup reads.
    /// Tile-scoped `RENT_HOUSES` prop wins; then a placed card of the tile's
    /// owner; else the standing house count. A stored override of `0` still
    /// wins (matches `if let Some(&v) = …` in `ops.rs`).
    pub fn rent_houses(&self, tile: i32) -> i32 {
        let key = rent_houses_id();
        // 1) tile-scoped override (a field card sitting on this tile)
        let mut i = 0usize;
        while i < MAX_FIELD {
            let f = &self.field[i];
            if f.player >= 0 && f.tile == tile {
                if let Some(v) = f.props.get_opt(key) {
                    return v.max(0);
                }
            }
            i += 1;
        }
        // 2) placed card of the owner (「你的所有格子上的房屋数视为…」)
        if tile >= 0 && (tile as usize) < 60 {
            let owner = self.tiles[tile as usize].owner;
            if owner >= 0 && (owner as usize) < MAX_PLAYERS {
                let mut j = 0usize;
                while j < MAX_FIELD {
                    let f = &self.field[j];
                    if f.player == owner {
                        if let Some(v) = f.props.get_opt(key) {
                            return v.max(0);
                        }
                    }
                    j += 1;
                }
            }
            return self.tiles[tile as usize].houses;
        }
        0
    }

    /// `WorldOps::rent_of` + `purchase::table_rent` (see `board::table_rent`).
    /// RiNG rent is the tile's base `price` (rolled at payment time; the
    /// table value is the base). Empty tables answer 0.
    pub fn rent_of(&self, tile: i32) -> i32 {
        let Some(t) = board::data_at(tile) else {
            return 0;
        };
        if board::is_deed(t) {
            t.price
        } else {
            board::table_rent(t, self.rent_houses(tile))
        }
    }
}

/// Push `card` onto a bounded pile.
pub fn pile_push(pile: &mut [u16; MAX_CARDS], n: &mut u8, card: u16) {
    if (*n as usize) < MAX_CARDS {
        pile[*n as usize] = card;
        *n += 1;
    }
}

/// Push `card` onto the **front** of a bounded pile (the `arm_cards_in` deck
/// order is front-first). Drops when full.
pub fn pile_unshift(pile: &mut [u16; MAX_CARDS], n: &mut u8, card: u16) {
    if (*n as usize) >= MAX_CARDS {
        return;
    }
    let mut i = *n as usize;
    while i > 0 {
        pile[i] = pile[i - 1];
        i -= 1;
    }
    pile[0] = card;
    *n += 1;
}

/// Move up to `k` cards off the **front** of `src` onto the front of `dst`, in
/// order. Returns how many moved.
pub fn pile_shift_front(
    src: &mut [u16; MAX_CARDS],
    src_n: &mut u8,
    k: usize,
    dst: &mut [u16; MAX_CARDS],
    dst_n: &mut u8,
) -> usize {
    let mut moved = 0usize;
    while moved < k && moved < *src_n as usize {
        moved += 1;
    }
    // Reverse so the first card stays first on `dst`.
    let mut j = moved;
    while j > 0 {
        j -= 1;
        pile_unshift(dst, dst_n, src[j]);
    }
    // Close the gap: shift the remainder down.
    let mut i = moved;
    while i < *src_n as usize {
        src[i - moved] = src[i];
        i += 1;
    }
    *src_n -= moved as u8;
    moved
}

/// Remove the first `card` from a bounded pile. Returns whether it was present.
pub fn pile_take(pile: &mut [u16; MAX_CARDS], n: &mut u8, card: u16) -> bool {
    let mut i = 0usize;
    let mut found = false;
    let mut out_n = 0u8;
    let mut tmp = [u16::MAX; MAX_CARDS];
    while i < *n as usize {
        if !found && pile[i] == card {
            found = true;
        } else {
            tmp[out_n as usize] = pile[i];
            out_n += 1;
        }
        i += 1;
    }
    if found {
        *pile = tmp;
        *n = out_n;
    }
    found
}

/// Count copies of `card` in a bounded pile.
pub fn pile_count(pile: &[u16; MAX_CARDS], n: u8, card: u16) -> i32 {
    let mut i = 0usize;
    let mut c = 0i32;
    while i < n as usize {
        if pile[i] == card {
            c += 1;
        }
        i += 1;
    }
    c
}

// ---------------------------------------------------------------------------
// Global cell
// ---------------------------------------------------------------------------

// Single-threaded model. A `static mut` avoids `thread_local!`, which Kani
// only partially models (and which contributed to an ICE in kani-compiler
// 0.68.0 during the pilot).
static mut WORLD: World = World::new();

fn world_mut() -> &'static mut World {
    // Const-initialized: no `Option` branch and no per-harness `World::new()`
    // store sequence (that init emitted hundreds of bounds checks and was a
    // large share of the CBMC property count).
    unsafe { &mut *core::ptr::addr_of_mut!(WORLD) }
}

/// Read the global world. `inline(always)` so a host arm's closure is a
/// direct field access — no `World` copy per host call (op-subset batch).
#[inline(always)]
pub fn with_world<R>(f: impl FnOnce(&World) -> R) -> R {
    f(world_mut())
}

/// Mutate the global world. See [`with_world`].
#[inline(always)]
pub fn with_world_mut<R>(f: impl FnOnce(&mut World) -> R) -> R {
    f(world_mut())
}

pub fn reset_world(w: World) {
    *world_mut() = w;
}

/// Open a new activation, mirroring `Match::start`'s bump before the routine
/// snapshot (`crates/game-core/src/engine/mod.rs`). A proof that models a
/// second body landing after the first calls this between the two.
pub fn begin_activation() {
    with_world_mut(|w| w.turn_activation_ordinal += 1);
}

/// Snapshot for condition/post comparison. Effects and narration are the log.
/// Field names are stable so existing proofs compile.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Snapshot {
    pub n_players: i32,
    pub money: [i32; MAX_PLAYERS],
    pub out: [bool; MAX_PLAYERS],
    pub pos: [i32; MAX_PLAYERS],
    pub charge: [i32; MAX_PLAYERS],
    pub stay: [i32; MAX_PLAYERS],
    pub stun: [i32; MAX_PLAYERS],
    pub exile: [i32; MAX_PLAYERS],
    pub fans_pos: [i32; MAX_PLAYERS],
    pub fans_neg: [i32; MAX_PLAYERS],
    pub hand_resonance: [i32; MAX_PLAYERS],
    pub discard: [bool; MAX_CARDS],
    pub hand: [bool; MAX_CARDS],
    pub trig_player: i32,
    pub trig_value: i32,
    pub pay_amount: i32,
    pub log_n: usize,
}

pub fn snapshot() -> Snapshot {
    with_world(|w| Snapshot {
        n_players: w.n_players,
        money: w.money,
        out: w.out,
        pos: w.pos,
        charge: w.charge,
        stay: w.stay,
        stun: w.stun,
        exile: w.exile,
        fans_pos: w.fans_pos,
        fans_neg: w.fans_neg,
        hand_resonance: w.hand_resonance,
        discard: w.discard,
        hand: w.hand,
        trig_player: w.trig_player,
        trig_value: w.trig_value,
        pay_amount: w.pay_amount,
        log_n: w.log_n,
    })
}

/// Element-wise array equality — avoids `memcmp` (a 16-byte `[i32;4]::eq`
/// unrolls past unwind 16). Class A on moca_01 / aao_01 / ex_quest_01.
pub fn arr4_eq(a: [i32; 4], b: [i32; 4]) -> bool {
    a[0] == b[0] && a[1] == b[1] && a[2] == b[2] && a[3] == b[3]
}
/// Element-wise `[bool; 4]` equality (`Snapshot::out`).
pub fn arr4b_eq(a: [bool; 4], b: [bool; 4]) -> bool {
    a[0] == b[0] && a[1] == b[1] && a[2] == b[2] && a[3] == b[3]
}
/// Element-wise `[bool; 8]` equality (`Snapshot::hand`).
pub fn arr8b_eq(a: [bool; 8], b: [bool; 8]) -> bool {
    a[0] == b[0]
        && a[1] == b[1]
        && a[2] == b[2]
        && a[3] == b[3]
        && a[4] == b[4]
        && a[5] == b[5]
        && a[6] == b[6]
        && a[7] == b[7]
}

#[cfg(test)]
mod namemap_tests {
    use super::*;
    use crate::intern;

    #[test]
    #[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
    fn direct_index_roundtrip() {
        intern::reset();
        let mut m = NameMap::new();
        let k = intern::intern("PAREO标记");
        eprintln!("key={:?} raw={} max={}", k, k.0, crate::intern::SLOTS);
        assert!(k.is_valid(), "intern failed");
        assert!(m.get_opt(k).is_none());
        m.set(k, 2);
        assert_eq!(m.get_opt(k), Some(2));
        assert_eq!(m.get(k), 2);
        let mut seen = 0;
        m.for_each(|key, v| {
            assert_eq!(key, k);
            assert_eq!(v, 2);
            seen += 1;
        });
        assert_eq!(seen, 1);
    }
}
