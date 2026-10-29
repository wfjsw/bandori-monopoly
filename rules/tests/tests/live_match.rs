//! Card modules running inside a real match, exercised through **shipped**
//! cards and character/band skills -- the game-specific half of `live_match`
//! (the fixture-only half is `crates/game-rules/tests/live_match.rs`).
//!
//! These tests are the end-to-end proof that the wasm seam works on real
//! content: a card played from hand runs its module, its prompts come out as
//! engine prompts, and its effects land in the committed match state.
//!
//! Build the modules first: `node tools/build-ruleset.mjs`.

use std::path::Path;
use std::sync::Arc;

use game_core::data::{GameData, Roster};
use game_core::engine::Match;
use game_core::msg::Arg;
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::stage;
use game_core::MatchMode;
use game_rules::{Ruleset, WasmRules};

fn data() -> Arc<GameData> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let roster = Roster::from_fixture(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/roster.json"),
        )
        .expect("roster fixture"),
    )
    .expect("roster fixture parses");
    Arc::new(
        GameData::load(
            |f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()),
            roster,
        )
        .unwrap(),
    )
}

fn rules() -> WasmRules {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/cards");
    WasmRules::load_dir(data(), &dir)
        .expect("ruleset should load")
        .expect("dist/cards must exist -- run tools/build-ruleset.sh")
}

fn member(id: i32, bot: bool) -> RoomMember {
    RoomMember {
        id,
        player: format!("P{id}"),
        bot,
        ..Default::default()
    }
}

fn play(card: &str) -> NetMessage {
    NetMessage {
        card: card.to_string(),
        ..NetMessage::act("play")
    }
}

/// Answer the opening-hand re-roll if it is up. It is a barrier with no expiry
/// (rulebook 时点流程 step 6) -- ticking alone would stall on a human seat, so
/// every seat answers with the fallback, exactly as an idle player would.
fn cross_mulligan(m: &mut Match) {
    let st = m.state();
    if st.prompt.kind != "mulligan" {
        return;
    }
    let members: Vec<i32> = st.players.iter().map(|p| p.member).collect();
    let (id, fallback) = (st.prompt.id, st.prompt.fallback);
    for member in members {
        let _ = m.act(
            member,
            &NetMessage {
                prompt: id,
                value: fallback,
                ..NetMessage::act("answer")
            },
        );
    }
}

/// A two-player match where player 1 is human and holds `card` on its own turn.
fn match_with(card: &str) -> Match {
    match_with_rules(card, rules())
}

