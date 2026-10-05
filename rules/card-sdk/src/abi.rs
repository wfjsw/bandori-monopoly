//! Host/guest ABI. Shared verbatim by the guest (ruleset.wasm) and the host
//! (`game-rules`). Bump [`ABI_VERSION`] on any incompatible change; the host
//! refuses rulesets built against a different version.

#[cfg(target_arch = "wasm32")]
use alloc::string::String;

/// Increment on any change to imports, exports, or their semantics.
/// v2: `play_card` (cross-module card calls); one module per card.
/// v6: full `TriggerKind` set + `trig_target` / `trig_tile` / `trig_value`.
/// v7: board/hand/status query + op wave (is_buyable ... in_band, spend_fire,
///     sweep_to_deck, trig_card_is; `Trigger.card`).
/// v8: `bandori_why_not` export (`CardDef.why_not`) + `add_to_deck_at`.
pub const ABI_VERSION: i32 = 8;

/// Wasm import module name for every host function.
pub const IMPORT_MODULE: &str = "bandori";

/// Guest exports.
pub mod export {
    pub const ABI_VERSION: &str = "bandori_abi_version";
    /// `() -> i64` packed `(ptr << 32) | len` pointing at a UTF-8 JSON manifest.
    pub const MANIFEST: &str = "bandori_manifest";
    /// `(card: i32, seat: i32)`
    pub const PLAY: &str = "bandori_play";
    /// `(card: i32, seat: i32) -> i32` (0/1)
    pub const CAN_REACT: &str = "bandori_can_react";
    /// `(card: i32, seat: i32)`
    pub const REACT: &str = "bandori_react";
    /// `(card: i32, seat: i32) -> i64` packed `(ptr << 32) | len` of a
    /// postcard `Msg` reason, or 0 when the card is playable (`CardDef.why_not`).
    pub const WHY_NOT: &str = "bandori_why_not";
    pub const MEMORY: &str = "memory";
}

/// `i32_exit` status the host uses to abort a run that reached an unanswered prompt.
/// Never observed by the guest.
pub const EXIT_NEED_INPUT: i32 = 0x0B_A0_D0;

/// Prompt kinds, mirroring `MatchPrompt.kind` in the C# (`MatchPrompt.cs`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptKind {
    /// Pick one of the pushed string options (`AskPick`).
    Pick = 0,
    /// Yes / no (`AskYes`); options are implicit.
    Yes = 1,
    /// Pick a tile index (`AskTileOf`).
    Tile = 2,
    /// Pick a seat (`AskSeat`).
    Seat = 3,
    /// Pick a card id (`AskCard`).
    Card = 4,
}

impl PromptKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Pick,
            1 => Self::Yes,
            2 => Self::Tile,
            3 => Self::Seat,
            4 => Self::Card,
            _ => return None,
        })
    }

    /// The C# `MatchPrompt.kind` string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pick => "pick",
            Self::Yes => "yes",
            Self::Tile => "tile",
            Self::Seat => "seat",
            Self::Card => "card",
        }
    }
}

/// Trigger kinds a reaction can be checked against (C# `Trigger.Kind`).
///
/// The values are a wire enum: the host fills them at the same points the C#
/// raises its `Trigger`s. Kinds the engine does not raise yet still exist here
/// so a card's `can_react` can state its real condition; it simply never sees
/// that kind until the engine raises it (TODO in `game-core`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TriggerKind {
    #[default]
    None = 0,
    Roll = 1,
    MoveRoll = 2,
    TurnStart = 3,
    Pass = 4,
    PassSeat = 5,
    SettleBefore = 6,
    Settle = 7,
    Mortgage = 8,
    Pay = 9,
    Paid = 10,
    Bankrupt = 11,
    Card = 12,
    Event = 13,
    Abnormal = 14,
    Target = 15,
    Stop = 16,
    Teleport = 17,
    SkillTeleport = 18,
    Stun = 19,
    Stay = 20,
    Exile = 21,
    Forced = 22,
    State = 23,
    Reacted = 24,
    DrawOut = 25,
    CircleAffected = 26,
    TwoCards = 27,
}

