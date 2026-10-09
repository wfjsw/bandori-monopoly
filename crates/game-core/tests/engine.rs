//! Match engine tests through the public API (P3).

use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{Match, StubRules};
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::stage;
use game_core::state::MatchState;
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

/// Invariants that must hold at every step of every game.
fn check_invariants(st: &MatchState, d: &GameData) {
    let n = st.players.len() as i32;
    for s in &st.players {
        assert!(s.money >= 0, "negative money: {} has {}", s.player, s.money);
        assert!(s.pos >= 0 && (s.pos as usize) < d.tiles.len());
    }
    for (t, &o) in st.owners.iter().enumerate() {
        assert!(o >= -1 && o < n, "tile {t} owner {o}");
        if o >= 0 {
            assert!(d.tiles[t].is_buyable(), "unbuyable tile {t} owned");
            assert!(
                !st.players[o as usize].out(),
                "tile {t} owned by a player that is out"
            );
        }
        let max = d.tiles[t].rent.len().saturating_sub(1) as i32;
        assert!(
            st.houses[t] >= 0 && st.houses[t] <= max,
            "tile {t} has {} houses",
            st.houses[t]
        );
    }
    for w in st.events.windows(2) {
        assert!(w[0].id < w[1].id, "event ids not increasing");
    }
}

/// Tick a bot-only game until it ends (or `max_rounds` pass, then finish it).
fn play_out(m: &mut Match, max_rounds: i32, d: &GameData) -> u32 {
    let mut ticks = 0;
    let mut last_event = 0;
    while !m.ended() {
        m.tick(0.25);
        ticks += 1;
        let st = m.state();
        if st.phase == "play" {
            check_invariants(&st, d);
        }
        let new = m.events_since(last_event);
        if let Some(e) = new.last() {
            assert!(
                new.windows(2).all(|w| w[1].id == w[0].id + 1),
                "event ids must be contiguous"
            );
            last_event = e.id;
        }
        if st.round > max_rounds {
            m.finish();
        }
        assert!(ticks < 2_000_000, "game did not progress");
    }
    ticks
}

