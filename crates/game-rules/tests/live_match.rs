//! Card modules running inside a real match.
//!
//! These tests are the end-to-end proof that the wasm seam works: a card played
//! from hand runs its module, its prompts come out as engine prompts, and its
//! effects land in the committed match state.
//!
//! Build the modules first: `tools/build-ruleset.sh`.

use std::path::Path;
use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::Match;
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;
use game_rules::WasmRules;

fn data() -> Arc<GameData> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string())).unwrap())
}

fn rules() -> WasmRules {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/cards");
    WasmRules::load_dir(data(), &dir)
        .expect("ruleset should load")
        .expect("dist/cards must exist -- run tools/build-ruleset.sh")
}

fn member(id: i32, bot: bool) -> RoomMember {
    RoomMember { id, player: format!("P{id}"), bot, ..Default::default() }
}

fn play(card: &str) -> NetMessage {
    NetMessage { card: card.to_string(), ..NetMessage::act("play") }
}

/// A two-seat match where seat 1 is human and holds `card` on its own turn.
fn match_with(card: &str) -> Match {
    let members = vec![member(1, false), member(2, true)];
    let mut m = Match::new(data(), Arc::new(rules()), &members, 20261004, MatchMode::Casual, ScoreWeights::default());
    m.quick_start();
    // Let the bots take their turns until the human is in the operations phase.
    for _ in 0..2000 {
        let st = m.state();
        let mine = st.phase == "play" && st.turn >= 0 && st.seats.get(st.turn as usize).is_some_and(|s| s.member == 1);
        if mine && st.step == 1 {
            break;
        }
        m.tick(0.25);
    }
    let st = m.state();
    assert_eq!(st.phase, "play", "match should reach play");
    assert!(st.seats.get(st.turn as usize).is_some_and(|s| s.member == 1), "should be the human's turn");
    m.give_cards(1, &[card]);
    m
}

#[test]
fn the_sample_cards_load_in_the_ruleset() {
    let r = rules();
    assert!(r.ruleset().card("AG:Y.O.L.O").is_some());
    assert!(r.ruleset().card("HHW:（育美）").is_some());
}

#[test]
fn a_played_card_runs_its_module_and_answers_a_prompt() {
    // HHW:（育美） rolls 4d20 twice, offers the resulting tiles, places a mark.
    let mut m = match_with("HHW:（育美）");
    let before = m.hand_of(1).len();
    m.act(1, &play("HHW:（育美）")).expect("play should start the effect");

    // The module blocked on its tile prompt; answer with the first option.
    let st = m.state();
    assert_eq!(st.prompt.kind, "tile", "the module's prompt is an engine tile prompt: {:?}", st.prompt);
    assert!(!st.prompt.options.is_empty());
    let prompt = st.prompt.id;
    m.act(1, &NetMessage { prompt, value: 0, ..NetMessage::act("answer") }).expect("answer");

    let st = m.state();
    assert!(
        st.events.iter().any(|e| e.msg.key() == "cards:card-hhw.hagumi_marks_placed"),
        "mark message logged: {:?}",
        st.events.iter().map(|e| e.msg.key()).collect::<Vec<_>>()
    );
    assert!(
        st.marks.iter().any(|mk| mk.note.key() == "cards:card-hhw.hagumi_marks_mark_note"),
        "a mark was placed: {:?}",
        st.marks
    );
    assert_eq!(m.hand_of(1).len(), before - 1, "the card left the hand: {:?}", m.hand_of(1));
}

#[test]
fn an_unported_card_falls_back_gracefully() {
    let mut m = match_with("AG:Y.O.L.O");
    let before = m.hand_of(1).len();
    // Y.O.L.O is a reaction card with no Play effect; playing it is legal but
    // does nothing beyond leaving the hand.
    m.act(1, &play("AG:Y.O.L.O")).expect("play");
    assert_eq!(m.hand_of(1).len(), before - 1, "the card left the hand: {:?}", m.hand_of(1));
}
#[test]
fn ported_official_cards_run_in_a_match() {
    // R:[衍生] 压 -- straight gain, no prompt.
    let mut m = match_with("R:[衍生] 压");
    let before = m.state().seats[1].money;
    m.act(1, &play("R:[衍生] 压")).expect("play");
    let st = m.state();
    assert!(st.seats[1].money >= before + 1000, "the card paid: {} -> {}", before, st.seats[1].money);
    assert!(st.events.iter().any(|e| format!("{:?}", e.msg).contains("press_why")), "its reason was logged");

    // R:（ykn）louder -- nudge the RiNG multiplier, no prompt. It is 凑友希那's
    // exclusive card, so the seat must be her to play it.
    let mut m = match_with("R:（ykn）louder");
    m.set_character(1, "凑友希那");
    m.act(1, &play("R:（ykn）louder")).expect("play");
    let st = m.state();
    assert!(st.events.iter().any(|e| format!("{:?}", e.msg).contains("louder")), "logged: {:?}", st.events.iter().map(|e| e.msg.key()).collect::<Vec<_>>());
}
