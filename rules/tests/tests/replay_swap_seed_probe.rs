//! Replay of a full recorded solo game in which the player swaps character
//! with Two-in-One every turn: every live swap must come back on the replay,
//! character marks included.

mod common;
use common::*;

use game_core::engine::SAVE_VERSION;
use game_core::net::{NetMessage, RoomMember};
use game_core::record::{seal, EngineStamp, MatchSetup, RecordedMatch, Replayer, RECORD_VERSION};
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;

fn stamp() -> EngineStamp {
    EngineStamp {
        format: RECORD_VERSION,
        save_version: SAVE_VERSION,
        abi: 119,
        ruleset_sha256: "cf6d9d31fb2ce03baa7797fd84b0d37f3d2cb009db9befcb8bba98782c5c70f3".into(),
        data_sha256: String::new(),
        engine: "game-core".into(),
        build: "0.1.0".into(),
        bundle: "b7111ec9c80d51098ec775be99dc6bffd0cf63a378e734280a8c2dce0bc57be8".into(),
    }
}

fn act(rm: &mut RecordedMatch, member: i32, msg: NetMessage) -> Result<(), String> {
    rm.act(member, &msg).map_err(|e| e.key().to_string())
}

/// Answer every open prompt with each asked seat's fallback.
fn answer_prompts(rm: &mut RecordedMatch) {
    loop {
        let st = rm.inner().state();
        let p = st.prompt;
        if p.id == 0 {
            return;
        }
        let mut any = false;
        for (k, &s) in p.players.iter().enumerate() {
            if p.answers.get(k).copied().unwrap_or(0) >= 0 {
                continue;
            }
            let member = st.players[s as usize].member;
            let _ = act(
                rm,
                member,
                NetMessage {
                    prompt: p.id,
                    value: p.fallback,
                    ..NetMessage::act("answer")
                },
            );
            any = true;
        }
        if !any {
            return;
        }
    }
}

fn settle(rm: &mut RecordedMatch) {
    for _ in 0..4000 {
        answer_prompts(rm);
        if rm.inner().state().prompt.id == 0 {
            return;
        }
        rm.tick_steps(1);
    }
    panic!("settle: prompt never closed");
}