#[test]
fn bot_games_play_to_the_end_and_rank_everyone() {
    let d = data();
    for seed in 1..=5u64 {
        let members: Vec<RoomMember> = (1..=4).map(|i| member(i, true)).collect();
        let mut m = new_match(&members, seed, MatchMode::Casual);
        m.quick_start();
        play_out(&mut m, 60, &d);

        let st = m.state();
        assert_eq!(st.phase, "ended");
        assert!(
            ["last", "settle", "out"].contains(&st.end_reason.as_str()),
            "{}",
            st.end_reason
        );
        let mut ranks: Vec<i32> = st.players.iter().map(|s| s.rank).collect();
        ranks.sort();
        assert_eq!(ranks, [1, 2, 3, 4], "seed {seed}");
        assert_eq!(st.players[st.winner as usize].rank, 1);
        // Something actually happened.
        let all = m.events_since(0);
        let kinds = |k: &str| all.iter().filter(|e| e.r#type == k).count();
        assert!(kinds("roll") > 0 && kinds("turn") > 0, "seed {seed}");
        // Survivors are ranked by score.
        let mut alive: Vec<_> = st.players.iter().filter(|s| !s.out()).collect();
        alive.sort_by_key(|s| s.rank);
        if st.end_reason == "settle" {
            assert!(
                alive.windows(2).all(|w| w[0].score >= w[1].score),
                "seed {seed}: survivors not ranked by score"
            );
        }
    }
}

#[test]
fn ranked_games_run_the_ban_phase() {
    let members: Vec<RoomMember> = (1..=5).map(|i| member(i, true)).collect();
    let mut m = new_match(&members, 3, MatchMode::Ranked);
    let mut saw_ban = false;
    for _ in 0..2000 {
        m.tick(0.5);
        saw_ban |= m.state().phase == "ban";
        if m.state().phase == "play" {
            break;
        }
    }
    assert!(saw_ban, "ranked must ban before picking");
    let st = m.state();
    assert_eq!(st.phase, "play");
    assert!(st
        .players
        .iter()
        .all(|s| !s.character.is_empty() && s.deck_ready));
    let mut chars: Vec<&str> = st.players.iter().map(|s| s.character.as_str()).collect();
    chars.sort();
    chars.dedup();
    assert_eq!(chars.len(), 5, "characters are unique");
    assert!(
        st.players.iter().all(|s| !st.bans.contains(&s.character)),
        "nobody picked a banned character"
    );
}

/// Solo with a character on every seat (the solo setup screen's picks) skips
/// the timed ban / pick entirely and opens the deck phase on those characters.
#[test]
fn solo_preset_characters_skip_the_pick_phase() {
    let d = data();
    let names: Vec<String> = d.characters.iter().take(3).map(|c| c.name.clone()).collect();
    let members: Vec<RoomMember> = (1..=3)
        .map(|i| RoomMember {
            id: i,
            player: format!("P{i}"),
            bot: i != 1,
            character: names[(i - 1) as usize].clone(),
            ..Default::default()
        })
        .collect();
    let m = new_match(&members, 42, MatchMode::Solo);
    let st = m.state();
    // Past ban / pick: the human still has a deck to choose.
    assert_eq!(st.phase, "deck", "preset characters land on the deck phase");
    assert!(st.bans.is_empty(), "solo preset skips bans too");
    for mem in &members {
        let p = st
            .players
            .iter()
            .find(|p| p.member == mem.id)
            .expect("member is seated");
        assert_eq!(p.character, mem.character, "member {}", mem.id);
    }
    // The roll still seats everyone; only the choice phases are skipped.
    let mut rolls: Vec<i32> = st.players.iter().map(|p| p.roll).collect();
    rolls.sort_unstable();
    assert!(rolls.iter().all(|&r| (1..=20).contains(&r)), "{rolls:?}");
    // Bots submit their decks the moment the deck phase opens; the human does not.
    for p in &st.players {
        assert_eq!(p.deck_ready, p.bot, "{}", p.player);
    }
    // Deterministic: the same seed rebuilds the same match on the same picks.
    let again = new_match(&members, 42, MatchMode::Solo);
    assert_eq!(m.save(), again.save());
    let other = new_match(&members, 43, MatchMode::Solo);
    assert_ne!(m.save(), other.save(), "the seed still decides the seating");
}

/// Without presets the pick phase stays: solo's test harness (and old saves)
/// still drive it, and `quick_start` still skips it.
#[test]
fn solo_without_presets_still_picks() {
    let members: Vec<RoomMember> = (1..=3).map(|i| member(i, true)).collect();
    let m = new_match(&members, 7, MatchMode::Solo);
    assert_eq!(m.state().phase, "order");
    let mut m = m;
    m.quick_start();
    assert_eq!(m.state().phase, "play");
    assert!(m.state().players.iter().all(|p| !p.character.is_empty()));
}

#[test]
fn same_seed_same_game() {
    let d = data();
    let run = |seed| {
        let members: Vec<RoomMember> = (1..=3).map(|i| member(i, true)).collect();
        let mut m = new_match(&members, seed, MatchMode::Casual);
        m.quick_start();
        play_out(&mut m, 25, &d);
        (
            serde_json::to_string(&m.state()).unwrap(),
            m.events_since(0).len(),
        )
    };
    assert_eq!(run(42), run(42));
    assert_ne!(run(42).0, run(43).0);
}

/// Answer every prompt that is waiting on `member` with its first option.
fn answer_first(m: &mut Match, member: i32) {
    let st = m.state();
    if st.prompt.id != 0 {
        if let Some(player_id) = st.players.iter().position(|s| s.member == member) {
            if st.prompt.waiting(player_id as i32) {
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
        }
    }
}

#[test]
fn a_human_plays_a_turn_with_commands() {
    let members = [member(1, false), member(2, true), member(3, true)];
    let mut m = new_match(&members, 7, MatchMode::Casual);
    m.quick_start();

    // Opening: the human is offered a mulligan.
    for _ in 0..40 {
        m.tick(0.1);
    }
    let st = m.state();
    assert_eq!(st.prompt.kind, "mulligan");
    assert_eq!(st.prompt.players.len(), 1, "only humans are asked");
    assert!(st.busy);
    assert_eq!(
        m.act(1, &NetMessage::act("roll")).unwrap_err().key(),
        "err.no_roll_now"
    );
    let wrong = NetMessage {
        prompt: st.prompt.id + 7,
        ..NetMessage::act("answer")
    };
    assert_eq!(m.act(1, &wrong).unwrap_err().key(), "err.prompt_over");
    answer_first(&mut m, 1);
    assert_eq!(m.state().prompt.id, 0);

    // Run until it is the human's turn to roll.
    let me = m
        .state()
        .players
        .iter()
        .position(|s| s.member == 1)
        .unwrap() as i32;
    let mut guard = 0;
    while !(m.state().turn == me && m.state().step == stage::OPS && !m.state().busy) {
        m.tick(0.25);
        answer_first(&mut m, 1);
        guard += 1;
        assert!(guard < 10_000, "never got a turn");
    }
    assert_eq!(
        m.act(1, &NetMessage::act("end")).unwrap_err().key(),
        "err.roll_first"
    );
    assert_eq!(
        m.act(1, &NetMessage::act("fly")).unwrap_err().key(),
        "err.unknown_act"
    );
    let before = m.state().players[me as usize].pos;
    m.act(1, &NetMessage::act("roll")).unwrap();
    while m.state().busy {
        answer_first(&mut m, 1);
        m.tick(0.1);
    }
    let st = m.state();
    assert_eq!(st.step, stage::END);
    assert_ne!(
        st.players[me as usize].pos, before,
        "the roll moved the token"
    );
    assert!(m
        .events_since(0)
        .iter()
        .any(|e| e.r#type == "roll" && e.player_id == me));

    // Buy if we landed on unowned land and can afford it.
    let pos = st.players[me as usize].pos as usize;
    let d = data();
    if d.tiles[pos].is_buyable() && st.owners[pos] < 0 && st.landed == pos as i32 {
        let price = d.tiles[pos].price + st.houses[pos] * d.tiles[pos].house;
        let money = st.players[me as usize].money;
        m.act(1, &NetMessage::act("buy")).unwrap();
        let st = m.state();
        assert_eq!(st.owners[pos], me);
        assert_eq!(st.players[me as usize].money, money - price);
        assert_eq!(
            m.act(1, &NetMessage::act("buy")).unwrap_err().key(),
            "err.cannot_buy"
        );
    }
    if m.state().players[me as usize].hand as usize <= 5 {
        m.act(1, &NetMessage::act("end")).unwrap();
        m.tick(3.0);
        m.tick(3.0);
        assert_ne!(m.state().turn, me, "turn passed on");
    }
}

#[test]
fn prompts_time_out_to_the_fallback_and_replay_without_duplicate_events() {
    let members = [member(1, false), member(2, false), member(3, true)];
    let mut m = new_match(&members, 11, MatchMode::Casual);
    m.quick_start();
    for _ in 0..40 {
        m.tick(0.1);
    }
    let st = m.state();
    assert_eq!(st.prompt.kind, "mulligan");
    assert_eq!(st.prompt.players.len(), 2, "both humans are asked together");
    let seen = st.events.last().unwrap().id;

    // Player 1 redraws; player 2 never answers.
    let redo = NetMessage {
        prompt: st.prompt.id,
        value: 1,
        ..NetMessage::act("answer")
    };
    m.act(1, &redo).unwrap();
    assert_eq!(m.act(1, &redo).unwrap_err().key(), "err.answered");
    assert!(m.state().busy, "still waiting for player 2");
    for _ in 0..200 {
        m.tick(0.1); // 20 s timer + grace
    }
    let st = m.state();
    assert!(!st.busy && st.prompt.id == 0, "timed out");
    let p1 = st.players.iter().find(|s| s.member == 1).unwrap();
    let p2 = st.players.iter().find(|s| s.member == 2).unwrap();
    assert!(
        p1.mulligan && !p2.mulligan,
        "answer applied; time-out used the fallback (keep)"
    );

    // The replay re-created the events before the prompt with the same ids: the
    // new ones simply continue the sequence.
    let after = m.events_since(seen);
    assert!(!after.is_empty());
    assert_eq!(after[0].id, seen + 1);
    assert!(after.windows(2).all(|w| w[1].id == w[0].id + 1));
}

#[test]
fn unanimous_vote_ends_the_match() {
    let members = [member(1, false), member(2, false), member(3, true)];
    let mut m = new_match(&members, 5, MatchMode::Casual);
    m.quick_start();
    while m.state().busy || m.state().turn < 0 {
        answer_first(&mut m, 1);
        answer_first(&mut m, 2);
        m.tick(0.25);
    }
    assert_eq!(
        m.act(
            2,
            &NetMessage {
                value: 0,
                ..NetMessage::act("vote")
            }
        )
        .unwrap_err()
        .key(),
        "err.no_vote"
    );
    m.act(
        1,
        &NetMessage {
            value: 1,
            ..NetMessage::act("vote")
        },
    )
    .unwrap();
    assert_eq!(
        m.state().vote.players.len(),
        3,
        "every seat still in the match votes (bots by the engine)"
    );
    assert_eq!(
        m.act(
            1,
            &NetMessage {
                value: 1,
                ..NetMessage::act("vote")
            }
        )
        .unwrap_err()
        .key(),
        "err.voted"
    );

    // A "no" fails the vote and starts a cool-down.
    m.act(
        2,
        &NetMessage {
            value: 0,
            ..NetMessage::act("vote")
        },
    )
    .unwrap();
    assert_eq!(m.state().vote.id, 0);
    assert!(
        m.act(
            1,
            &NetMessage {
                value: 1,
                ..NetMessage::act("vote")
            }
        )
        .unwrap_err()
        .key()
            == "err.vote_cooldown"
    );

    for _ in 0..140 {
        m.tick(0.25); // cool-down
        answer_first(&mut m, 1);
        answer_first(&mut m, 2);
    }
    m.act(
        1,
        &NetMessage {
            value: 1,
            ..NetMessage::act("vote")
        },
    )
    .unwrap();
    m.act(
        2,
        &NetMessage {
            value: 1,
            ..NetMessage::act("vote")
        },
    )
    .unwrap();
    // The bot seat's vote is cast by the engine (user ruling 2026-10-09) on
    // the next tick -- the drivers have no vote path.
    m.tick(0.25);
    let st = m.state();
    assert_eq!(
        (st.phase.as_str(), st.end_reason.as_str()),
        ("ended", "vote")
    );
}

/// An 进阶 (Advanced) bot seat is held for its external driver (`ai` off), but
/// the drivers never answer a vote surface and a solo vote never expires. The
/// engine must cast the seat's vote (user ruling 2026-10-09: "have the engine
/// vote for advanced bot seats") so the vote resolves with no external answer.
#[test]
fn an_advanced_bot_seat_gets_its_vote_cast_by_the_engine() {
    let mut members = [member(1, false), member(2, true), member(3, true)];
    members[1].mentality = game_core::state::BotMentality::Advanced;
    members[2].mentality = game_core::state::BotMentality::Advanced;
    let mut m = new_match(&members, 5, MatchMode::Solo);
    m.quick_start();
    while m.state().busy || m.state().turn < 0 {
        m.tick(0.25);
        answer_first(&mut m, 1);
        answer_first(&mut m, 2);
        answer_first(&mut m, 3);
    }
    // The advanced seats are `ai = false` (held for the driver) but still bots.
    let st = m.state();
    let advanced: Vec<_> = st
        .players
        .iter()
        .filter(|p| p.bot && p.mentality == game_core::state::BotMentality::Advanced)
        .collect();
    assert!(!advanced.is_empty(), "the advanced seats are present");
    assert!(
        advanced.iter().all(|p| !p.ai),
        "advanced seats are held for their driver: {:?}",
        advanced.iter().map(|p| p.ai).collect::<Vec<_>>()
    );

    // The human starts the end-match vote. Solo: the vote never expires, so
    // without the engine's vote for the advanced seats this would hang.
    m.act(
        1,
        &NetMessage {
            value: 1,
            ..NetMessage::act("vote")
        },
    )
    .unwrap();
    assert_eq!(m.state().vote.id, 1, "the vote is open: {:?}", m.state().vote);
    assert_eq!(
        m.state().vote.players.len(),
        3,
        "every seat votes -- humans by hand, bots by the engine"
    );
    // One tick: `tick_vote` casts the bot seats' votes.
    m.tick(0.25);
    let st = m.state();
    assert_eq!(
        (st.phase.as_str(), st.end_reason.as_str()),
        ("ended", "vote"),
        "the vote resolved with no external answer: {:?}",
        st.vote
    );
}

#[test]
fn leaving_forfeits_and_the_last_human_leaving_ends_the_match() {
    let members = [
        member(1, false),
        member(2, false),
        member(3, true),
        member(4, true),
    ];
    let mut m = new_match(&members, 9, MatchMode::Casual);
    m.quick_start();
    while m.state().turn < 0 {
        answer_first(&mut m, 1);
        answer_first(&mut m, 2);
        m.tick(0.25);
    }
    m.act(1, &NetMessage::act("leave")).unwrap();
    for _ in 0..20 {
        m.tick(0.25);
        answer_first(&mut m, 2);
    }
    let st = m.state();
    let p1 = st.players.iter().find(|s| s.member == 1).unwrap();
    assert!(p1.left && p1.out() && p1.money == 0);
    assert_eq!(st.phase, "play", "one human is still in");
    assert!(st
        .owners
        .iter()
        .all(|&o| o < 0 || st.players[o as usize].member != 1));

    m.act(2, &NetMessage::act("leave")).unwrap();
    for _ in 0..20 {
        m.tick(0.25);
    }
    let st = m.state();
    assert_eq!(
        (st.phase.as_str(), st.end_reason.as_str()),
        ("ended", "out")
    );
}

#[test]
fn disconnect_hands_the_player_to_the_ai_and_reconnect_takes_it_back() {
    let members = [member(1, false), member(2, true)];
    let mut m = new_match(&members, 2, MatchMode::Casual);
    m.quick_start();
    for _ in 0..400 {
        m.tick(0.25);
        if !m.state().busy {
            break;
        }
        m.member_left(1, true); // during the mulligan prompt: deferred, auto-answered
    }
    assert!(
        m.player_of_member(1).unwrap().ai,
        "applied once the prompt finished"
    );
    m.member_back(1);
    assert!(!m.player_of_member(1).unwrap().ai);
    let log = m.events_since(0);
    assert!(log
        .iter()
        .any(|e| e.r#type == "ai" && e.msg.key().starts_with("log.dropped")));
    assert!(log
        .iter()
        .any(|e| e.r#type == "ai" && e.msg.key() == "log.reconnected"));
}

#[test]
fn save_and_restore_continue_identically() {
    let d = data();
    let members: Vec<RoomMember> = (1..=4).map(|i| member(i, true)).collect();
    let mut a = new_match(&members, 77, MatchMode::Casual);
    // Save at several points, including mid-prompt and mid-wait.
    for checkpoint in [10, 137, 401, 900] {
        while (a.state().seq as usize) < checkpoint && !a.ended() {
            a.tick(0.25);
        }
        let json = a.save();
        let mut b = Match::restore(d.clone(), Arc::new(StubRules), &json).unwrap();
        assert_eq!(
            a.state(),
            b.state(),
            "restored state differs at {checkpoint}"
        );
        for _ in 0..200 {
            a.tick(0.25);
            b.tick(0.25);
        }
        assert_eq!(
            a.state(),
            b.state(),
            "diverged after restore at {checkpoint}"
        );
        assert_eq!(a.save(), b.save());
    }
    assert!(Match::restore(d, Arc::new(StubRules), "{\"version\":0}").is_err());
}
