//! Seat-view extras == server `view_extra` (`docs/BOT.md` §1).
//!
//! The key equivalence test: for seeded games at several points, every seat's
//! view is fed through the client path (`view_extras` -- determinize once,
//! run the same gates) and compared against the server path
//! (`Match::view_extra_typed` on the live match).
//!
//! Hard invariants (the gates read only public / own-exact state):
//! * `playable` -- one flag per hand card, equal every time.
//! * `est_cost` -- static card property, equal every time.
//! * `skills` -- same list, same `enabled` / `reason`.
//!
//! `ai_answer` may diverge whenever the live match's precomputed [`Ask`] fill
//! drew from the match RNG (auction ceilings, [反击] propensity) -- the fork
//! owns a fresh stream. Those divergences are counted here and listed in the
//! hidden-state audit. Both sides must still agree on *whether* an answer
//! exists.

use std::sync::Arc;

use bot_core::{view_extras, SeatView};
use game_core::data::GameData;
use game_core::engine::{Match, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{BotMentality, MatchState};
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string())).unwrap(),
    )
}

fn bots(n: i32) -> Vec<RoomMember> {
    (1..=n)
        .map(|i| RoomMember {
            id: i,
            player: format!("Bot{i}"),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        })
        .collect()
}

/// The public state with the volatile clocks stripped (for error messages).
fn normalized(st: &MatchState) -> MatchState {
    let mut s = st.clone();
    s.seq = 0;
    s.time_left = 0.0;
    s.think_time = 0;
    s.shield = 0.0;
    s.bank = 0.0;
    s.prompt.time_left = 0.0;
    s
}

#[derive(Default, Debug, Clone, Copy)]
struct Tallies {
    views: usize,
    playable_ok: usize,
    playable_bad: usize,
    est_cost_ok: usize,
    est_cost_bad: usize,
    skills_ok: usize,
    skills_bad: usize,
    /// Both sides say "no live prompt for me".
    ai_both_none: usize,
    /// Both sides carry an answer and every field matches.
    ai_exact: usize,
    /// Both sides carry an answer but at least one field differs (RNG).
    ai_diverged: usize,
    /// One side has an answer and the other does not (must be 0).
    ai_presence_bad: usize,
}

/// Strip the extras the online frame no longer carries -- the client path
/// must work from `state` + own hand + own draw alone.
fn strip_extras(view: &mut SeatView) {
    view.ai_answer = None;
    view.playable = Vec::new();
    view.est_cost = Vec::new();
}

fn compare_one(
    live: &Match,
    member: i32,
    data: &Arc<GameData>,
    rules: &Arc<dyn game_core::engine::CardRules>,
    seed: u64,
    t: &mut Tallies,
    label: &str,
    strip: bool,
) {
    let want = live.view_extra_typed(member);
    let mut view = SeatView::from_match(live, member);
    if strip {
        strip_extras(&mut view);
    }
    let got = view_extras(&view, data, rules, seed).expect("view_extras");

    t.views += 1;

    if got.playable == want.playable {
        t.playable_ok += 1;
    } else {
        t.playable_bad += 1;
        panic!(
            "{label} member {member}: playable diverged\n  server: {:?}\n  client: {:?}\n  hand: {:?}\n  state: {}",
            want.playable,
            got.playable,
            view.hand,
            serde_json::to_string(&normalized(&view.state)).unwrap_or_default(),
        );
    }

    if got.est_cost == want.est_cost {
        t.est_cost_ok += 1;
    } else {
        t.est_cost_bad += 1;
        panic!("{label} member {member}: est_cost diverged: {:?} vs {:?}", want.est_cost, got.est_cost);
    }

    let skills_equal = got.skills.len() == want.skills.len()
        && got
            .skills
            .iter()
            .zip(want.skills.iter())
            .all(|(a, b)| a.id == b.id && a.enabled == b.enabled && a.reason == b.reason);
    if skills_equal {
        t.skills_ok += 1;
    } else {
        t.skills_bad += 1;
        panic!(
            "{label} member {member}: skills diverged\n  server: {:?}\n  client: {:?}",
            want.skills.iter().map(|s| (&s.id, s.enabled)).collect::<Vec<_>>(),
            got.skills.iter().map(|s| (&s.id, s.enabled)).collect::<Vec<_>>(),
        );
    }

    match (&want.ai_answer, &got.ai_answer) {
        (None, None) => t.ai_both_none += 1,
        (Some(_), None) | (None, Some(_)) => {
            t.ai_presence_bad += 1;
            panic!(
                "{label} member {member}: aiAnswer presence diverged: server {:?} client {:?}",
                want.ai_answer, got.ai_answer
            );
        }
        (Some(a), Some(b)) => {
            let same = a.answer == b.answer && a.picked == b.picked && a.worth == b.worth;
            if same {
                t.ai_exact += 1;
            } else {
                t.ai_diverged += 1;
            }
        }
    }
}

