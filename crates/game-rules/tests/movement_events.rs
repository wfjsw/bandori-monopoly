//! Movement playback must reach CiRCLE before its reward prompt is shown.

mod common;

use common::*;

fn circle_stop(from: usize, reverse: bool, steps: i32, segment: i32, landing: usize) {
    let mut t = Table::vanilla(2);
    t.set_pos(0, from);
    t.set_pos(1, 20);
    t.m.world_mut().turn.plan.reverse = reverse;
    t.dice(&[steps]);
    let mark = t.mark();
    t.roll(0).unwrap();

    assert_eq!(t.expect_prompt().title.key(), "ask.circle.title");
    assert_eq!(t.pos(0), 0);
    assert_eq!(t.money(0), 10_000, "the reward has not been chosen yet");
    let events = t.events_since(mark);
    let walks: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.r#type.as_str(), "roll" | "move"))
        .collect();
    assert_eq!(
        walks.len(),
        1,
        "the approach must be available before the prompt: {events:?}"
    );
    let first = walks[0];
    assert_eq!(first.r#type, "roll");
    assert_eq!(
        (first.from, first.to, first.value, first.dice),
        (from as i32, 0, segment, steps)
    );
    let first_id = first.id;
    t.m.tick(0.5);
    assert_eq!(t.pos(0), 0, "wait at CiRCLE until the reward is chosen");
    assert_eq!(
        t.mark(),
        events.last().unwrap().id,
        "no continuation while waiting"
    );

    t.answer_one(0).unwrap();
    assert_eq!(t.pos(0), landing);
    assert_eq!(t.money(0), 12_000);
    let events = t.events_since(mark);
    let reward = events
        .iter()
        .find(|e| e.msg.key() == "log.circle_money")
        .unwrap();
    assert!(first_id < reward.id, "walk to CiRCLE, then pay the reward");
    let walks: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.r#type.as_str(), "roll" | "move"))
        .collect();
    assert_eq!(
        walks[0].id, first_id,
        "answering must not repeat the approach"
    );
    if landing == 0 {
        assert_eq!(walks.len(), 1, "landing on CiRCLE needs no continuation");
    } else {
        assert_eq!(walks.len(), 2);
        let rest = walks[1];
        assert_eq!(rest.r#type, "move");
        assert_eq!(
            (rest.from, rest.to, rest.value),
            (
                0,
                landing as i32,
                if reverse {
                    -steps - segment
                } else {
                    steps - segment
                }
            )
        );
        assert!(
            reward.id < rest.id,
            "resolve the reward before continuing the walk"
        );
    }
}

#[test]
fn forward_walk_stops_for_circle_reward() {
    circle_stop(57, false, 6, 3, 3);
}

#[test]
fn reverse_walk_stops_for_circle_reward() {
    circle_stop(3, true, 6, -3, 57);
}

#[test]
fn landing_on_circle_emits_the_walk_before_the_reward() {
    circle_stop(58, false, 2, 2, 0);
}

#[test]
fn a_long_walk_announces_each_lap_before_its_reward() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 58);
    t.set_pos(1, 20);
    t.m.world_mut().turn.fixed_roll = Some(64);
    let mark = t.mark();
    t.roll(0).unwrap();
    assert_eq!(t.expect_prompt().title.key(), "ask.circle.title");
    let first = t
        .events_since(mark)
        .into_iter()
        .find(|e| e.r#type == "roll")
        .expect("announce the first segment before asking");
    assert_eq!(
        (first.from, first.to, first.value, first.dice),
        (58, 0, 2, 64)
    );
    t.answer_one(0).unwrap();
    assert_eq!(t.expect_prompt().title.key(), "ask.circle.title");
    let lap = t
        .events_since(first.id)
        .into_iter()
        .find(|e| e.r#type == "move")
        .expect("announce the lap before asking again");
    assert_eq!((lap.from, lap.to, lap.value), (0, 0, 60));
    t.answer_one(0).unwrap();
    let rest = t
        .events_since(lap.id)
        .into_iter()
        .find(|e| e.r#type == "move")
        .unwrap();
    assert_eq!((rest.from, rest.to, rest.value), (0, 2, 2));
    assert_eq!(t.pos(0), 2);
    assert_eq!(t.money(0), 14_000);
}
