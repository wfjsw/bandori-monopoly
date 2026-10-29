//! `policy::act_for_view` answers the viewer's own prompt off their view alone.
//!
//! A rich [pass-reward] view takes the card option, a poor one the money print
//! (`docs/BOT-DRIVE.md`: the policy is the client's, determinize then decide).
//! The fixture is the same StubRules match `view_extra_equiv.rs` builds; the
//! prompt is stamped onto the view because StubRules has no start-square rule.

use std::sync::Arc;

use bot_core::determinize::DeterminizerRng;
use bot_core::policy::act_for_view;
use bot_core::SeatView;
use game_core::data::{GameData, Roster};
use game_core::engine::{CardRules, Match, StubRules};
use game_core::msg::Msg;
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{BotMentality, MatchPrompt};
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let roster = Roster::from_fixture(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../game-core/tests/fixtures/roster.json"),
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

/// A two-bot match with `money` on the viewer and a [pass-reward] prompt open
/// on its view.
fn poor_or_rich(money: i32) -> (Arc<GameData>, Arc<dyn CardRules>, SeatView) {
    let data = data();
    let rules: Arc<dyn CardRules> = Arc::new(StubRules);
    let members: Vec<RoomMember> = (1..=2)
        .map(|i| RoomMember {
            id: i,
            player: format!("Bot{i}"),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        })
        .collect();
    let mut m = Match::new(
        data.clone(),
        rules.clone(),
        &members,
        7,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    let member = m.state().players[0].member;
    let mut view = SeatView::from_match(&m, member);
    view.ai_answer = None;
    view.state.players[view.player_id as usize].money = money;
    view.state.prompt = MatchPrompt {
        id: 9,
        kind: "choice".into(),
        title: Msg::new("ask.pass_reward.title"),
        options: vec![
            Msg::new("ask.pass_reward.money"),
            Msg::new("ask.pass_reward.card"),
        ],
        fallback: 0,
        players: vec![view.player_id],
        answers: vec![-1],
        ..MatchPrompt::default()
    };
    (data, rules, view)
}

fn pick(money: i32) -> i32 {
    let (data, rules, view) = poor_or_rich(money);
    let mut rng = DeterminizerRng::new(11);
    let cmd = act_for_view(&data, &rules, &view, &mut rng).expect("an answer from the view");
    assert_eq!(cmd.act, "answer");
    cmd.value
}

#[test]
fn a_rich_view_takes_the_card() {
    assert_eq!(pick(100_000), 1, "above the cap the card option wins");
}

#[test]
fn a_poor_view_takes_the_money() {
    assert_eq!(pick(4_000), 0, "below the ramp the money option wins");
}
