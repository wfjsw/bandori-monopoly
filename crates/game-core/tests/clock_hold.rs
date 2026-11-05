//! Turn-clock anim hold: pause until every pending human seat acks
//! `anim_done`, the 10s cap, stale-ack rejection, and Solo / AI skipping the
//! hold. The hold is tick-fed and recorded as `Input::AnimDone`, so these
//! tests pin the deterministic inputs the replay stream depends on.

use std::sync::Arc;

use game_core::engine::{Match, StubRules};
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::stage;
use game_core::MatchMode;
mod common;
use common::data_arc as data;
use common::Pump;

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
        m.pump_tick(0.1);
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
        m.pump_tick(0.25);
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
    // Default settings refill bonus to 10 at turn start and drain **bonus
    // first**, so pin the combined pool (`time_left`) rather than raw
    // `base_time` -- base remains at 15 until the bonus is gone.
    assert!(
        st.bonus_time > 0.0,
        "turn start refills bonus (got {})",
        st.bonus_time
    );
    let total = st.time_left;
    let base = st.base_time;
    let bonus = st.bonus_time;
    let seq = st.seq;

    // Ticking while held must not consume base/bonus.
    for _ in 0..20 {
        m.pump_tick(0.1);
    }
    let st = m.state();
    assert!(st.clock_paused, "still held without an ack");
    assert_eq!(st.time_left, total, "held clock does not consume");
    assert_eq!(st.base_time, base, "held clock does not consume base");
    assert_eq!(st.bonus_time, bonus, "held clock does not consume bonus");

    // A matching ack releases the hold; the clock runs again, bonus first.
    assert!(m.anim_done(me, seq), "matching ack releases");
    assert!(!m.state().clock_paused);
    for _ in 0..10 {
        m.pump_tick(0.1);
    }
    let st = m.state();
    assert!(!st.clock_paused);
    assert!(
        st.time_left < total,
        "released clock consumes pool time ({} -> {})",
        total,
        st.time_left
    );
    assert!(
        st.bonus_time < bonus,
        "bonus drains first ({} -> {})",
        bonus,
        st.bonus_time
    );
    assert_eq!(st.base_time, base, "base is untouched while bonus remains");
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
    // Bonus-first drain: pin the combined pool, not raw base.
    let total = m.state().time_left;

    // 10s of ticks, no ack: the cap is reached and the clock starts.
    // ANIM_HOLD_CAP = 10.0; tick 9.9s first (still held), then over the cap.
    for _ in 0..99 {
        m.pump_tick(0.1);
    }
    assert!(m.state().clock_paused, "under the cap still holds");
    assert_eq!(m.state().time_left, total);
    for _ in 0..5 {
        m.pump_tick(0.1);
    }
    let st = m.state();
    assert!(!st.clock_paused, "the 10s cap releases the hold");
    // Cap consumed 10s of hold, then the extra ticks drain the clock.
    assert!(st.time_left < total);
}

