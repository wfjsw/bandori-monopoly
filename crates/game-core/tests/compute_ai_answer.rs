//! `Match::compute_ai_answer` must reproduce the prompt-open [`Ask`] fill.
//!
//! The online seat-view engine (`bot-core`'s `view_extras`) has no precomputed
//! fill on its determinized fork, so it recomputes the viewer's `aiAnswer` via
//! [`Match::compute_ai_answer`]. The web client's 托管 then *trusts* that
//! number over its own policy table (`autopilot.ts`'s `botPrompt`).
//!
//! The prompts `offer_buy` / `offer_build` / `offer_force_buy` raise stamp a
//! **deterministic** policy answer with `Ask::with_ai` (`ai_wants_buy` /
//! `ai_wants_build` / the force-buy reserve test) -- not the fallback
//! `Cx::ai_answer_for_entry` would give. A recompute that just returns the
//! prompt's fallback therefore diverges from the live fill on *every* prompt
//! whose policy says "yes", and online 托管 never buys.

use std::sync::Arc;

use game_core::engine::{Match, RawAiAnswer, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;

mod common;
use common::data_arc as data;

fn bots(n: i32) -> Vec<RoomMember> {
    (1..=n)
        .map(|i| RoomMember {
            id: i,
            player: format!("B{i}"),
            bot: true,
            ..Default::default()
        })
        .collect()
}

/// Titles whose `Ask::with_ai` fill is a pure policy test (no RNG draw).
fn deterministic_offer(title: &str) -> bool {
    matches!(
        title,
        "ask.buy.title" | "ask.build.title" | "ask.force_buy.title"
    )
}

fn assert_same(a: &RawAiAnswer, b: &RawAiAnswer, label: &str) {
    assert_eq!(
        (a.answer, &a.picked, a.worth),
        (b.answer, &b.picked, b.worth),
        "{label}: compute_ai_answer diverged from the prompt-open fill"
    );
}

/// For every live deterministic offer, the recomputed answer must equal the
/// fill the live match stamped at prompt-open. This is the invariant the
/// online extras path relies on -- and the one that made 托管 refuse every
/// buy online while solo (which reads the fill) kept buying.
#[test]
fn compute_ai_answer_reproduces_the_prompt_open_fill_on_offers() {
    let data = data();
    let mut checked = 0usize;
    let mut buys = 0usize;
    let mut yes = 0usize;
    let mut no = 0usize;

    for seed in 0..12u64 {
        let members = bots(4);
        let mut m = Match::new(
            data.clone(),
            Arc::new(StubRules),
            &members,
            seed,
            MatchMode::Casual,
            ScoreWeights::default(),
        );
        m.quick_start();
        for _ in 0..8000 {
            m.tick(0.25);
            if m.ended() {
                break;
            }
            let st = m.state();
            let title = st.prompt.title.k.as_ref().to_string();
            if st.prompt.id <= 0 || !deterministic_offer(&title) {
                continue;
            }
            for (seat, p) in st.players.iter().enumerate() {
                if !st.prompt.waiting(seat as i32) {
                    continue;
                }
                let want = m.view_extra_typed(p.member).ai_answer.unwrap_or_else(|| {
                    panic!("seed {seed}: live fill missing on a waiting {title} prompt")
                });
                let got = m
                    .compute_ai_answer(p.member)
                    .unwrap_or_else(|| panic!("seed {seed}: no recomputed answer for {title}"));
                assert_same(&got, &want, &format!("seed {seed} seat {seat} {title}"));
                checked += 1;
                if title == "ask.buy.title" {
                    buys += 1;
                    if want.answer == 0 {
                        yes += 1;
                    } else {
                        no += 1;
                    }
                }
            }
        }
    }

    eprintln!("offer parity: {checked} checks ({buys} buys: {yes} yes / {no} no)");
    assert!(buys > 20, "too few buy prompts sampled: {buys}");
    // The bug was "always decline" -- a healthy run must see both answers,
    // and in particular must see the engine actually say yes.
    assert!(yes > 0, "the engine never said yes to a buy across {buys} prompts");
}

/// The user-visible symptom, stated directly: with funds that leave the buy
/// reserve, the recomputed answer must be "buy", not the prompt's fallback.
#[test]
fn compute_ai_answer_buys_when_the_reserve_is_met() {
    let data = data();
    let mut checked = 0usize;
    for seed in 0..12u64 {
        let members = bots(4);
        let mut m = Match::new(
            data.clone(),
            Arc::new(StubRules),
            &members,
            seed + 40,
            MatchMode::Casual,
            ScoreWeights::default(),
        );
        m.quick_start();
        for _ in 0..8000 {
            m.tick(0.25);
            if m.ended() {
                break;
            }
            let st = m.state();
            if st.prompt.id <= 0 || st.prompt.title.k.as_ref() != "ask.buy.title" {
                continue;
            }
            let tile = st.prompt.tile as usize;
            let price = st.prompt.price;
            for (seat, p) in st.players.iter().enumerate() {
                if !st.prompt.waiting(seat as i32) {
                    continue;
                }
                // "Funds are available" in the user's sense: cash covers the
                // price and leaves the default buy reserve. The fill says yes
                // on exactly that condition -- so the recompute must too.
                if price < 0 || p.money < price || p.money - price < 2_000 {
                    continue;
                }
                let want = m
                    .view_extra_typed(p.member)
                    .ai_answer
                    .expect("live fill");
                let got = m.compute_ai_answer(p.member).expect("recomputed answer");
                assert_eq!(
                    want.answer, 0,
                    "seed {seed}: fill declined a fundable buy (tile {tile}, price {price}, money {})",
                    p.money
                );
                assert_eq!(
                    got.answer, 0,
                    "seed {seed}: compute_ai_answer declined a fundable buy (tile {tile}, price {price}, money {}) -- online 托管 would skip it",
                    p.money
                );
                checked += 1;
            }
        }
    }
    eprintln!("fundable buys checked: {checked}");
    assert!(checked > 5, "too few fundable buy prompts sampled: {checked}");
}