#[test]
fn seat_view_extras_match_the_server_on_seeded_games() {
    let data = data();
    let rules: Arc<dyn game_core::engine::CardRules> = Arc::new(StubRules);
    let mut t = Tallies::default();

    for seed in 0..12u64 {
        let members = bots(4);
        let mut m = Match::new(
            data.clone(),
            rules.clone(),
            &members,
            seed,
            MatchMode::Casual,
            ScoreWeights::default(),
        );
        m.quick_start();
        let steps = 5 + (seed as usize % 7) * 3;
        for s in 0..steps {
            for _ in 0..(7 + s * 3) {
                m.tick(0.25);
            }
            if m.ended() {
                break;
            }
            for seat in 0..4usize {
                let member = m.state().players.get(seat).map(|p| p.member);
                let Some(member) = member else {
                    continue;
                };
                compare_one(
                    &m,
                    member,
                    &data,
                    &rules,
                    seed * 1000 + s as u64 * 10 + seat as u64,
                    &mut t,
                    &format!("seed {seed} step {s}"),
                    true,
                );
            }
        }
    }

    eprintln!(
        "seat-view extras equivalence: {} views | playable {}/{} | estCost {}/{} | skills {}/{} | ai both-none {} exact {} diverged {} presence-bad {}",
        t.views,
        t.playable_ok,
        t.playable_ok + t.playable_bad,
        t.est_cost_ok,
        t.est_cost_ok + t.est_cost_bad,
        t.skills_ok,
        t.skills_ok + t.skills_bad,
        t.ai_both_none,
        t.ai_exact,
        t.ai_diverged,
        t.ai_presence_bad,
    );

    assert!(t.views > 50, "too few views sampled: {}", t.views);
    assert_eq!(t.playable_bad, 0, "playable must agree every time");
    assert_eq!(t.est_cost_bad, 0, "est_cost must agree every time");
    assert_eq!(t.skills_bad, 0, "skills must agree every time");
    assert_eq!(t.ai_presence_bad, 0, "aiAnswer presence must agree every time");
    // `ai_exact` covers the no-RNG prompts (the fallback fill). Divergences
    // are the audit's RNG rows; they must stay a minority.
    assert!(
        t.ai_exact + t.ai_both_none >= t.ai_diverged,
        "aiAnswer diverged more often than it agreed -- something beyond RNG moved: {t:?}"
    );
}

/// The same comparison on a **solo** match (a human seat among bots), and with
/// the extras left on the view (the solo frame still carries them). The client
/// path must not regress when the frame is the rich solo one either.
#[test]
fn seat_view_extras_match_on_a_solo_frame_too() {
    let data = data();
    let rules: Arc<dyn game_core::engine::CardRules> = Arc::new(StubRules);
    let mut t = Tallies::default();

    for seed in 0..6u64 {
        let members: Vec<RoomMember> = (1..=3)
            .map(|i| RoomMember {
                id: i,
                player: format!("P{i}"),
                bot: i != 1,
                mentality: BotMentality::Standard,
                ..Default::default()
            })
            .collect();
        let mut m = Match::new(
            data.clone(),
            rules.clone(),
            &members,
            seed + 40,
            MatchMode::Solo,
            ScoreWeights::default(),
        );
        m.quick_start();
        for s in 0..12 {
            for _ in 0..(6 + s * 2) {
                m.tick(0.25);
            }
            if m.ended() {
                break;
            }
            for seat in 0..3usize {
                let member = m.state().players.get(seat).map(|p| p.member);
                let Some(member) = member else {
                    continue;
                };
                // Solo frame: extras stay on the view (`SeatView::from_match`
                // filled them). `view_extras` must still agree with the live
                // `view_extra_typed`.
                compare_one(
                    &m,
                    member,
                    &data,
                    &rules,
                    seed * 100 + s as u64,
                    &mut t,
                    &format!("solo seed {seed} step {s}"),
                    false,
                );
            }
        }
    }
    eprintln!("solo-frame equivalence: {t:?}");
    assert!(t.views > 20, "too few solo views: {}", t.views);
    assert_eq!(t.playable_bad + t.est_cost_bad + t.skills_bad + t.ai_presence_bad, 0);
}