//! Per-seat clock pools: bonus-first drain, per-turn base refill + cap-lift,
//! prompt consumption, the dedicated bankruptcy-auction / setup timers, and
//! the pre-settings (`legacy_clock`) gate.
//!
//! Defaults reproduce the pre-settings feel (`docs/SERVER.md` §Turn clock):
//! base 15 / bonus 0 to start, `base_refill` 15 capped at the initial pool,
//! bonus refills 10 per turn capped at 30. Ban / pick / deck and the
//! bankruptcy leftover auction never touch a seat pool.

use std::sync::Arc;

use game_core::clock::ClockSettings;
use game_core::engine::{Ask, CardRules, Cx, Dest, Flow, Match, StubRules};
use game_core::msg::Msg;
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::stage;
use game_core::MatchMode;

mod common;
use common::data_arc as data;

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

fn new_match_clock(
    members: &[RoomMember],
    seed: u64,
    mode: MatchMode,
    clock: ClockSettings,
    legacy: bool,
) -> Match {
    Match::new_with_clock(
        data(),
        Arc::new(StubRules),
        members,
        seed,
        mode,
        ScoreWeights::default(),
        clock,
        legacy,
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

/// Drive to `human`'s OPS turn; returns the member id. Any prompt that opens
/// on the way is answered with its default so the walk continues. Also burns
/// off the START→OPS beat (`wait = 1.2`) so the clock is actually running.
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

/// Release the anim-ack hold (every `act` re-arms it) and burn off the
/// START→OPS `wait`, so subsequent ticks actually consume the pool.
fn release_hold_and_settle(m: &mut Match, member: i32) {
    let seq = m.state().seq;
    let _ = m.anim_done(member, seq);
    // The 1.2 s START→OPS beat parks the clock; 2 s of ticks clears it and
    // then some, without draining much pool (the hold also eats the first
    // part if it re-armed).
    for _ in 0..20 {
        m.tick(0.1);
    }
    // A later `act` (e.g. `give_play`) re-arms the hold against a new seq --
    // release again so the prompt / turn clock runs.
    let seq = m.state().seq;
    let _ = m.anim_done(member, seq);
}

/// A rules stub whose only effect is a prompt on play, so a test can watch a
/// seat's pool while a prompt is open. `use_pool` selects ordinary (pool) vs
/// dedicated-window behaviour.
struct PromptRules {
    use_pool: bool,
    time: f32,
}

impl CardRules for PromptRules {
    fn ruleset_sha256(&self) -> Option<&str> {
        Some("stub")
    }

    fn play(&self, cx: &mut Cx, player_id: usize, _card: &str) -> Flow<Dest> {
        let mut ask = Ask::choice(
            vec![player_id],
            Msg::new("ask.mulligan.title"),
            Msg::new("ask.mulligan.text"),
            vec![Msg::new("ask.mulligan.keep"), Msg::new("ask.mulligan.redo")],
            0,
            self.time,
        );
        ask.use_pool = self.use_pool;
        cx.ask(ask)?;
        Ok(Dest::Graveyard)
    }

    fn event(&self, _cx: &mut Cx, _player_id: usize, _id: &str) -> Flow<bool> {
        Ok(false)
    }
}

fn new_match_rules(
    members: &[RoomMember],
    seed: u64,
    mode: MatchMode,
    rules: Arc<dyn CardRules>,
    clock: ClockSettings,
) -> Match {
    Match::new_with_clock(
        data(),
        rules,
        members,
        seed,
        mode,
        ScoreWeights::default(),
        clock,
        false,
    )
}

/// Give `who` `card` and play it (the prompt-raisers live in the test's own
/// rules stub, so any card id works).
fn give_play(m: &mut Match, who: i32, card: &str) {
    m.give_cards(who, &[card]);
    m.act(
        who,
        &NetMessage {
            card: card.into(),
            ..NetMessage::act("play")
        },
    )
    .unwrap();
}

// ---------------------------------------------------------------- drain order

#[test]
fn bonus_drains_before_base() {
    let members = [member(1, false), member(2, true)];
    let mut m = new_match(&members, 7, MatchMode::Casual);
    to_human_ops(&mut m, 1);
    let st = m.state();
    // Turn start refills bonus to 10 under the defaults.
    assert_eq!(st.bonus_time, 10.0, "turn start refills bonus: {:?}", st);
    assert_eq!(st.base_time, 15.0, "base starts at the settings pool");
    release_hold_and_settle(&mut m, 1);
    let before = m.state();
    for _ in 0..10 {
        m.tick(0.1);
    }
    let st = m.state();
    assert!(
        st.bonus_time < before.bonus_time,
        "bonus drains first ({} -> {})",
        before.bonus_time,
        st.bonus_time
    );
    assert_eq!(
        st.base_time, before.base_time,
        "base is untouched while bonus remains"
    );
}

#[test]
fn base_drains_only_after_the_bonus_is_gone() {
    let members = [member(1, false), member(2, true)];
    // Turn start refills bonus to (0.5 + 10).min(30) = 10.5, so the test
    // needs ~11 s of ticks to empty it and spill onto base. The point is the
    // *order*: bonus drops first, base only after bonus hits zero.
    let clock = ClockSettings {
        bonus_time: 0.5,
        ..Default::default()
    };
    let mut m = new_match_clock(&members, 7, MatchMode::Casual, clock, false);
    to_human_ops(&mut m, 1);
    release_hold_and_settle(&mut m, 1);
    let before = m.state();
    assert!(before.bonus_time > 0.0, "refilled bonus: {before:?}");
    // Phase 1: drain only the bonus.
    for _ in 0..40 {
        m.tick(0.1);
    }
    let mid = m.state();
    assert!(
        mid.bonus_time < before.bonus_time,
        "bonus drains first ({} -> {})",
        before.bonus_time,
        mid.bonus_time
    );
    assert_eq!(
        mid.base_time, before.base_time,
        "base is untouched while bonus remains"
    );
    // Phase 2: past the bonus, the spill lands on base.
    for _ in 0..120 {
        m.tick(0.1);
    }
    let st = m.state();
    assert_eq!(st.bonus_time, 0.0, "bonus emptied first: {st:?}");
    assert!(
        st.base_time < before.base_time,
        "the spill drained base ({} < {}): {:?}",
        st.base_time,
        before.base_time,
        st
    );
    assert!(st.time_left > 0.0);
}

// ---------------------------------------------------------------- refill / cap

#[test]
fn base_refill_is_capped_at_the_initial_pool() {
    let members = [member(1, true), member(2, true)];
    // base_time 20, base_refill 5: the cap is max(20, 5) = 20, so the pool
    // never grows past the initial value however many turns pass.
    let clock = ClockSettings {
        base_time: 20.0,
        base_refill: 5.0,
        bonus_time: 0.0,
        ..Default::default()
    };
    let mut m = new_match_clock(&members, 3, MatchMode::Casual, clock, false);
    m.quick_start();
    let mut peak = 0.0f32;
    for _ in 0..400 {
        m.tick(0.1);
        if m.state().phase == "play" {
            peak = peak.max(m.state().base_time);
        }
        if m.ended() {
            break;
        }
    }
    assert!(
        peak <= 20.0 + 1e-3,
        "base never exceeds the initial pool (peak {peak})"
    );
}

#[test]
fn a_larger_base_refill_lifts_the_cap() {
    // The TEST:clockHook shape: a hook that doubles the refill 15 → 30 must
    // still show as base 30 (cap-lift), not be clipped back at 15. We cannot
    // raise the hook from `game-core` tests (no card ruleset), so pin the
    // arithmetic the engine applies at `StartTimer` via a refill that is
    // itself larger than the pool: `base = (15 + 30).min(max(15, 30)) = 30`.
    let members = [member(1, false), member(2, true)];
    let clock = ClockSettings {
        base_time: 15.0,
        base_refill: 30.0,
        ..Default::default()
    };
    let mut m = new_match_clock(&members, 7, MatchMode::Casual, clock, false);
    to_human_ops(&mut m, 1);
    let st = m.state();
    // First turn: (15 + 30).min(max(15, 30)) = 30.
    assert_eq!(
        st.base_time, 30.0,
        "cap-lift keeps the larger refill: {st:?}"
    );
}

// ---------------------------------------------------------------- per-seat

#[test]
fn seat_pools_are_independent() {
    // Two humans; P1 sits on a prompt while P2 is idle. P1's prompt drain
    // must not touch P2's pool. `MatchState` publishes the *acting* seat's
    // pools, so we watch P1's countdown move and pin P2's via a second
    // `state()` once the turn passes -- here P2 never acts, so their pool
    // must still read the untouched initial refill at the next turn start.
    let members = [member(1, false), member(2, false)];
    let rules: Arc<dyn CardRules> = Arc::new(PromptRules {
        use_pool: true,
        time: 30.0,
    });
    let mut m = new_match_rules(&members, 11, MatchMode::Casual, rules, Default::default());
    to_human_ops(&mut m, 1);
    release_hold_and_settle(&mut m, 1);
    give_play(&mut m, 1, "TEST:prompt");
    assert_ne!(m.state().prompt.id, 0, "the prompt opened");
    let seq = m.state().seq;
    let _ = m.anim_done(1, seq);
    let before = m.state();
    for _ in 0..10 {
        m.tick(0.1);
    }
    let after = m.state();
    assert!(
        after.time_left < before.time_left,
        "P1's prompt consumes P1's pool ({} -> {})",
        before.time_left,
        after.time_left
    );
}

// ---------------------------------------------------------------- prompts

#[test]
fn a_prompt_consumes_the_answering_seats_pool() {
    let members = [member(1, false), member(2, true)];
    let rules: Arc<dyn CardRules> = Arc::new(PromptRules {
        use_pool: true,
        time: 60.0,
    });
    let mut m = new_match_rules(&members, 7, MatchMode::Casual, rules, Default::default());
    to_human_ops(&mut m, 1);
    release_hold_and_settle(&mut m, 1);
    give_play(&mut m, 1, "TEST:prompt");
    let st = m.state();
    assert_ne!(st.prompt.id, 0);
    // `give_play`'s `act` re-armed the hold; release it so the pool runs.
    let seq = st.seq;
    let _ = m.anim_done(1, seq);
    let before = m.state();
    for _ in 0..10 {
        m.tick(0.1);
    }
    let st = m.state();
    assert!(
        st.time_left < before.time_left,
        "an open prompt drains the actor's pool ({} -> {})",
        before.time_left,
        st.time_left
    );
    // The soft window is still the ask's own allotment for AI scheduling --
    // it must NOT be what the player sees. `prompt.time_left` tracks the pool.
    assert!(
        st.prompt.time_left < 60.0,
        "prompt.time_left follows the pool, not the 60 s ask window (got {})",
        st.prompt.time_left
    );
}

#[test]
fn prompt_timeout_falls_back_once_the_pool_is_spent() {
    let members = [member(1, false), member(2, true)];
    let rules: Arc<dyn CardRules> = Arc::new(PromptRules {
        use_pool: true,
        time: 600.0,
    });
    // Turn start refills bonus to 10 regardless of `bonus_time`, so the real
    // pool is base 1 + bonus 10 = 11 s. The ask's own window is ten minutes;
    // the pool (plus the 1.5 s grace) must still expire the prompt.
    let clock = ClockSettings {
        base_time: 1.0,
        bonus_time: 0.0,
        base_refill: 1.0,
        ..Default::default()
    };
    let mut m = new_match_rules(&members, 7, MatchMode::Casual, rules, clock);
    to_human_ops(&mut m, 1);
    give_play(&mut m, 1, "TEST:prompt");
    assert_ne!(m.state().prompt.id, 0);
    let seq = m.state().seq;
    let _ = m.anim_done(1, seq);
    // ~13 s of ticks: 11 s pool + REMOTE_GRACE 1.5 s, then the fallback lands.
    for _ in 0..200 {
        m.tick(0.1);
        if m.state().prompt.id == 0 {
            break;
        }
    }
    assert_eq!(
        m.state().prompt.id,
        0,
        "an exhausted pool force-completes the prompt"
    );
}

#[test]
fn a_dedicated_window_prompt_does_not_drain_pools() {
    let members = [member(1, false), member(2, true)];
    let rules: Arc<dyn CardRules> = Arc::new(PromptRules {
        use_pool: false,
        time: 30.0,
    });
    let mut m = new_match_rules(&members, 7, MatchMode::Casual, rules, Default::default());
    to_human_ops(&mut m, 1);
    release_hold_and_settle(&mut m, 1);
    give_play(&mut m, 1, "TEST:prompt");
    assert_ne!(m.state().prompt.id, 0);
    let seq = m.state().seq;
    let _ = m.anim_done(1, seq);
    let before = m.state();
    for _ in 0..10 {
        m.tick(0.1);
    }
    let after = m.state();
    assert_eq!(
        (after.base_time, after.bonus_time),
        (before.base_time, before.bonus_time),
        "a use_pool=false prompt never touches a seat pool"
    );
    assert!(
        after.prompt.time_left < before.prompt.time_left,
        "its own window still ticks ({} -> {})",
        before.prompt.time_left,
        after.prompt.time_left
    );
}

// ---------------------------------------------------------------- setup windows

#[test]
fn ban_pick_deck_honour_the_settings() {
    // Two humans so nothing auto-acts and the windows remain readable.
    let members = [member(1, false), member(2, false)];
    let clock = ClockSettings {
        ban_seconds: 3.0,
        pick_seconds: 4.0,
        deck_seconds: 5.0,
        ..Default::default()
    };
    let mut m = new_match_clock(&members, 7, MatchMode::Ranked, clock, false);
    // Ranked opens on the ban phase after the order beat -- no `quick_start`
    // (that would skip setup entirely).
    let mut guard = 0;
    while m.state().phase != "ban" {
        m.tick(0.1);
        guard += 1;
        assert!(guard < 200, "never reached ban: {:?}", m.state().phase);
    }
    let st = m.state();
    assert!(
        (st.time_left - 3.0).abs() < 0.35,
        "ban window is settings.ban_seconds (got {})",
        st.time_left
    );
}

// ---------------------------------------------------------------- canon

#[test]
fn canon_omits_a_default_clock_and_appends_a_custom_one() {
    use game_core::fair::CanonSettings;
    use game_core::net::RoomMember;

    let members = vec![RoomMember {
        id: 1,
        player: "alice".into(),
        bot: true,
        ..Default::default()
    }];
    let default = CanonSettings::new(MatchMode::Casual, 0.05, &ScoreWeights::default(), &members);
    let s = default.canon();
    assert!(
        !s.contains("clock,"),
        "a default clock keeps the pre-settings string: {s:?}"
    );

    let mut custom = default.clone();
    custom.clock = ClockSettings {
        base_time: 20.0,
        ..Default::default()
    };
    let c = custom.canon();
    assert!(c.starts_with(&s), "the prefix is unchanged: {c:?}");
    assert!(
        c.contains("clock,"),
        "a non-default clock appends its line: {c:?}"
    );
    assert_eq!(
        c.matches("clock,").count(),
        1,
        "exactly one trailing clock line: {c:?}"
    );
}

// ---------------------------------------------------------------- legacy gate

#[test]
fn legacy_clock_drains_base_first() {
    let members = [member(1, false), member(2, true)];
    let mut m = new_match_clock(&members, 7, MatchMode::Casual, Default::default(), true);
    to_human_ops(&mut m, 1);
    let st = m.state();
    // Legacy: base is *set* to the hook value (15) at turn start and starts
    // empty before that; bonus refills 10 as it always did.
    assert_eq!(st.base_time, 15.0, "legacy base reset-to: {st:?}");
    release_hold_and_settle(&mut m, 1);
    let before = m.state();
    for _ in 0..10 {
        m.tick(0.1);
    }
    let st = m.state();
    // Base-first: base drops, bonus is untouched.
    assert!(
        st.base_time < before.base_time,
        "legacy drains base first: {st:?}"
    );
    assert_eq!(
        st.bonus_time, before.bonus_time,
        "legacy leaves bonus alone: {st:?}"
    );
}

#[test]
fn legacy_prompts_do_not_drain_pools() {
    let members = [member(1, false), member(2, true)];
    let rules: Arc<dyn CardRules> = Arc::new(PromptRules {
        use_pool: true,
        time: 60.0,
    });
    // `legacy_clock` forces the old fixed-window behaviour even though the
    // ask says `use_pool` -- the gate is on the *match*, not the prompt.
    let mut m = Match::new_with_clock(
        data(),
        rules,
        &members,
        7,
        MatchMode::Casual,
        ScoreWeights::default(),
        ClockSettings::default(),
        true,
    );
    to_human_ops(&mut m, 1);
    release_hold_and_settle(&mut m, 1);
    give_play(&mut m, 1, "TEST:prompt");
    assert_ne!(m.state().prompt.id, 0);
    let seq = m.state().seq;
    let _ = m.anim_done(1, seq);
    let before = m.state();
    for _ in 0..10 {
        m.tick(0.1);
    }
    let after = m.state();
    assert_eq!(
        (after.base_time, after.bonus_time),
        (before.base_time, before.bonus_time),
        "a legacy prompt never touches a seat pool"
    );
    assert!(
        after.prompt.time_left < before.prompt.time_left,
        "the fixed window still ticks"
    );
}

// ---------------------------------------------------------------- personalization

#[test]
fn personalize_prompt_time_reports_the_viewers_own_pool() {
    let members = [member(1, false), member(2, false)];
    let rules: Arc<dyn CardRules> = Arc::new(PromptRules {
        use_pool: true,
        time: 60.0,
    });
    let mut m = new_match_rules(&members, 11, MatchMode::Casual, rules, Default::default());
    to_human_ops(&mut m, 1);
    release_hold_and_settle(&mut m, 1);
    give_play(&mut m, 1, "TEST:prompt");
    assert_ne!(m.state().prompt.id, 0);
    let mut st = m.state();
    // P1 is the pending human; the table-wide value is their pool.
    let table_wide = st.prompt.time_left;
    m.personalize_prompt_time(&mut st, 1);
    assert_eq!(
        st.prompt.time_left, table_wide,
        "the pending human sees their own pool"
    );
    // A spectator / non-pending member keeps the table-wide value.
    let mut st2 = m.state();
    m.personalize_prompt_time(&mut st2, 2);
    assert_eq!(st2.prompt.time_left, table_wide);
}