impl TriggerKind {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Roll,
            2 => Self::MoveRoll,
            3 => Self::TurnStart,
            4 => Self::Pass,
            5 => Self::PassSeat,
            6 => Self::SettleBefore,
            7 => Self::Settle,
            8 => Self::Mortgage,
            9 => Self::Pay,
            10 => Self::Paid,
            11 => Self::Bankrupt,
            12 => Self::Card,
            13 => Self::Event,
            14 => Self::Abnormal,
            15 => Self::Target,
            16 => Self::Stop,
            17 => Self::Teleport,
            18 => Self::SkillTeleport,
            19 => Self::Stun,
            20 => Self::Stay,
            21 => Self::Exile,
            22 => Self::Forced,
            23 => Self::State,
            24 => Self::Reacted,
            25 => Self::DrawOut,
            26 => Self::CircleAffected,
            27 => Self::TwoCards,
            _ => Self::None,
        }
    }

    /// The C# `Trigger.Kind` string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Roll => "roll",
            Self::MoveRoll => "moveRoll",
            Self::TurnStart => "turnStart",
            Self::Pass => "pass",
            Self::PassSeat => "passSeat",
            Self::SettleBefore => "settleBefore",
            Self::Settle => "settle",
            Self::Mortgage => "mortgage",
            Self::Pay => "pay",
            Self::Paid => "paid",
            Self::Bankrupt => "bankrupt",
            Self::Card => "card",
            Self::Event => "event",
            Self::Abnormal => "abnormal",
            Self::Target => "target",
            Self::Stop => "stop",
            Self::Teleport => "teleport",
            Self::SkillTeleport => "skillTeleport",
            Self::Stun => "stun",
            Self::Stay => "stay",
            Self::Exile => "exile",
            Self::Forced => "forced",
            Self::State => "state",
            Self::Reacted => "reacted",
            Self::DrawOut => "drawOut",
            Self::CircleAffected => "circleAffected",
            Self::TwoCards => "twoCards",
        }
    }

    /// Inverse of [`as_str`] (`game-rules` maps engine trigger kinds through here).
    pub fn from_str(s: &str) -> Self {
        match s {
            "roll" => Self::Roll,
            "moveRoll" => Self::MoveRoll,
            "turnStart" => Self::TurnStart,
            "pass" => Self::Pass,
            "passSeat" => Self::PassSeat,
            "settleBefore" => Self::SettleBefore,
            "settle" => Self::Settle,
            "mortgage" => Self::Mortgage,
            "pay" => Self::Pay,
            "paid" => Self::Paid,
            "bankrupt" => Self::Bankrupt,
            "card" => Self::Card,
            "event" => Self::Event,
            "abnormal" => Self::Abnormal,
            "target" => Self::Target,
            "stop" => Self::Stop,
            "teleport" => Self::Teleport,
            "skillTeleport" => Self::SkillTeleport,
            "stun" => Self::Stun,
            "stay" => Self::Stay,
            "exile" => Self::Exile,
            "forced" => Self::Forced,
            "state" => Self::State,
            "reacted" => Self::Reacted,
            "drawOut" => Self::DrawOut,
            "circleAffected" => Self::CircleAffected,
            "twoCards" => Self::TwoCards,
            _ => Self::None,
        }
    }
}

pub fn pack(ptr: u32, len: u32) -> i64 {
    (((ptr as u64) << 32) | len as u64) as i64
}

pub fn unpack(v: i64) -> (u32, u32) {
    let v = v as u64;
    ((v >> 32) as u32, (v & 0xFFFF_FFFF) as u32)
}

/// One entry of the guest card manifest (`rt::manifest`). The index in the
/// array is the card handle the host passes back. Shared with the host so the
/// two sides cannot disagree on the wire layout.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestEntry {
    pub id: String,
    pub play: bool,
    pub react: bool,
}
