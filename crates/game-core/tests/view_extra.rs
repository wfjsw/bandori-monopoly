//! `Match::view_extra` -- the per-viewer fields the browser's 托管 autopilot
//! reads on top of the shared `MatchState`: the engine's own AI answer for the
//! viewer's own prompt, and which hand cards `cant_play` would allow.

use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{Match, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string())).unwrap(),
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

/// A solo match: member 1 is the human, the rest are bots. `quick_start` skips
/// ban/pick/deck so the match is in `play`.
fn solo(seed: u64) -> Match {
    let members: Vec<RoomMember> = (1..=3)
        .map(|i| member(i, i != 1))
        .collect();
    let mut m = Match::new(
        data(),
        Arc::new(StubRules),
        &members,
        seed,
        MatchMode::Solo,
        ScoreWeights::default(),
    );
    m.quick_start();
    m
}

#[test]
fn view_extra_is_empty_for_an_unknown_member() {
    let m = solo(1);
    let extra = m.view_extra(99);
    assert_eq!(extra["aiAnswer"], serde_json::Value::Null);
    assert_eq!(extra["playable"], serde_json::json!([]));
}

#[test]
fn view_extra_reports_the_viewers_own_hand_playability() {
    let m = solo(2);
    let hand = m.hand_of(1);
    let extra = m.view_extra(1);
    let playable = extra["playable"].as_array().expect("playable list");
    assert_eq!(playable.len(), hand.len(), "one flag per hand card");
    // The opening hand is dealt before the first turn, so no card is playable
    // yet -- the OPS / your-turn gate in `cant_play` refuses them all.
    assert!(playable.iter().all(|v| v == &serde_json::json!(false)));
}

#[test]
fn view_extra_carries_the_ai_answer_for_the_asked_player_only() {
    // A bot-only match runs prompts of its own; drive one until the engine
    // raises a live prompt, then check the shape of the extra.
    let members: Vec<RoomMember> = (1..=3).map(|i| member(i, true)).collect();
    let mut m = Match::new(
        data(),
        Arc::new(StubRules),
        &members,
        3,
        MatchMode::Solo,
        ScoreWeights::default(),
    );
    m.quick_start();
    let mut saw_prompt = false;
    for _ in 0..4000 {
        m.tick(0.25);
        let st = m.state();
        if st.prompt.id > 0 {
            saw_prompt = true;
            // `aiAnswer` is only for the asked player's own member; a seat that
            // is not asked must see `null` (the auction ceiling is hidden).
            for (seat, p) in st.players.iter().enumerate() {
                let extra = m.view_extra(p.member);
                let waiting = st.prompt.waiting(seat as i32);
                if waiting {
                    let ai = &extra["aiAnswer"];
                    assert!(ai.is_object(), "asked player gets an aiAnswer");
                    assert!(ai["answer"].is_number(), "answer is a number");
                    assert!(ai["picked"].is_array(), "picked is a list");
                    assert!(ai["worth"].is_number(), "worth is a number");
                } else {
                    assert_eq!(
                        extra["aiAnswer"],
                        serde_json::Value::Null,
                        "a player not waiting on the prompt sees no aiAnswer"
                    );
                }
            }
        }
        if m.ended() {
            break;
        }
    }
    assert!(saw_prompt, "the bot game should raise at least one prompt");
}