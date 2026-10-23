//! Shared wire vocabulary -- one definition, both sides of the hostcall.
//!
//! Engine (`game-core` / `game-rules`) and guest (`card-sdk`) both speak these
//! types. Flattening to `i32` lives only at the wasm import surface
//! (`game-rules/src/hostfns.rs`) via the `as_i32` / `from_i32` helpers here;
//! nothing else touches the encoding.
//!
//! Data-only: no engine logic, `no_std`.

/// A seat at the table. `i32` on the wire.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Seat(pub i32);

/// A tile index. `i32` on the wire.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Tile(pub i32);

/// A log / denial message key (`key!` expansion, e.g.
/// `"cards:skill-groups.poppin_locked"`). Replaces intern ids.
pub type MsgKey = &'static str;

/// Who money / a deed goes to. Replaces the bare `-1 = bank` encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Payee {
    /// The bank (guest / wire `-1`).
    Bank,
    /// A seat.
    Seat(Seat),
}

impl Payee {
    /// The guest / wire encoding: `-1` for the bank, else the seat.
    pub const fn as_i32(self) -> i32 {
        match self {
            Payee::Bank => -1,
            Payee::Seat(n) => n.0,
        }
    }

    /// Decode the guest / wire encoding (`-1` = bank).
    pub fn from_i32(v: i32) -> Self {
        if v < 0 {
            Payee::Bank
        } else {
            Payee::Seat(Seat(v))
        }
    }
}

/// A dice pool: `count` dice of `sides`. `sides == 0` = a flat `count`.
/// The dice shape only; engine `Roll` keeps its `why: String` on top.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Dice {
    pub count: i32,
    pub sides: i32,
}

/// How the player gets there: exactly one per move -- a walk that travels a
/// [路径], or a teleport to its destination.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum MoveKind {
    /// The player steps along the board, tile by tile.
    #[default]
    Walk,
    /// The player jumps straight to the destination.
    Teleport,
}

impl MoveKind {
    /// Decode the guest / wire encoding (`0` = walk, `1` = teleport).
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Walk),
            1 => Some(Self::Teleport),
            _ => None,
        }
    }

    /// The guest / wire encoding.
    pub const fn as_i32(self) -> i32 {
        match self {
            MoveKind::Walk => 0,
            MoveKind::Teleport => 1,
        }
    }

    /// The canonical spelling (`"walk"` / `"teleport"`).
    pub const fn as_str(self) -> &'static str {
        match self {
            MoveKind::Walk => "walk",
            MoveKind::Teleport => "teleport",
        }
    }
}

/// Which way a move travels along the board loop (turn order).
///
/// Replaces the ±1 `move_dir` integer: forward and backward are the only two
/// directions a walk has. The guest ABI and CEL still see `1` / `-1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Dir {
    /// 「前进」 -- towards higher tile indices (the guest / CEL `1`).
    #[default]
    Forward,
    /// 「后退」 -- towards lower tile indices (the guest / CEL `-1`).
    Backward,
}

impl Dir {
    /// The guest / CEL encoding: `1` forward, `-1` backward.
    pub const fn as_i32(self) -> i32 {
        match self {
            Dir::Forward => 1,
            Dir::Backward => -1,
        }
    }

    /// Decode the guest / CEL encoding (`1` / `-1`; anything else reads forward).
    pub fn from_i32(v: i32) -> Self {
        if v < 0 {
            Dir::Backward
        } else {
            Dir::Forward
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payee_round_trips_the_wire_form() {
        assert_eq!(Payee::Bank.as_i32(), -1);
        assert_eq!(Payee::from_i32(-1), Payee::Bank);
        assert_eq!(Payee::from_i32(-7), Payee::Bank, "any negative is bank");
        assert_eq!(Payee::Seat(Seat(3)).as_i32(), 3);
        assert_eq!(Payee::from_i32(3), Payee::Seat(Seat(3)));
    }

    #[test]
    fn move_kind_round_trips() {
        assert_eq!(MoveKind::from_i32(0), Some(MoveKind::Walk));
        assert_eq!(MoveKind::from_i32(1), Some(MoveKind::Teleport));
        assert_eq!(MoveKind::from_i32(2), None);
        assert_eq!(MoveKind::Teleport.as_i32(), 1);
        assert_eq!(MoveKind::Walk.as_str(), "walk");
    }

    #[test]
    fn dir_encodes_as_a_sign() {
        assert_eq!(Dir::Forward.as_i32(), 1);
        assert_eq!(Dir::Backward.as_i32(), -1);
        assert_eq!(Dir::from_i32(-1), Dir::Backward);
        assert_eq!(Dir::from_i32(1), Dir::Forward);
        assert_eq!(Dir::from_i32(0), Dir::Forward);
    }
}
