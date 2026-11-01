//! Routine-level tests on hand-built board situations.

use std::cell::RefCell;
use std::sync::Arc;

use super::*;
use crate::data::{GameData, Roster};
use crate::engine::cx::{Answered, HaltKind};
use crate::engine::rules::CardRules;
use crate::engine::world::World;
use crate::engine::{Match, StubRules};
use crate::net::RoomMember;
use crate::scoring::ScoreWeights;
use crate::state::MatchPrompt;
use crate::MatchMode;

fn data() -> Arc<GameData> {
    // Unit tests live inside the crate, so they cannot use the integration-test
    // `common` harness. Least-invasive roster: the synthetic fixture
    // (see `Roster::from_fixture`).
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let roster = Roster::from_fixture(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/roster.json"),
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

/// A world in the play phase, player 0 to move, nobody owning anything.
fn setup(players: i32) -> (Arc<GameData>, World) {
    let d = data();
    let members: Vec<RoomMember> = (1..=players)
        .map(|i| RoomMember {
            id: i,
            player: format!("P{i}"),
            bot: true,
            ..Default::default()
        })
        .collect();
    let mut m = Match::new(
        d.clone(),
        Arc::new(StubRules),
        &members,
        1,
        // Solo is the practice table (not the book's 娱乐): these routine
        // tests hand-build 2-seat boards and must not trip G-FLOW-12's
        // 「游玩人数为3到10人」 bound.
        MatchMode::Solo,
        ScoreWeights::default(),
    );
    m.quick_start();
    let mut w = m.world.clone();
    w.st.turn = 0;
    w.st.step = stage::OPS;
    w.next_turn_pending = false;
    for s in &mut w.st.players {
        s.money = 10_000;
        s.pos = 0;
    }
    (d, w)
}

type Run = (World, Option<MatchPrompt>);

fn run_with(
    d: &GameData,
    w: &World,
    rules: &dyn CardRules,
    answers: &[Answered],
    f: impl FnOnce(&mut PlayCtx) -> Flow<()>,
) -> Run {
    let mut cx = PlayCtx::new(w.clone(), d, rules, answers);
    match f(&mut cx) {
        Ok(()) | Err(Halt(HaltKind::Ended)) => (cx.into_world(), None),
        Err(Halt(HaltKind::Ask(a))) => (cx.into_world(), Some(a.view)),
        Err(Halt(HaltKind::Suspended)) => {
            let _ = cx.drain_work();
            (cx.into_world(), None)
        }
    }
}

fn run(
    d: &GameData,
    w: &World,
    answers: &[Answered],
    f: impl FnOnce(&mut PlayCtx) -> Flow<()>,
) -> Run {
    run_with(d, w, &StubRules, answers, f)
}

// A counteraction sink the G04 tests park a closure in (one test per thread,
// so a thread-local keeps the `fn`-only helper signature free of captures).
thread_local! {
    static HOOK: RefCell<Option<Box<dyn Fn(&mut PlayCtx, &mut Trigger) -> Flow<()>>>> =
        const { RefCell::new(None) };
}

struct HookRules;

impl CardRules for HookRules {
    fn play(&self, _cx: &mut PlayCtx, _player: usize, _card: &str) -> Flow<Dest> {
        Ok(Dest::Graveyard)
    }
    fn event(&self, _cx: &mut PlayCtx, _player: usize, _id: &str) -> Flow<bool> {
        Ok(false)
    }
    fn counteract(&self, cx: &mut PlayCtx, t: &mut Trigger) -> Flow<()> {
        HOOK.with(|h| match h.borrow().as_ref() {
            Some(f) => f(cx, t),
            None => Ok(()),
        })
    }
}

/// Run `f` with `hook` installed as the ruleset's `counteract`.
fn run_hooked(
    d: &GameData,
    w: &World,
    hook: impl Fn(&mut PlayCtx, &mut Trigger) -> Flow<()> + 'static,
    f: impl FnOnce(&mut PlayCtx) -> Flow<()>,
) -> Run {
    HOOK.with(|h| *h.borrow_mut() = Some(Box::new(hook)));
    let r = run_with(d, w, &HookRules, &[], f);
    HOOK.with(|h| *h.borrow_mut() = None);
    r
}

/// Shape a `void_walk` replacement teleport to `target` on the live frame (the
/// same writes `plan::set_main_override` / `plan::void_walk` /
/// `plan::set_teleport_to` make).
fn shape_void_teleport(w: &mut World, target: i32) {
    let p = w.plan_mut();
    p.main_override = true;
    p.void_walk = true;
    p.teleport_to = Some(Tile(target));
    p.kind = MoveKind::Teleport;
}

/// Drive a `run_planned_move` off the plan the body has shaped.
fn planned(cx: &mut PlayCtx) -> Flow<()> {
    let plan = cx.world().plan().clone();
    cx.run_planned_move(0, plan).map(|_| ())
}

fn pick(v: i32) -> Answered {
    Answered {
        answers: vec![v],
        ..Default::default()
    }
}

fn first(d: &GameData, f: impl Fn(&crate::data::TileData) -> bool) -> usize {
    d.tiles.iter().position(f).expect("tile exists")
}

#[test]
fn rent_follows_the_rent_table() {
    let (d, mut w) = setup(2);
    let t = first(&d, |x| x.is_buyable() && x.rent.len() >= 3);
    w.st.owners[t] = 1;
    w.st.houses[t] = 2;
    w.st.players[0].pos = t as i32;
    let rent = d.tiles[t].rent[2];
    let (w2, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    assert!(p.is_none());
    assert_eq!(w2.st.players[0].money, 10_000 - rent);
    assert_eq!(w2.st.players[1].money, 10_000 + rent);
    let e = w2.recent.back().unwrap();
    assert_eq!(
        (e.r#type.as_str(), e.value, e.other, e.to),
        ("rent", rent, 1, t as i32)
    );
}

#[test]
fn dice_rent_unit_is_count_owned_kind_times_mult() {
    let (d, mut w) = setup(2);
    let deeds: Vec<usize> = (0..d.tiles.len())
        .filter(|&t| d.tiles[t].is_buyable() && d.tiles[t].rent.len() >= 2)
        .take(3)
        .collect();
    assert!(deeds.len() >= 2);
    w.st.owners[deeds[0]] = 1;
    w.st.owners[deeds[1]] = 1;
    let kind = d.tiles[deeds[0]].kind.as_str().to_string();
    let unit = crate::engine::play::purchase::dice_rent_unit(&d, &w, 1, &kind, 10);
    assert_eq!(
        unit,
        crate::engine::play::purchase::count_owned_kind(&d, &w, 1, &kind).max(1) * 10
    );
    assert_eq!(unit, 20, "two owned deeds x mult 10");
}

#[test]
fn non_deed_land_is_a_plain_log() {
    let (d, mut w) = setup(2);
    let t = first(&d, |x| !x.is_buyable());
    w.st.players[0].pos = t as i32;
    let (w2, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    assert!(p.is_none());
    assert_eq!(w2.st.players[0].money, 10_000, "no charge on a non-deed");
}

#[test]
fn short_of_cash_mortgages_then_pays() {
    let (d, mut w) = setup(2);
    let t = first(&d, |x| {
        x.is_buyable() && x.rent.len() >= 3 && x.rent[2] >= 1000
    });
    let rent = d.tiles[t].rent[2];
    w.st.owners[t] = 1;
    w.st.houses[t] = 2;
    w.st.players[0].pos = t as i32;
    w.st.players[0].money = 100;
    let deeds: Vec<usize> = (0..d.tiles.len())
        .filter(|&x| d.tiles[x].is_buyable() && d.tiles[x].rent.len() >= 2 && x != t)
        .take(4)
        .collect();
    for &x in &deeds {
        w.st.owners[x] = 0;
    }

    let (_, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    let p = p.expect("mortgage prompt");
    assert_eq!(p.kind, "mortgage");
    assert_eq!(p.bid, rent - 100, "asks for the shortfall");

    // Pick every deed: enough to cover it.
    let answer = Answered {
        answers: vec![0],
        picked: p.items.clone(),
        ..Default::default()
    };
    let (w2, p2) = run(&d, &w, &[answer], |cx| cx.land(0, true));
    assert!(p2.is_none());
    let raised: i32 = deeds.iter().map(|&x| d.tiles[x].price / 2).sum();
    assert!(deeds.iter().all(|&x| w2.st.mortgaged[x]));
    assert_eq!(w2.st.players[0].money, 100 + raised - rent);
    assert!(!w2.st.players[0].bankrupt);
    assert_eq!(w2.st.players[1].money, 10_000 + rent);
}

#[test]
fn bankruptcy_pays_the_creditor_and_can_end_the_game() {
    let (d, mut w) = setup(2);
    let t = first(&d, |x| x.is_buyable() && x.rent.len() >= 3);
    w.st.owners[t] = 1;
    w.st.houses[t] = 3;
    w.st.players[0].pos = t as i32;
    w.st.players[0].money = 7;
    let (w2, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    assert!(p.is_none());
    assert!(w2.st.players[0].bankrupt && w2.st.players[0].out_order == 1);
    assert_eq!(
        w2.st.players[1].money, 10_007,
        "creditor receives everything"
    );
    assert_eq!(w2.st.phase, "ended");
    assert_eq!((w2.st.end_reason.as_str(), w2.st.winner), ("last", 1));
    assert_eq!((w2.st.players[1].rank, w2.st.players[0].rank), (1, 2));
}

#[test]
fn bankrupt_land_is_freed_and_auctioned() {
    let (d, mut w) = setup(3);
    let t = first(&d, |x| x.is_buyable() && x.rent.len() >= 3);
    let mine = first(&d, |x| {
        x.is_buyable() && x.rent.len() >= 2 && x.name != d.tiles[t].name
    });
    w.st.owners[t] = 1;
    w.st.houses[t] = 3;
    w.st.owners[mine] = 0;
    w.st.mortgaged[mine] = true; // nothing left to mortgage -> straight to bankruptcy
    w.st.players[0].pos = t as i32;
    w.st.players[0].money = 0;
    let (w2, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    assert!(w2.st.players[0].bankrupt);
    assert_eq!(w2.st.owners[mine], -1, "deed returned to the bank");
    assert!(!w2.st.mortgaged[mine]);
    let p = p.expect("auction prompt");
    assert_eq!((p.kind.as_str(), p.tile), ("auction", mine as i32));
    assert_eq!(p.players, vec![1, 2], "remaining players bid");
    // Bankruptcy leftover auction runs on its own dedicated timer and never
    // draws on a seat pool (`Ask::use_pool` -- see `auction_tile`).
    assert_eq!(
        p.time_left, w.clock.bankrupt_auction_time,
        "its window is ClockSettings::bankrupt_auction_time"
    );

    let won = Answered {
        answers: vec![1, -1],
        bid: 300,
        bidder: 2,
        ..Default::default()
    };
    let (w3, _) = run(&d, &w, &[won], |cx| cx.land(0, true));
    assert_eq!(w3.st.owners[mine], 2);
    assert_eq!(w3.st.players[2].money, 10_000 - 300);
}

#[test]
fn mortgaged_land_can_be_force_bought_at_double() {
    let (d, mut w) = setup(2);
    let t = first(&d, |x| x.is_buyable() && x.rent.len() >= 3);
    w.st.owners[t] = 1;
    w.st.houses[t] = 1;
    w.st.mortgaged[t] = true;
    w.st.players[0].pos = t as i32;
    let price = 2 * (d.tiles[t].price + d.tiles[t].house);

    let (w1, p) = run(&d, &w, &[], |cx| cx.land(0, true));
    assert_eq!(p.expect("prompt").title.key(), "ask.force_buy.title");
    assert_eq!(
        w1.st.players[0].money, 10_000,
        "mortgaged land charges no rent"
    );

    let (w2, _) = run(&d, &w, &[pick(0)], |cx| cx.land(0, true));
    assert_eq!(w2.st.owners[t], 0);
    assert!(w2.st.mortgaged[t], "remains mortgaged");
    assert_eq!(w2.st.players[0].money, 10_000 - price);
    assert_eq!(w2.st.players[1].money, 10_000 + price);

    let (w3, _) = run(&d, &w, &[pick(1)], |cx| cx.land(0, true));
    assert_eq!(w3.st.owners[t], 1, "declined");
}

#[test]
fn walk_lands_without_a_builtin_pass_reward() {
    let (d, mut w) = setup(2);
    let n = d.tiles.len() as i32;
    w.st.players[0].pos = n - 2;
    w.turn.plan.settles = false;
    let walk = |cx: &mut PlayCtx| {
        let mut m = Move::new(0);
        m.roll = 5;
        m.settles = false;
        cx.walk(&mut m)
    };
    let (w2, p) = run(&d, &w, &[], walk);
    // StubRules pays no pass reward; `settles = false` skips the settle.
    assert!(p.is_none(), "no built-in pass-reward prompt");
    assert_eq!(w2.st.players[0].pos, 3);
    assert_eq!(w2.st.players[0].money, 10_000);
}

#[test]
fn replays_are_exact() {
    let (d, w) = setup(3);
    // A real main move: random roll, landing, rent...
    for seed in 0..30 {
        let mut w = w.clone();
        w.rng = crate::rng::Rng::new(seed);
        w.st.players[0].pos = d.tiles.len() as i32 - 3;
        let (a, pa) = run(&d, &w, &[], |cx| cx.main_move(0, 0));
        let (b, pb) = run(&d, &w, &[], |cx| cx.main_move(0, 0));
        assert_eq!(a, b, "seed {seed}");
        assert_eq!(pa, pb);
        if let Some(p) = pa {
            // Answering continues from exactly the halted world.
            let (c, _) = run(&d, &w, &[pick(p.fallback)], |cx| cx.main_move(0, 0));
            let shared = a.recent.len();
            assert_eq!(
                &c.recent.iter().take(shared).cloned().collect::<Vec<_>>(),
                &a.recent.iter().cloned().collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn final_score_and_ranking() {
    let (d, mut w) = setup(4);
    let props: Vec<usize> = (0..d.tiles.len())
        .filter(|&t| d.tiles[t].is_buyable() && d.tiles[t].rent.len() >= 2)
        .collect();
    w.st.players[0].money = 5_000;
    w.st.owners[props[0]] = 0;
    w.st.houses[props[0]] = 2;
    w.st.players[1].money = 6_000;
    w.st.owners[props[1]] = 1;
    w.st.mortgaged[props[1]] = true;
    w.st.players[2].bankrupt = true;
    w.st.players[2].out_order = 1;
    w.st.players[3].left = true;
    w.st.players[3].out_order = 2;
    w.st.score_money = 1.0;
    w.st.score_property = 2.0;
    w.st.score_houses = 0.5;
    let (w2, _) = run(&d, &w, &[], |cx| {
        cx.finish("settle", None);
        Ok(())
    });
    let s = &w2.st.players;
    let p0 = &d.tiles[props[0]];
    let p1 = &d.tiles[props[1]];
    assert_eq!(s[0].assets, 5_000 + p0.price + 2 * p0.house);
    assert_eq!(
        s[0].score,
        (5_000.0 + 2.0 * p0.price as f32 + 0.5 * (2 * p0.house) as f32).round() as i32
    );
    assert_eq!(
        s[1].score,
        6_000 + 2 * (p1.price / 2),
        "mortgaged land counts half"
    );
    let order: Vec<i32> = s.iter().map(|x| x.rank).collect();
    let (r0, r1) = if s[0].score >= s[1].score {
        (1, 2)
    } else {
        (2, 1)
    };
    assert_eq!(order, vec![r0, r1, 4, 3], "later elimination ranks higher");
}

/// P0 (`docs/PURCHASE.md`): `quote_native` matches today's buy price on every
/// tile -- land price plus the houses already standing on it -- and the
/// force-buy base is twice that. No behaviour change, just the surface.
#[test]
fn quote_native_matches_todays_prices_on_every_tile() {
    let d = data();
    let (_d, w) = setup(4);
    let st = &w.st;
    for t in 0..d.tiles.len() {
        let tile = &d.tiles[t];
        let expect = tile.price + st.houses.get(t).copied().unwrap_or(0) * tile.house;
        assert_eq!(
            purchase::quote_native(&d, st, t),
            expect,
            "quote_native(tile {t}) == price + houses*house"
        );
        assert_eq!(
            purchase::force_price_native(&d, st, t),
            2 * expect.max(0),
            "force_price_native(tile {t}) == 2x"
        );
        // The engine's own `buy_price` is the same formula.
        assert_eq!(
            w.buy_price(&d, t as i32),
            expect,
            "World::buy_price(tile {t})"
        );
    }
    // A tile with houses on it counts them in.
    let prop = d
        .tiles
        .iter()
        .position(|t| t.is_buyable() && t.house > 0)
        .expect("a buyable tile with a house cost");
    let mut w2 = w.clone();
    w2.st.houses[prop] = 2;
    let expect = d.tiles[prop].price + 2 * d.tiles[prop].house;
    assert_eq!(purchase::quote_native(&d, &w2.st, prop), expect);
    assert_eq!(purchase::force_price_native(&d, &w2.st, prop), 2 * expect);
}

/// P0: `assign_deed` is the one ownership primitive -- owner / houses /
/// mortgage in one write -- and `default_deal` picks the rulebook defaults.
#[test]
fn assign_deed_writes_owner_houses_mortgage() {
    let d = data();
    let (_d, mut w) = setup(2);
    let t = d.tiles.iter().position(|x| x.is_buyable()).unwrap();
    purchase::assign_deed(&mut w, t, 1, 3, true);
    assert_eq!(w.st.owners[t], 1);
    assert_eq!(w.st.houses[t], 3);
    assert!(w.st.mortgaged[t]);
    // A land buy clears the mortgage and keeps the houses.
    let deal = purchase::default_deal(&d, &w.st, t, 0, purchase::BuyKind::Land, 500, false);
    assert_eq!(deal.seller, -1, "land buy pays the bank");
    assert_eq!(deal.owner_after, 0);
    assert_eq!(deal.houses_after, 3, "houses remain unless a hook razes");
    assert!(!deal.mortgaged_after, "a land buy clears the mortgage");
    // A force-buy names the owner as payee and keeps the mortgage by default.
    let deal = purchase::default_deal(&d, &w.st, t, 0, purchase::BuyKind::Force, 1000, true);
    assert_eq!(deal.seller, 1, "force-buy pays the owner");
    assert!(deal.mortgaged_after, "FORCE_STAYS_MORTGAGED keeps it");
    assert!(!deal.direct, "direct is set by FORCE_FIXED, not by kind");
}

// --------------------------------------------------------------------- G04
// Move plan frames, forcedness / mainness, the roll-window replacement and
// the walk's shape reads (`docs/rulebook/spec/abi/G04-move-plan.md` §8.1).

/// The event keys in order (markers are logged as `Msg` keys).
fn keys(w: &World) -> Vec<String> {
    w.recent.iter().map(|e| e.msg.key().to_string()).collect()
}

fn index_of(w: &World, key: &str) -> usize {
    keys(w)
        .iter()
        .position(|k| k == key)
        .unwrap_or_else(|| panic!("no `{key}` event in {:?}", keys(w)))
}

/// G04 §8.1: a spent slot refuses a card main move, and the staging the body
/// wrote (a next-main teleport and factor) is exactly as it stood.
#[test]
fn g04_refused_main_keeps_staging() {
    let (d, mut w) = setup(2);
    let target = first(&d, |x| !x.is_buyable()) as i32;
    w.turn.main_moved = true;
    w.set_teleport_to(Some(Tile(target)));
    w.turn.plan.pay_factor = 0.25;
    w.set_settles(false);
    let staged = w.turn.plan.clone();
    let mut ran = true;
    let (w2, p) = run(&d, &w, &[], |cx| {
        ran = cx.run_planned_move(0, cx.world().plan().clone())?;
        Ok(())
    });
    assert!(p.is_none());
    assert!(!ran, "the spent slot refuses the main move");
    assert_eq!(w2.turn.plan, staged, "the staging is untouched");
    assert_eq!(w2.turn.plan.teleport_to, Some(Tile(target)));
    assert_eq!(w2.turn.plan.pay_factor, 0.25);
    assert!(w2.turn.main_moved);
    assert_eq!(w2.st.players[0].pos, 0, "nothing moved");
    assert!(w2.turn.plan_stack.is_empty(), "no frame was left open");
}

/// G04 §8.1 / I3: a forced move of the turn player is never the main move --
/// it runs with the slot spent, does not consume it again, and consumes
/// nothing from staging.
#[test]
fn g04_forced_never_refused() {
    let (d, mut w) = setup(2);
    w.turn.main_moved = true;
    w.set_forced(true);
    w.turn.plan.set_steps(3);
    w.set_settles(false);
    let (w2, p) = run(&d, &w, &[], |cx| {
        assert!(
            cx.run_planned_move(0, cx.world().plan().clone())?,
            "a forced move always runs"
        );
        Ok(())
    });
    assert!(p.is_none());
    assert_eq!(w2.st.players[0].pos, 3, "the walk ran");
    assert!(w2.turn.main_moved, "the spent slot stays spent");
    assert!(w2.turn.plan_stack.is_empty());
    assert!(
        w2.recent.iter().any(|e| e.r#type == "move"),
        "the side walk logged"
    );
}

/// G04 §8.1 / I3: a forced move marked `main_override` on a free slot is the
/// main move -- it latches the slot and consumes the staging.
#[test]
fn g04_main_override_consumes_when_free() {
    let (d, mut w) = setup(2);
    w.set_forced(true);
    w.set_main_override(true);
    w.turn.plan.set_steps(2);
    w.set_settles(false);
    let mut ran = false;
    let (w2, p) = run(&d, &w, &[], |cx| {
        ran = cx.run_planned_move(0, cx.world().plan().clone())?;
        Ok(())
    });
    assert!(p.is_none());
    assert!(ran);
    assert!(w2.turn.main_moved, "the free slot is consumed");
    assert_eq!(w2.st.players[0].pos, 2);
    assert_eq!(w2.turn.plan, Move::default(), "staging is consumed");
}

/// G04 §8.1 / I7: a `void_walk` replacement from the roll window teleports
/// instead of walking -- the walk raises no pass events, the teleport's tail
/// does.
#[test]
fn g04_void_walk_replaces_the_walk() {
    let (d, mut w) = setup(2);
    // A destination off the rolled route (the roll walks 1..=5).
    let target = first(&d, |x| !x.is_buyable() && x.name != d.tiles[0].name) as i32;
    w.turn.fixed_roll = Some(5);
    w.set_settles(false);
    let hook = move |cx: &mut PlayCtx, t: &mut Trigger| {
        if t.kind == "passTile" {
            cx.log(0, Msg::new(format!("marker.pass.{}", t.tile)));
        }
        if t.kind == "moveAfter" {
            cx.log(0, Msg::new("marker.after"));
        }
        if t.kind != "diceRollResult" {
            return Ok(());
        }
        let mut w = cx.world_copy();
        shape_void_teleport(&mut w, target);
        cx.swap_world(w);
        let plan = cx.world().plan().clone();
        assert!(
            cx.run_planned_move(0, plan)?,
            "the replacement teleport ran"
        );
        Ok(())
    };
    let (w2, p) = run_hooked(&d, &w, hook, planned);
    assert!(p.is_none());
    assert_eq!(
        w2.st.players[0].pos, target,
        "the teleport replaced the walk"
    );
    let passes: Vec<i32> = w2
        .recent
        .iter()
        .filter_map(|e| e.msg.key().strip_prefix("marker.pass.")?.parse().ok())
        .collect();
    assert_eq!(
        passes,
        vec![target],
        "only the teleport endpoint passes; the walk's tiles do not"
    );
    assert_eq!(
        keys(&w2).iter().filter(|k| *k == "marker.after").count(),
        1,
        "exactly the teleport's tail"
    );
    assert!(
        w2.recent.iter().any(|e| e.r#type == "teleport"),
        "the teleport was logged"
    );
    assert!(
        !w2.recent
            .iter()
            .any(|e| e.r#type == "diceRollBefore" || e.r#type == "move"),
        "no walk head was logged"
    );
}

/// G04 §8.1 / §4.4: the replacement settle and its tail complete before the
/// body resumes (the card's 「随后…支付」 order).
#[test]
fn g04_replacement_settles_before_the_body_resumes() {
    let (d, mut w) = setup(2);
    let target = first(&d, |x| !x.is_buyable() && x.name != d.tiles[0].name) as i32;
    w.turn.fixed_roll = Some(5);
    let hook = move |cx: &mut PlayCtx, t: &mut Trigger| {
        match t.kind.as_str() {
            "moveAfter" => cx.log(0, Msg::new("marker.after")),
            "moveResolved" => cx.log(0, Msg::new("marker.resolved")),
            "diceRollResult" => {
                let mut w = cx.world_copy();
                shape_void_teleport(&mut w, target);
                cx.swap_world(w);
                let plan = cx.world().plan().clone();
                assert!(cx.run_planned_move(0, plan)?, "the replacement ran");
                // The body resumes here -- its own follow-up/pay would come now.
                cx.log(0, Msg::new("marker.body_pay"));
            }
            _ => {}
        }
        Ok(())
    };
    let (w2, p) = run_hooked(&d, &w, hook, planned);
    assert!(p.is_none());
    let body = index_of(&w2, "marker.body_pay");
    let teleport = w2
        .recent
        .iter()
        .position(|e| e.r#type == "teleport")
        .expect("teleport logged");
    let settle = index_of(&w2, "log.land");
    let after = index_of(&w2, "marker.after");
    let resolved = index_of(&w2, "marker.resolved");
    assert!(teleport < settle, "the teleport precedes its settle");
    assert!(settle < body, "the settle precedes the body's resume");
    assert!(after < body, "the move tail precedes the body's resume");
    assert!(resolved < body, "the move resolved before the body resumes");
    assert_eq!(w2.st.players[0].pos, target);
}

/// G04 §8.1 / §4.2: a gate refusal clears the replacement attempt, so the
/// outer move walks its roll normally with `kind = Walk` and no teleport.
#[test]
fn g04_gate_refusal_restores_the_walk() {
    let (d, mut w) = setup(2);
    let target = first(&d, |x| !x.is_buyable() && x.name != d.tiles[0].name) as i32;
    w.turn.fixed_roll = Some(5);
    w.set_settles(false);
    let hook = move |cx: &mut PlayCtx, t: &mut Trigger| {
        if t.kind == "moveBefore" {
            cx.log(0, Msg::new(format!("marker.main.{}", t.is_main_move)));
            cx.log(
                0,
                Msg::new(format!("marker.kind.{:?}", cx.world().plan().kind)),
            );
        }
        if t.kind != "diceRollResult" {
            return Ok(());
        }
        let mut w = cx.world_copy();
        shape_void_teleport(&mut w, target);
        cx.swap_world(w);
        // The abnormal gate refuses before `run_planned_move` is called: clear
        // every one-shot the attempt wrote and leave a plain walk (§4.2).
        let mut w = cx.world_copy();
        let p = w.plan_mut();
        p.teleport_to = None;
        p.void_walk = false;
        p.start = None;
        p.start_reason.clear();
        p.by_skill = false;
        p.kind = MoveKind::Walk;
        cx.swap_world(w);
        assert_eq!(cx.world().plan().kind, MoveKind::Walk);
        cx.log(0, Msg::new("marker.refused"));
        Ok(())
    };
    let (w2, p) = run_hooked(&d, &w, hook, planned);
    assert!(p.is_none());
    assert_eq!(w2.st.players[0].pos, 5, "the roll walk ran");
    assert!(w2.turn.main_moved, "the main slot stays consumed");
    assert!(
        !w2.recent.iter().any(|e| e.r#type == "teleport"),
        "no teleport ran"
    );
    assert!(
        w2.recent.iter().any(|e| e.r#type == "diceRollBefore"),
        "the main walk logged its head"
    );
    assert!(
        keys(&w2).iter().any(|k| k == "marker.main.true"),
        "the walk is still the main move"
    );
    assert!(
        keys(&w2).iter().any(|k| k == "marker.kind.Walk"),
        "the walk kept kind = Walk"
    );
    assert_eq!(
        keys(&w2).iter().filter(|k| *k == "marker.refused").count(),
        1
    );
    assert!(w2.turn.plan_stack.is_empty());
}

/// G04 §8.1 / I4: `next_steps` is visible in the `diceRollResult` window and
/// is the length the walk takes.
#[test]
fn g04_effective_steps_prefers_next_steps() {
    let (d, mut w) = setup(2);
    w.hidden[0].next_steps = Some(7);
    w.turn.fixed_roll = Some(3);
    w.set_settles(false);
    let hook = |cx: &mut PlayCtx, t: &mut Trigger| {
        if t.kind == "diceRollResult" {
            let mv = t.mv.as_ref().expect("the window carries the move");
            assert_eq!(t.value, 7, "the window's face is next_steps");
            assert_eq!(mv.effective, 7, "the effective length is next_steps");
            cx.log(0, Msg::new("marker.window"));
        }
        Ok(())
    };
    let (w2, p) = run_hooked(&d, &w, hook, |cx| cx.main_move(0, 0));
    assert!(p.is_none());
    assert_eq!(
        index_of(&w2, "marker.window"),
        index_of(&w2, "marker.window")
    );
    assert_eq!(w2.st.players[0].pos, 7, "the walk used next_steps");
    assert!(w2.hidden[0].next_steps.is_none(), "the slot is consumed");
}

/// G04 §8.1 / I4: `Move.from` is the planned start before the first roll
/// window, so `trigger::move_from()` is right for the whole move.
#[test]
fn g04_from_is_stamped_before_the_roll() {
    let (d, mut w) = setup(2);
    let start = first(&d, |x| !x.is_buyable()) as i32;
    w.set_start(Some(Tile(start)), "test");
    w.turn.fixed_roll = Some(2);
    w.set_settles(false);
    let hook = move |cx: &mut PlayCtx, t: &mut Trigger| {
        if t.kind == "diceRollBefore" {
            let mv = t.mv.as_ref().expect("the roll window carries the move");
            assert_eq!(mv.from, start, "from is the planned start before the roll");
            cx.log(0, Msg::new("marker.from"));
        }
        Ok(())
    };
    let (w2, p) = run_hooked(&d, &w, hook, |cx| cx.main_move(0, 0));
    assert!(p.is_none());
    index_of(&w2, "marker.from");
    assert_eq!(
        w2.st.players[0].pos,
        start + 2,
        "the walk ran from the start"
    );
}

/// G04 §8.1 / §5.3: `add_extra_steps` composes and the running walk reads the
/// frame each step, so two +2s extend the walk by four.
#[test]
fn g04_extra_steps_add_is_relative() {
    let (d, mut w) = setup(2);
    w.turn.fixed_roll = Some(3);
    w.set_settles(false);
    let hook = |cx: &mut PlayCtx, t: &mut Trigger| {
        if t.kind == "passTile" && (t.tile == 1 || t.tile == 2) {
            cx.world_mut().add_extra_steps(2);
            let n = cx.world().plan().extra_steps;
            cx.log(0, Msg::new(format!("marker.extra.{n}")));
        }
        Ok(())
    };
    let (w2, p) = run_hooked(&d, &w, hook, planned);
    assert!(p.is_none());
    let added: Vec<i32> = w2
        .recent
        .iter()
        .filter_map(|e| e.msg.key().strip_prefix("marker.extra.")?.parse().ok())
        .collect();
    assert_eq!(added, vec![2, 4], "the two adds compose");
    assert_eq!(
        w2.st.players[0].pos, 7,
        "3 rolled steps + 4 added steps walked"
    );
}
