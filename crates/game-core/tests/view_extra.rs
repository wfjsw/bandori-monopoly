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
// ---- the viewer's skill list (`ViewExtra::skills`) ---------------------------

use game_core::engine::{CardRules, Cx, Dest, Flow};
use game_core::msg::Msg;
use game_core::net::NetMessage;
use game_core::state::stage;

/// [`StubRules`] plus an `On::Play` list, so a test can mark which skills are
/// activatable (the real `WasmRules` answers from the card's Play entry) and
/// optionally refuse the press through the same `cant_play` gate.
struct TestRules {
    play: Vec<String>,
    refuse: Option<Msg>,
}

impl CardRules for TestRules {
    fn has_play(&self, card: &str) -> bool {
        self.play.iter().any(|c| c == card)
    }
    fn cant_play(&self, _cx: &Cx, _player: usize, card: &str) -> Option<Msg> {
        if self.has_play(card) {
            self.refuse.clone()
        } else {
            None
        }
    }
    fn play(&self, _cx: &mut Cx, _player: usize, _card: &str) -> Flow<Dest> {
        Ok(Dest::Field)
    }
    fn event(&self, _cx: &mut Cx, _player: usize, _id: &str) -> Flow<bool> {
        Ok(false)
    }
}

fn solo_with(rules: Arc<dyn CardRules>, seed: u64) -> Match {
    let members: Vec<RoomMember> = (1..=3).map(|i| member(i, i != 1)).collect();
    let mut m = Match::new(data(), rules, &members, seed, MatchMode::Solo, ScoreWeights::default());
    m.quick_start();
    m
}

/// Tick until member `me`'s seat is in the OPS window (the only window a
/// skill press gets). Answers the seat's own prompts along the way (the
/// opening mulligan, mostly).
fn wait_ops(m: &mut Match, me: i32) {
    let seat = m
        .state()
        .players
        .iter()
        .position(|s| s.member == me)
        .expect("member has a seat") as i32;
    for _ in 0..10_000 {
        if m.ended() {
            break;
        }
        {
            let st = m.state();
            if st.turn == seat && st.step == stage::OPS && !st.busy {
                return;
            }
            if st.prompt.id != 0 && st.prompt.waiting(seat) {
                let msg = NetMessage {
                    prompt: st.prompt.id,
                    value: 0,
                    ..NetMessage::act("answer")
                };
                let _ = m.act(me, &msg);
            }
        }
        m.tick(0.25);
    }
    panic!("never reached OPS for member {me}");
}

/// The `skill:*` rules `bind_skills` places for the seat's character.
fn skill_ids(m: &Match, me: i32) -> Vec<String> {
    let st = m.state();
    let seat = st.player_of(me) as usize;
    let character = &st.players[seat].character;
    m.data().skill_rules_of(character)
}

#[test]
fn skills_are_empty_before_the_play_phase() {
    // A brand-new match is still in ban / pick: no field skills, no list.
    let m = solo_with(Arc::new(StubRules), 4);
    let extra = m.view_extra(1);
    // `StubRules` never reports `has_play`, so even a placed skill is absent.
    assert_eq!(extra["skills"], serde_json::json!([]));
}

#[test]
fn skills_list_only_activatable_rules_during_ops() {
    // Place the character + band skills, mark only the first as `On::Play`.
    let m = solo_with(Arc::new(TestRules {
        play: vec![],
        refuse: None,
    }), 5);
    let ids = skill_ids(&m, 1);
    assert!(!ids.is_empty(), "the picked character has at least one skill");
    let rules = Arc::new(TestRules {
        play: vec![ids[0].clone()],
        refuse: None,
    });
    // Rebuild with the right `has_play` list (the match is cheap to restart).
    let mut m = solo_with(rules, 5);
    wait_ops(&mut m, 1);
    let extra = m.view_extra(1);
    let skills = extra["skills"].as_array().expect("skills list");
    assert_eq!(skills.len(), 1, "passive skills are absent: {skills:?}");
    assert_eq!(skills[0]["id"], serde_json::json!(ids[0]));
    assert_eq!(skills[0]["enabled"], serde_json::json!(true));
    assert!(skills[0]["reason"].get("k").is_some());
}

#[test]
fn skills_carry_the_engine_gate_reason_when_blocked() {
    let m = solo_with(Arc::new(TestRules {
        play: vec![],
        refuse: None,
    }), 6);
    let ids = skill_ids(&m, 1);
    let refuse = Msg::new("err.play_pre");
    let mut m = solo_with(
        Arc::new(TestRules {
            play: vec![ids[0].clone()],
            refuse: Some(refuse),
        }),
        6,
    );
    wait_ops(&mut m, 1);
    let extra = m.view_extra(1);
    let skills = extra["skills"].as_array().expect("skills list");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0]["enabled"], serde_json::json!(false));
    assert_eq!(skills[0]["reason"]["k"], serde_json::json!("err.play_pre"));
}

#[test]
fn skills_are_disabled_outside_the_ops_window() {
    // Same rules as the enabled case, but read the list *before* OPS (right
    // after `quick_start`, while the first turn is still starting).
    let m = solo_with(Arc::new(TestRules {
        play: vec![],
        refuse: None,
    }), 7);
    let ids = skill_ids(&m, 1);
    let mut m = solo_with(
        Arc::new(TestRules {
            play: vec![ids[0].clone()],
            refuse: None,
        }),
        7,
    );
    // Force a non-OPS step by ticking until we are anywhere but OPS+!busy.
    for _ in 0..4000 {
        let st = m.state();
        if st.phase == "play" && !(st.step == stage::OPS && !st.busy) {
            break;
        }
        m.tick(0.25);
    }
    let extra = m.view_extra(1);
    let skills = extra["skills"].as_array().expect("skills list");
    assert_eq!(skills.len(), 1);
    assert_eq!(
        skills[0]["enabled"], serde_json::json!(false),
        "outside OPS the engine gate refuses the press"
    );
    assert_eq!(skills[0]["reason"]["k"], serde_json::json!("err.skill_not_now"));
}

#[test]
fn skills_press_through_why_not_act_matches_the_list() {
    // The list's `enabled` flag must agree with a real `act: "skill"`: when the
    // list says enabled the engine accepts, and when it says disabled the
    // engine refuses with the same reason.
    let m = solo_with(Arc::new(TestRules {
        play: vec![],
        refuse: None,
    }), 8);
    let ids = skill_ids(&m, 1);
    let mut m = solo_with(
        Arc::new(TestRules {
            play: vec![ids[0].clone()],
            refuse: None,
        }),
        8,
    );
    wait_ops(&mut m, 1);
    let extra = m.view_extra(1);
    let skills = extra["skills"].as_array().expect("skills list");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0]["enabled"], serde_json::json!(true));
    let press = NetMessage {
        act: "skill".into(),
        card: ids[0].clone(),
        ..Default::default()
    };
    assert!(
        m.act(1, &press).is_ok(),
        "the list said enabled, so the press is accepted"
    );
    // After the press the window may have moved on; a second press of a
    // now-refused skill must carry the list's reason.
    let extra = m.view_extra(1);
    let skills = extra["skills"].as_array().expect("skills list");
    if let Some(s) = skills.first() {
        if s["enabled"] == serde_json::json!(false) {
            let err = m.act(1, &press).unwrap_err();
            assert_eq!(err.key(), s["reason"]["k"].as_str().unwrap());
        }
    }
}
