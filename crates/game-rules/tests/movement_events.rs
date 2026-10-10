//! Movement playback order: the client must walk to a tile before the card
//! that fired there flashes, and a walk split by mid-route events must chain
//! (`from` == previous `to`, total `value` == the step count).

mod common;

use common::*;

use game_core::state::MatchEvent;

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

// =====================================================================
// Lazy walk flush: a card flash (or any other event) mid-walk must land
// **after** the `roll`/`move` event that walks the client to the tile it
// fired on, and the split segments must chain.
// =====================================================================

fn walks(events: &[MatchEvent]) -> Vec<&MatchEvent> {
    events
        .iter()
        .filter(|e| matches!(e.r#type.as_str(), "roll" | "move"))
        .collect()
}

fn cards(events: &[MatchEvent]) -> Vec<&MatchEvent> {
    events.iter().filter(|e| e.r#type == "card").collect()
}

/// The split segments must chain (`from` == previous `to`, `to` ==
/// `from + value` wrapped) and their `value`s must sum to `steps`.
fn assert_chain(segs: &[&MatchEvent], start: i32, steps: i32, label: &str) {
    let n = data().tiles.len() as i32;
    let mut pos = start;
    let mut total = 0;
    for w in segs {
        assert_eq!(
            w.from, pos,
            "{label}: segment must start where the previous ended: {segs:?}"
        );
        let end = ((w.from + w.value) % n + n) % n;
        assert_eq!(w.to, end, "{label}: to must be from+value wrapped: {w:?}");
        pos = w.to;
        total += w.value;
    }
    assert_eq!(
        total, steps,
        "{label}: segment values must sum to the step count: {segs:?}"
    );
}

/// `TEST:pass_flash` / `TEST:pass_before_flash`: place the card and aim its
/// one-shot hook at `hook_tile`.
fn flash_walk(card: &str, tile_key: &str, hook_tile: i32) -> Table {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 20);
    t.set_pos(1, 50);
    t.set_state(0, tile_key, hook_tile);
    t.give_play(0, card).unwrap();
    t
}

#[test]
fn a_pass_tile_flash_waits_for_the_walk_to_the_hook_tile() {
    let mut t = flash_walk("TEST:pass_flash", "TEST.pass_flash.tile", 21);
    t.dice(&[4]);
    let mark = t.mark();
    t.roll(0).unwrap();

    let events = t.events_since(mark);
    let segs = walks(&events);
    let flashes = cards(&events);
    assert_eq!(flashes.len(), 1, "one card flash: {:?}", t.recent_keys(20));
    // The walk onto tile 21 must be announced before the flash.
    let first = segs.first().expect("a walk segment");
    assert_eq!(first.r#type, "roll");
    assert_eq!((first.from, first.to, first.value, first.dice), (20, 21, 1, 4));
    assert!(
        first.id < flashes[0].id,
        "walk to the hook tile, then the card flash: {:?}",
        events
            .iter()
            .map(|e| (e.id, e.r#type.as_str(), e.from, e.to, e.value))
            .collect::<Vec<_>>()
    );
    // The remainder continues from the hook tile; segments chain; total = 4.
    assert_chain(&segs, 20, 4, "pass_tile flash");
    assert_eq!(segs.len(), 2, "split at the flash: {segs:?}");
    assert_eq!((segs[1].from, segs[1].to, segs[1].value), (21, 24, 3));
    assert_eq!(t.pos(0), 24);
}

#[test]
fn a_mid_route_pass_tile_flash_splits_the_walk_around_it() {
    let mut t = flash_walk("TEST:pass_flash", "TEST.pass_flash.tile", 23);
    t.dice(&[5]);
    let mark = t.mark();
    t.roll(0).unwrap();

    let events = t.events_since(mark);
    let segs = walks(&events);
    let flashes = cards(&events);
    assert_eq!(flashes.len(), 1, "one card flash: {:?}", t.recent_keys(20));
    // Steps 21-22 are silent (no flush); the flash on 23 flushes 20 -> 23.
    let first = segs.first().expect("a walk segment");
    assert_eq!(first.r#type, "roll");
    assert_eq!((first.from, first.to, first.value, first.dice), (20, 23, 3, 5));
    assert!(
        first.id < flashes[0].id,
        "walk to the hook tile, then the card flash: {:?}",
        events
            .iter()
            .map(|e| (e.id, e.r#type.as_str(), e.from, e.to, e.value))
            .collect::<Vec<_>>()
    );
    assert_chain(&segs, 20, 5, "mid-route pass_tile flash");
    assert_eq!(segs.len(), 2, "split at the flash: {segs:?}");
    assert_eq!((segs[1].from, segs[1].to, segs[1].value), (23, 25, 2));
    assert_eq!(t.pos(0), 25);
}

#[test]
fn a_pass_before_flash_waits_for_the_walk_to_the_previous_tile() {
    // `passBefore` fires before the step onto 23, so the flush it forces
    // covers steps up to tile 22 -- the tile before the hook's.
    let mut t = flash_walk("TEST:pass_before_flash", "TEST.pass_before_flash.tile", 23);
    t.dice(&[5]);
    let mark = t.mark();
    t.roll(0).unwrap();

    let events = t.events_since(mark);
    let segs = walks(&events);
    let flashes = cards(&events);
    assert_eq!(flashes.len(), 1, "one card flash: {:?}", t.recent_keys(20));
    let first = segs.first().expect("a walk segment");
    assert_eq!(first.r#type, "roll");
    assert_eq!((first.from, first.to, first.value, first.dice), (20, 22, 2, 5));
    assert!(
        first.id < flashes[0].id,
        "walk to the tile before the hook tile, then the card flash: {:?}",
        events
            .iter()
            .map(|e| (e.id, e.r#type.as_str(), e.from, e.to, e.value))
            .collect::<Vec<_>>()
    );
    assert_chain(&segs, 20, 5, "pass_before flash");
    assert_eq!(segs.len(), 2, "split at the flash: {segs:?}");
    assert_eq!((segs[1].from, segs[1].to, segs[1].value), (22, 25, 3));
    assert_eq!(t.pos(0), 25);
}

#[test]
fn a_flash_on_the_first_pass_before_shows_the_dice_first() {
    // The very first `passBefore` has walked nothing yet. The head `roll`
    // still goes out first (dice-only, 0 steps) so the client sees the roll
    // before the card that reacted to it; the walk then continues from there.
    let mut t = flash_walk("TEST:pass_before_flash", "TEST.pass_before_flash.tile", 21);
    t.dice(&[4]);
    let mark = t.mark();
    t.roll(0).unwrap();

    let events = t.events_since(mark);
    let segs = walks(&events);
    let flashes = cards(&events);
    assert_eq!(flashes.len(), 1, "one card flash: {:?}", t.recent_keys(20));
    let first = segs.first().expect("a walk segment");
    assert_eq!(first.r#type, "roll", "the head is the main move's roll");
    assert_eq!(
        (first.from, first.to, first.value, first.dice),
        (20, 20, 0, 4),
        "a dice-only roll before the flash: {first:?}"
    );
    assert!(
        first.id < flashes[0].id,
        "the dice-only roll, then the card flash: {:?}",
        events
            .iter()
            .map(|e| (e.id, e.r#type.as_str(), e.from, e.to, e.value))
            .collect::<Vec<_>>()
    );
    assert_chain(&segs, 20, 4, "first pass_before flash");
    assert_eq!(segs.len(), 2, "split at the flash: {segs:?}");
    assert_eq!((segs[1].from, segs[1].to, segs[1].value), (20, 24, 4));
    assert_eq!(t.pos(0), 24);
}

#[test]
fn a_silent_walk_is_not_split() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 20);
    t.set_pos(1, 50);
    t.dice(&[4]);
    let mark = t.mark();
    t.roll(0).unwrap();

    let events = t.events_since(mark);
    let segs = walks(&events);
    assert_eq!(segs.len(), 1, "no events mid-walk, no split: {:?}", segs);
    let only = segs[0];
    assert_eq!(only.r#type, "roll");
    assert_eq!((only.from, only.to, only.value, only.dice), (20, 24, 4, 4));
    assert!(cards(&events).is_empty(), "no card fired: {events:?}");
    assert_eq!(t.pos(0), 24);
}
