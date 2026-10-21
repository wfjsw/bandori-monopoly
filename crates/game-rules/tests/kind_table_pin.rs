//! Pin `game_kinds::ENGINE_KINDS` against the typed mirror in `card-sdk`, and
//! the ruleset registries in `bandori-std`.
//!
//! The table in `crates/game-kinds` is the authoritative engine-kind
//! declaration; `TriggerKind::PaySplitAdd = Self::engine(55)` etc. are the
//! typed mirror. This walk keeps the two sides from drifting, and also pins
//! the payload-shape mapping (`Filter::KINDS.shape`) and the hook-only flags
//! (`game_kinds::hook_only` / `game_rules::hook_only` /
//! `bandori_std::kind::kind_hook_only`).

use card_sdk::abi::{KindNs, TriggerKind};
use card_sdk::kinds::{Filter, FilterShape};
use card_sdk::Action;
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
        (110, "PaySplitMulBefore"),
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

/// Payload-shape mapping: every engine filter kind's `FilterShape` (from the
/// `filter_kinds!` table) matches the payload family its constructor takes.
/// Actions carry no filter shape.
///
/// | shape | body | kinds |
/// |---|---|---|
/// | Value | `fn(i32, i32) -> i32` | scalar folds |
/// | Flag | `fn(i32, i32) -> bool` | pure decisions |
/// | Gate | `fn(i32, GateValue) -> GateValue` | the 10 gates + ImmuneAll/Untargetable/ExclusiveUse/AbnormalGuard |
/// | Payload | `fn(i32, P) -> P` | typed record rewrites |
#[test]
fn filter_payload_shape_mapping() {
    let expect: &[(&str, FilterShape)] = &[
        ("PaySplitChoose", FilterShape::Flag),
        ("PaySplitAdd", FilterShape::Payload),
        ("PaySplitMul", FilterShape::Value),
        ("PaySplitAt", FilterShape::Value),
        ("DiceRollPlan", FilterShape::Payload),
        ("AbnormalGuard", FilterShape::Gate),
        ("ImmuneAll", FilterShape::Gate),
        ("Untargetable", FilterShape::Gate),
        ("Redirect", FilterShape::Payload),
        ("BuyGate", FilterShape::Gate),
        ("BuyAdd", FilterShape::Value),
        ("BuyMul", FilterShape::Value),
        ("BuySet", FilterShape::Value),
        ("BuyAssign", FilterShape::Payload),
        ("PayCombinedAdd", FilterShape::Value),
        ("PayCombinedMul", FilterShape::Value),
        ("PayCombinedCancel", FilterShape::Flag),
        ("ActGate", FilterShape::Gate),
        ("PayGate", FilterShape::Gate),
        ("MoveGate", FilterShape::Gate),
        ("ExclusiveUse", FilterShape::Gate),
        ("TurnBonusTime", FilterShape::Value),
        ("TurnBaseTime", FilterShape::Value),
        ("PlayGate", FilterShape::Gate),
        ("DiceRollValue", FilterShape::Payload),
        ("OnBoardGate", FilterShape::Gate),
        ("TargetableGate", FilterShape::Gate),
    ];
    assert_eq!(expect.len(), Filter::KINDS.len(), "shape table size");
    for k in Filter::KINDS {
        let want = expect
            .iter()
            .find(|(n, _)| *n == k.name)
            .unwrap_or_else(|| panic!("Filter::{} missing from the shape table", k.name));
        assert_eq!(k.shape, want.1, "Filter::{} shape", k.name);
    }
    // Actions are `fn(i32) -> Asked` -- the shape column is meaningless and
    // the macro parks it at Value. Pin that so nobody reads it as a fold.
    for k in Action::KINDS {
        assert_eq!(k.shape, FilterShape::Value, "Action::{} shape is inert", k.name);
    }
}

/// Hook-only flags: engine side from `game_kinds` (already pinned above) and
/// the ruleset side from the declaration `bandori-std` hands the host. Walk
/// both registries instead of a hand-copied id list.
#[test]
fn hook_only_flags_match_registries() {
    // Engine: every Action/Filter registry row agrees with `game_kinds`.
    for k in Action::KINDS.iter().chain(Filter::KINDS.iter()) {
        assert_eq!(
            game_rules::hook_only(k.kind),
            game_kinds::hook_only(k.kind.id),
            "engine hook_only {}",
            k.name
        );
    }
    // Ruleset: `kind_hook_only()` is the declaration; every `Actions` row
    // must appear in it (and vice versa).
    game_rules::set_kind_flags(bandori_std::kind::kind_hook_only());
    let decl = bandori_std::kind::kind_hook_only();
    assert_eq!(
        decl.len(),
        bandori_std::kind::Actions::KINDS.len(),
        "every ruleset action kind declares a hook_only flag"
    );
    for k in bandori_std::kind::Actions::KINDS {
        let want = decl
            .iter()
            .find(|(id, _)| *id == k.kind.id)
            .unwrap_or_else(|| panic!("Actions::{} missing from kind_hook_only()", k.name))
            .1;
        assert_eq!(game_rules::hook_only(k.kind), want, "ruleset hook_only {}", k.name);
    }
    // Filters are pure folds -- they never ride the counteraction-window
    // path at all, so they are absent from `kind_hook_only()` (the host only
    // asks the flag for action / hook raises). An unregistered ruleset id
    // defaults to "opens a window"; that default is irrelevant here.
    assert!(
        bandori_std::kind::Filters::KINDS
            .iter()
            .all(|k| k.is_filter && !k.kind.is_filter()),
        "ruleset filters are pure folds"
    );
}

/// The ruleset registries (`bandori_std::kind::{Actions, Filters}`) pin the
/// ownership boundary: every row is `KindNs::Ruleset`, spellings are
/// `"r:{key}:{id}"`, and the side matches the constructor that produced it.
#[test]
fn ruleset_registry_rows_are_ruleset_ns() {
    let key = bandori_std::kind::RULESET_KEY;
    for k in bandori_std::kind::Actions::KINDS {
        assert_eq!(k.kind.ns, KindNs::Ruleset(key), "Actions::{} ns", k.name);
        assert!(!k.is_filter, "Actions::{} is !is_filter", k.name);
        let sp = k.spelling();
        assert!(
            sp.starts_with(&format!("r:{key}:")),
            "Actions::{} spelling {sp}",
            k.name
        );
    }
    for k in bandori_std::kind::Filters::KINDS {
        assert_eq!(k.kind.ns, KindNs::Ruleset(key), "Filters::{} ns", k.name);
        assert!(k.is_filter, "Filters::{} is is_filter", k.name);
        let sp = k.spelling();
        assert!(
            sp.starts_with(&format!("r:{key}:")),
            "Filters::{} spelling {sp}",
            k.name
        );
    }
    // Spot-check the documented kinds: Exile is a ruleset action; MyRewrite
    // is the ruleset filter fixture.
    let exile = bandori_std::kind::Actions::KINDS
        .iter()
        .find(|k| k.name == "Exile")
        .expect("Actions::Exile");
    assert_eq!(exile.kind, bandori_std::kind::Exile.0);
    assert_eq!(exile.spelling(), "r:bandori:21");
    let my = bandori_std::kind::Filters::KINDS
        .iter()
        .find(|k| k.name == "MyRewrite")
        .expect("Filters::MyRewrite");
    assert!(my.is_filter);
    assert_eq!(my.kind.id, bandori_std::kind::MY_REWRITE);
    assert_eq!(my.spelling(), "r:bandori:1");
    assert_eq!(my.shape, FilterShape::Payload);
}