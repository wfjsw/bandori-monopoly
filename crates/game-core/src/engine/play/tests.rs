//! Routine-level tests on hand-built board situations.

use std::sync::Arc;

use super::*;
use crate::data::GameData;
use crate::engine::cx::{Answered, HaltKind};
use crate::engine::world::World;
use crate::engine::{Match, StubRules};
use crate::net::RoomMember;
use crate::scoring::ScoreWeights;
use crate::state::MatchPrompt;
use crate::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string())).unwrap())
}

/// A world in the play phase, seat 0 to move, nobody owning anything.
fn setup(seats: i32) -> (Arc<GameData>, World) {
    let d = data();
    let members: Vec<RoomMember> = (1..=seats).map(|i| RoomMember { id: i, player: format!("P{i}"), bot: true, ..Default::default() }).collect();
    let mut m = Match::new(d.clone(), Arc::new(StubRules), &members, 1, MatchMode::Casual, ScoreWeights::default());
    m.quick_start();
    let mut w = m.world.clone();
    w.st.turn = 0;
    w.st.step = 1;
    w.next_turn_pending = false;
    for s in &mut w.st.seats {
        s.money = 10_000;
        s.pos = 0;
    }
    (d, w)
}

type Run = (World, Option<MatchPrompt>);

fn run(d: &GameData, w: &World, answers: &[Answered], f: impl FnOnce(&mut Cx) -> Flow<()>) -> Run {
    let mut cx = Cx::new(w.clone(), d, &StubRules, answers);
    match f(&mut cx) {
        Ok(()) | Err(Halt(HaltKind::Ended)) => (cx.w, None),
        Err(Halt(HaltKind::Ask(a))) => (cx.w, Some(a.view)),
    }
}

fn pick(v: i32) -> Answered {
    Answered { answers: vec![v], ..Default::default() }
}

fn first(d: &GameData, f: impl Fn(&crate::data::TileData) -> bool) -> usize {
    d.tiles.iter().position(f).expect("tile exists")
}

