//! Pin `game_kinds::ENGINE_KINDS` against the typed mirror in `card-sdk`.
//!
//! The table in `crates/game-kinds` is the authoritative engine-kind
//! declaration; `TriggerKind::PaySplitAdd = Self::engine(55)` etc. are the
//! typed mirror. This walk keeps the two sides from drifting.

use card_sdk::abi::TriggerKind;
use game_kinds::ENGINE_KINDS;

#[test]
fn engine_kind_rows_match_trigger_kind() {
    for row in ENGINE_KINDS {
        let k = TriggerKind::engine(row.id);
        assert_eq!(k.id, row.id);
        assert_eq!(k.spelling(), row.wire, "id {}", row.id);
        assert_eq!(k.is_filter(), row.is_filter, "is_filter id {}", row.id);
        assert_eq!(
            game_kinds::hook_only(row.id),
            row.hook_only,
            "hook_only id {}",
            row.id
        );
    }
}

/// `game_kinds::hook_only` agrees with `game_rules::hook_only` for engine
/// kinds, and the ruleset declaration (seeded here) agrees for the four
/// bandori kinds. The engine names no ruleset id -- the declaration does.
#[test]
fn hook_only_matches_tables() {
    for row in ENGINE_KINDS {
        let k = TriggerKind::engine(row.id);
        assert_eq!(
            game_rules::hook_only(k),
            row.hook_only,
            "engine hook_only id {}",
            row.id
        );
    }
    // Seed the ruleset side from the declaration this test crate links.
    // The ids below are pin examples of that declaration (this test may spell
    // them; the host itself never does).
    game_rules::set_kind_flags(bandori_std::kind::kind_hook_only());
    let cases = [
        (21i32, false), // Exile -- window opens
        (26, false),    // PassRewardAffected -- window opens
        (78, true),     // ChargePaying -- hook-only
        (107, true),    // FansFlip -- hook-only
    ];
    for (id, want) in cases {
        let k = TriggerKind::ruleset("bandori", id);
        assert_eq!(game_rules::hook_only(k), want, "ruleset hook_only id {id}");
    }
    // An unregistered ruleset kind defaults to opening a window.
    assert!(!game_rules::hook_only(TriggerKind::ruleset("bandori", 999)));
}

/// Every named engine const in the typed mirror has a table row (and the
/// spelling is non-empty). Ruleset consts (`ActionKind::Exile` etc.) live in
/// `bandori-std` and are not engine rows.
#[test]
fn typed_engine_consts_have_table_rows() {
    let named: &[(i32, &str)] = &[
        (1, "DiceRollBefore"),
        (2, "DiceRollResult"),
        (3, "TurnStart"),
        (4, "Pass"),
        (5, "PassPlayer"),
        (6, "SettleBefore"),
        (7, "Settle"),
        (8, "Mortgage"),
        (9, "Pay"),
        (10, "Paid"),
        (11, "Bankrupt"),
        (12, "Card"),
        (13, "Event"),
        (14, "Abnormal"),
        (15, "Target"),
        (23, "State"),
        (24, "Counteracted"),
        (25, "DrawOut"),
        (27, "TwoCards"),
        (28, "TurnStartBefore"),
        (29, "PassBefore"),
        (30, "MortgageBefore"),
        (31, "BankruptBefore"),
        (32, "CardAfter"),
        (33, "EventAfter"),
        (34, "SettleAfter"),
        (35, "BuyBefore"),
        (36, "BuyAfter"),
        (37, "BuildBefore"),
        (38, "BuildAfter"),
        (39, "DiscardBefore"),
        (40, "DiscardAfter"),
        (41, "EndTurnBefore"),
        (42, "EndTurnAfter"),
        (43, "LeaveBefore"),
        (44, "LeaveAfter"),
        (45, "TurnEnd"),
        (46, "Drawn"),
        (47, "PassTile"),
        (48, "PayAfter"),
        (49, "DiceRollAfter"),
        (50, "CardPlayed"),
        (51, "Targeted"),
        (52, "PaySplitChoose"),
        (53, "TurnEndBefore"),
        (54, "TurnEndAfter"),
        (55, "PaySplitAdd"),
        (56, "PaySplitMul"),
        (57, "PaySplitAt"),
        (58, "Discarded"),
        (59, "DeckBeforeGame"),
        (60, "DeckAtGameStart"),
        (61, "Drew"),
        (62, "Reshuffled"),
        (63, "Bought"),
        (64, "SettleBody"),
        (65, "BeforeOut"),
        (66, "Teleported"),
        (67, "DiceRollPlan"),
        (68, "AbnormalGuard"),
        (69, "ImmuneAll"),
        (70, "Untargetable"),
        (71, "Redirect"),
        (72, "Effect"),
        (73, "MarkerSpent"),
        (74, "CardUsed"),
        (75, "HouseAdded"),
        (76, "CounterChanged"),
        (77, "DrewBefore"),
        (79, "BuyGate"),
        (80, "BuyAdd"),
        (81, "BuyMul"),
        (82, "BuySet"),
        (83, "BuyAssign"),
        (84, "PayCombinedAdd"),
        (85, "PayCombinedMul"),
        (86, "PayCombinedCancel"),
        (87, "TileResolved"),
        (88, "MoveResolved"),
        (89, "BankruptResolved"),
        (90, "MoveBefore"),
        (91, "MoveAfter"),
        (92, "MarkerSpend"),
        (93, "MarkerGain"),
        (95, "ActGate"),
        (96, "PayGate"),
        (97, "MoveGate"),
        (98, "ExclusiveUse"),
        (99, "TurnBonusTime"),
        (100, "TurnBaseTime"),
        (101, "PlayGate"),
        (102, "DiceRollValue"),
        (103, "AtEnd"),
        (105, "PaySplitChooseBefore"),
        (106, "PayCombinedAddBefore"),
        (108, "OnBoardGate"),
        (109, "TargetableGate"),
    ];
    for (id, name) in named {
        let row = game_kinds::row(*id).unwrap_or_else(|| panic!("{name} (id {id}) has no table row"));
        assert_eq!(row.id, *id);
        assert!(!row.wire.is_empty(), "{name} wire empty");
        assert_eq!(
            TriggerKind::engine(*id).spelling(),
            row.wire,
            "{name} spelling"
        );
    }
    // The typed mirror must cover every live table row (count pin).
    assert_eq!(named.len(), ENGINE_KINDS.len(), "mirror/table size");
}