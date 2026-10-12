//! Context the IR interpreter reads. Integer-only, fixed capacity, no heap.
//!
//! The Kani host fills this from its symbolic world; the differential tests
//! fill it from a synthetic view. Every function goes through one narrow
//! accessor family so a proof can model it without the `cel` crate.

use crate::vars::{VarId, VarTy, SCALAR_COUNT, VARS};
use crate::Val;

/// Vocabulary functions (after `effect.has` → `chain_has` rewrite).
/// Order is stable; the discriminant is the IR `Call` payload.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum FnId {
    Slot = 0,
    Tok,
    MoveTag,
    CardCounter,
    CounterIs,
    TileNamed,
    IsCircle,
    IsRing,
    IsLiveHouse,
    IsBuyable,
    Money,
    Fire,
    Crystals,
    Hand,
    Pos,
    Character,
    Band,
    Tiles,
    CharacterIs,
    BandIs,
    Blocked,
    Neighbor,
    ChainHas,
    ChainHits,
    GainsThisTurn,
    TargetedCount,
    RepeatedDigits,
    Dist,
    PlayersOn,
    NextDist,
    OthersWithin,
    OwnedWithin,
    OnPath,
    Between,
}

/// Per-seat scalar fields (index into `IrCtx::seat`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum SeatField {
    Money = 0,
    Fire,
    Crystals,
    Hand,
    Pos,
    Out,
    Stay,
    Stun,
    Exile,
    NoHand,
    Character,
    Band,
    Tiles,
    Gains,
    Targeted,
    Between,
}

pub const SEAT_FIELD_COUNT: usize = 16;
pub const MAX_PLAYERS: usize = 4;

/// Membership sets backing `is_*` / `chain_has` / `chain_hits` / `blocked`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum SetId {
    Circle = 0,
    Ring,
    LiveHouse,
    Buyable,
    ChainKinds,
    ChainHits,
    BlockedBands,
}

pub const SET_COUNT: usize = 7;
/// Bits per set; tile/kind ids used in practice are well under this.
pub const SET_BITS: usize = 128;

/// Named counters (`slot` / `tok` / `card_counter` / `tile_named`): a fixed
/// linear table of `(name_id, value)`. `name_id` is `rules_cond::id_of`.
/// Missing name answers the function's documented default.
///
/// Sized for the full board name table (60) plus the owner's live tokens /
/// slots / card counters — `fill_window` registers every `tile_named`, so the
/// IR twin must not drop names (the guard-fill differential pins this).
pub const NAMED_CAP: usize = 80;

/// Bin-map tables for the geometry scans (`players_on`, `next_dist`,
/// `others_within`, `owned_within`, `on_path`, `between`, `dist`): packed
/// `(a * 1024 + b) -> v`, same packing as `rules-cond`'s eager fill.
pub const BIN_CAP: usize = 64;

#[derive(Copy, Clone, Debug)]
pub struct IrCtx {
    /// Flat scalars ([`VarId`] as index), raw i64. `OptInt` uses `-1` = null;
    /// `Bool` uses 0/1.
    pub scalars: [i64; SCALAR_COUNT],
    /// Per-seat fields ([`SeatField`] as index), seat 0..3.
    pub seat: [[i64; SEAT_FIELD_COUNT]; MAX_PLAYERS],
    pub n_players: i64,
    /// `(name_id, value)` pairs; `named_n` live entries.
    pub named: [(i64, i64); NAMED_CAP],
    pub named_n: u8,
    /// Bitsets ([`SetId`] as index).
    pub sets: [[u64; SET_BITS / 64]; SET_COUNT],
    /// Board tile count — backs `dist` / ring wraps.
    pub tile_count: i64,
    /// Packed `(a,b) -> v` geometry.
    pub bin: [(i64, i64); BIN_CAP],
    pub bin_n: u8,
}

impl Default for IrCtx {
    fn default() -> Self {
        IrCtx {
            scalars: [0; SCALAR_COUNT],
            seat: [[0; SEAT_FIELD_COUNT]; MAX_PLAYERS],
            n_players: 0,
            named: [(0, 0); NAMED_CAP],
            named_n: 0,
            sets: [[0; SET_BITS / 64]; SET_COUNT],
            tile_count: 0,
            bin: [(0, 0); BIN_CAP],
            bin_n: 0,
        }
    }
}

impl IrCtx {
    pub fn set_scalar(&mut self, id: VarId, v: i64) {
        self.scalars[id as usize] = v;
    }

    /// Store a typed scalar: `Bool` → 0/1. `OptInt` stores the raw i64; any
    /// negative reads back as `Null` (matching `vocab::opt`).
    pub fn set_scalar_ty(&mut self, id: VarId, ty: VarTy, v: i64, is_null: bool) {
        let raw = match (ty, is_null) {
            (VarTy::Bool, _) => {
                if v != 0 {
                    1
                } else {
                    0
                }
            }
            // A null OptInt is stored as any negative (`-1` by convention).
            (VarTy::OptInt, true) => -1,
            (_, _) => v,
        };
        self.set_scalar(id, raw);
    }

