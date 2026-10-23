//! Action abstraction must never offer a command the engine would refuse
//! (`docs/BOT.md` §3.4; `bot_cpu` §6's forced-buy refusal loop).
//!
//! Buy / build are no longer END-step turn acts. They happen only by answering
//! a settle Ask (`ctx::offer_buy` / `ctx::offer_build` → `Action::Offer` →
//! `act:"answer"`). These tests pin that split: the END surface is decline /
//! end only, and settle buy / build prompts remain on the prompt-answer path.

use std::sync::Arc;

use bot_core::action::{self, Action};
use game_core::data::{GameData, Roster};
use game_core::engine::{Match, StubRules};
use game_core::msg::Msg;
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{stage, MatchPrompt, MatchState};
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    // bot-core must not grow a hard dep on the card crates, so the roster
    // comes from the generated roster fixture (see `Roster::from_fixture`).
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

/// A first property tile the data table actually has.
fn some_property(data: &GameData) -> usize {
    data.tiles
        .iter()
        .position(|t| t.is_buyable())
        .expect("the data table has a buyable tile")
}

/// A 结束 surface: `seat` stands on `tile`, with `money` in hand. The old
/// `buy_price` / `can_buy_here` / `land_acted` gates are gone -- END never
/// offers buy / build.
fn end_surface(data: &GameData, tile: usize, money: i32) -> (MatchState, Vec<String>) {
    let mut st = MatchState::default();
    st.phase = "play".into();
    st.turn = 0;
    st.step = stage::END;
    st.landed = tile as i32;
    st.owners = vec![-1; data.tiles.len()];
    st.houses = vec![0; data.tiles.len()];
    st.mortgaged = vec![false; data.tiles.len()];
    // The engine's end gate (why_not_act's end branch): a player under the
    // hand limit at 结束 may end the turn.
    st.can_end_here = true;
    st.can_roll_here = false;
    st.players = vec![{
        let mut p = game_core::state::MatchPlayer::default();
        p.member = 1;
        p.money = money;
        p.pos = tile as i32;
        p
    }];
    (st, Vec::new())
}

/// A settle buy Ask (the `ctx::offer_buy` prompt shape): yes / no, waiting on
/// seat 1, quoted price on the ask.
fn settle_buy_prompt(tile: usize, price: i32) -> MatchPrompt {
    let mut p = MatchPrompt::default();
    p.id = 11;
    p.kind = "choice".into();
    p.title = Msg::new("ask.buy.title");
    p.text = Msg::new("ask.buy.text").n("price", price);
    p.options = vec![Msg::new("ask.buy.yes"), Msg::new("ask.no_buy")];
    p.fallback = 1;
    p.players = vec![1];
    p.answers = vec![-1];
    p.tile = tile as i32;
    p.price = price;
    p
}

/// A settle build Ask (the `ctx::offer_build` prompt shape).
fn settle_build_prompt(tile: usize, cost: i32) -> MatchPrompt {
    let mut p = MatchPrompt::default();
    p.id = 12;
    p.kind = "choice".into();
    p.title = Msg::new("ask.build.title");
    p.text = Msg::new("ask.build.text").n("cost", cost);
    p.options = vec![Msg::new("ask.build.yes"), Msg::new("ask.build.no")];
    p.fallback = 1;
    p.players = vec![1];
    p.answers = vec![-1];
    p.tile = tile as i32;
    p
}

fn state_with_prompt(p: MatchPrompt) -> MatchState {
    let mut st = MatchState::default();
    st.phase = "play".into();
    st.players.resize(2, Default::default());
    st.prompt = p;
    st
}

fn kinds(acts: &[Action]) -> Vec<String> {
    acts.iter()
        .map(|a| match a {
            Action::Decline => "decline".into(),
            Action::Play { .. } => "play".into(),
            other => format!("{other:?}"),
        })
        .collect()
}

/// No END-surface action may serialize to the retired `buy` / `build` acts.
fn assert_no_buy_build_act(acts: &[Action], st: &MatchState, seat: usize) {
    for a in acts {
        let msg = action::to_net_message(a, st, seat);
        assert_ne!(msg.act, "buy", "END must not emit act:buy: {a:?}");
        assert_ne!(msg.act, "build", "END must not emit act:build: {a:?}");
    }
}

#[test]
fn end_surface_offers_decline_not_buy_or_build() {
    // The END duplicate path is gone. Even standing on a rich unowned deed,
    // the only turn action is end -- buy is a settle Ask.
    let data = data();
    let tile = some_property(&data);
    let (st, hand) = end_surface(&data, tile, 10_000);
    let acts = action::legal_actions(&data, &st, &hand, &[], 0);
    assert_eq!(
        kinds(&acts),
        vec!["decline".to_string()],
        "END is decline only: {:?}",
        kinds(&acts)
    );
    assert_no_buy_build_act(&acts, &st, 0);
}

#[test]
fn end_surface_on_owned_tile_is_still_only_decline() {
    let data = data();
    let tile = some_property(&data);
    let (mut st, hand) = end_surface(&data, tile, 10_000);
    st.owners[tile] = 0;
    let acts = action::legal_actions(&data, &st, &hand, &[], 0);
    assert_eq!(
        kinds(&acts),
        vec!["decline".to_string()],
        "owned tile does not reopen a build act: {:?}",
        kinds(&acts)
    );
    assert_no_buy_build_act(&acts, &st, 0);
}