#[test]
fn rent_follows_the_rent_table() {
    let (d, mut w) = setup(2);
    let t = first(&d, |x| x.kind == "property" && x.rent.len() >= 3);
    w.st.owners[t] = 1;
    w.st.houses[t] = 2;
    w.st.seats[0].pos = t as i32;
    let rent = d.tiles[t].rent[2];
    let (w2, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    assert!(p.is_none());
    assert_eq!(w2.st.seats[0].money, 10_000 - rent);
    assert_eq!(w2.st.seats[1].money, 10_000 + rent);
    let e = w2.recent.back().unwrap();
    assert_eq!((e.r#type.as_str(), e.value, e.other, e.to), ("rent", rent, 1, t as i32));
}

#[test]
fn ring_rent_is_rings_times_multiplier_times_d20() {
    let (d, mut w) = setup(2);
    let rings: Vec<usize> = (0..d.tiles.len()).filter(|&t| d.tiles[t].kind == "ring").collect();
    assert_eq!(rings.len(), 4);
    w.st.owners[rings[0]] = 1;
    w.st.owners[rings[1]] = 1;
    w.st.seats[0].pos = rings[0] as i32;
    for seed in 0..20 {
        w.rng = crate::rng::Rng::new(seed);
        let (w2, _) = run(&d, &w, &[], |cx| cx.land(0, true));
        let paid = 10_000 - w2.st.seats[0].money;
        assert!(paid % 20 == 0 && (20..=400).contains(&paid), "2 rings x 10 x 1d20, got {paid}");
    }
}

#[test]
fn agent_charges_half_rent_when_the_whole_group_belongs_to_others() {
    let (d, mut w) = setup(2);
    let agent = first(&d, |x| x.kind == "agent");
    let g = d.tiles[agent].group;
    let group: Vec<usize> = (0..d.tiles.len()).filter(|&t| d.tiles[t].is_buyable() && d.tiles[t].group == g).collect();
    assert!(!group.is_empty());
    for &t in &group {
        w.st.owners[t] = 1;
    }
    w.st.seats[0].pos = agent as i32;
    let expected: i32 = group.iter().map(|&t| ((d.tiles[t].rent[0] as f64 / 2.0 / 10.0).ceil() as i32) * 10).sum();
    let (w2, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    assert!(p.is_none());
    assert_eq!(10_000 - w2.st.seats[0].money, expected);
}

#[test]
fn agent_offers_purchases_in_its_group() {
    let (d, mut w) = setup(2);
    let agent = first(&d, |x| x.kind == "agent");
    let g = d.tiles[agent].group;
    w.st.seats[0].pos = agent as i32;
    let (_, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    let p = p.expect("tile prompt");
    assert_eq!(p.kind, "tile");
    assert_eq!(p.fallback, p.items.len() as i32, "fallback = choose nothing");
    let t: usize = p.items[0].parse().unwrap();
    assert_eq!(d.tiles[t].group, g);

    let (w2, p) = run(&d, &w, &[pick(0)], |cx| cx.land(0, true));
    assert!(p.is_none());
    assert_eq!(w2.st.owners[t], 0);
    assert_eq!(w2.st.seats[0].money, 10_000 - d.tiles[t].price);

    // Any answer outside the option list means "choose nothing".
    let (w3, _) = run(&d, &w, &[pick(99)], |cx| cx.land(0, true));
    assert_eq!(w3.st.owners[t], -1, "choosing nothing buys nothing");
}

#[test]
fn short_of_cash_mortgages_then_pays() {
    let (d, mut w) = setup(2);
    let t = first(&d, |x| x.kind == "property" && x.rent.len() >= 3 && x.rent[2] >= 1000);
    let rent = d.tiles[t].rent[2];
    w.st.owners[t] = 1;
    w.st.houses[t] = 2;
    w.st.seats[0].pos = t as i32;
    w.st.seats[0].money = 100;
    let deeds: Vec<usize> = (0..d.tiles.len()).filter(|&x| d.tiles[x].kind == "property" && x != t).take(4).collect();
    for &x in &deeds {
        w.st.owners[x] = 0;
    }

    let (_, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    let p = p.expect("mortgage prompt");
    assert_eq!(p.kind, "mortgage");
    assert_eq!(p.bid, rent - 100, "asks for the shortfall");

    // Pick every deed: enough to cover it.
    let answer = Answered { answers: vec![0], picked: p.items.clone(), ..Default::default() };
    let (w2, p2) = run(&d, &w, &[answer], |cx| cx.land(0, true));
    assert!(p2.is_none());
    let raised: i32 = deeds.iter().map(|&x| d.tiles[x].price / 2).sum();
    assert!(deeds.iter().all(|&x| w2.st.mortgaged[x]));
    assert_eq!(w2.st.seats[0].money, 100 + raised - rent);
    assert!(!w2.st.seats[0].bankrupt);
    assert_eq!(w2.st.seats[1].money, 10_000 + rent);
}

#[test]
fn bankruptcy_pays_the_creditor_and_can_end_the_game() {
    let (d, mut w) = setup(2);
    let t = first(&d, |x| x.kind == "property" && x.rent.len() >= 3);
    w.st.owners[t] = 1;
    w.st.houses[t] = 3;
    w.st.seats[0].pos = t as i32;
    w.st.seats[0].money = 7;
    let (w2, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    assert!(p.is_none());
    assert!(w2.st.seats[0].bankrupt && w2.st.seats[0].out_order == 1);
    assert_eq!(w2.st.seats[1].money, 10_007, "creditor receives everything");
    assert_eq!(w2.st.phase, "ended");
    assert_eq!((w2.st.end_reason.as_str(), w2.st.winner), ("last", 1));
    assert_eq!((w2.st.seats[1].rank, w2.st.seats[0].rank), (1, 2));
}

#[test]
fn bankrupt_land_is_freed_and_auctioned() {
    let (d, mut w) = setup(3);
    let t = first(&d, |x| x.kind == "property" && x.rent.len() >= 3);
    let mine = first(&d, |x| x.kind == "property" && x.name != d.tiles[t].name);
    w.st.owners[t] = 1;
    w.st.houses[t] = 3;
    w.st.owners[mine] = 0;
    w.st.mortgaged[mine] = true; // nothing left to mortgage -> straight to bankruptcy
    w.st.seats[0].pos = t as i32;
    w.st.seats[0].money = 0;
    let (w2, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    assert!(w2.st.seats[0].bankrupt);
    assert_eq!(w2.st.owners[mine], -1, "deed returned to the bank");
    assert!(!w2.st.mortgaged[mine]);
    let p = p.expect("auction prompt");
    assert_eq!((p.kind.as_str(), p.tile), ("auction", mine as i32));
    assert_eq!(p.seats, vec![1, 2], "remaining players bid");

    let won = Answered { answers: vec![1, -1], bid: 300, bidder: 2, ..Default::default() };
    let (w3, _) = run(&d, &w, &[won], |cx| cx.land(0, true));
    assert_eq!(w3.st.owners[mine], 2);
    assert_eq!(w3.st.seats[2].money, 10_000 - 300);
}

#[test]
fn mortgaged_land_can_be_force_bought_at_double() {
    let (d, mut w) = setup(2);
    let t = first(&d, |x| x.kind == "property" && x.rent.len() >= 3);
    w.st.owners[t] = 1;
    w.st.houses[t] = 1;
    w.st.mortgaged[t] = true;
    w.st.seats[0].pos = t as i32;
    let price = 2 * (d.tiles[t].price + d.tiles[t].house);

    let (w1, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    assert_eq!(p.expect("prompt").title.key(), "ask.force_buy.title");
    assert_eq!(w1.st.seats[0].money, 10_000, "mortgaged land charges no rent");

    let (w2, _) = run(&d, &w, &[pick(0)], |cx| cx.land(0, true));
    assert_eq!(w2.st.owners[t], 0);
    assert!(w2.st.mortgaged[t], "stays mortgaged");
    assert_eq!(w2.st.seats[0].money, 10_000 - price);
    assert_eq!(w2.st.seats[1].money, 10_000 + price);

    let (w3, _) = run(&d, &w, &[pick(1)], |cx| cx.land(0, true));
    assert_eq!(w3.st.owners[t], 1, "declined");
}

#[test]
fn passing_circle_offers_money_or_a_card() {
    let (d, mut w) = setup(2);
    let n = d.tiles.len() as i32;
    assert_eq!(d.tiles[0].kind, "circle");
    w.st.seats[0].pos = n - 2;
    let walk = |cx: &mut Cx| {
        let mut m = Move::new(0);
        m.roll = 5;
        m.resolve = false;
        cx.walk(&mut m)
    };
    let (_, p) = run(&d, &w, &[], walk);
    let p = p.expect("reward prompt");
    assert_eq!((p.title.key(), p.options.len()), ("ask.circle.title", 2));

    let (money, _) = run(&d, &w, &[pick(0)], walk);
    assert_eq!(money.st.seats[0].money, 10_000 + CIRCLE_MONEY);
    assert_eq!(money.st.seats[0].pos, 3);

    let hand = w.hidden[0].hand.len();
    let (card, _) = run(&d, &w, &[pick(1)], walk);
    assert_eq!(card.st.seats[0].money, 10_000);
    assert_eq!(card.hidden[0].hand.len(), hand + 1);
}

#[test]
fn replays_are_exact() {
    let (d, w) = setup(3);
    // A real main move: random roll, possibly a CiRCLE prompt, landing, rent...
    for seed in 0..30 {
        let mut w = w.clone();
        w.rng = crate::rng::Rng::new(seed);
        w.st.seats[0].pos = d.tiles.len() as i32 - 3;
        let (a, pa) = run(&d, &w, &[], |cx| cx.main_move(0, 0));
        let (b, pb) = run(&d, &w, &[], |cx| cx.main_move(0, 0));
        assert_eq!(a, b, "seed {seed}");
        assert_eq!(pa, pb);
        if let Some(p) = pa {
            // Answering continues from exactly the halted world.
            let (c, _) = run(&d, &w, &[pick(p.fallback)], |cx| cx.main_move(0, 0));
            let shared = a.recent.len();
            assert_eq!(&c.recent.iter().take(shared).cloned().collect::<Vec<_>>(), &a.recent.iter().cloned().collect::<Vec<_>>());
        }
    }
}

#[test]
fn final_score_and_ranking() {
    let (d, mut w) = setup(4);
    let props: Vec<usize> = (0..d.tiles.len()).filter(|&t| d.tiles[t].kind == "property").collect();
    w.st.seats[0].money = 5_000;
    w.st.owners[props[0]] = 0;
    w.st.houses[props[0]] = 2;
    w.st.seats[1].money = 6_000;
    w.st.owners[props[1]] = 1;
    w.st.mortgaged[props[1]] = true;
    w.st.seats[2].bankrupt = true;
    w.st.seats[2].out_order = 1;
    w.st.seats[3].left = true;
    w.st.seats[3].out_order = 2;
    w.st.score_money = 1.0;
    w.st.score_property = 2.0;
    w.st.score_houses = 0.5;
    let (w2, _) = run(&d, &w, &[], |cx| {
        cx.finish("settle", None);
        Ok(())
    });
    let s = &w2.st.seats;
    let p0 = &d.tiles[props[0]];
    let p1 = &d.tiles[props[1]];
    assert_eq!(s[0].assets, 5_000 + p0.price + 2 * p0.house);
    assert_eq!(s[0].score, (5_000.0 + 2.0 * p0.price as f32 + 0.5 * (2 * p0.house) as f32).round() as i32);
    assert_eq!(s[1].score, 6_000 + 2 * (p1.price / 2), "mortgaged land counts half");
    let order: Vec<i32> = s.iter().map(|x| x.rank).collect();
    let (r0, r1) = if s[0].score >= s[1].score { (1, 2) } else { (2, 1) };
    assert_eq!(order, vec![r0, r1, 4, 3], "later elimination ranks higher");
}