    pub fn scalar_val(&self, idx: u16) -> Val {
        let (id, ty) = match VARS.get(idx as usize) {
            Some((id, _, ty)) => (*id, *ty),
            None => return Val::Err,
        };
        // `id` is the VarId at this index; the table is kept in discriminant
        // order by `ir_vars_match_vocab`.
        let _ = id;
        let raw = self.scalars[idx as usize];
        match ty {
            VarTy::Int => Val::Int(raw),
            VarTy::Bool => Val::Bool(raw != 0),
            // Host rule (`vocab::opt`): **any negative** binds CEL `null`, not
            // just `-1`. Confirmed by the differential tests against the real
            // evaluator — see COND-SINGLE-SOURCE.md findings.
            VarTy::OptInt => {
                if raw < 0 {
                    Val::Null
                } else {
                    Val::Int(raw)
                }
            }
        }
    }

    pub fn seat_field(&self, seat: i64, f: SeatField) -> i64 {
        if seat < 0 || seat >= self.n_players {
            return 0;
        }
        self.seat[seat as usize][f as usize]
    }

    pub fn named_get(&self, name_id: i64) -> Option<i64> {
        // 10×8 = 80 = NAMED_CAP chunk walk (unwind bound 10). Must cover the
        // whole table: `fill_from_world` inserts 60 tile names before the
        // card counters, so a 64-slot walk would miss them.
        let n = self.named_n as usize;
        let mut outer = 0usize;
        while outer < 10 {
            let mut k = 0usize;
            while k < 8 {
                let i = outer * 8 + k;
                if i < n && i < NAMED_CAP && self.named[i].0 == name_id {
                    return Some(self.named[i].1);
                }
                k += 1;
            }
            outer += 1;
        }
        None
    }

    pub fn named_set(&mut self, name_id: i64, v: i64) {
        let n = self.named_n as usize;
        let mut outer = 0usize;
        while outer < 10 {
            let mut k = 0usize;
            while k < 8 {
                let i = outer * 8 + k;
                if i < n && i < NAMED_CAP && self.named[i].0 == name_id {
                    self.named[i].1 = v;
                    return;
                }
                k += 1;
            }
            outer += 1;
        }
        if n < NAMED_CAP {
            self.named[n] = (name_id, v);
            self.named_n = (n + 1) as u8;
        }
    }

    pub fn set_insert(&mut self, set: SetId, id: i64) {
        if id < 0 || id as usize >= SET_BITS {
            return;
        }
        let w = (id as usize) / 64;
        let b = (id as usize) % 64;
        self.sets[set as usize][w] |= 1u64 << b;
    }

    pub fn set_contains(&self, set: SetId, id: i64) -> bool {
        if id < 0 || id as usize >= SET_BITS {
            return false;
        }
        let w = (id as usize) / 64;
        let b = (id as usize) % 64;
        (self.sets[set as usize][w] & (1u64 << b)) != 0
    }

    pub fn bin_get(&self, a: i64, b: i64) -> i64 {
        let key = a * 1024 + b;
        let n = self.bin_n as usize;
        // 8×8 chunk walk (unwind bound 8, not BIN_CAP=64).
        let mut outer = 0usize;
        while outer < 8 {
            let mut k = 0usize;
            while k < 8 {
                let i = outer * 8 + k;
                if i < n && self.bin[i].0 == key {
                    return self.bin[i].1;
                }
                k += 1;
            }
            outer += 1;
        }
        0
    }

    pub fn bin_set(&mut self, a: i64, b: i64, v: i64) {
        let key = a * 1024 + b;
        let n = self.bin_n as usize;
        let mut outer = 0usize;
        while outer < 8 {
            let mut k = 0usize;
            while k < 8 {
                let i = outer * 8 + k;
                if i < n && self.bin[i].0 == key {
                    self.bin[i].1 = v;
                    return;
                }
                k += 1;
            }
            outer += 1;
        }
        if n < BIN_CAP {
            self.bin[n] = (key, v);
            self.bin_n = (n + 1) as u8;
        }
    }
}

/// `FNV-1a 64`, identical to `rules_cond::id_of`. Baked at lower time for
/// string arguments (`slot('name')`).
/// `const fn` so `id_of("literal")` is a compile-time constant and never
/// enters the CBMC unwind path. The 8×8 chunk walk is only for the rare
/// runtime name; a `const` evaluation of a literal produces the same hash.
pub const fn id_of(name: &str) -> i64 {
    let b = name.as_bytes();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut outer = 0usize;
    while outer < 8 {
        let base = outer * 8;
        let mut k = 0usize;
        while k < 8 {
            let i = base + k;
            if i < b.len() {
                h ^= b[i] as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
            k += 1;
        }
        outer += 1;
    }
    h as i64
}

/// Pure predicate backing `repeated_digits` — must match `vocab`'s `IntPred`.
/// Fixed 20-iteration bound (i64 has at most 19 decimal digits) so Kani can
/// unwind it.
pub fn repeated_digits(x: i64) -> bool {
    let mut n = x.unsigned_abs();
    if n < 10 {
        return false;
    }
    let mut seen = [false; 10];
    let mut i = 0;
    while i < 20 {
        i += 1;
        if n == 0 {
            break;
        }
        let d = (n % 10) as usize;
        if seen[d] {
            return true;
        }
        seen[d] = true;
        n /= 10;
    }
    false
}

/// Wrap-around neighbour — must match `Fx::Neighbor` in `rules-cond`.
pub fn neighbor(p: i64, delta: i64, n_seats: i64) -> i64 {
    if n_seats <= 0 {
        return -1;
    }
    ((p + delta) % n_seats + n_seats) % n_seats
}