/// [`match_with`] for an **exclusive** card: pin the seat-1 persona so the
/// play is not at the mercy of `quick_start`'s assignment (the event deck is
/// shuffled at setup, so the stream is not something to rely on).
fn match_with_exclusive(card: &str, persona: &str) -> Match {
    let members = vec![member(1, false), member(2, true)];
    let mut m = Match::new(
        data(),
        Arc::new(rules()),
        &members,
        20261004,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.set_persona(1, persona);
    m.quick_start();
    // Same wait as `match_with_rules`: the human must be in 运营 before a play.
    for _ in 0..2000 {
        cross_mulligan(&mut m);
        let st = m.state();
        let mine = st.phase == "play"
            && st.turn >= 0
            && st
                .players
                .get(st.turn as usize)
                .is_some_and(|s| s.member == 1);
        if mine && st.step == stage::OPS {
            break;
        }
        m.tick(0.25);
        // The engine decides for nobody; the bots are this loop's to
        // drive (`docs/BOT-DRIVE.md`).
        bot_core::sim::pump_bots(&mut m);
    }
    let st = m.state();
    assert_eq!(st.phase, "play", "match should reach play");
    assert!(
        st.players
            .get(st.turn as usize)
            .is_some_and(|s| s.member == 1),
        "should be the human's turn"
    );
    m.give_cards(1, &[card]);
    m
}

fn match_with_rules(card: &str, rules: WasmRules) -> Match {
    let members = vec![member(1, false), member(2, true)];
    let mut m = Match::new(
        data(),
        Arc::new(rules),
        &members,
        20261004,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    // Let the bots take their turns until the human is in the operations phase.
    for _ in 0..2000 {
        cross_mulligan(&mut m);
        let st = m.state();
        let mine = st.phase == "play"
            && st.turn >= 0
            && st
                .players
                .get(st.turn as usize)
                .is_some_and(|s| s.member == 1);
        if mine && st.step == stage::OPS {
            break;
        }
        m.tick(0.25);
        // The engine decides for nobody; the bots are this loop's to
        // drive (`docs/BOT-DRIVE.md`).
        bot_core::sim::pump_bots(&mut m);
    }
    let st = m.state();
    assert_eq!(st.phase, "play", "match should reach play");
    assert!(
        st.players
            .get(st.turn as usize)
            .is_some_and(|s| s.member == 1),
        "should be the human's turn"
    );
    m.give_cards(1, &[card]);
    m
}

#[test]
fn a_characters_skill_binds_to_whoever_picked_them() {
    // The binding: a player's two skill rules follow from the character they
    // picked and land on their field, which is what makes `On::Action` reach them
    // exactly as it reaches any other field card. See `game_core::data::skill_id`.
    let members = vec![member(1, false), member(2, true)];
    let mut m = Match::new(
        data(),
        Arc::new(rules()),
        &members,
        20261004,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.set_persona(1, "户山香澄");
    m.quick_start();
    // The binding is observable the moment the match starts. Assert it *here*:
    // `PPP:Returns` is in a Poppin' Party deck pool and its `DeckAtGameStart`
    // hook places it, after which its (2) legitimately swaps the band skill for
    // another player's -- so the field at the first turn is no longer the
    // binding, it is the binding *after* Returns has had its say.
    {
        let st = m.state();
        let ids: Vec<&str> = st.players[1]
            .field
            .iter()
            .map(|f| f.card.as_str())
            .collect();
        assert!(
            ids.contains(&"skill:户山香澄:非凡之星"),
            "character skill bound: {ids:?}"
        );
        assert!(
            ids.iter().any(|id| id.starts_with("skill:Poppin' Party:")),
            "band skill follows from the character's group: {ids:?}"
        );
    }
    // Let the bots run so a `TurnStartBefore` has fired for the human.
    for _ in 0..2000 {
        cross_mulligan(&mut m);
        let st = m.state();
        let mine = st.phase == "play"
            && st.turn >= 0
            && st
                .players
                .get(st.turn as usize)
                .is_some_and(|s| s.member == 1);
        if mine && st.step == stage::OPS {
            break;
        }
        m.tick(0.25);
        // The engine decides for nobody; the bots are this loop's to
        // drive (`docs/BOT-DRIVE.md`).
        bot_core::sim::pump_bots(&mut m);
    }
    let st = m.state();
    let ids: Vec<&str> = st.players[1]
        .field
        .iter()
        .map(|f| f.card.as_str())
        .collect();
    // The character skill survives whatever else the deck did; the band skill
    // may have been swapped by `PPP:Returns`' (2), which is that card's rule.
    assert!(
        ids.contains(&"skill:户山香澄:非凡之星"),
        "character skill stays bound: {ids:?}"
    );
    // 「初始0，上限1」 -- the skill card is what assigns fire.max.
    // Fire is a held token of the persona-skill instance (MARKERS.md §7).
    assert_eq!(
        st.players[1].token_max("charge"),
        1,
        "the skill assigned the cap"
    );
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
    let mut m = match_with_exclusive("HHW:（育美）", "北泽育美");
    let before = m.hand_of(1).len();
    m.act(1, &play("HHW:（育美）"))
        .expect("play should start the effect");

    // The module blocked on its roll-option prompt; answer with the first option.
    let st = m.state();
    assert_eq!(
        st.prompt.kind, "choice",
        "the module's prompt offers one option per roll: {:?}",
        st.prompt
    );
    assert!(!st.prompt.options.is_empty());
    let prompt = st.prompt.id;
    m.act(
        1,
        &NetMessage {
            prompt,
            value: 0,
            ..NetMessage::act("answer")
        },
    )
    .expect("answer");

    let st = m.state();
    assert!(
        st.events
            .iter()
            .any(|e| unadorned(&e.msg).key() == "cards:card-hhw.hagumi_marks_placed"),
        "mark message logged: {:?}",
        st.events.iter().map(|e| e.msg.key()).collect::<Vec<_>>()
    );
    assert!(
        st.marks
            .iter()
            .any(|mk| mk.note.key() == "cards:card-hhw.hagumi_marks_mark_note"),
        "a mark was placed: {:?}",
        st.marks
    );
    assert_eq!(
        m.hand_of(1).len(),
        before - 1,
        "the card left the hand: {:?}",
        m.hand_of(1)
    );
}

#[test]
fn an_unported_card_falls_back_gracefully() {
    let mut m = match_with("AG:Y.O.L.O");
    let before = m.hand_of(1).len();
    // Y.O.L.O is a counteraction (「你的任意掷骰结算前打出此卡」) with no
    // `On::Play`. A hand `play` is therefore refused (`err.play_timing`) and
    // the card stays in hand -- a graceful refusal, not a crash. (Same
    // correction as `inter_yolo_pushes_haneoka_over_20`: do not `play` a
    // counteract-only card.)
    let r = m.act(1, &play("AG:Y.O.L.O"));
    assert!(
        r.is_err(),
        "counteract-only cards are not hand plays: {r:?}"
    );
    assert_eq!(
        m.hand_of(1).len(),
        before,
        "the card stayed in hand: {:?}",
        m.hand_of(1)
    );
}

#[test]
fn ported_official_cards_run_in_a_match() {
    // R:[衍生] 压 -- straight gain, no prompt.
    let mut m = match_with("R:[衍生] 压");
    let before = m.state().players[1].money;
    m.act(1, &play("R:[衍生] 压")).expect("play");
    let st = m.state();
    assert!(
        st.players[1].money >= before + 1000,
        "the card paid: {} -> {}",
        before,
        st.players[1].money
    );
    assert!(
        st.events
            .iter()
            .any(|e| format!("{:?}", e.msg).contains("press_why")),
        "its reason was logged"
    );

    // R:（ykn）louder -- nudge the RiNG multiplier, no prompt. It is 凑友希那's
    // exclusive card, so the player must be her to play it.
    let mut m = match_with_exclusive("R:（ykn）louder", "凑友希那");
    m.set_persona(1, "凑友希那");
    m.act(1, &play("R:（ykn）louder")).expect("play");
    let st = m.state();
    assert!(
        st.events
            .iter()
            .any(|e| format!("{:?}", e.msg).contains("louder")),
        "logged: {:?}",
        st.events.iter().map(|e| e.msg.key()).collect::<Vec<_>>()
    );
}

/// The outcome line a log event speaks as: itself, or the `what` inside an
/// in-play card's attribution (「<卡名> 的效果：<what>」, `log.card_effect`).
fn unadorned(msg: &game_core::msg::Msg) -> &game_core::msg::Msg {
    if msg.key() == "log.card_effect" {
        if let Some(Arg::Msg(m)) = msg.a.get("what") {
            return m;
        }
    }
    msg
}
