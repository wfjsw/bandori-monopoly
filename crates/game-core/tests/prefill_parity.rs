//! The engine's remaining policy copy -- the prompt **prefill** in
//! `engine/ai.rs` -- must agree with `bot-core`'s (`docs/BOT-DRIVE.md` §3 step
//! 4). The prefill is not a decision anybody schedules any more, but a seat
//! whose window runs out is forced onto it, and the search ranks with the
//! `aiAnswer` hint; a drift would make a seat answer differently from its own
//! driver, silently.
//!
//! Only the deterministic entries are compared here. Everything the two copies
//! derive from the RNG (the chaos non-counteract pick, the counteraction pass
//! roll) draws from a different stream position by construction, so they can
//! be pinned by formula alone -- they share the one `chaos_pick` /
//! `counteract_pick` body shape, and the constants are asserted in
//! `mentality.rs`.

use std::sync::Arc;

use game_core::engine::{Match, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;
use game_core::MatchMode;

mod common;
use common::data_arc as data;

fn new_match(mentality: BotMentality, seed: u64) -> Match {
    let members: Vec<RoomMember> = (1..=3)
        .map(|i| RoomMember {
            id: i,
            player: format!("P{i}"),
            bot: true,
            mentality,
            ..Default::default()
        })
        .collect();
    let mut m = Match::new(
        data(),
        Arc::new(StubRules),
        &members,
        seed,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    m
}

/// Money the buy / build / force-buy reserves are tested around, and every
/// tile on the board.
const MONEY: [i32; 9] = [0, 500, 1_000, 1_500, 2_000, 2_500, 3_500, 10_000, 100_000];

/// Both copies must give the same buy / build / force-buy answer for every
/// (seat, money, tile) combination, owned and unowned.
#[test]
fn purchase_prefills_agree() {
    for mentality in [BotMentality::Standard, BotMentality::Chaos] {
        let mut m = new_match(mentality, 7);
        let n = m.world().st.owners.len();
        for money in MONEY {
            for t in 0..n {
                for owned in [false, true] {
                    {
                        let w = m.world_mut();
                        w.st.players[0].money = money;
                        w.st.players[1].money = money;
                        w.st.owners[t] = if owned { 0 } else { -1 };
                    }
                    let cx = m.bot_cx();
                    assert_eq!(
                        bot_core::policy::ai_wants_buy(&cx, 0, t),
                        cx.ai_wants_buy(0, t),
                        "buy: {mentality:?} money {money} tile {t} owned {owned}"
                    );
                    assert_eq!(
                        bot_core::policy::ai_wants_build(&cx, 0, t),
                        cx.ai_wants_build(0, t),
                        "build: {mentality:?} money {money} tile {t} owned {owned}"
                    );
                    for price in [0, 500, 1_000, 3_000] {
                        assert_eq!(
                            bot_core::policy::ai_wants_force_buy(&cx, 0, price),
                            cx.ai_wants_force_buy(0, price),
                            "force buy: {mentality:?} money {money} price {price}"
                        );
                    }
                    // The mentality predicates the two copies branch on.
                    for seat in 0..3 {
                        assert_eq!(
                            bot_core::policy::bot_mentality(&cx, seat),
                            cx.bot_mentality(seat)
                        );
                        assert_eq!(bot_core::policy::is_chaos(&cx, seat), cx.is_chaos(seat));
                    }
                }
            }
        }
    }
}

/// A mentality on a **human** seat reaches both copies (托管 混沌 tags the
/// seat; the driver and the forced timeout fill must agree).
#[test]
fn a_human_seat_reaches_both_copies() {
    let members = [
        RoomMember {
            id: 1,
            player: "P1".into(),
            mentality: BotMentality::Chaos,
            ..Default::default()
        },
        RoomMember {
            id: 2,
            player: "P2".into(),
            bot: true,
            ..Default::default()
        },
    ];
    let mut m = Match::new(
        data(),
        Arc::new(StubRules),
        &members,
        11,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    // Seats are in rolled order, so find the human by member.
    let seat = m
        .state()
        .players
        .iter()
        .position(|p| p.member == 1)
        .unwrap();
    let cx = m.bot_cx();
    assert!(cx.is_chaos(seat) && bot_core::policy::is_chaos(&cx, seat));
    assert_eq!(
        cx.bot_mentality(seat),
        bot_core::policy::bot_mentality(&cx, seat)
    );
}
