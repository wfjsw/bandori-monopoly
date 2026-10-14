//! Card modules running inside a real match -- the fixture-only half
//! (`TEST:*` from `rules/fixtures/test-cards`). The shipped-card half lives
//! in `rules/tests/tests/live_match.rs`.
//!
//! These tests are the end-to-end proof that the wasm seam works: a card
//! played from hand runs its module, its prompts come out as engine prompts,
//! and its effects land in the committed match state.
//!
//! Build the modules first: `node tools/build-ruleset.mjs`.

use std::path::Path;
use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::Match;
use game_core::msg::Arg;
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::stage;
use game_core::MatchMode;
use game_rules::{Ruleset, WasmRules};

fn data() -> Arc<GameData> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("../../data");
    let roster = game_core::data::Roster::from_fixture(
        &std::fs::read_to_string(root.join("../game-core/tests/fixtures/roster.json")).expect("roster fixture"),
    )
    .expect("roster fixture parses");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()), roster)
            .unwrap(),
    )
}


/// The shipped cards plus the test-only fixture cards (`TEST:*`), one ruleset.
fn rules_with_fixtures() -> WasmRules {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist");
    let mut b = Ruleset::builder();
    for sub in ["cards", "fixtures"] {
        let dir = root.join(sub);
        let index: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("index.json")).expect("run tools/build-ruleset.mjs"),
        )
        .unwrap();
        for m in index["modules"].as_array().unwrap() {
            b.add(&std::fs::read(dir.join(m["file"].as_str().unwrap())).unwrap())
                .unwrap();
        }
    }
    WasmRules::new(b.build().unwrap(), data())
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
fn a_card_whose_crystals_run_out_leaves_the_field() {
    // TEST:crystal places itself with one crystal and spends it in the same
    // effect. The 「no crystals -> discard」 check is its `crystalsChanged`
    // handler, not a re-check at the spend site.
    let mut m = match_with_rules("TEST:crystal", rules_with_fixtures());
    m.act(1, &play("TEST:crystal")).expect("play should run");
    let st = m.state();
    let me = st
        .players
        .iter()
        .find(|p| p.member == 1)
        .expect("the human player");
    assert!(
        me.field.iter().all(|f| f.card != "TEST:crystal"),
        "the card left the field: {:?}",
        me.field
    );
    assert!(
        me.discard.iter().any(|c| c == "TEST:crystal"),
        "it went to the discard pile: {:?}",
        me.discard
    );
    // The spend emptied it, so the removal spoke -- not the earlier write that
    // took the count up, which the guard must let pass.
    assert_eq!(
        fixture_events(&st, "crystal_empty").len(),
        1,
        "the spend that emptied the card spoke"
    );
    assert!(
        fixture_events(&st, "crystal_none").is_empty(),
        "the earlier write must not speak for the count the run ended on"
    );
}

#[test]
fn an_immediate_dest_move_lands_before_the_rest_of_the_run() {
    // TEST:dest_now places itself and `send_to_dest`s straight out, then logs
    // `is_placed`. `set_dest` would still have it on the field there.
    let mut m = match_with_rules("TEST:dest_now", rules_with_fixtures());
    m.act(1, &play("TEST:dest_now")).expect("play");
    let st = m.state();
    let left: Vec<i64> = fixture_events(&st, "dest_now_left")
        .iter()
        .map(|e| int_arg(e, "placed"))
        .collect();
    assert_eq!(left, vec![0], "the card was gone before its effect ended");
    let me = st.players.iter().find(|p| p.member == 1).expect("human");
    assert!(
        me.discard.iter().any(|c| c == "TEST:dest_now"),
        "and it landed in the discard: {:?}",
        me.discard
    );
}

#[test]
fn a_transferred_dest_lands_in_the_named_players_pile() {
    // TEST:dest_to names the *other* player's discard -- 「将此卡放入[使用者]
    // 弃卡区」 when the user is not the holder.
    let mut m = match_with_rules("TEST:dest_to", rules_with_fixtures());
    m.act(1, &play("TEST:dest_to")).expect("play");
    let st = m.state();
    let me = st.players.iter().find(|p| p.member == 1).expect("human");
    let user = st.players.iter().find(|p| p.member == 2).expect("the user");
    assert!(
        me.discard.iter().all(|c| c != "TEST:dest_to"),
        "not the holder's discard: {:?}",
        me.discard
    );
    assert!(
        user.discard.iter().any(|c| c == "TEST:dest_to"),
        "the user's discard: {:?}",
        user.discard
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

/// Logged events with this fixture key, newest last.
fn fixture_events<'a>(
    st: &'a game_core::state::MatchState,
    key: &str,
) -> Vec<&'a game_core::msg::Msg> {
    let full = format!("cards:fixture-test-cards.{key}");
    st.events
        .iter()
        .filter_map(|e| {
            let m = unadorned(&e.msg);
            (m.key() == full).then_some(m)
        })
        .collect()
}

