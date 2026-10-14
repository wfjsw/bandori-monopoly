//! Turn-clock anim hold: pause until the acting seat acks `anim_done`,
//! the 10s cap, stale-ack rejection, and Solo / AI skipping the hold.
//! The hold is tick-fed and recorded as `Input::AnimDone`, so these tests
//! pin the deterministic inputs the replay stream depends on.

use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{Match, StubRules};
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::stage;
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .unwrap(),
    )
}

fn member(id: i32, bot: bool) -> RoomMember {
    RoomMember {
        id,
        player: format!("P{id}"),
        bot,
        ..Default::default()
    }
}

fn new_match(members: &[RoomMember], seed: u64, mode: MatchMode) -> Match {
    Match::new(
        data(),
        Arc::new(StubRules),
        members,
        seed,
        mode,
        ScoreWeights::default(),
    )
}

fn answer_first(m: &mut Match, member: i32) {
    let st = m.state();
    if st.prompt.id == 0 {
        return;
    }
    let Some(player_id) = st.players.iter().position(|s| s.member == member) else {
        return;
    };
    if !st.prompt.waiting(player_id as i32) {
        return;
    }
    let msg = match st.prompt.kind.as_str() {
        "mortgage" => NetMessage {
            cards: st.prompt.items.clone(),
            ..NetMessage::act("answer")
        },
        "auction" => NetMessage {
            value: -1,
            ..NetMessage::act("answer")
        },
        _ => NetMessage {
            value: 0,
            ..NetMessage::act("answer")
        },
    };
    let msg = NetMessage {
        prompt: st.prompt.id,
        ..msg
    };
    m.act(member, &msg).unwrap();
}

/// Drive to the human's OPS turn; returns the human's member id (what `anim_done` takes).
fn to_human_ops(m: &mut Match, human: i32) -> i32 {
    m.quick_start();
    for _ in 0..40 {
        m.tick(0.1);
    }
    answer_first(m, human);
    let me = m
        .state()
        .players
        .iter()
        .position(|s| s.member == human)
        .unwrap() as i32;
    let mut guard = 0;
    while !(m.state().turn == me && m.state().step == stage::OPS && !m.state().busy) {
        m.tick(0.25);
        answer_first(m, human);
        guard += 1;
        assert!(guard < 10_000, "never got a human OPS turn");
    }
    human
}

#[test]
fn the_clock_holds_until_the_acting_seat_acks_anim_done() {
    let members = [member(1, false), member(2, true), member(3, true)];
    let mut m = new_match(&members, 7, MatchMode::Casual);
    let me = to_human_ops(&mut m, 1);
    let st = m.state();
    assert!(st.clock_paused, "a human turn arms the hold");
    let base = st.base_time;
    let seq = st.seq;

    // Ticking while held must not consume base/bonus.
    for _ in 0..20 {
        m.tick(0.1);
    }
    let st = m.state();
    assert!(st.clock_paused, "still held without an ack");
    assert_eq!(st.base_time, base, "held clock does not consume");

    // A matching ack releases the hold; the clock runs again.
    assert!(m.anim_done(me, seq), "matching ack releases");
    assert!(!m.state().clock_paused);
    for _ in 0..10 {
        m.tick(0.1);
    }
    let st = m.state();
    assert!(!st.clock_paused);
    assert!(
        st.base_time < base,
        "released clock consumes base time ({} -> {})",
        base,
        st.base_time
    );
}

#[test]
fn the_ack_names_the_room_member_not_the_seat_index() {
    // Member ids that cannot equal any seat index: an ack by seat number would
    // never match and the hold would always ride out the 10s cap.
    let members = [member(11, false), member(12, true), member(13, true)];
    let mut m = new_match(&members, 7, MatchMode::Casual);
    let human = to_human_ops(&mut m, 11);
    assert_eq!(human, 11);
    let seq = m.state().seq;
    assert!(m.state().clock_paused);
    assert!(!m.anim_done(0, seq), "a seat index is not a member id");
    assert!(m.anim_done(11, seq), "the member id releases");
    assert!(!m.state().clock_paused);
}

