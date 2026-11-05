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

/// Where a card instance is. `Nowhere` is off the board (mid-transition or a
/// dead instance). Wire `i32`; an unknown value reads `Nowhere`.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Zone {
    #[default]
    Nowhere,
    Deck,
    Hand,
    Field,
    /// The owner's discard heap (「弃牌堆」).
    Graveyard,
    Banished,
    EventDeck,
    EventDiscard,
    EventRemoved,
    /// On another instance (`FieldCard.on`).
    Attached,
}

impl Zone {
    /// The guest / wire encoding.
    pub const fn as_i32(self) -> i32 {
        match self {
            Zone::Nowhere => 0,
            Zone::Deck => 1,
            Zone::Hand => 2,
            Zone::Field => 3,
            Zone::Graveyard => 4,
            Zone::Banished => 5,
            Zone::EventDeck => 6,
            Zone::EventDiscard => 7,
            Zone::EventRemoved => 8,
            Zone::Attached => 9,
        }
    }

    /// Decode the guest / wire encoding (an unknown value reads `Nowhere`).
    pub const fn from_i32(v: i32) -> Self {
        match v {
            1 => Zone::Deck,
            2 => Zone::Hand,
            3 => Zone::Field,
            4 => Zone::Graveyard,
            5 => Zone::Banished,
            6 => Zone::EventDeck,
            7 => Zone::EventDiscard,
            8 => Zone::EventRemoved,
            9 => Zone::Attached,
            _ => Zone::Nowhere,
        }
    }

    /// On the play field, including as an attachment.
    pub const fn is_played(self) -> bool {
        matches!(self, Zone::Field | Zone::Attached)
    }

    /// One of the pile zones a hand-style move lands in.
    pub const fn is_pile(self) -> bool {
        matches!(
            self,
            Zone::Deck
                | Zone::Hand
                | Zone::Graveyard
                | Zone::Banished
                | Zone::EventDeck
                | Zone::EventDiscard
                | Zone::EventRemoved
        )
    }
}

/// Why a card transition happened. Wire `i32`; an unknown value reads `Other`.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum ZoneChangeCause {
    Draw,
    /// The play's own hand -> destination transition.
    Play,
    /// A deferred destination applied at run commit.
    #[default]
    Resolve,
    /// An import called by a card body.
    Effect,
    Cost,
    HandLimit,
    Mulligan,
    Opening,
    /// Cascade from the host leaving.
    HostLeft,
    /// Cascade from the creator leaving.
    CreatorLeft,
    Bankruptcy,
    Forfeit,
    /// The row moved between seats unchanged (`rehome_card`).
    Rehome,
    /// A lingering or event instance expiring.
    Expire,
    Attach,
    Detach,
    Setup,
    Other,
}

impl ZoneChangeCause {
    /// The guest / wire encoding.
    pub const fn as_i32(self) -> i32 {
        match self {
            ZoneChangeCause::Draw => 0,
            ZoneChangeCause::Play => 1,
            ZoneChangeCause::Resolve => 2,
            ZoneChangeCause::Effect => 3,
            ZoneChangeCause::Cost => 4,
            ZoneChangeCause::HandLimit => 5,
            ZoneChangeCause::Mulligan => 6,
            ZoneChangeCause::Opening => 7,
            ZoneChangeCause::HostLeft => 8,
            ZoneChangeCause::CreatorLeft => 9,
            ZoneChangeCause::Bankruptcy => 10,
            ZoneChangeCause::Forfeit => 11,
            ZoneChangeCause::Rehome => 12,
            ZoneChangeCause::Expire => 13,
            ZoneChangeCause::Attach => 14,
            ZoneChangeCause::Detach => 15,
            ZoneChangeCause::Setup => 16,
            ZoneChangeCause::Other => 17,
        }
    }

    /// Decode the guest / wire encoding (an unknown value reads `Other`).
    pub const fn from_i32(v: i32) -> Self {
        match v {
            0 => ZoneChangeCause::Draw,
            1 => ZoneChangeCause::Play,
            2 => ZoneChangeCause::Resolve,
            3 => ZoneChangeCause::Effect,
            4 => ZoneChangeCause::Cost,
            5 => ZoneChangeCause::HandLimit,
            6 => ZoneChangeCause::Mulligan,
            7 => ZoneChangeCause::Opening,
            8 => ZoneChangeCause::HostLeft,
            9 => ZoneChangeCause::CreatorLeft,
            10 => ZoneChangeCause::Bankruptcy,
            11 => ZoneChangeCause::Forfeit,
            12 => ZoneChangeCause::Rehome,
            13 => ZoneChangeCause::Expire,
            14 => ZoneChangeCause::Attach,
            15 => ZoneChangeCause::Detach,
            16 => ZoneChangeCause::Setup,
            _ => ZoneChangeCause::Other,
        }
    }
}

