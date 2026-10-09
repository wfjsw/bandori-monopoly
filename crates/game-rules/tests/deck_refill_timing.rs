//! A card-driven draw must commit its refill before another effect can prompt.
mod common;
use common::*;

#[test]
fn event_tile_refills_both_decks_before_the_event_choice() {
    let mut t = Table::vanilla(2);
    let cafe = data()
        .tiles
        .iter()
        .position(|v| v.kind == "cafe")
        .unwrap();
    t.set_pos(0, cafe - 5);
    t.set_draw(0, &["通用:GREAT"]);
    t.set_discard(0, &["通用:登上武道馆", "通用:雨啊，快点来吧"]);
    t.set_event_deck(&["很噜的感觉"]);
    t.m.world_mut().event_discard = vec!["对邦".into(), "冲榜".into()];
    t.dice(&[5]);
    t.roll(0).unwrap();
    assert_eq!(t.expect_prompt().title.key(), "log.event.lulu_ask");
    assert_eq!(t.hand(0), vec!["通用:GREAT"]);
    assert_eq!(t.draw_pile(0).len(), 2);
    assert!(t.discard(0).is_empty());
    assert_eq!(t.m.world().event_deck.len(), 2);
    assert!(t.m.world().event_discard.is_empty());
    assert!(!t.m.world().event_deck.iter().any(|c| c == "很噜的感觉"));
    let keys = t.keys_since(0);
    let at = |key: &str| keys.iter().position(|k| k == key).unwrap();
    assert!(at("log.reshuffle") < at("log.draw"));
    assert!(at("log.draw") < at("log.events_reshuffled"));
    assert!(at("log.events_reshuffled") < at("log.event"));
    t.answer(0, 1).unwrap();
    t.answer(1, 1).unwrap();
    assert_eq!(
        t.hand(0),
        vec!["通用:GREAT"],
        "guest replay cannot draw twice"
    );
    assert_eq!(t.draw_pile(0).len(), 2);
    assert_eq!(t.m.world().event_discard, vec!["很噜的感觉"]);
}
