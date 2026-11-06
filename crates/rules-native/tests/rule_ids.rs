//! The ruleset's **rule-entity** table (`CardDef::rule`, `CardRules::rule_ids`):
//! every rule def is flagged, no card def is, and the roster carries cards only.
//!
//! The engine classifies a name by this table alone (`World::is_rule`); the id
//! namespaces here are only the external check that the authored flags cover
//! the right defs.

use game_core::data::GameData;
use game_rules::RulesHandle;

const NAMESPACES: [&str; 5] = ["tile:", "event:", "skill:", "mark:", "status:"];

#[test]
fn every_rule_namespace_def_is_flagged_and_no_card_is() {
    let rules = rules_native::NativeModules::new();
    let cards = rules.handle().cards();
    for c in cards {
        let namespaced = NAMESPACES.iter().any(|p| c.id.starts_with(p));
        assert_eq!(c.rule, namespaced, "{}: rule={}", c.id, c.rule);
    }
    for id in [
        "tile:circle",
        "event:意外的对邦",
        "skill:仓田真白:向后全速前进",
        "mark:cp",
        "status:stay",
    ] {
        assert!(
            cards.iter().any(|c| c.id == id && c.rule),
            "{id} is not flagged"
        );
    }
    assert!(
        cards.iter().any(|c| c.id == "Mor:夏日合宿" && !c.rule),
        "a real card is flagged"
    );
    assert!(
        cards.iter().any(|c| c.rule),
        "no rule entity at all"
    );
}

#[test]
fn the_roster_carries_cards_only() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let data = GameData::load(
        |f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()),
        rules_native::roster(),
    )
    .expect("game data");
    let rules = rules_native::NativeModules::new();
    for c in &data.cards {
        let info = rules.handle().by_id(&c.id).map(|i| &rules.handle().cards()[i as usize]);
        match info {
            Some(info) => assert!(!info.rule, "rule entity in the roster: {}", c.id),
            None => panic!("roster id without a rule: {}", c.id),
        }
    }
}
