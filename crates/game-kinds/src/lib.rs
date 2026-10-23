//! Data-only engine trigger-kind table. Engine vocabulary, defined in `crates/`.
//! `rules/` treats these ids as opaque tokens (except this crate's own rows).
//! `rules/cond` authors neither this table nor the ruleset table — its constants
//! mirror their owners.
//!
//! # Retired ids (never reuse)
//!
//! * 16 was "stop" -- retired in v84
//! * 17 was "teleport" -- retired in v84
//! * 18 was "skillTeleport" -- retired in v85
//! * 19 was the second abnormal kind -- retired in v84
//! * 20 was the first abnormal kind -- retired in v84
//! * 22 was the third abnormal kind -- retired in v84
//! * 94 was "flipEffect" -- retired in v83
//! * 104 was "flipAfter" -- retired in v83
//!
//! Those ids stay out of [`ENGINE_KINDS`] so [`wire`] reads `""` for them.
//! Ids 21 / 26 / 78 / 107 are ruleset-declared (bandori) and are **not**
//! engine rows either.

#![no_std]

pub mod vocab;

pub use vocab::{Dice, Dir, MoveKind, MsgKey, Payee, Seat, Tile};

/// One live engine trigger kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineKind {
    pub id: i32,
    /// camelCase wire spelling, e.g. `"paySplitAdd"`.
    pub wire: &'static str,
    /// Filter-pipeline / gate kind (`On::Filter`) vs action / hook / link.
    pub is_filter: bool,
    /// No counteraction window opens at this raise (field-card hook / pure fold).
    pub hook_only: bool,
}

/// Every live engine kind id. Retired ids stay out (see the module doc).
pub const ENGINE_KINDS: &[EngineKind] = &[
    EngineKind {
        id: 1,
        wire: "diceRollBefore",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 2,
        wire: "diceRollResult",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 3,
        wire: "turnStart",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 4,
        wire: "pass",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 5,
        wire: "passPlayer",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 6,
        wire: "settleBefore",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 7,
        wire: "settle",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 8,
        wire: "mortgage",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 9,
        wire: "pay",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 10,
        wire: "paid",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 11,
        wire: "bankrupt",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 12,
        wire: "card",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 13,
        wire: "event",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 14,
        wire: "abnormal",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 15,
        wire: "target",
        is_filter: false,
        hook_only: false,
    },
    // 16..=20 and 22 retired -- never reuse (module doc).
    // 21 is ruleset-declared, not an engine id.
    EngineKind {
        id: 23,
        wire: "state",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 24,
        wire: "counteracted",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 25,
        wire: "drawOut",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 27,
        wire: "twoCards",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 28,
        wire: "turnStartBefore",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 29,
        wire: "passBefore",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 30,
        wire: "mortgageBefore",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 31,
        wire: "bankruptBefore",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 32,
        wire: "cardAfter",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 33,
        wire: "eventAfter",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 34,
        wire: "settleAfter",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 35,
        wire: "buyBefore",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 36,
        wire: "buyAfter",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 37,
        wire: "buildBefore",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 38,
        wire: "buildAfter",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 39,
        wire: "discardBefore",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 40,
        wire: "discardAfter",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 41,
        wire: "endTurnBefore",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 42,
        wire: "endTurnAfter",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 43,
        wire: "leaveBefore",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 44,
        wire: "leaveAfter",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 45,
        wire: "turnEnd",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 46,
        wire: "drawn",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 47,
        wire: "passTile",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 48,
        wire: "payAfter",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 49,
        wire: "diceRollAfter",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 50,
        wire: "cardPlayed",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 51,
        wire: "targeted",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 52,
        wire: "paySplitChoose",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 53,
        wire: "turnEndBefore",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 54,
        wire: "turnEndAfter",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 55,
        wire: "paySplitAdd",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 56,
        wire: "paySplitMul",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 57,
        wire: "paySplitAt",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 58,
        wire: "discarded",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 59,
        wire: "deckBeforeGame",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 60,
        wire: "deckAtGameStart",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 61,
        wire: "drew",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 62,
        wire: "reshuffled",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 63,
        wire: "bought",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 64,
        wire: "settleBody",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 65,
        wire: "beforeOut",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 66,
        wire: "teleported",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 67,
        wire: "diceRollPlan",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 68,
        wire: "abnormalGuard",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 69,
        wire: "immuneAll",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 70,
        wire: "untargetable",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 71,
        wire: "redirect",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 72,
        wire: "effect",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 73,
        wire: "markerSpent",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 74,
        wire: "cardUsed",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 75,
        wire: "houseAdded",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 76,
        wire: "counterChanged",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 77,
        wire: "drewBefore",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 79,
        wire: "buyGate",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 80,
        wire: "buyAdd",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 81,
        wire: "buyMul",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 82,
        wire: "buySet",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 83,
        wire: "buyAssign",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 84,
        wire: "payCombinedAdd",
        is_filter: true,
        hook_only: false,
    },
    EngineKind {
        id: 85,
        wire: "payCombinedMul",
        is_filter: true,
        hook_only: false,
    },
    EngineKind {
        id: 86,
        wire: "payCombinedCancel",
        is_filter: true,
        hook_only: false,
    },
    EngineKind {
        id: 87,
        wire: "tileResolved",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 88,
        wire: "moveResolved",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 89,
        wire: "bankruptResolved",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 90,
        wire: "moveBefore",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 91,
        wire: "moveAfter",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 92,
        wire: "markerSpend",
        is_filter: false,
        hook_only: false,
    },
    EngineKind {
        id: 93,
        wire: "markerGain",
        is_filter: false,
        hook_only: false,
    },
    // 94 retired -- never reuse (module doc).
    EngineKind {
        id: 95,
        wire: "actGate",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 96,
        wire: "payGate",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 97,
        wire: "moveGate",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 98,
        wire: "exclusiveUse",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 99,
        wire: "turnBonusTime",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 100,
        wire: "turnBaseTime",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 101,
        wire: "playGate",
        is_filter: true,
        hook_only: false,
    },
    EngineKind {
        id: 102,
        wire: "diceRollValue",
        is_filter: true,
        hook_only: true,
    },
    EngineKind {
        id: 103,
        wire: "atEnd",
        is_filter: false,
        hook_only: true,
    },
    // 104 retired -- never reuse (module doc).
    EngineKind {
        id: 105,
        wire: "paySplitChooseBefore",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 106,
        wire: "payCombinedAddBefore",
        is_filter: false,
        hook_only: true,
    },
    EngineKind {
        id: 108,
        wire: "onBoardGate",
        is_filter: true,
        hook_only: false,
    },
    EngineKind {
        id: 109,
        wire: "targetableGate",
        is_filter: true,
        hook_only: false,
    },
    EngineKind {
        id: 110,
        wire: "paySplitMulBefore",
        is_filter: false,
        hook_only: true,
    },
];