#[test]
fn end_surface_poor_seat_still_only_decline() {
    // The old unaffordable-Buy refusal loop is gone with the act: there is
    // nothing to refuse when the menu never offers a buy.
    let data = data();
    let tile = some_property(&data);
    let (st, hand) = end_surface(&data, tile, 0);
    let acts = action::legal_actions(&data, &st, &hand, &[], 0);
    assert!(
        acts.contains(&Action::Decline),
        "decline must be on the menu: {:?}",
        kinds(&acts)
    );
    assert_eq!(kinds(&acts), vec!["decline".to_string()]);
    assert_no_buy_build_act(&acts, &st, 0);
}

#[test]
fn settle_buy_prompt_is_an_offer_answer_not_a_buy_act() {
    // The settle Ask (`ctx::offer_buy`) is the ONE buy path: both options are
    // `Action::Offer`, and they serialize to `act:"answer"`.
    let data = data();
    let tile = some_property(&data);
    let price = data.tiles.get(tile).map(|t| t.price).unwrap_or(0);
    let st = state_with_prompt(settle_buy_prompt(tile, price));
    let acts = action::legal_actions(&data, &st, &[], &[], 1);
    assert_eq!(
        acts,
        vec![Action::Offer { index: 0 }, Action::Offer { index: 1 }],
        "buy Ask is yes/no offers: {:?}",
        acts
    );
    for (i, a) in acts.iter().enumerate() {
        let msg = action::to_net_message(a, &st, 1);
        assert_eq!(msg.act, "answer", "settle buy answers the prompt: {msg:?}");
        assert_eq!(msg.prompt, 11);
        assert_eq!(msg.value, i as i32);
    }
}

#[test]
fn settle_build_prompt_is_an_offer_answer_not_a_build_act() {
    let data = data();
    let tile = some_property(&data);
    let cost = data.tiles.get(tile).map(|t| t.house).unwrap_or(0);
    let st = state_with_prompt(settle_build_prompt(tile, cost));
    let acts = action::legal_actions(&data, &st, &[], &[], 1);
    assert_eq!(
        acts,
        vec![Action::Offer { index: 0 }, Action::Offer { index: 1 }],
        "build Ask is yes/no offers: {:?}",
        acts
    );
    for (i, a) in acts.iter().enumerate() {
        let msg = action::to_net_message(a, &st, 1);
        assert_eq!(
            msg.act, "answer",
            "settle build answers the prompt: {msg:?}"
        );
        assert_eq!(msg.prompt, 12);
        assert_eq!(msg.value, i as i32);
    }
}

#[test]
fn settle_buy_is_always_optional_so_no_forced_mortgage_to_buy() {
    // The old "legal Buy is forced" C1 menu is gone with the END act. A settle
    // buy Ask always carries the no-buy fallback, so the bot can decline
    // instead of mortgaging to fund a land buy (user ruling 2026-10-08). The
    // search sees both options and `has_stakes` keeps the root searchable.
    let data = data();
    let tile = some_property(&data);
    let price = data.tiles.get(tile).map(|t| t.price).unwrap_or(0);
    let st = state_with_prompt(settle_buy_prompt(tile, price));
    let acts = action::legal_actions(&data, &st, &[], &[], 1);
    assert!(
        acts.contains(&Action::Offer { index: 1 }),
        "the no-buy fallback must remain on the menu: {:?}",
        acts
    );
    let trivial = action::trivial_decision(&acts);
    assert!(
        trivial.is_none(),
        "a buy Ask is a staked decision the search must keep: {trivial:?}"
    );
}

#[test]
fn a_real_match_at_end_never_offers_buy_or_build() {
    // End-to-end wiring: a broke seat standing on unowned land at 结束 gets
    // only the end/decline surface. `Match::state` no longer fills buy gates.
    let data = data();
    let members = vec![
        RoomMember {
            id: 1,
            player: "P".into(),
            bot: true,
            ..Default::default()
        },
        RoomMember {
            id: 2,
            player: "Q".into(),
            bot: true,
            ..Default::default()
        },
    ];
    let mut m = Match::new(
        data.clone(),
        Arc::new(StubRules),
        &members,
        3,
        MatchMode::Solo,
        ScoreWeights::default(),
    );
    m.quick_start();
    let tile = some_property(&data);
    {
        let w = m.world_mut();
        let me = 0;
        w.st.players[me].money = 0;
        w.st.players[me].pos = tile as i32;
        w.st.turn = me as i32;
        w.st.step = stage::END;
        w.st.landed = tile as i32;
        w.st.busy = false;
        w.st.owners = vec![-1; data.tiles.len()];
        w.st.owners[tile] = -1;
        w.st.houses = vec![0; data.tiles.len()];
        w.st.mortgaged = vec![false; data.tiles.len()];
    }
    let st = m.state();
    assert_eq!(st.step, stage::END);
    let hand = m.hand_of(1);
    let playable = vec![false; hand.len()];
    let acts = action::legal_actions(&data, &st, &hand, &playable, 0);
    assert!(
        acts.contains(&Action::Decline),
        "decline must be on the menu: {:?}",
        kinds(&acts)
    );
    assert_no_buy_build_act(&acts, &st, 0);
    assert_eq!(
        kinds(&acts),
        vec!["decline".to_string()],
        "END is decline only even on unowned land: {:?}",
        kinds(&acts)
    );
    // The heuristic agrees: it ends the turn instead of proposing a buy.
    let view = bot_core::SeatView::from_match(&m, 1);
    let msg = bot_core::heuristic_message_view(&data, &view);
    assert_eq!(msg.act, "end", "the heuristic ends at 结束: {msg:?}");
}