#[test]
fn a_stale_anim_done_ack_is_ignored() {
    let members = [member(1, false), member(2, true), member(3, true)];
    let mut m = new_match(&members, 7, MatchMode::Casual);
    let me = to_human_ops(&mut m, 1);
    let seq = m.state().seq;

    // An ack naming an older seq (or the wrong seat) must not release.
    assert!(!m.anim_done(me, seq - 1), "stale seq is ignored");
    assert!(m.state().clock_paused);
    assert!(!m.anim_done(me + 1, seq), "foreign seat is ignored");
    assert!(m.state().clock_paused);

    assert!(m.anim_done(me, seq));
    assert!(!m.state().clock_paused);
}

#[test]
fn the_anim_hold_cap_releases_after_ten_seconds_of_ticks() {
    let members = [member(1, false), member(2, true), member(3, true)];
    let mut m = new_match(&members, 7, MatchMode::Casual);
    let _me = to_human_ops(&mut m, 1);
    assert!(m.state().clock_paused);
    let base = m.state().base_time;

    // 10s of ticks, no ack: the cap fires and the clock starts.
    // ANIM_HOLD_CAP = 10.0; tick 9.9s first (still held), then over the cap.
    for _ in 0..99 {
        m.tick(0.1);
    }
    assert!(m.state().clock_paused, "under the cap still holds");
    assert_eq!(m.state().base_time, base);
    for _ in 0..5 {
        m.tick(0.1);
    }
    let st = m.state();
    assert!(!st.clock_paused, "the 10s cap releases the hold");
    // Cap consumed 10s of hold, then the extra ticks drain the clock.
    assert!(st.base_time < base);
}

#[test]
fn solo_never_holds_the_clock() {
    let members = [member(1, false), member(2, true)];
    let mut m = new_match(&members, 3, MatchMode::Solo);
    m.quick_start();
    for _ in 0..200 {
        m.tick(0.1);
        answer_first(&mut m, 1);
        if m.state().phase == "play" && m.state().step == stage::OPS && !m.state().busy {
            break;
        }
    }
    let st = m.state();
    assert_eq!(st.phase, "play");
    assert!(
        !st.clock_paused,
        "solo has no anim hold (and never expires a clock)"
    );
    let base = st.base_time;
    for _ in 0..20 {
        m.tick(0.1);
    }
    assert_eq!(m.state().base_time, base, "solo clock does not run down");
}

#[test]
fn a_bot_seat_never_holds_the_clock() {
    let members = [member(1, true), member(2, true), member(3, true)];
    let mut m = new_match(&members, 11, MatchMode::Casual);
    m.quick_start();
    // Let the bots get into play; no seat is a human, so nothing holds.
    let mut saw_play = false;
    for _ in 0..400 {
        m.tick(0.1);
        if m.state().phase == "play" {
            saw_play = true;
            assert!(
                !m.state().clock_paused,
                "machine seats never arm the anim hold"
            );
        }
        if m.ended() {
            break;
        }
    }
    assert!(saw_play, "the bot game reached play");
}

#[test]
fn anim_done_replay_input_is_recorded() {
    let members = [member(1, false), member(2, true), member(3, true)];
    let mut m = new_match(&members, 7, MatchMode::Casual);
    let me = to_human_ops(&mut m, 1);
    let seq = m.state().seq;
    assert!(m.anim_done(me, seq));
    // The ack is part of the match's public call log shape: save/restore
    // keeps the released clock (hold cleared) either way.
    let saved = m.save();
    let mut m2 = Match::restore(data(), Arc::new(StubRules), &saved).unwrap();
    assert!(!m2.clock_paused());
    // A second ack with nothing held is a no-op.
    assert!(!m2.anim_done(me, seq));
}