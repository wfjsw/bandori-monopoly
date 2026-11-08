//! Record a game whose only character swap is a **counteraction** (the user's
//! record path), then replay it: the face must come back on both sides with no
//! checkpoint divergence. The replay runs on a fresh engine in the same
//! process, so a torn-down live match must not leak state into it.

mod common;
use common::*;

use game_core::net::{NetMessage, RoomMember};
use game_core::record::{seal, MatchSetup, RecordedMatch, Replayer};
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;

fn act(rm: &mut RecordedMatch, member: i32, msg: NetMessage) -> Result<(), String> {
    rm.act(member, &msg).map_err(|e| e.key().to_string())
}

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
fn counter_swap_replays() {
    let d = data();
    let deck0 = game_core::deck::preset(&d, d.persona("纯田真奈").expect("persona"));
    let deck1 = game_core::deck::preset(&d, d.persona("三角初华（Sumimi）").expect("persona"));
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
    for _ in 0..20000 {
        let st = rm.inner().state().clone();
        if st.phase == "play" {
            break;
        }
        match st.phase.as_str() {
            "ban" => {
                let member = st.players[st.turn.max(0) as usize].member;
                let _ = act(&mut rm, member, NetMessage::act("ban"));
            }
            "pick" => {
                let member = st.players[st.turn.max(0) as usize].member;
                let persona = if st.players[st.turn.max(0) as usize].persona.is_empty() {
                    if member == 1 {
                        "纯田真奈"
                    } else {
                        "三角初华（Sumimi）"
                    }
                    .to_string()
                } else {
                    String::new()
                };
                let _ = act(
                    &mut rm,
                    member,
                    NetMessage {
                        persona,
                        ..NetMessage::act("pick")
                    },
                );
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
                    let _ = act(
                        &mut rm,
                        p.member,
                        NetMessage {
                            cards,
                            ..NetMessage::act("deck")
                        },
                    );
                }
            }
            _ => {}
        }
        settle(&mut rm);
        rm.tick_steps(1);
    }
    assert_eq!(rm.inner().state().phase, "play", "setup finished");
    settle(&mut rm);

    // Hand the card in, roll, take the counter.
    let mut swapped = 0;
    for _ in 0..600 {
        let st = rm.inner().state().clone();
        if st.prompt.id != 0 {
            settle(&mut rm);
            continue;
        }
        let turn = st.turn.max(0) as usize;
        let member = st.players[turn].member;
        let ops = st.step == game_core::state::stage::OPS;
        if ops && member == 1 {
            let holds = rm
                .inner()
                .world()
                .hidden
                .get(turn)
                .is_some_and(|h| h.hand.iter().any(|c| c == "Sumimi:Sumimi是二人一体的"));
            let no_hand = st.players[turn]
                .state
                .get("noHand")
                .is_some_and(|v| v.value > 0);
            if !holds && !no_hand {
                let _ = act(
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
                continue;
            }
            if holds && !no_hand {
                let before = st.players[turn].persona.clone();
                let _ = act(&mut rm, member, NetMessage::act("roll"));
                let p = rm.inner().state().prompt;
                if p.id != 0 {
                    // Take the counter offer when the window names it.
                    if p.players.iter().any(|&s| s as usize == turn) {
                        let k = p
                            .options
                            .iter()
                            .position(|o| format!("{o:?}").contains("Sumimi是二人一体的"));
                        let _ = act(
                            &mut rm,
                            member,
                            NetMessage {
                                prompt: p.id,
                                value: k.map(|k| k as i32).unwrap_or(p.fallback),
                                ..NetMessage::act("answer")
                            },
                        );
                    }
                    settle(&mut rm);
                }
                let after = rm.inner().state().players[turn].persona.clone();
                if after != before {
                    swapped += 1;
                }
                let _ = act(&mut rm, member, NetMessage::act("end"));
                settle(&mut rm);
                rm.tick_steps(1);
                if swapped >= 1 {
                    break;
                }
            }
        }
        if ops {
            let _ = act(&mut rm, member, NetMessage::act("end"));
        }
        settle(&mut rm);
        rm.tick_steps(1);
    }
    assert!(swapped >= 1, "a counter swap happened live");

    let mut rec = rm.export(dev_stamp(), "test");
    seal(&mut rec);
    if let Ok(out) = std::env::var("DUMP") {
        std::fs::write(out, game_core::record::encode_record_zst(&rec)).unwrap();
    }

    let mut rp = Replayer::from_body(data(), fresh_rules(), rec.header.clone(), rec.body.clone())
        .expect("open");
    let mut seen = 0;
    // Drain the whole log, trailing non-tick inputs included.
    while !rp.status().ended {
        let before = rp.match_ref().state().players[0].persona.clone();
        rp.next_input();
        let after = rp.match_ref().state().players[0].persona.clone();
        // The pick screen fills an empty face; a *swap* replaces a real one.
        if !before.is_empty() && after != before {
            eprintln!("DBG replay swap #{seen} -> {after}");
            seen += 1;
        }
    }
    eprintln!(
        "DBG replay swaps={seen} diverged={} first={:?}",
        rp.status().diverged,
        rp.first_mismatch()
    );
    assert_eq!(seen, swapped, "every live swap replayed");
    assert!(!rp.status().diverged);
}
