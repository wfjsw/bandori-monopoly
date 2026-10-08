//! Console mutations must survive saves and deterministic replay, and be
//! impossible in online matches even when a client bypasses the UI.
use std::sync::Arc;

use game_core::{
    data::GameData,
    engine::{Match, StubRules},
    net::{NetMessage, RoomMember},
    record::{EngineStamp, RecordedMatch, Replayer, RECORD_VERSION},
    scoring::ScoreWeights,
    MatchMode,
};

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .unwrap(),
    )
}

fn game(mode: MatchMode) -> Match {
    let mut m = Match::new(
        data(),
        Arc::new(StubRules),
        &[
            RoomMember {
                id: 1,
                player: "Human".into(),
                ..Default::default()
            },
            RoomMember {
                id: 2,
                player: "Bot".into(),
                bot: true,
                ..Default::default()
            },
        ],
        42,
        mode,
        ScoreWeights::default(),
    );
    m.quick_start();
    m
}

fn cheat(op: &str, value: i32, target: i32) -> NetMessage {
    NetMessage {
        act: "debug".into(),
        debug: op.into(),
        value,
        target,
        ..Default::default()
    }
}

fn ready_game() -> Match {
    let mut m = game(MatchMode::Solo);
    for _ in 0..100 {
        let st = m.state();
        if st.prompt.id == 0 && !st.busy {
            return m;
        }
        for (&seat, &answer) in st.prompt.players.iter().zip(&st.prompt.answers) {
            if answer < 0 {
                m.act(
                    st.players[seat as usize].member,
                    &NetMessage {
                        act: "answer".into(),
                        prompt: st.prompt.id,
                        value: st.prompt.fallback,
                        ..Default::default()
                    },
                )
                .unwrap();
            }
        }
        m.tick(0.25);
    }
    panic!("opening prompt did not finish");
}

#[test]
fn cheats_update_views_and_survive_save_restore() {
    let mut m = ready_game();
    let human = m.state().player_of(1) as usize;
    let bot = m.state().player_of(2) as usize;
    m.world_mut().st.players[human].state_set_bounds("fire", 0, 5);
    m.act(1, &cheat("money", 50_000, -1)).unwrap();
    m.act(1, &cheat("tp", 10, bot as i32)).unwrap();
    let card = data().cards[0].id.clone();
    let before = m.hand_of(1).len();
    let mut give = cheat("give", 2, human as i32);
    give.card = card.clone();
    m.act(1, &give).unwrap();
    assert_eq!(m.hand_of(1).len(), before + 2);
    assert_eq!(
        m.hand_of(1).iter().rev().take(2).collect::<Vec<_>>(),
        vec![&card, &card]
    );
    let mut state = cheat("state", 1_000, human as i32);
    state.character = "fire".into();
    m.act(1, &state).unwrap();
    let st = m.state();
    assert!(st.debug_open);
    assert_eq!(st.players[human].money, 50_000);
    assert_eq!(st.players[bot].pos, 10);
    let cap = st.players[human].state_max("fire");
    assert_eq!(
        st.players[human].state_get("fire"),
        if cap > 0 { 1_000.min(cap) } else { 1_000 }
    );
    assert!(m.take_changed());
    let restored = Match::restore(data(), Arc::new(StubRules), &m.save()).unwrap();
    assert_eq!(restored.save(), m.save());
    assert_eq!(restored.hand_of(1), m.hand_of(1));
    assert_eq!(restored.state().players[human].money, 50_000);
}

#[test]
fn invalid_commands_and_online_cheats_leave_the_world_unchanged() {
    for mode in [MatchMode::Casual, MatchMode::Ranked] {
        let mut m = game(mode);
        let before = m.save();
        assert_eq!(
            m.act(1, &cheat("money", 1, 0)).unwrap_err().key(),
            "err.debug_solo_only"
        );
        assert_eq!(m.save(), before);
    }
    let mut m = ready_game();
    for cmd in [
        cheat("money", -1, 0),
        cheat("money", 100_000_001, 0),
        cheat("tp", -1, 0),
        cheat("tp", 99_999, 0),
        cheat("draw", 101, 0),
        cheat("give", 1, 0),
        cheat("state", 1, 0),
        cheat("money", 1, -2),
        cheat("money", 1, 50),
        cheat("unknown", 1, 0),
    ] {
        let before = m.save();
        assert!(m.act(1, &cmd).is_err());
        assert_eq!(m.save(), before, "{cmd:?}");
    }
}

#[test]
fn pending_prompts_reject_cheats_without_losing_answers() {
    let mut m = game(MatchMode::Solo);
    for _ in 0..200 {
        if m.state().prompt.id > 0 {
            break;
        }
        m.tick(0.25);
    }
    assert!(m.state().prompt.id > 0);
    let before = m.save();
    assert_eq!(
        m.act(1, &cheat("money", 50_000, 0)).unwrap_err().key(),
        "err.debug_busy"
    );
    assert_eq!(m.save(), before);
}

#[test]
fn cheats_replay_to_the_identical_world_and_event_stream() {
    let mut rm = RecordedMatch::from_snapshot(ready_game());
    rm.act(1, &cheat("money", 50_000, 0)).unwrap();
    rm.act(1, &cheat("tp", 5, 1)).unwrap();
    rm.act(1, &cheat("draw", 2, 0)).unwrap();
    let mut give = cheat("give", 2, 0);
    give.card = data().cards[0].id.clone();
    rm.act(1, &give).unwrap();
    rm.act(1, &cheat("tp", -1, 0)).unwrap_err();
    let file = rm.export(
        EngineStamp {
            format: RECORD_VERSION,
            ..Default::default()
        },
        "test",
    );
    let mut replay = Replayer::new(data(), Arc::new(StubRules), &file, false).unwrap();
    while !replay.status().ended {
        assert!(!replay.step_ticks(8).diverged);
    }
    assert_eq!(replay.match_ref().save(), rm.inner().save());
    assert_eq!(replay.events(), rm.inner().events_since(0));
}
