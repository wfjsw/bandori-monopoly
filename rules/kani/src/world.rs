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
//!   `hand_resonance` fields remain as *views* so existing proofs compile.
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
/// Named keys per map (tokens / slots / state / props). **8** is the model
/// bound: a full map is a documented cut (the old `set` overwrite-slot-0
/// behaviour). The loop trip count is `MAX_KEYS` itself, so a harness with
/// `#[kani::unwind(8)]` covers every `NameMap` walk — `while i < self.n`
/// would still demand `unwind >= 255` for a `u8` index.
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
pub fn card_slot_of_bytes(b: &[u8]) -> u8 {
    fn eq(a: &[u8], lit: &str) -> bool {
        let e = lit.as_bytes();
        if a.len() != e.len() {
            return false;
        }
        let mut i = 0usize;
        let mut ok = true;
        while i < e.len() {
            if a[i] != e[i] {
                ok = false;
            }
            i += 1;
        }
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

pub const TOK_PP_POS: &str = "P✽P粉丝(正)";
pub const TOK_PP_NEG: &str = "P✽P粉丝(反)";
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
pub const ROLL_FIRE: i32 = 1;
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

/// One `(key, value)` slot. Empty slots have `key == NameId::INVALID`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

/// A fixed-capacity map from interned name → i32.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NameMap {
    pub slots: [NamedVal; MAX_KEYS],
    pub n: u8,
}

impl NameMap {
    pub const fn new() -> Self {
        Self {
            slots: [NamedVal::EMPTY; MAX_KEYS],
            n: 0,
        }
    }

    pub fn get(&self, key: NameId) -> i32 {
        self.get_opt(key).unwrap_or(0)
    }

    /// `Some(value)` when `key` is present (even if the value is 0).
    /// `set` never inserts a duplicate key, so first-match is the match.
    /// Loop bound is [`MAX_KEYS`] (not `self.n`) so CBMC's unwinding
    /// assertion is satisfied at `unwind == MAX_KEYS`.
    pub fn get_opt(&self, key: NameId) -> Option<i32> {
        let mut i = 0usize;
        while i < MAX_KEYS {
            if i < self.n as usize && self.slots[i].key == key {
                return Some(self.slots[i].value);
            }
            i += 1;
        }
        None
    }

    /// Set `key` to `value`, inserting if needed. Returns the value.
    /// A full table overwrites slot 0 (bounded, documented).
    pub fn set(&mut self, key: NameId, value: i32) -> i32 {
        let mut i = 0usize;
        while i < MAX_KEYS {
            if i < self.n as usize && self.slots[i].key == key {
                self.slots[i].value = value;
                return value;
            }
            i += 1;
        }
        if (self.n as usize) < MAX_KEYS {
            self.slots[self.n as usize] = NamedVal { key, value };
            self.n += 1;
        } else {
            self.slots[0] = NamedVal { key, value };
        }
        value
    }

    pub fn add(&mut self, key: NameId, delta: i32) -> i32 {
        let v = self.get(key).saturating_add(delta);
        self.set(key, v)
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

pub struct World {
    // ---- players ----
    pub n_players: i32,
    pub money: [i32; MAX_PLAYERS],
    pub out: [bool; MAX_PLAYERS],
    pub pos: [i32; MAX_PLAYERS],
    pub fire: [i32; MAX_PLAYERS],
    pub fire_max: [i32; MAX_PLAYERS],
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
    pub character_id: [u16; MAX_PLAYERS],
    pub band_id: [u16; MAX_PLAYERS],
    /// Generic per-player named counters (tokens, held, state).
    pub player_keys: [NameMap; MAX_PLAYERS],
    /// Band crystals / skills (kept explicit; also mirrored in player_keys).
    pub band_crystals: [i32; MAX_PLAYERS],

    // ---- compatibility views for the pilot proofs ----
    /// `P✽P粉丝(正)` / `(反)` counts (also in `player_keys`).
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
    pub trig_kind: i32,
    pub trig_target: i32,
    pub trig_tile: i32,
    pub trig_cancelled: bool,
    pub trig_negate: bool,
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
            fire: [0; MAX_PLAYERS],
            fire_max: [3; MAX_PLAYERS],
            stay: [0; MAX_PLAYERS],
            stun: [0; MAX_PLAYERS],
            exile: [0; MAX_PLAYERS],
            exile_to: [-1; MAX_PLAYERS],
            extra_turns: [false; MAX_PLAYERS],
            play_doubled: -1,
            character_id: [u16::MAX; MAX_PLAYERS],
            band_id: [u16::MAX; MAX_PLAYERS],
            player_keys: [NameMap::new(); MAX_PLAYERS],
            band_crystals: [0; MAX_PLAYERS],
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
            trig_kind: 0,
            trig_target: -1,
            trig_tile: -1,
            trig_cancelled: false,
            trig_negate: false,
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
        self.player_keys[player as usize].set(key, v);
    }

    pub fn key_add(&mut self, player: i32, key: NameId, delta: i32) -> i32 {
        if player < 0 || player as usize >= MAX_PLAYERS {
            return 0;
        }
        self.player_keys[player as usize].add(key, delta)
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
        self.field_slot(uid).map(|f| f.crystals).unwrap_or(0)
    }
    pub fn set_field_crystals(&mut self, uid: i32, v: i32) {
        if let Some(f) = self.field_slot_mut(uid) {
            f.crystals = v;
        }
    }
    pub fn field_cp(&self, uid: i32) -> i32 {
        self.field_slot(uid).map(|f| f.props.get(NameId(1))).unwrap_or(0)
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

    pub fn sync_views(&mut self) {
        let pos_id = intern::intern(TOK_PP_POS);
        let neg_id = intern::intern(TOK_PP_NEG);
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
        let key = intern::intern(RENT_HOUSES);
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
        if board::is_ring(t) {
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

pub fn with_world<R>(f: impl FnOnce(&World) -> R) -> R {
    f(world_mut())
}

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
    pub fire: [i32; MAX_PLAYERS],
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
    with_world(|w| {
        Snapshot {
            n_players: w.n_players,
            money: w.money,
            out: w.out,
            pos: w.pos,
            fire: w.fire,
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
        }
    })
}