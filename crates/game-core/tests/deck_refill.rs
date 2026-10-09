//! An exhausted deck is restored before draw/event effects can observe it.
use game_core::data::GameData;
use game_core::engine::{CardRules, Cx, Dest, Flow, Match, StubRules, Trigger, World};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;
use std::sync::{Arc, Mutex};

fn setup() -> (Arc<GameData>, World) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let data = Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .unwrap(),
    );
    let members = (1..=2)
        .map(|id| RoomMember {
            id,
            player: format!("P{id}"),
            bot: true,
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let mut m = Match::new(
        data.clone(),
        Arc::new(StubRules),
        &members,
        7,
        MatchMode::Solo,
        ScoreWeights::default(),
    );
    m.quick_start();
    let mut w = m.world().clone();
    w.st.phase = "play".into();
    for p in &mut w.st.players {
        p.ai = false;
        p.bot = false;
    }
    w.hidden[0].hand.clear();
    w.hidden[0].draw = vec!["last".into()];
    w.hidden[0].discard = vec!["a".into(), "b".into()];
    (data, w)
}

#[derive(Default)]
struct Probe {
    seen: Mutex<Vec<&'static str>>,
    cancel_draw: bool,
}
impl CardRules for Probe {
    fn play(&self, _cx: &mut Cx, _player: usize, _card: &str) -> Flow<Dest> {
        Ok(Dest::Graveyard)
    }
    fn counteract(&self, cx: &mut Cx, t: &mut Trigger) -> Flow<()> {
        if t.kind == "drewBefore" && self.cancel_draw {
            t.negate_activation();
        }
        if matches!(t.kind, "reshuffled" | "drawn" | "drew") {
            if t.kind != "reshuffled" {
                assert!(
                    !cx.world_copy().hidden[0].draw.is_empty(),
                    "refill must precede {}",
                    t.kind
                );
            }
            self.seen.lock().unwrap().push(t.kind);
        }
        Ok(())
    }
    fn event(&self, cx: &mut Cx, _player: usize, id: &str) -> Flow<bool> {
        let w = cx.world_copy();
        let mut deck = w.event_deck.clone();
        deck.sort();
        assert_eq!(
            deck,
            ["a", "b"],
            "old discards must be available before the event body"
        );
        assert!(
            !deck.iter().any(|c| c == id),
            "the resolving event cannot be recycled"
        );
        assert!(w.event_discard.is_empty());
        self.seen.lock().unwrap().push("body");
        Ok(false)
    }
}

#[test]
fn last_card_refills_before_drawn_and_drew() {
    let (data, w) = setup();
    let rules = Probe::default();
    let mut cx = Cx::new(w, &data, &rules, &[]);
    assert_eq!(cx.draw_cards_with_hooks(0, 1, true).unwrap(), 1);
    assert_eq!(*rules.seen.lock().unwrap(), ["reshuffled", "drawn", "drew"]);
    let w = cx.world_copy();
    assert_eq!(w.hidden[0].hand, ["last"]);
    assert_eq!(w.hidden[0].draw.len(), 2);
    assert!(w.hidden[0].discard.is_empty());
}

#[test]
fn multi_draw_refills_between_cards_without_an_extra_draw() {
    let (data, w) = setup();
    let rules = Probe::default();
    let mut cx = Cx::new(w, &data, &rules, &[]);
    assert_eq!(cx.draw_cards_with_hooks(0, 2, true).unwrap(), 2);
    assert_eq!(
        *rules.seen.lock().unwrap(),
        ["reshuffled", "drawn", "drew", "drawn", "drew"]
    );
    let w = cx.world_copy();
    assert_eq!(w.hidden[0].hand.len(), 2);
    assert_eq!(w.hidden[0].draw.len(), 1);
}

#[test]
fn cancelled_draw_does_not_empty_or_shuffle_the_deck() {
    let (data, w) = setup();
    let rules = Probe {
        cancel_draw: true,
        ..Default::default()
    };
    let mut cx = Cx::new(w, &data, &rules, &[]);
    assert_eq!(cx.draw_cards_with_hooks(0, 1, true).unwrap(), 0);
    assert!(rules.seen.lock().unwrap().is_empty());
    let w = cx.world_copy();
    assert_eq!(w.hidden[0].draw, ["last"]);
    assert_eq!(w.hidden[0].discard.len(), 2);
}

#[test]
fn raw_draw_also_restores_the_deck_and_filing_refills_a_dry_deck() {
    let (_, mut w) = setup();
    assert_eq!(w.draw_cards(0, 1, true), 1);
    assert_eq!(w.hidden[0].draw.len(), 2);
    assert!(w.hidden[0].discard.is_empty());
    w.hidden[0].draw.clear();
    w.hidden[0].hand = vec!["filed".into()];
    assert!(w.to_discard(0, "filed"));
    assert_eq!(w.hidden[0].draw, ["filed"]);
    assert!(w.hidden[0].discard.is_empty());
    assert!(
        !w.refill_draw_pile(0),
        "a nonempty deck must not reshuffle twice"
    );
}

#[test]
fn last_event_refills_before_the_event_body_and_files_current_event_later() {
    let (data, mut w) = setup();
    let rules = Probe::default();
    w.event_deck = vec!["对邦".into()];
    w.event_discard = vec!["a".into(), "b".into()];
    let mut cx = Cx::new(w, &data, &rules, &[]);
    cx.card_draw_event(0).unwrap();
    let w = cx.world_copy();
    assert_eq!(*rules.seen.lock().unwrap(), ["body"]);
    assert_eq!(w.event_deck.len(), 2);
    assert_eq!(w.event_discard, ["对邦"]);
}

#[test]
fn removing_last_event_also_restores_the_deck_immediately() {
    let (_, mut w) = setup();
    w.event_deck = vec!["last".into()];
    w.event_discard = vec!["a".into(), "b".into()];
    w.event_banish("last");
    assert_eq!(w.event_deck.len(), 2);
    assert!(w.event_discard.is_empty());
    assert_eq!(w.event_removed.last().map(String::as_str), Some("last"));
}