/// How a play ended. Wire `i32`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PlayOutcome {
    #[default]
    Resolved,
    Cancelled,
}

impl PlayOutcome {
    pub const fn as_i32(self) -> i32 {
        match self {
            PlayOutcome::Resolved => 0,
            PlayOutcome::Cancelled => 1,
        }
    }

    /// Decode the guest / wire encoding (`-1` / unknown reads `Resolved`).
    pub const fn from_i32(v: i32) -> Self {
        if v == 1 {
            PlayOutcome::Cancelled
        } else {
            PlayOutcome::Resolved
        }
    }
}

/// Why a player left the game. Wire `i32`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OutCause {
    #[default]
    Bankrupt,
    Forfeit,
    Removed,
}

impl OutCause {
    pub const fn as_i32(self) -> i32 {
        match self {
            OutCause::Bankrupt => 0,
            OutCause::Forfeit => 1,
            OutCause::Removed => 2,
        }
    }

    /// Decode the guest / wire encoding (`-1` / unknown reads `Bankrupt`).
    pub const fn from_i32(v: i32) -> Self {
        match v {
            1 => OutCause::Forfeit,
            2 => OutCause::Removed,
            _ => OutCause::Bankrupt,
        }
    }
}

/// The transition a zone hook is about. One definition, both sides of the
/// hostcall; rides `Trigger.zone` on the zone family of kinds and reaches a
/// body through the postcard `trigger_zone` seam.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ZoneChange {
    pub from: Zone,
    pub to: Zone,
    pub cause: ZoneChangeCause,
    /// The seat whose area it lands in; `-1` = the owner.
    pub holder: i32,
    /// A discard entry that never took effect.
    pub wasted: bool,
    /// The transition came from the draw routine.
    pub counted: bool,
}

/// A dice pool: `count` dice of `sides`. `sides == 0` = a flat `count`.
/// The dice shape only; engine `Roll` keeps its `reason: String` on top.
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
    fn zone_round_trips() {
        for z in [
            Zone::Nowhere,
            Zone::Deck,
            Zone::Hand,
            Zone::Field,
            Zone::Graveyard,
            Zone::Banished,
            Zone::EventDeck,
            Zone::EventDiscard,
            Zone::EventRemoved,
            Zone::Attached,
        ] {
            assert_eq!(Zone::from_i32(z.as_i32()), z);
        }
        assert_eq!(Zone::from_i32(77), Zone::Nowhere);
        assert!(Zone::Field.is_played() && Zone::Attached.is_played());
        assert!(!Zone::Hand.is_played() && Zone::Hand.is_pile());
        assert!(!Zone::Attached.is_pile());
    }

    #[test]
    fn zone_change_cause_round_trips() {
        for c in [
            ZoneChangeCause::Draw,
            ZoneChangeCause::Play,
            ZoneChangeCause::Resolve,
            ZoneChangeCause::Effect,
            ZoneChangeCause::Cost,
            ZoneChangeCause::HandLimit,
            ZoneChangeCause::Mulligan,
            ZoneChangeCause::Opening,
            ZoneChangeCause::HostLeft,
            ZoneChangeCause::CreatorLeft,
            ZoneChangeCause::Bankruptcy,
            ZoneChangeCause::Forfeit,
            ZoneChangeCause::Rehome,
            ZoneChangeCause::Expire,
            ZoneChangeCause::Attach,
            ZoneChangeCause::Detach,
            ZoneChangeCause::Setup,
            ZoneChangeCause::Other,
        ] {
            assert_eq!(ZoneChangeCause::from_i32(c.as_i32()), c);
        }
        assert_eq!(ZoneChangeCause::from_i32(99), ZoneChangeCause::Other);
        assert_eq!(ZoneChangeCause::default(), ZoneChangeCause::Resolve);
    }

    #[test]
    fn outcome_and_out_cause_decode() {
        assert_eq!(PlayOutcome::from_i32(1), PlayOutcome::Cancelled);
        assert_eq!(PlayOutcome::from_i32(-1), PlayOutcome::Resolved);
        assert_eq!(OutCause::from_i32(2), OutCause::Removed);
        assert_eq!(OutCause::from_i32(1), OutCause::Forfeit);
        assert_eq!(OutCause::from_i32(-1), OutCause::Bankrupt);
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
