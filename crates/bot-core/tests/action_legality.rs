//! Action abstraction must never offer a command the engine would refuse
//! (`docs/BOT.md` §3.4; `bot_cpu` §6's forced-buy refusal loop).
//!
//! At 结束 the search used to have no "decline" and `buyable` had no funds /
//! eligibility gate, so an unaffordable Buy was proposed, refused
//! (`err.buy_poor` / `err.cannot_buy`), and replayed from the service cache
//! every 200 ms until the turn bank expired. These tests pin the fix: the
//! decline is always on the menu where the engine accepts `end`, and Buy /
//! Build ride the engine's own `can_buy_here` / `can_build_here` flags.

use std::sync::Arc;

use bot_core::action::{self, Action};
use game_core::data::GameData;
use game_core::engine::{Match, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{stage, MatchState};
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string())).unwrap(),
    )
}

/// A first property tile the data table actually has.
fn some_property(data: &GameData) -> usize {
    data.tiles
        .iter()
        .position(|t| matches!(t.kind.as_str(), "property" | "ring"))
        .expect("the data table has a buyable tile")
}

/// A 结束 surface: `seat` stands on `tile`, unowned, with `money` in hand.
fn end_surface(data: &GameData, tile: usize, money: i32, can_buy_here: bool) -> (MatchState, Vec<String>) {
    let mut st = MatchState::default();
    st.phase = "play".into();
    st.turn = 0;
    st.step = stage::END;
    st.landed = tile as i32;
    st.owners = vec![-1; data.tiles.len()];
    st.houses = vec![0; data.tiles.len()];
    st.mortgaged = vec![false; data.tiles.len()];
    st.buy_price = data.tiles.get(tile).map(|t| t.price).unwrap_or(0);
    st.can_buy_here = can_buy_here;
    st.can_build_here = false;
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

fn kinds(acts: &[Action]) -> Vec<String> {
    acts.iter()
        .map(|a| match a {
            Action::Buy { .. } => "buy".into(),
            Action::Build { .. } => "build".into(),
            Action::Decline => "decline".into(),
            Action::Play { .. } => "play".into(),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
fn unaffordable_tile_offers_decline_not_buy() {
    let data = data();
    let tile = some_property(&data);
    // The engine's gate said no (`can_buy_here == false`: err.buy_poor), and
    // the money is far under the price.
    let (st, hand) = end_surface(&data, tile, 0, false);
    let acts = action::legal_actions(&data, &st, &hand, &[], 0);
    assert!(
        acts.contains(&Action::Decline),
        "decline must be on the menu: {:?}",
        kinds(&acts)
    );
    assert!(
        !acts.iter().any(|a| matches!(a, Action::Buy { .. })),
        "an unaffordable Buy must not be offered: {:?}",
        kinds(&acts)
    );
}

#[test]
fn affordable_tile_offers_buy_forced() {
    // The C1 bisect (`docs/BOT.md` §5 C1): a Decline next to a *legal* Buy
    // costs ~5 wins (the eval is buy-neutral at the instant, so the tie goes
    // to the bias and the bot passes on buys the old policy took). A paid
    // option the engine accepts stays forced; Decline covers the empty menu.
    let data = data();
    let tile = some_property(&data);
    let (st, hand) = end_surface(&data, tile, 10_000, true);
    let acts = action::legal_actions(&data, &st, &hand, &[], 0);
    let ks = kinds(&acts);
    assert!(ks.contains(&"buy".to_string()), "{ks:?}");
    assert_eq!(
        ks,
        vec!["buy".to_string()],
        "a legal buy is forced -- no decline beside it: {ks:?}"
    );
}

#[test]
fn build_is_gated_on_the_engine_flag_too() {
    let data = data();
    let tile = some_property(&data);
    let (mut st, hand) = end_surface(&data, tile, 10_000, true);
    // Own the tile, but the engine refused the build (err.build_denied /
    // err.poor): `can_build_here` is false.
    st.owners[tile] = 0;
    st.can_buy_here = false;
    st.can_build_here = false;
    st.build_cost = 100;
    let acts = action::legal_actions(&data, &st, &hand, &[], 0);
    assert!(
        !acts.iter().any(|a| matches!(a, Action::Build { .. })),
        "an illegal Build must not be offered: {:?}",
        kinds(&acts)
    );
    assert!(acts.contains(&Action::Decline), "{:?}", kinds(&acts));
}

#[test]
fn a_real_match_computes_the_gate_and_the_bot_passes_when_poor() {
    // End-to-end wiring: `Match::state` fills `can_buy_here` from the engine's
    // own `why_not_act` predicates, and the action list follows it.
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
        w.st.bought = false;
        w.st.built = false;
        w.st.busy = false;
        w.st.owners = vec![-1; data.tiles.len()];
        w.st.owners[tile] = -1;
        w.st.houses = vec![0; data.tiles.len()];
        w.st.mortgaged = vec![false; data.tiles.len()];
        w.turn.plan.can_build = true;
        w.turn.plan.no_buy = false;
    }
    let st = m.state();
    assert_eq!(st.step, stage::END);
    assert!(
        !st.can_buy_here,
        "a broke player must not be offered a buy (buy_price {})",
        st.buy_price
    );
    let hand = m.hand_of(1);
    let playable = vec![false; hand.len()];
    let acts = action::legal_actions(&data, &st, &hand, &playable, 0);
    assert!(
        acts.contains(&Action::Decline),
        "decline must be on the menu: {:?}",
        kinds(&acts)
    );
    assert!(
        !acts.iter().any(|a| matches!(a, Action::Buy { .. })),
        "poor seat must not be offered Buy: {:?}",
        kinds(&acts)
    );
    // The heuristic agrees: it ends the turn instead of proposing a refused buy.
    let view = bot_core::SeatView::from_match(&m, 1);
    let msg = bot_core::heuristic_message_view(&data, &view);
    assert_eq!(msg.act, "end", "the heuristic passes: {msg:?}");
}