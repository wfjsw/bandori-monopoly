//! Determinizer consistency guard (`docs/BOT.md` risk 3).
//!
//! For many seeded games at random points, take a seat's view, determinize N
//! times, and assert every fork reproduces the view exactly for that seat:
//!
//! * same public state (volatile clocks stripped),
//! * same own hand / draw composition,
//! * hidden-zone counts equal for every seat,
//! * no card appears in two of one seat's hidden zones,
//! * sampled opponent hands only contain cards their deck rules allow minus
//!   public sightings.

use std::collections::BTreeSet;
use std::sync::Arc;

use bot_core::{determinize, DeterminizerRng, SeatView};
use game_core::data::GameData;
use game_core::deck;
use game_core::engine::{Match, StubRules};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{MatchState, BotMentality};
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string())).unwrap(),
    )
}

/// The public state with the volatile clocks stripped -- the determinizer
/// does not (and must not) reproduce ticking clocks.
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

/// Public sightings of `seat`'s cards: discard + their field + board fields
/// they own (the same set the determinizer subtracts).
fn sightings(st: &MatchState, seat: usize) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    if let Some(p) = st.players.get(seat) {
        for c in &p.discard {
            s.insert(c.clone());
        }
        for f in &p.field {
            s.insert(f.card.clone());
        }
    }
    for f in &st.board_field {
        if f.owner == seat as i32 {
            s.insert(f.card.clone());
        }
    }
    s
}

fn allowed_for(data: &GameData, st: &MatchState, seat: usize) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    if let Some(c) = st.players.get(seat).and_then(|p| data.character(&p.character)) {
        for k in deck::pool(data, c) {
            s.insert(k.id.clone());
        }
        for k in deck::derived(data, c) {
            s.insert(k.id.clone());
        }
    }
    let seen = sightings(st, seat);
    s.retain(|id| !seen.contains(id));
    s
}

#[test]
fn determinize_reproduces_the_view() {
    let data = data();
    let rules: Arc<dyn game_core::engine::CardRules> = Arc::new(StubRules);
    let mut checked = 0usize;
    for seed in 0..12u64 {
        let members: Vec<RoomMember> = (1..=4)
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
            seed,
            MatchMode::Casual,
            ScoreWeights::default(),
        );
        m.quick_start();
        // Walk to a random-ish point: a few dozen ticks per step.
        let steps = 5 + (seed as usize % 7) * 3;
        for s in 0..steps {
            for _ in 0..(7 + s * 3) {
                m.tick(0.25);
            }
            if m.ended() {
                break;
            }
            for seat in 0..4usize {
                // `roll_order` reorders seats; member ids are stable, seat
                // indices are not. Address seats through the state.
                let member = match m.state().players.get(seat) {
                    Some(p) => p.member,
                    None => continue,
                };
                let view = SeatView::from_match(&m, member);
                let seat = view.player_id as usize;
                let mut rng = DeterminizerRng::new(seed * 1000 + s as u64 * 10 + seat as u64);
                for k in 0..4 {
                    let (fork, _report) = determinize(&view, &data, &rules, &mut rng)
                        .unwrap_or_else(|e| {
                            panic!("seed {seed} step {s} seat {seat} sample {k}: {e}")
                        });

                    // 1. Same public state (volatile clocks stripped).
                    let got = normalized(&fork.state());
                    let want = normalized(&view.state);
                    assert_eq!(
                        serde_json::to_string(&got).unwrap(),
                        serde_json::to_string(&want).unwrap(),
                        "seed {seed} step {s} seat {seat} sample {k}: public state diverged"
                    );

                    // 2. Own hand / draw composition.
                    assert_eq!(
                        fork.hand_of(member),
                        view.hand,
                        "seed {seed} step {s} seat {seat} sample {k}: hand"
                    );
                    let mut draw = fork.draw_of(member);
                    draw.sort();
                    let mut want_draw = view.draw.clone();
                    want_draw.sort();
                    assert_eq!(
                        draw, want_draw,
                        "seed {seed} step {s} seat {seat} sample {k}: draw composition"
                    );

                    // 3. Hidden-zone counts equal for every seat.
                    let fst = fork.state();
                    for i in 0..4 {
                        let p = &fst.players[i];
                        assert_eq!(
                            p.hand, view.state.players[i].hand,
                            "seed {seed} step {s} seat {seat} sample {k}: hand count seat {i}"
                        );
                        assert_eq!(
                            p.draw, view.state.players[i].draw,
                            "seed {seed} step {s} seat {seat} sample {k}: draw count seat {i}"
                        );
                    }

                    // 4. No card appears in two hidden zones of one seat
                    //    (hand + draw are disjoint from each other and from
                    //    the public sightings; discards are public and exact).
                    for i in 0..4 {
                        let member_i = view.state.players[i].member;
                        let hand: BTreeSet<String> =
                            fork.hand_of(member_i).into_iter().collect();
                        let draw: BTreeSet<String> =
                            fork.draw_of(member_i).into_iter().collect();
                        assert!(
                            hand.is_disjoint(&draw),
                            "seed {seed} step {s} seat {seat} sample {k}: seat {i} hand/draw overlap: {hand:?} vs {draw:?}"
                        );
                        let seen = sightings(&view.state, i);
                        assert!(
                            hand.is_disjoint(&seen),
                            "seed {seed} step {s} seat {seat} sample {k}: seat {i} hand holds a sighting"
                        );
                        assert!(
                            draw.is_disjoint(&seen),
                            "seed {seed} step {s} seat {seat} sample {k}: seat {i} draw holds a sighting"
                        );
                    }

                    // 5. Opponent hands only contain cards their deck rules
                    //    allow, minus public sightings.
                    for i in 0..4 {
                        if i == seat {
                            continue;
                        }
                        let allowed = allowed_for(&data, &view.state, i);
                        let member_i = view.state.players[i].member;
                        for c in fork.hand_of(member_i) {
                            if !allowed.contains(&c) {
                                let ch = &view.state.players[i].character;
                                let seen = sightings(&view.state, i);
                                let in_pool = data
                                    .character(ch)
                                    .map(|cd| {
                                        deck::pool(&data, cd).iter().any(|x| x.id == c)
                                            || deck::derived(&data, cd).iter().any(|x| x.id == c)
                                    })
                                    .unwrap_or(false);
                                panic!(
                                    "seed {seed} step {s} seat {seat} sample {k}: seat {i} hand card {c} not allowed; \
                                     char={ch:?} in_pool_or_derived={in_pool} in_seen={} allowed_len={}",
                                    seen.contains(&c),
                                    allowed.len()
                                );
                            }
                        }
                        for c in fork.draw_of(member_i) {
                            assert!(
                                allowed.contains(&c),
                                "seed {seed} step {s} seat {seat} sample {k}: seat {i} draw card {c} not allowed"
                            );
                        }
                    }

                    // 6. Event deck count + pinned top.
                    assert_eq!(
                        fst.event_deck, view.state.event_deck,
                        "seed {seed} step {s} seat {seat} sample {k}: event deck count"
                    );
                    assert_eq!(
                        fst.event_top, view.state.event_top,
                        "seed {seed} step {s} seat {seat} sample {k}: event_top"
                    );
                }
            }
            checked += 1;
        }
    }
    assert!(checked > 10, "expected to walk real game points, got {checked}");
}

