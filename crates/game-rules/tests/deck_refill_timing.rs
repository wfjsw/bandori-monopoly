//! A card-driven draw must commit its refill before another effect can prompt;
//! the event deck waits for the draw to resolve (ruling 2026-10-07).
mod common;
use common::*;

#[test]
fn event_tile_refills_the_hand_deck_before_the_event_choice() {
    let mut t = Table::vanilla(2);
    let cafe = data()
        .tiles
        .iter()
        .position(|v| v.kind == game_core::data::TileKind::Cafe)
        .unwrap();
    t.set_pos(0, cafe - 5);
    t.set_draw(0, &["通用:GREAT"]);
    t.set_discard(0, &["通用:登上武道馆", "通用:雨啊，快点来吧"]);
    t.set_event_deck(&["很噜的感觉"]);
    t.m.world_mut().event_discard = vec!["对邦".into(), "冲榜".into()];
    t.dice(&[5]);
    t.roll(0).unwrap();
    assert_eq!(t.expect_prompt().title.key(), "log.event.lulu_ask");
    // The hand-card draw pile refills the moment it empties (「当抽卡区抽光时
    // 将弃卡区洗卡并放回抽卡区」), before the event can prompt.
    assert_eq!(t.hand(0), vec!["通用:GREAT"]);
    assert_eq!(t.draw_pile(0).len(), 2);
    assert!(t.discard(0).is_empty());
    // The event deck does not: the resolving event is outside both piles and
    // the old discards wait for everything to resolve (ruling 2026-10-07).
    assert!(t.m.world().event_deck.is_empty());
    assert_eq!(
        t.m.world().event_discard,
        vec!["对邦".to_string(), "冲榜".to_string()]
    );
    let keys = t.keys_since(0);
    let at = |key: &str| keys.iter().position(|k| k == key).unwrap();
    assert!(at("log.reshuffle") < at("log.draw"));
    assert!(at("log.draw") < at("log.event"));
    assert!(
        !keys.iter().any(|k| k == "log.events_reshuffled"),
        "no event-deck reshuffle mid-resolution"
    );
    t.answer(0, 1).unwrap();
    t.answer(1, 1).unwrap();
    assert_eq!(
        t.hand(0),
        vec!["通用:GREAT"],
        "guest replay cannot draw twice"
    );
    assert_eq!(t.draw_pile(0).len(), 2);
    // Only now does the emptied event deck take the shuffled discard, the
    // just-filed event included.
    let mut deck = t.m.world().event_deck.clone();
    deck.sort();
    assert_eq!(deck, ["冲榜", "对邦", "很噜的感觉"]);
    assert!(t.m.world().event_discard.is_empty());
    let keys = t.keys_since(0);
    assert!(keys.iter().any(|k| k == "log.events_reshuffled"));
}