/// The table row for `id`, or `None` when the id is retired / unknown.
pub const fn row(id: i32) -> Option<&'static EngineKind> {
    let mut i = 0;
    while i < ENGINE_KINDS.len() {
        if ENGINE_KINDS[i].id == id {
            return Some(&ENGINE_KINDS[i]);
        }
        i += 1;
    }
    None
}

/// camelCase wire spelling, `""` when unknown / retired.
pub const fn wire(id: i32) -> &'static str {
    match row(id) {
        Some(r) => r.wire,
        None => "",
    }
}

/// Inverse of [`wire`]. `None` when the spelling is not a live engine kind.
pub fn from_wire(s: &str) -> Option<i32> {
    ENGINE_KINDS.iter().find(|r| r.wire == s).map(|r| r.id)
}

/// Filter-pipeline / gate kind (`On::Filter`)? Unknown / retired ids are `false`.
pub const fn is_filter(id: i32) -> bool {
    match row(id) {
        Some(r) => r.is_filter,
        None => false,
    }
}

/// No counteraction window opens at this raise. Unknown / retired ids are
/// `false` (they never raise).
pub const fn hook_only(id: i32) -> bool {
    match row(id) {
        Some(r) => r.hook_only,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_round_trips() {
        for r in ENGINE_KINDS {
            assert_eq!(wire(r.id), r.wire, "id {}", r.id);
            assert_eq!(from_wire(r.wire), Some(r.id), "{}", r.wire);
            assert_eq!(is_filter(r.id), r.is_filter);
            assert_eq!(hook_only(r.id), r.hook_only);
        }
    }

    #[test]
    fn retired_ids_are_unnamed() {
        for id in [0, 16, 17, 18, 19, 20, 22, 94, 104] {
            assert_eq!(wire(id), "", "id {id}");
            assert!(!is_filter(id));
            assert!(!hook_only(id));
        }
        assert_eq!(from_wire(""), None);
        assert_eq!(from_wire("stop"), None);
        assert_eq!(from_wire("flipEffect"), None);
    }

    #[test]
    fn filter_and_hook_sets() {
        const FILTERS: &[i32] = &[
            52, 55, 56, 57, 67, 68, 69, 70, 71, 79, 80, 81, 82, 83, 84, 85, 86, 95, 96, 97, 98, 99,
            100, 101, 102, 108, 109,
        ];
        const HOOKS: &[i32] = &[
            45, 46, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 65, 66, 67, 68,
            69, 70, 71, 76, 77, 79, 80, 81, 82, 83, 95, 96, 97, 98, 99, 100, 102, 103, 105, 106,
            110,
        ];
        for r in ENGINE_KINDS {
            assert_eq!(
                r.is_filter,
                FILTERS.contains(&r.id),
                "is_filter id {}",
                r.id
            );
            assert_eq!(r.hook_only, HOOKS.contains(&r.id), "hook_only id {}", r.id);
        }
        assert_eq!(FILTERS.len(), 27);
        assert_eq!(HOOKS.len(), 43);
    }
}