fn int_arg(msg: &game_core::msg::Msg, name: &str) -> i64 {
    match msg.a.get(name) {
        Some(Arg::I(v)) | Some(Arg::N(v)) => *v,
        other => panic!("{name} is not an integer arg: {other:?}"),
    }
}

#[test]
fn a_played_cards_own_counteract_runs_once() {
    // `cardAfter` / `cardPlayed` also name the card on `t.card`; only the play
    // itself may run the card's own follow-up.
    let mut m = match_with_rules("TEST:echo", rules_with_fixtures());
    m.act(1, &play("TEST:echo")).expect("play");
    let st = m.state();
    assert_eq!(fixture_events(&st, "echo_play").len(), 1, "play ran once");
    assert_eq!(
        fixture_events(&st, "echo_counteract").len(),
        1,
        "own counteract ran exactly once"
    );
}

#[test]
fn cards_in_lists_the_hand_and_take_card_moves_one() {
    let mut m = match_with_rules("TEST:lister", rules_with_fixtures());
    // The played card has left the hand before its effect runs.
    let expected = m.hand_of(1).len() as i64 - 1;
    m.act(1, &play("TEST:lister")).expect("play");
    let st = m.state();
    let counted = fixture_events(&st, "lister_count");
    assert_eq!(counted.len(), 1);
    assert_eq!(int_arg(counted[0], "n"), expected, "cards_in(Hand) length");
    assert_eq!(int_arg(counted[0], "size"), expected, "hand_size agrees");
    let moved = fixture_events(&st, "lister_moved");
    assert_eq!(moved.len(), 1, "a hand card was moved");
    assert_eq!(
        int_arg(moved[0], "found"),
        1,
        "it shows up in cards_in(Discard)"
    );
    assert_eq!(m.hand_of(1).len() as i64, expected - 1, "and left the hand");
}

/// The other player of the two-player test match.
fn other_player(m: &Match) -> usize {
    m.state()
        .players
        .iter()
        .position(|s| s.member != 1)
        .expect("a second player")
}

#[test]
fn a_card_stun_passes_the_abnormal_gate() {
    let mut m = match_with_rules("TEST:stunner", rules_with_fixtures());
    let other = other_player(&m);
    let before = m.state().players[other].stun();
    m.act(1, &play("TEST:stunner")).expect("play");
    let st = m.state();
    assert_eq!(
        st.players[other].stun(),
        before + 1,
        "the stun went through the gate"
    );
    let done = fixture_events(&st, "stunner_done");
    assert_eq!(done.len(), 1, "the effect resumed after the gate");
    assert_eq!(
        int_arg(done[0], "count"),
        1,
        "it counts as an abnormal effect this turn"
    );
}

#[test]
fn a_card_can_shape_a_move_and_run_it_now() {
    let mut m = match_with_rules("TEST:mover", rules_with_fixtures());
    let before = m.state().players[1].pos;
    m.act(1, &play("TEST:mover")).expect("play the mover");
    let st = m.state();
    assert!(fixture_events(&st, "mover_planned").len() == 1, "planned");
    let done = fixture_events(&st, "mover_done");
    assert_eq!(
        done.len(),
        1,
        "the effect resumed after the move: {:?}",
        fixture_events(&st, "mover_done")
    );
    assert_eq!(
        int_arg(done[0], "pos"),
        st.players[1].pos as i64,
        "the log reports where it landed"
    );
    assert_ne!(
        st.players[1].pos, before,
        "the player actually moved 3 tiles"
    );
}

#[test]
fn a_card_targets_another_player_and_counts_it() {
    let mut m = match_with_rules("TEST:aimer", rules_with_fixtures());
    let other = other_player(&m);
    m.act(1, &play("TEST:aimer")).expect("play the aimer");
    let st = m.state();
    let done = fixture_events(&st, "aimer_done");
    assert_eq!(done.len(), 1);
    assert_eq!(
        int_arg(done[0], "got"),
        other as i64,
        "the target went through"
    );
    assert_eq!(int_arg(done[0], "count"), 1, "the _targeted counter moved");
}