#[test]
fn determinize_own_draw_order_varies() {
    // The own draw composition is known but the order is not: two samples
    // must be able to disagree on order (when the pile has >1 card).
    let data = data();
    let rules: Arc<dyn game_core::engine::CardRules> = Arc::new(StubRules);
    let members: Vec<RoomMember> = (1..=4)
        .map(|i| RoomMember {
            id: i,
            player: format!("Bot{i}"),
            bot: true,
            ..Default::default()
        })
        .collect();
    let mut m = Match::new(
        data.clone(),
        rules.clone(),
        &members,
        3,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    let view = SeatView::from_match(&m, 1);
    if view.draw.len() < 2 {
        return; // nothing to shuffle
    }
    let mut orders = BTreeSet::new();
    for k in 0..32 {
        let mut rng = DeterminizerRng::new(9000 + k);
        let (fork, _) = determinize(&view, &data, &rules, &mut rng).unwrap();
        let raw = fork.world().hidden[view.player_id as usize].draw.clone();
        orders.insert(raw);
        // ... but the composition always matches.
        let mut sorted = fork.draw_of(1);
        sorted.sort();
        let mut want = view.draw.clone();
        want.sort();
        assert_eq!(sorted, want);
    }
    assert!(
        orders.len() > 1,
        "own draw order never varied across 32 samples: {orders:?}"
    );
}

#[test]
fn determinize_samples_opponent_hands_differently() {
    let data = data();
    let rules: Arc<dyn game_core::engine::CardRules> = Arc::new(StubRules);
    let members: Vec<RoomMember> = (1..=4)
        .map(|i| RoomMember {
            id: i,
            player: format!("Bot{i}"),
            bot: true,
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
    let view = SeatView::from_match(&m, 1);
    let mut hands = BTreeSet::new();
    for k in 0..32 {
        let mut rng = DeterminizerRng::new(4000 + k);
        let (fork, _) = determinize(&view, &data, &rules, &mut rng).unwrap();
        let hand = fork.hand_of(2);
        hands.insert(hand);
    }
    if view.state.players[2].hand > 1 {
        assert!(
            hands.len() > 1,
            "opponent hand never varied across 32 samples: {hands:?}"
        );
    }
}