#[test]
fn solo_never_holds_the_clock() {
    let members = [member(1, false), member(2, true)];
    let mut m = new_match(&members, 3, MatchMode::Solo);
    m.quick_start();
    for _ in 0..200 {
        m.pump_tick(0.1);
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
        m.pump_tick(0.1);
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
        m.pump_tick(0.1);
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

/// Drive to the opening re-roll with every bot answered (they answer through
/// the same drive list a host uses), leaving only the human seats pending.
fn to_mulligan(m: &mut Match) {
    for _ in 0..1_000 {
        m.pump_tick(0.1);
        if m.state().prompt.kind == "mulligan" {
            // The bots take a tick or two to answer; settle them.
            for _ in 0..5 {
                m.pump_tick(0.1);
            }
            return;
        }
    }
    panic!("never reached the mulligan");
}

#[test]
fn the_mulligan_window_freezes_during_the_animation_wait() {
    // The opening re-roll runs before the first turn (`turn` = -1) on its own
    // fixed window. It must still hold: the seat cannot answer until the
    // opening deal has animated, so that presentation may not eat the window.
    let members = [member(1, false), member(2, true), member(3, true)];
    let mut m = new_match(&members, 7, MatchMode::Casual);
    m.quick_start();
    to_mulligan(&mut m);
    let st = m.state();
    assert_eq!(st.phase, "play");
    assert!(
        st.clock_paused,
        "a prompt before the first turn still holds"
    );
    let left = st.prompt.time_left;
    assert!(
        left > 10.0,
        "the window is intact at the prompt (got {left})"
    );

    for _ in 0..30 {
        m.pump_tick(0.1);
    }
    let st = m.state();
    assert!(st.clock_paused, "still held without an ack");
    assert_eq!(st.prompt.time_left, left, "the held window does not move");

    // The ack releases; the window runs again. (Take the current seq: a bot
    // answering late would have re-keyed the hold.)
    let seq = st.seq;
    assert!(m.anim_done(1, seq));
    assert!(!m.state().clock_paused);
    for _ in 0..10 {
        m.pump_tick(0.1);
    }
    assert!(
        m.state().prompt.time_left < left,
        "a released window consumes time"
    );
}

#[test]
fn a_multi_human_prompt_waits_for_every_held_seat() {
    let members = [member(1, false), member(2, false), member(3, true)];
    let mut m = new_match(&members, 7, MatchMode::Casual);
    m.quick_start();
    to_mulligan(&mut m);
    let seq = m.state().seq;
    assert!(m.state().clock_paused);
    assert!(m.anim_done(1, seq), "one human's ack registers");
    assert!(
        m.state().clock_paused,
        "the window still waits on the other human"
    );
    let left = m.state().prompt.time_left;
    for _ in 0..10 {
        m.pump_tick(0.1);
    }
    assert_eq!(m.state().prompt.time_left, left, "still frozen");
    let seq = m.state().seq;
    assert!(m.anim_done(2, seq));
    assert!(!m.state().clock_paused, "the last ack releases");
}

/// Play until an idle human turn surface (answering every prompt for `humans`).
fn to_play(m: &mut Match, humans: &[i32]) {
    for _ in 0..2_000 {
        for &h in humans {
            answer_first(m, h);
        }
        let st = m.state();
        if st.phase == "play" && st.prompt.id == 0 && !st.busy && st.turn >= 0 {
            return;
        }
        m.pump_tick(0.1);
    }
    panic!("never reached play");
}

#[test]
fn the_vote_window_freezes_during_the_animation_wait() {
    let members = [member(1, false), member(2, false), member(3, true)];
    let mut m = new_match(&members, 7, MatchMode::Casual);
    m.quick_start();
    to_play(&mut m, &[1, 2]);
    // Clear whatever hold the live turn is under, then open the vote off-turn.
    if m.state().clock_paused {
        let seq = m.state().seq;
        let holder = m
            .state()
            .players
            .iter()
            .find(|p| !p.bot && !p.ai)
            .unwrap()
            .member;
        m.anim_done(holder, seq);
    }
    m.act(
        1,
        &NetMessage {
            value: 1,
            ..NetMessage::act("vote")
        },
    )
    .unwrap();
    assert!(m.state().vote.id != 0);
    // Let the bot's auto vote land (it re-arms against the new seq).
    m.pump_tick(0.1);
    let st = m.state();
    assert!(st.vote.id != 0, "the human seat is still pending");
    assert!(st.clock_paused, "the vote holds");
    let left = st.vote.time_left;

    let bot_answered = st.vote.answers[2] >= 0;
    assert!(bot_answered, "the machine seat already voted");
    for _ in 0..30 {
        m.pump_tick(0.1);
    }
    let st = m.state();
    assert!(st.vote.id != 0, "the held window did not run out");
    assert_eq!(st.vote.time_left, left, "the vote window stays frozen");

    // The held voter's ack releases; the window runs again. (Bots keep acting
    // during the ticks, re-keying the hold, so take the current seq.)
    let seq = st.seq;
    assert!(m.anim_done(2, seq));
    assert!(!m.state().clock_paused);
    for _ in 0..10 {
        m.pump_tick(0.1);
    }
    assert!(
        m.state().vote.time_left < left || m.state().vote.id == 0,
        "a released vote window consumes time"
    );
}