#[test]
fn an_immune_player_is_named_but_the_effect_lands_as_nothing() {
    // `ImmuneAll` is a **resolution** gate: the effect names its recipient, the
    // chain forms, and only then does the immunity void what lands. So the
    // player *is* 「成为目标」 -- the declaration reached them -- while the
    // effect settles to nothing. `Untargetable` is the declaration gate and is
    // the one that stops a player being named at all.
    let mut m = match_with_rules("TEST:shield", rules_with_fixtures());
    m.give_cards(1, &["TEST:aimer", "TEST:stunner"]);
    let other = other_player(&m);
    m.act(1, &play("TEST:shield")).expect("place the shield");
    let before = m.state().players[other].stun();
    m.act(1, &play("TEST:aimer")).expect("play the aimer");
    m.act(1, &play("TEST:stunner")).expect("play the stun");
    let st = m.state();
    let done = fixture_events(&st, "aimer_done");
    assert_eq!(
        int_arg(done[0], "got"),
        -1,
        "ImmuneAll fails the targeting at resolution"
    );
    assert_eq!(
        int_arg(done[0], "count"),
        1,
        "but the player was named, so the designation counts"
    );
    assert_eq!(
        st.players[other].stun(),
        before,
        "ImmuneAll blocks the abnormal effect"
    );
    assert_eq!(
        fixture_events(&st, "shield_held").len(),
        2,
        "asked once for the target, once for the stun"
    );
}

#[test]
fn a_counter_negates_the_effect_declaration_before_it_settles() {
    // The chain: the effect declaration is L1, the counter pushes onto it as L2
    // and resolves **before** L1. `set_cancelled` negates L1's activation, so
    // the effect never settles at all -- as against `negate_effect`, which
    // would let a listener see the effect and only void what lands.
    //
    // Both players are human: a [反击] window is never offered to a bot.
    let members = vec![member(1, false), member(2, false)];
    let mut m = Match::new(
        data(),
        Arc::new(rules_with_fixtures()),
        &members,
        20261004,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    for _ in 0..2000 {
        let st = m.state();
        if st.phase == "play"
            && st.turn >= 0
            && st.step == stage::OPS
            && st
                .players
                .get(st.turn as usize)
                .is_some_and(|s| s.member == 1)
        {
            break;
        }
        m.tick(0.25);
    }
    let (actor, other) = (1, 2);
    m.give_cards(actor, &["TEST:aimer"]);
    m.give_cards(other, &["TEST:counter"]);
    m.act(actor, &play("TEST:aimer")).expect("play the aimer");
    // The counteraction window is offered to the named recipient; play the counter.
    let st = m.state();
    assert_eq!(
        st.prompt.kind, "choice",
        "a [反击] window should be pending: {:?}",
        st.prompt
    );
    let prompt = st.prompt.id;
    m.act(
        other,
        &NetMessage {
            prompt,
            value: 0,
            ..NetMessage::act("answer")
        },
    )
    .expect("play the counter");
    let st = m.state();
    let done = fixture_events(&st, "aimer_done");
    assert_eq!(done.len(), 1);
    assert_eq!(
        int_arg(done[0], "got"),
        -1,
        "the counter negated the declaration, so the targeting never landed"
    );
    assert_eq!(
        fixture_events(&st, "counter_fired").len(),
        1,
        "the counter ran"
    );
}

#[test]
fn a_field_guard_blocks_an_abnormal_effect() {
    let mut m = match_with_rules("TEST:guard", rules_with_fixtures());
    m.give_cards(1, &["TEST:stunner"]);
    let other = other_player(&m);
    m.act(1, &play("TEST:guard")).expect("place the guard");
    let before = m.state().players[other].stun();
    m.act(1, &play("TEST:stunner")).expect("play the stun");
    let st = m.state();
    assert_eq!(
        st.players[other].stun(),
        before,
        "the guard blocked the stun"
    );
    assert_eq!(
        fixture_events(&st, "guard_blocked").len(),
        1,
        "the guard's hook ran once"
    );
    assert_eq!(
        int_arg(fixture_events(&st, "stunner_done")[0], "count"),
        0,
        "a blocked effect does not count"
    );
}
