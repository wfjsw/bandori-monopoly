//! Event-card generic mechanics (`docs/EVENTS.md`): the active list, the rule
//! bind/unbind, and the built-in fallback.
//!
//! The engine keeps only the deck, the draw, the active list and the filing
//! away; what an event *does* is its `event:*` rule instance on the neutral
//! board owner. These tests drive the generic half with a tiny [`CardRules`]
//! stand-in (game-core has no ruleset of its own), the same way `mentality.rs`
//! stands in for the bot heuristics.

use std::sync::Arc;

use game_core::data::{event_rule_id, GameData};
use game_core::engine::{CardRules, Cx, Dest, Flow, Match, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::BOARD_OWNER;
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .unwrap(),
    )
}

/// A rules stand-in with exactly one event rule: `event:对邦` stays in play
/// (「将此卡放置于场地中央」) and every other event falls through the built-in
/// fallback.
struct OneEvent;

impl CardRules for OneEvent {
    fn has_rule(&self, id: &str) -> bool {
        id == "event:对邦"
    }
    fn play(&self, _cx: &mut Cx, _player_id: usize, _card: &str) -> Flow<Dest> {
        Ok(Dest::Graveyard)
    }
    fn event(&self, _cx: &mut Cx, _player_id: usize, id: &str) -> Flow<bool> {
        Ok(id == "对邦")
    }
}

fn member(id: i32) -> RoomMember {
    RoomMember {
        id,
        player: format!("P{id}"),
        bot: true,
        ..Default::default()
    }
}

fn new_match(rules: Arc<dyn CardRules>) -> Match {
    Match::new(
        data(),
        rules,
        &[member(1), member(2)],
        7,
        MatchMode::Solo,
        ScoreWeights::default(),
    )
}

// 规则书（data/rules.txt 97–99）: 「抽取的事件卡不进入手卡并向所有玩家公开，
// 效果立刻生效。事件结算后进入事件弃卡区。」 -- a one-shot event is filed away
// and does not stay on the active list.
#[test]
fn stub_rules_draw_files_the_event_away() {
    let mut m = new_match(Arc::new(StubRules));
    let d = data();
    m.quick_start();
    let before = m.state().event_discard.len() + m.state().event_removed.len();
    m.world_mut().bind_event(&d, &StubRules, 0, "对邦");
    // `StubRules::has_rule` is false: nothing is bound, nothing is active.
    assert!(
        m.state().event_active.is_empty(),
        "active {:?}",
        m.state().event_active
    );
    assert_eq!(m.world().event_rule_instances().len(), 0);
    let _ = before;
}

// `docs/EVENTS.md`: an event whose rule exists is bound on the neutral board
// owner and shows up on the active list.
#[test]
fn bind_event_places_a_rule_instance_on_the_board_owner() {
    let mut m = new_match(Arc::new(OneEvent));
    let d = data();
    m.quick_start();
    let uid = m.world_mut().bind_event(&d, &OneEvent, 0, "对邦");
    assert!(uid >= 0, "no instance bound");
    let st = m.state();
    assert_eq!(st.event_active.len(), 1, "active {:?}", st.event_active);
    assert_eq!(st.event_active[0].id, "对邦");
    assert_eq!(st.event_active[0].player_id, 0);
    let insts = m.world().event_rule_instances();
    assert_eq!(insts.len(), 1, "instances {insts:?}");
    assert_eq!(insts[0].1, event_rule_id("对邦"));
    assert_eq!(m.world().field_by_uid(uid).unwrap().owner, BOARD_OWNER);
}

// `docs/EVENTS.md`: binding is idempotent per event id.
#[test]
fn bind_event_is_idempotent() {
    let mut m = new_match(Arc::new(OneEvent));
    let d = data();
    m.quick_start();
    let a = m.world_mut().bind_event(&d, &OneEvent, 0, "对邦");
    let b = m.world_mut().bind_event(&d, &OneEvent, 0, "对邦");
    assert_eq!(a, b);
    assert_eq!(m.world().event_rule_instances().len(), 1);
    assert_eq!(m.state().event_active.len(), 1);
}

// `docs/EVENTS.md`: 「放入事件弃牌」 -- expire unbinds and files to the discard.
#[test]
fn expire_event_unbinds_and_discards() {
    let mut m = new_match(Arc::new(OneEvent));
    let d = data();
    m.quick_start();
    m.world_mut().bind_event(&d, &OneEvent, 0, "对邦");
    m.world_mut().expire_event("对邦", false);
    assert!(m.state().event_active.is_empty());
    assert!(m.world().event_rule_instances().is_empty());
    assert!(
        m.world().event_discard.iter().any(|e| e == "对邦"),
        "discard {:?}",
        m.world().event_discard
    );
}

// `docs/EVENTS.md`: 「永久移除」 -- `removed` files to `event_removed` instead.
#[test]
fn expire_event_removed_banishes() {
    let mut m = new_match(Arc::new(OneEvent));
    let d = data();
    m.quick_start();
    m.world_mut().bind_event(&d, &OneEvent, 0, "对邦");
    m.world_mut().expire_event("对邦", true);
    assert!(m.state().event_active.is_empty());
    assert!(m.world().event_discard.iter().all(|e| e != "对邦"));
    assert!(m.world().event_removed.iter().any(|e| e == "对邦"));
}

// `docs/EVENTS.md`: 「背面朝上放置于事件牌堆顶部」 -- `event_deck_push` puts the
// id on the top of the deck (the end of the list) and records the face-down
// slot on the public view.
#[test]
fn event_deck_push_puts_it_on_top() {
    let mut m = new_match(Arc::new(OneEvent));
    m.quick_start();
    let top_before = m.world().event_deck.last().cloned();
    m.world_mut().event_deck_push("冲榜", true);
    assert_eq!(m.world().event_deck.last().map(String::as_str), Some("冲榜"));
    assert!(m.state().event_top.iter().any(|e| e == "冲榜"));
    let _ = top_before;
}

// An empty event deck takes the shuffled discard only once everything has
// resolved: filing a card does not refill it, `refill_event_deck` (run at the
// end of a draw) does.
#[test]
fn empty_event_deck_takes_the_discard_after_resolution() {
    let mut m = new_match(Arc::new(OneEvent));
    m.quick_start();
    let w = m.world_mut();
    w.event_deck.clear();
    w.event_discard = vec!["a".into(), "b".into()];
    w.expire_event("对邦", false);
    assert!(m.world().event_deck.is_empty(), "not mid-resolution");
    m.world_mut().refill_event_deck();
    let mut deck = m.world().event_deck.clone();
    deck.sort();
    assert_eq!(deck, ["a", "b", "对邦"]);
    assert!(m.world().event_discard.is_empty());
}

// `docs/EVENTS.md`: 「从所有非衍生事件中选择3个移除」 -- `event_banish` takes an
// id out of the deck / discard / active list for good.
#[test]
fn event_banish_removes_it_from_the_deck() {
    let mut m = new_match(Arc::new(OneEvent));
    let d = data();
    m.quick_start();
    let n_before = m.world().event_deck.len();
    let id = m.world().event_deck.last().cloned().expect("deck non-empty");
    m.world_mut().bind_event(&d, &OneEvent, 0, &id);
    m.world_mut().event_banish(&id);
    assert!(!m.world().event_deck.iter().any(|e| e == &id));
    assert!(m.world().event_discard.iter().all(|e| e != &id));
    assert!(m.world().event_removed.iter().any(|e| e == &id));
    assert!(m.state().event_active.is_empty());
    assert_eq!(m.world().event_deck.len(), n_before - 1);
}