#[test]
fn seed_game_repeats_swap_and_replays() {
    let d = data();
    let p0 = d.persona("纯田真奈").expect("persona");
    let p1 = d.persona("三角初华（Sumimi）").expect("persona");
    let deck0 = game_core::deck::preset(&d, p0);
    let deck1 = game_core::deck::preset(&d, p1);
    assert!(
        deck0.iter().any(|c| c == "Sumimi:Sumimi是二人一体的"),
        "{deck0:?}"
    );
    eprintln!("deck0: {deck0:?}");

    let members = vec![
        RoomMember {
            id: 1,
            player: "P0".into(),
            bot: false,
            ..Default::default()
        },
        RoomMember {
            id: 2,
            player: "P1".into(),
            bot: false,
            ..Default::default()
        },
    ];
    let setup = MatchSetup {
        members,
        seed: SEED,
        seed256: None,
        weights: ScoreWeights::default(),
        ..Default::default()
    };
    let mut rm = RecordedMatch::new(d.clone(), rules(), setup, MatchMode::Solo);

    // Setup: ban, pick, deck -- through the recorded entry point.
    for n in 0..20000 {
        let st = rm.inner().state().clone();
        if st.phase == "play" {
            break;
        }
        if n % 200 == 0 {
            eprintln!(
                "DBG n={n} phase={} turn={} prompt={} ready={:?} personas={:?}",
                st.phase,
                st.turn,
                st.prompt.id,
                st.players.iter().map(|p| p.deck_ready).collect::<Vec<_>>(),
                st.players
                    .iter()
                    .map(|p| p.persona.clone())
                    .collect::<Vec<_>>()
            );
        }
        match st.phase.as_str() {
            "ban" => {
                let turn = st.turn.max(0) as usize;
                let member = st.players[turn].member;
                let _ = act(&mut rm, member, NetMessage::act("ban"));
            }
            "pick" => {
                let turn = st.turn.max(0) as usize;
                let member = st.players[turn].member;
                let want = if member == 1 {
                    "纯田真奈"
                } else {
                    "三角初华（Sumimi）"
                };
                let persona = if st.players[turn].persona.is_empty() {
                    want.to_string()
                } else {
                    String::new()
                };
                let r = act(
                    &mut rm,
                    member,
                    NetMessage {
                        persona: persona.clone(),
                        ..NetMessage::act("pick")
                    },
                );
                if n % 200 == 0 {
                    eprintln!("DBG   pick m{member} {persona:?} -> {r:?}");
                }
            }
            "deck" => {
                for p in st.players.clone() {
                    if p.deck_ready {
                        continue;
                    }
                    let cards = if p.member == 1 {
                        deck0.clone()
                    } else {
                        deck1.clone()
                    };
                    let r = act(
                        &mut rm,
                        p.member,
                        NetMessage {
                            cards,
                            ..NetMessage::act("deck")
                        },
                    );
                    if n % 200 == 0 {
                        eprintln!("DBG   deck m{} -> {r:?}", p.member);
                    }
                }
            }
            _ => {}
        }
        settle(&mut rm);
        rm.tick_steps(1);
    }
    assert_eq!(rm.inner().state().phase, "play", "setup finished");
    settle(&mut rm);
    // The card is handed in through the recorded debug act (the console
    // cheat) on each turn that plays it -- see the loop below.
    settle(&mut rm);

    // Play: p0 plays Two-in-One whenever it holds one, then ends the turn.
    let mut swaps = 0;
    for it in 0..4000 {
        {
            let st = rm.inner().state();
            if it % 250 == 0 {
                eprintln!(
                    "DBG it={it} phase={} turn={} step={} prompt={} hands={:?}",
                    st.phase,
                    st.turn,
                    st.step,
                    st.prompt.id,
                    rm.inner()
                        .world()
                        .hidden
                        .iter()
                        .map(|h| h.hand.len())
                        .collect::<Vec<_>>()
                );
            }
        }
        if rm.inner().state().prompt.id != 0 {
            settle(&mut rm);
            continue;
        }
        let st = rm.inner().state().clone();
        let turn = st.turn.max(0) as i32;
        let member = st.players[turn as usize].member;
        let before = st.players[turn as usize].persona.clone();
        let ops = st.step == game_core::state::stage::OPS;
        let holds = rm
            .inner()
            .world()
            .hidden
            .get(turn as usize)
            .is_some_and(|h| h.hand.iter().any(|c| c == "Sumimi:Sumimi是二人一体的"));
        let no_hand = st.players[turn as usize]
            .state
            .get("noHand")
            .is_some_and(|v| v.value > 0);
        // The 4 debug copies put the seat over the hand limit; the CryChic
        // play gate also refuses at hand >= 6, so trim back under the limit
        // before anything else.
        let limit = st.players[turn as usize].hand_limit().unwrap_or(5);
        let held = rm.inner().world().hidden[turn as usize].hand.len() as i32;
        if ops && member == 1 && held >= limit {
            let hand = rm.inner().world().hidden[turn as usize].hand.clone();
            let card = hand
                .iter()
                .find(|c| *c != "Sumimi:Sumimi是二人一体的")
                .cloned()
                .unwrap_or_else(|| hand[0].clone());
            let r = act(
                &mut rm,
                member,
                NetMessage {
                    card: card.clone(),
                    ..NetMessage::act("discard")
                },
            );
            if it < 30 {
                eprintln!("DBG   discard {card} -> {r:?}");
            }
            continue;
        }
        if ops && member == 1 && !holds && !no_hand {
            let r = act(
                &mut rm,
                member,
                NetMessage {
                    debug: "give".into(),
                    card: "Sumimi:Sumimi是二人一体的".into(),
                    value: 1,
                    target: 0,
                    ..NetMessage::act("debug")
                },
            );
            if it < 30 {
                eprintln!("DBG   give -> {r:?}");
            }
            continue;
        }
        if ops && member == 1 && holds && !no_hand {
            let r = act(
                &mut rm,
                member,
                NetMessage {
                    card: "Sumimi:Sumimi是二人一体的".into(),
                    ..NetMessage::act("play")
                },
            );
            if it < 30 {
                eprintln!("DBG   two-in-one -> {r:?}");
            }
            if let Err(e) = &r {
                eprintln!(
                    "DBG   two-in-one REFUSED: {e} state={:?}",
                    st.players[turn as usize].state
                );
            }
            let after = rm.inner().state().players[turn as usize].persona.clone();
            if after != before {
                swaps += 1;
                eprintln!("DBG live swap #{swaps}: {before} -> {after}");
            }
            settle(&mut rm);
            if swaps >= 6 {
                break;
            }
        }
        if ops {
            let r0 = act(&mut rm, member, NetMessage::act("roll"));
            if it < 20 {
                eprintln!("DBG   roll m{member} -> {r0:?}");
            }
        }
        if ops || st.step == game_core::state::stage::END {
            let r = act(&mut rm, member, NetMessage::act("end"));
            if it < 20 || it % 50 == 0 {
                eprintln!("DBG   end m{member} turn={turn} step={} -> {r:?}", st.step);
            }
        }
        settle(&mut rm);
        rm.tick_steps(1);
    }
    assert!(swaps >= 6, "played six swaps, got {swaps}");

    let live_tokens: Vec<usize> = rm
        .inner()
        .state()
        .players
        .iter()
        .map(|p| p.tokens.len())
        .collect();
    eprintln!("DBG live tokens per seat: {live_tokens:?}");

    let mut rec = rm.export(stamp(), "test");
    seal(&mut rec);
    if let Ok(out) = std::env::var("OUT") {
        std::fs::write(&out, game_core::record::encode_record_zst(&rec)).unwrap();
        eprintln!("wrote {out}");
    }
    // A fresh engine, as the replay worker gets: the live match above has
    // been driven on the shared one.
    let mut rp = Replayer::from_body(data(), fresh_rules(), rec.header.clone(), rec.body.clone())
        .expect("open");
    let total = rp.total_ticks();
    let mut seen = 0;
    while !rp.status().ended && rp.status().tick < total {
        let before = rp.match_ref().state().players[0].persona.clone();
        rp.next_input();
        let after = rp.match_ref().state().players[0].persona.clone();
        if after != before {
            seen += 1;
            eprintln!("DBG replay swap #{seen}: {before} -> {after}");
        }
    }
    eprintln!(
        "DBG diverged={} mismatch={:?}",
        rp.status().diverged,
        rp.first_mismatch()
    );
    let replay_tokens: Vec<usize> = rp
        .match_ref()
        .state()
        .players
        .iter()
        .map(|p| p.tokens.len())
        .collect();
    eprintln!("DBG replay tokens per seat: {replay_tokens:?}");
    assert_eq!(live_tokens, replay_tokens, "player tokens replayed");
    assert_eq!(seen, swaps, "every live swap replayed");
    assert!(!rp.status().diverged);
}
