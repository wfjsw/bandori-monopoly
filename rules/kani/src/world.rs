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
/// Tile marks.
pub const MAX_MARKS: usize = 16;
/// Active event ids.
pub const MAX_EVENTS: usize = 8;

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

/// `game_core::state::prop::RENT_HOUSES` — house-count override for rent.
pub const RENT_HOUSES: &str = "rentHouses";

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
    pub resolve: bool,
    /// Direction: 1 forward / -1 backward (guest + CEL encoding).
    pub dir: i32,
    /// 移动起点 (origin tile, 0-based).
    pub from: i32,
    /// Steps the move has left to walk.
    pub remaining: i32,
    /// Path length.
    pub total: i32,
    /// `t.Move.Roll`; `-1` = no face to show (`None` in the engine).
    pub roll: i32,
    /// Per-card counters on the move (a [火罐] roll is card-owned state).
    pub tags: NameMap,
    /// Was this settle initiated by a card (`card_settle_at`)?
    pub settle_redirect: bool,
}

impl MoveSnap {
    pub const NONE: MoveSnap = MoveSnap {
        kind: -1,
        resolve: false,
        dir: 1,
        from: -1,
        remaining: 0,
        total: 0,
        roll: -1,
        tags: NameMap::new(),
        settle_redirect: false,
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
/// Presence is a `u32` bitset over `intern::SLOTS` (64) buckets.
/// `NameId::INVALID` / out-of-range ids never alias a live key: `get_opt`
/// is `None`, `set` is a documented no-op.
/// Extra (key, value) slots for derived ids (`key + 0x2000/0x4000/0x8000`
/// MIN/MAX/EXPIRES columns) that live outside the intern bucket range.
const MAX_EXTRA: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NameMap {
    vals: [i32; crate::intern::SLOTS],
    /// Bit `i` set = key `NameId(i)` is present (even if `vals[i] == 0`).
    present: u64,
    /// Linear fallback for `NameId.0 >= SLOTS` (derived MIN/MAX/EXPIRES).
    extra: [(NameId, i32); MAX_EXTRA],
    extra_n: u8,
}

impl NameMap {
    pub const fn new() -> Self {
        Self {
            vals: [0; crate::intern::SLOTS],
            present: 0,
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
            if ((self.present >> i) & 1) == 1 {
                return Some(self.vals[i]);
            }
            return None;
        }
        // Derived MIN/MAX/EXPIRES ids — tiny linear table (≤ MAX_EXTRA).
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
            self.present |= 1u64 << i;
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
        // 8×8 chunk walk over the 64 intern slots (unwind bound 8, not 64).
        let mut outer = 0usize;
        while outer < 8 {
            let mut k = 0usize;
            while k < 8 {
                let i = outer * 8 + k;
                if i < crate::intern::SLOTS && ((self.present >> i) & 1) == 1 {
                    f(NameId(i as u16), self.vals[i]);
                }
                k += 1;
            }
            outer += 1;
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
    pub player: i32,
    pub tile: i32,
    pub face_down: bool,
    pub immune: bool,
    pub crystals: i32,
    pub props: NameMap,
}

impl FieldCard {
    pub const EMPTY: FieldCard = FieldCard {
        uid: 0,
        card: u16::MAX,
        player: -1,
        tile: -1,
        face_down: false,
        immune: false,
        crystals: 0,
        props: NameMap::new(),
    };
}

// ---------------------------------------------------------------------------
// Tile runtime state
// ---------------------------------------------------------------------------

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
    /// Generic per-player named counters (tokens, held, state).
    pub player_keys: [NameMap; MAX_PLAYERS],
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

    // ---- field cards ----
    pub field: [FieldCard; MAX_FIELD],
    pub field_n: u8,
    pub next_uid: i32,
    /// Pending `place_card` destination (`ctx::set_dest`).
    pub dest: i32,
    pub last_uid: i32,
    /// Crystals of the running instance when it has no `field[]` slot
    /// (events: heat_head phase, circle_rebuild's 5 miracle crystals).
    pub self_crystals: i32,

    // ---- tiles ----
    pub tiles: [TileState; 60],

    // ---- marks / events ----
    pub marks: [Mark; MAX_MARKS],
    pub marks_n: u8,
    pub events: [u16; MAX_EVENTS],
    pub events_n: u8,
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
    /// `id_of` of the same spelling (`VarTy::Str`).
    pub trig_kind: &'static str,
    pub trig_target: i32,
    pub trig_tile: i32,
    pub trig_cancelled: bool,
    pub trig_negate: bool,
    /// 真奈 join (CH-05): the seat a joiner's self-targeted effects are
    /// redirected to; `-1` outside a join (hostfns `join_redirect_target`).
    pub trig_join_redirect: i32,
    /// When non-zero, `roll_dice` returns this value instead of a symbolic
    /// face (harness control for dice-clause proofs; 0 = symbolic).
    pub forced_roll: i32,
    /// A joiner used the redirect (hostfns `set_join_used`).
    pub trig_join_used: bool,
    /// `Trigger::main` — fired as part of the turn's [主要移动]?
    /// (`hostfns::trig_move_main` reads this, not `mv.main`.)
    pub trig_main: bool,
    /// `Trigger::roll_source` guest code (`ROLL_*`).
    pub trig_roll_source: i32,
    /// `Trigger::pay.is_rent` — pay-only fact (`hostfns::trig_pay_is_rent`).
    pub trig_pay_is_rent: bool,
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
    /// Bands currently blocked from skills (`ir_fill` `BlockedBands`).
    /// Non-zero entries are band name ids.
    pub blocked_bands: [i32; 4],
    pub blocked_bands_n: u8,

    // ---- cross-card message context (`ctx::send` / `On::Message`) ----
    /// Interned message name id (`mark:cp`'s `"place"` / …). 0 = none.
    pub msg_name: u16,
    /// `Message.seat` — the holder / subject seat of the payload.
    pub msg_seat: i32,
    pub msg_a: i32,
    pub msg_b: i32,
    pub msg_c: i32,
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

impl World {
    pub fn new() -> Self {
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
            player_keys: [NameMap::new(); MAX_PLAYERS],
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
            field: [FieldCard::EMPTY; MAX_FIELD],
            field_n: 0,
            next_uid: 1,
            dest: 0,
            last_uid: 1,
            self_crystals: 0,
            tiles: [TileState::new(); 60],
            marks: [Mark::EMPTY; MAX_MARKS],
            marks_n: 0,
            events: [u16::MAX; MAX_EVENTS],
            events_n: 0,
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
            trig_cancelled: false,
            trig_negate: false,
            trig_join_redirect: -1,
            forced_roll: 0,
            trig_join_used: false,
            trig_main: false,
            trig_roll_source: ROLL_UNATTRIBUTED,
            trig_pay_is_rent: false,
            move_snap: MoveSnap::NONE,
            buy_snap: BuySnap::NONE,
            trig_card: 0,
            counter_name: 0,
            plan_fixed_roll: -1,
            blocked_bands: [0; 4],
            blocked_bands_n: 0,
            msg_name: 0,
            msg_seat: -1,
            msg_a: 0,
            msg_b: 0,
            msg_c: 0,
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
    /// `state_set_flags`. Matches the engine's floor in `MatchPlayer::state_set`.
    pub fn key_never_negative(&self, player: i32, key: NameId) -> bool {
        self.key_flags(player, key) & 1 != 0
    }

    /// Flag byte declared for `key` (0 when none). Stored at `key + 0x1000`,
    /// beside the MIN/MAX/EXPIRES derived columns.
    pub fn key_flags(&self, player: i32, key: NameId) -> u8 {
        if player < 0 || player as usize >= MAX_PLAYERS {
            return 0;
        }
        let id = NameId(key.0.wrapping_add(0x1000));
        self.player_keys[player as usize].get(id) as u8
    }

    /// Declare [`state_flag`] bits for `key` (hostfns `state_set_flags`).
    pub fn key_set_flags(&mut self, player: i32, key: NameId, flags: u8) {
        if player < 0 || player as usize >= MAX_PLAYERS {
            return;
        }
        let id = NameId(key.0.wrapping_add(0x1000));
        self.player_keys[player as usize].set(id, flags as i32);
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
        let p = player_idx(player);
        let id = NameId(key.0.wrapping_add(0x8000));
        self.player_keys[p].get(id)
    }
    pub fn key_min(&self, player: i32, key: NameId) -> i32 {
        let p = player_idx(player);
        let id = NameId(key.0.wrapping_add(0x4000));
        self.player_keys[p].get(id)
    }
    /// `Tick` code: 0 none / 1 TurnStart / 2 TurnEnd (hostfns field 3).
    pub fn key_expires(&self, player: i32, key: NameId) -> i32 {
        let p = player_idx(player);
        let id = NameId(key.0.wrapping_add(0x2000));
        self.player_keys[p].get(id)
    }
    pub fn key_set_max(&mut self, player: i32, key: NameId, v: i32) {
        let p = player_idx(player);
        let id = NameId(key.0.wrapping_add(0x8000));
        self.player_keys[p].set(id, v);
    }
    pub fn key_set_min(&mut self, player: i32, key: NameId, v: i32) {
        let p = player_idx(player);
        let id = NameId(key.0.wrapping_add(0x4000));
        self.player_keys[p].set(id, v);
    }
    pub fn key_set_expires(&mut self, player: i32, key: NameId, v: i32) {
        let p = player_idx(player);
        let id = NameId(key.0.wrapping_add(0x2000));
        self.player_keys[p].set(id, v);
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
    /// Used by the host's unplace / deferred-fate (`set_dest`) commit.
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
            if self.field[i].uid == uid && self.field[i].player >= 0 {
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
            if self.field[i].uid == uid && self.field[i].player >= 0 {
                idx = Some(i);
            }
            i += 1;
        }
        idx.map(|i| &mut self.field[i])
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
static mut WORLD: Option<World> = None;

fn world_mut() -> &'static mut World {
    unsafe {
        if WORLD.is_none() {
            WORLD = Some(World::new());
        }
        WORLD.as_mut().unwrap_unchecked()
    }
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

/// Snapshot for pre/post comparison. Effects and narration are the log.
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
