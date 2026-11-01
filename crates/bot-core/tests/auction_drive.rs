//! A real leftover auction, driven the way the page drives a Solo seat.
//!
//! Slot in a Solo auction is not an answer: a seat that bid stays pending
//! (`answers[k] < 0`) and owes a fresh decision every time somebody outbids
//! it; only a pass marks the slot. A page driver that records a bid as "this
//! prompt is answered" therefore leaves every bot silent while the engine
//! still counts it open -- and a Solo auction has no deadline, so the table
//! parks on the prompt forever.
//!
//! This test reproduces the engine side of that (`botDrive.ts`
//! `needsAnswer` / `recordAnswer` ported verbatim) and requires the auction to
//! run to a close: every seat either raises or passes, then the deed sells.

use std::collections::HashMap;
use std::sync::Arc;

use bot_core::determinize::DeterminizerRng;
use bot_core::policy::act_for_view;
use bot_core::SeatView;
use game_core::data::{GameData, Roster};
use game_core::engine::{CardRules, Match, StubRules};
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::{stage, BotMentality, MatchState};
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

/// A 3-seat Solo match with a bankruptcy leftover auction already open on the
/// frame: seat 0 owns mortgaged deeds, lands on a housed tile it cannot pay
/// for, and goes out. Seat 0 has no roll to take part with.
fn leftover_auction(d: &Arc<GameData>) -> (Arc<dyn CardRules>, Match, i32) {
    let rules: Arc<dyn CardRules> = Arc::new(StubRules);
    let members: Vec<RoomMember> = (1..=3)
        .map(|i| RoomMember {
            id: i,
            player: format!("P{i}"),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        })
        .collect();
    for seed in 1..400u64 {
        let mut m = Match::new(
            d.clone(),
            rules.clone(),
            &members,
            seed,
            MatchMode::Solo,
            ScoreWeights::default(),
        );
        m.quick_start();
        let actor = m.state().players[0].member;
        {
            let w = m.world_mut();
            w.st.phase = "play".into();
            w.st.turn = 0;
            w.st.roller = 0;
            w.st.step = stage::OPS;
            w.st.players[0].pos = 0;
            w.st.players[0].money = 0;
            // Everything the dice can reach is player 1's and housed.
            for t in 1..=12usize {
                if d.tiles[t].is_buyable() {
                    w.st.owners[t] = 1;
                    w.st.houses[t] = 3;
                }
            }
            // The bankrupt seat's own deeds -- mortgaged, so it has nothing to
            // raise against and goes straight out. These are what is auctioned.
            for t in 20..d.tiles.len() {
                if d.tiles[t].is_buyable() && w.st.owners[t] < 0 {
                    w.st.owners[t] = 0;
                    w.st.mortgaged[t] = true;
                }
            }
        }
        if m.act(actor, &NetMessage::act("roll")).is_err() {
            continue;
        }
        for _ in 0..400 {
            if m.state().prompt.id > 0 {
                break;
            }
            m.tick(0.05);
        }
        if m.state().prompt.kind != "auction" {
            continue;
        }
        let bk = m.state().player_of(actor);
        return (rules, m, bk);
    }
    panic!("no leftover auction reached");
}

/// `botDrive.ts::needsAnswer`, auction branch: re-arm until the seat answers,
/// and after that only when the live bid moved past the one it left. A pass
/// (slot answered) is final.
fn needs(answered: &HashMap<i32, (i32, i32, i32, i32)>, member: i32, st: &MatchState) -> bool {
    let me = st.player_of(member);
    let Some(k) = st.prompt.players.iter().position(|&p| p == me) else {
        return false;
    };
    if st.prompt.answers[k] >= 0 {
        return false;
    }
    match answered.get(&member) {
        None => true,
        Some(&(turn, step, id, _)) if (turn, step, id) != (st.turn, st.step, st.prompt.id) => true,
        Some(&(_, _, _, bid)) => bid != st.prompt.bid,
    }
}

#[test]
fn a_driven_auction_raises_and_passes_until_it_closes() {
    let d = data();
    let (rules, mut m, bankrupt) = leftover_auction(&d);
    let bidders: Vec<i32> = m
        .state()
        .players
        .iter()
        .filter(|p| p.member != m.state().players[bankrupt as usize].member && !p.out())
        .map(|p| p.member)
        .collect();
    let mut answered: HashMap<i32, (i32, i32, i32, i32)> = HashMap::new();
    let mut asks = 0usize;
    for _ in 0..800 {
        let st = m.state();
        if st.prompt.kind != "auction" || st.prompt.id == 0 {
            break;
        }
        let mut acted = false;
        for &member in &bidders {
            if !needs(&answered, member, &st) {
                continue;
            }
            let view = SeatView::from_match(&m, member);
            let mut rng = DeterminizerRng::new(asks as u64 + 1);
            let Some(cmd) = act_for_view(&d, &rules, &view, &mut rng) else {
                continue;
            };
            assert_eq!(cmd.act, "answer", "a bidder answers the auction");
            acted = true;
            asks += 1;
            // The page records the bid the answer leaves live (`answerFor`).
            let bid = if cmd.value < 0 {
                -1
            } else {
                cmd.value / 100 * 100
            };
            match m.act(member, &cmd) {
                Ok(()) => {
                    answered.insert(member, (st.turn, st.step, st.prompt.id, bid));
                }
                // A refusal re-arms (`recordAnswer`); a pass already on the
                // slot is the one refusal that keeps the record.
                Err(e) => {
                    if e.key() != "err.auction_passed" {
                        answered.remove(&member);
                    }
                }
            }
        }
        assert!(acted, "the prompt stayed open with nobody answering");
    }
    eprintln!("asks={asks}");
    let st = m.state();
    assert_eq!(st.prompt.kind, "", "the auction closed");
    assert!(asks > 0, "the seats actually answered");
    // Exactly one seat holds the deed; the other answered and is done.
    let owners = m
        .state()
        .owners
        .iter()
        .filter(|&&o| o >= 0 && o != bankrupt)
        .count();
    assert!(owners >= 1, "the leftover deed found a buyer");
    assert!(
        asks <= 800,
        "the drive terminates instead of spinning ({asks} asks)"
    );
}
