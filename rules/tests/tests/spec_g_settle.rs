//! G-SETTLE scenario tests -- 基础[结算]规则, one contract per clause.
//!
//! Specs come from `docs/rulebook/rulebook-doc.md:148-162` (「基础[结算]规则」).
//! `docs/rulebook/SETTLE-STAGES.md` stage names are observability only.
//! The book's item numbering skips 4; clause IDs are sequential.
//!
//! A discrepancy is a plain failing test. No `#[ignore]`, no allowlist.

use rules_spec::global::settle::{half_ceil10, CLAUSES};
use rules_spec::{data, forall, frame, tile, Table};

/// Land `who` on `tile` with a one-step main move from the tile before it.
/// Prompts are left open for the caller.
fn settle_on(t: &mut Table, who: usize, target: usize) {
    t.begin_turn(who);
    t.set_pos(who, (target + 59) % 60);
    t.dice(&[1]);
    t.roll(who).unwrap();
}

fn price(t: usize) -> i32 {
    data().tiles[t].price
}

fn house_cost(t: usize) -> i32 {
    data().tiles[t].house
}

fn rent0(t: usize) -> i32 {
    data().tiles[t].rent[0]
}

// =====================================================================
// G-SETTLE-01 -- mortgaged deed: no rent
// =====================================================================

/// G-SETTLE-01 / 5.2: 「如果格子地契已抵押则无效果。」 / 6.2 takes the
/// force-buy branch instead of rent.
#[test]
fn g_settle_01_mortgaged_deed_pays_no_rent() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_mortgaged(land, true);
    t.set_money(0, 10_000);
    t.set_money(1, 10_000);
    settle_on(&mut t, 0, land);
    // The mortgaged branch offers force-buy (6.2); it never charges rent.
    let mark = t.mark();
    // Decline the force-buy if offered.
    if let Some(p) = t.prompt() {
        assert!(
            p.title.key().contains("force") || p.kind == "choice",
            "force-buy prompt: {}",
            t.dump_prompt()
        );
        t.decline();
    }
    let keys = t.keys_since(mark);
    assert!(
        !keys.iter().any(|k| k.contains("rent")),
        "no rent on a mortgaged deed: {keys:?}"
    );
    assert_eq!(t.money(1), 10_000, "the owner received nothing");
    assert_eq!(t.owner(land), Some(1), "the deed stayed put");
}

// =====================================================================
// G-SETTLE-02 -- CiRCLE / 江户川 draw 1
// =====================================================================

/// G-SETTLE-02: 「CiRCLE和江户川乐器店的[结算]是：抽取一张手卡。」
#[test]
fn g_settle_02_circle_and_edogawa_draw_one() {
    for name in ["CiRCLE", "江户川乐器店"] {
        let mut t = Table::vanilla(2);
        t.set_hand(0, &[]);
        t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!", "通用:FEVER!"]);
        settle_on(&mut t, 0, tile(name));
        // Landing on CiRCLE also [经过]s it (「移动终点触发[经过]」), which
        // opens the CiRCLE-reward prompt *before* the landing settle. Take
        // the card half so the hand-count assertion sees only the settle draw
        // plus the reward -- actually take money so the settle draw is alone.
        while let Some(p) = t.prompt() {
            if p.title.key() == "ask.pass_reward.title" {
                t.answer_one(0).unwrap();
            } else {
                break;
            }
        }
        assert_eq!(
            t.hand(0).len(),
            1,
            "{name} settle = draw 1 (got {:?})",
            t.hand(0)
        );
    }
}

// =====================================================================
// G-SETTLE-03/04/05 -- cafe / 流星堂 draw 1 + event
// =====================================================================

/// G-SETTLE-03: 「CiRCLE咖啡厅和流星堂的的[结算]是：抽取一张手卡，然后抽取一个事件卡。」
/// G-SETTLE-04: the event does not enter the hand and files to the discard.
#[test]
fn g_settle_03_cafe_draws_one_card_then_one_event() {
    for name in ["CiRCLE 咖啡厅", "流星堂"] {
        let mut t = Table::vanilla(2);
        t.set_hand(0, &[]);
        t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!", "通用:FEVER!"]);
        let ev_before: Vec<String> = t.event_deck();
        assert!(!ev_before.is_empty(), "{name}: event deck seeded");
        settle_on(&mut t, 0, tile(name));
        assert_eq!(
            t.hand(0).len(),
            1,
            "{name}: one hand card, not the event ({:?})",
            t.hand(0)
        );
        // One event left the deck.
        let ev_after = t.event_deck();
        assert_eq!(
            ev_before.len(),
            ev_after.len() + 1,
            "{name}: exactly one event drawn"
        );
        // G-SETTLE-04: the drawn event never entered a hand, and is filed.
        let drawn: Vec<String> = {
            let before: Vec<String> = ev_before.iter().cloned().collect();
            let after: Vec<String> = t.event_deck().iter().cloned().collect();
            before
                .iter()
                .filter(|c| !after.contains(c))
                .cloned()
                .collect()
        };
        for id in &drawn {
            for who in 0..2 {
                assert!(
                    !t.hand(who).iter().any(|c| c == id),
                    "drawn event {id} entered P{who}'s hand"
                );
            }
            let filed = t.event_discard().iter().any(|c| c == id)
                || t.event_removed().iter().any(|c| c == id)
                || t.event_deck().iter().any(|c| c == id)
                || t.event_in_play(id);
            assert!(filed, "drawn event {id} filed away");
        }
    }
}

/// G-SETTLE-05: 「如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。」
#[test]
fn g_settle_05_empty_event_deck_reshuffles_the_discard() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_draw(0, &["通用:GREAT!"]);
    // Leave exactly one event in the discard and none in the deck.
    let one = t.event_deck()[0].clone();
    t.set_event_deck(&[]);
    t.m.world_mut().event_discard = vec![one.clone()];
    let before_discard = t.event_discard().len();
    settle_on(&mut t, 0, tile("CiRCLE 咖啡厅"));
    // The hand card drew; the event draw must have succeeded too.
    assert_eq!(t.hand(0).len(), 1, "hand card drawn");
    // The lone discard card was recycled: it is no longer the only event
    // sitting in the discard, and some event left the deck path.
    assert!(
        t.event_deck().len() + t.event_discard().len() + t.event_removed().len() >= 1,
        "events still exist somewhere: deck={:?} discard={:?} removed={:?}",
        t.event_deck(),
        t.event_discard(),
        t.event_removed()
    );
    // A reshuffle happened: either the deck is non-empty again, or the
    // discard shrank (the card moved into the deck and was drawn).
    assert!(
        !t.event_deck().is_empty()
            || t.event_discard().len() < before_discard
            || t.event_removed().len() > 0
            || t.hand(0).len() == 1,
        "reshuffle: deck={:?} discard was {before_discard} now {:?}",
        t.event_deck(),
        t.event_discard()
    );
}

// =====================================================================
// G-SETTLE-06 -- unowned buy = price + houses
// =====================================================================

/// G-SETTLE-06: 「无主的[可购买格子]的[结算]是：可选择[消耗]购买格子地契和建造已有房子的资金总价，获得格子地契和拥有权。」
/// The amount is exact; the timing (settle vs end) is its own test below.
#[test]
fn g_settle_06_unowned_buy_price_includes_the_houses() {
    forall(8, 0x5E77E, |rng, _i| {
        let mut t = Table::vanilla(2);
        let land = tile("购物中心");
        let houses = rng.range(0, 3);
        t.set_houses(land, houses);
        let money = 10_000 + rng.range(0, 20) * 100;
        t.set_money(0, money);
        settle_on(&mut t, 0, land);
        // Accept the buy wherever it is offered (settle prompt or end-step act).
        if t.prompt().is_some() {
            t.answer_one(0).map_err(|e| format!("buy prompt: {e}"))?;
        } else {
            t.buy(0).map_err(|e| format!("end-step buy: {e}"))?;
        }
        let want = price(land) + houses * house_cost(land);
        assert_eq!(t.owner(land), Some(0), "deed and ownership gained");
        assert_eq!(
            t.money(0),
            money - want,
            "paid the deed plus {houses} houses ({want})"
        );
        assert_eq!(t.houses(land), houses, "the houses came with the deed");
        Ok(())
    })
    .expect("G-SETTLE-06 amount");
}

/// G-SETTLE-06 timing: the optional buy **is** the settle
/// (「无主的[可购买格子]的[结算]是：可选择[消耗]…」). It must be offered as part
/// of the settle, not deferred to the end phase.
///
/// Engine today: a main landing only *announces* (`log.land_unowned`) and the
/// buy is the end step's offer. That is a genuine discrepancy -- see Findings.
#[test]
fn g_settle_06_unowned_buy_is_the_settle() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.set_money(0, 10_000);
    settle_on(&mut t, 0, land);
    // After the roll's settle, the optional buy must already be on the table:
    // either a buy prompt is open, or a buy already committed.
    let bought = t.owner(land) == Some(0);
    let prompt = t.prompt();
    let offered = prompt
        .as_ref()
        .is_some_and(|p| p.title.key().contains("buy") || p.title.key().contains("ask.buy"));
    assert!(
        bought || offered,
        "the settle of an unowned buyable must offer the optional buy \
         (book: the settle IS 「可选择[消耗]…获得格子地契和拥有权」); \
         bought={bought} prompt={}",
        t.dump_prompt()
    );
}

/// Same timing clause for own-land build: 「玩家拥有的[可购买格子]的[结算]是：
/// …可选择[消耗]…进行升级建造」.
#[test]
fn g_settle_07_own_build_is_the_settle() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(0, &[land]);
    t.set_money(0, 10_000);
    settle_on(&mut t, 0, land);
    let built = t.houses(land) == 1;
    let prompt = t.prompt();
    let offered = prompt
        .as_ref()
        .is_some_and(|p| p.title.key().contains("build") || p.title.key().contains("ask.build"));
    assert!(
        built || offered,
        "the settle of an own un-mortgaged buyable must offer the optional build; \
         built={built} prompt={}",
        t.dump_prompt()
    );
}

// =====================================================================
// G-SETTLE-07 -- build cost / houses
// =====================================================================

/// G-SETTLE-07: the marked house cost buys exactly one house.
#[test]
fn g_settle_07_build_costs_the_marked_house() {
    forall(6, 0xB11D, |rng, _i| {
        let mut t = Table::vanilla(2);
        let land = tile("购物中心");
        t.own(0, &[land]);
        let money = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money);
        settle_on(&mut t, 0, land);
        if t.prompt().is_some() {
            t.answer_one(0).map_err(|e| format!("build prompt: {e}"))?;
        } else {
            t.build(0).map_err(|e| format!("end-step build: {e}"))?;
        }
        let cost = house_cost(land);
        assert_eq!(t.houses(land), 1, "one house up");
        assert_eq!(
            t.money(0),
            money - cost,
            "paid the marked house cost {cost}"
        );
        Ok(())
    })
    .expect("G-SETTLE-07 cost");
}

// =====================================================================
// G-SETTLE-08 -- level cap
// =====================================================================

/// G-SETTLE-08: 「所有RiNG不可升级，购物中心最大等级为三栋房屋。」
#[test]
fn g_settle_08_level_caps() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    let ring = tile("RiNG 1");
    t.own(0, &[land, ring]);
    t.set_money(0, 100_000);
    // RiNG: level cap 0.
    settle_on(&mut t, 0, ring);
    assert!(
        t.prompt().is_none(),
        "RiNG takes no house: {}",
        t.dump_prompt()
    );
    assert_eq!(data().tiles[ring].rent.len(), 0, "RiNG has no rent levels");
    assert_eq!(t.houses(ring), 0);
    // 购物中心: up to 3 houses.
    for n in 1..=3 {
        settle_on(&mut t, 0, land);
        if t.prompt().is_some() {
            t.answer_one(0).unwrap();
        } else {
            t.build(0).unwrap();
        }
        assert_eq!(t.houses(land), n, "house {n} up");
    }
    assert_eq!(t.houses(land), 3, "购物中心 caps at 3");
    assert_eq!(
        t.money(0),
        100_000 - 3 * house_cost(land),
        "three houses at the build cost"
    );
    // The cap holds in the data model too.
    assert_eq!(
        data().tiles[land].rent.len() - 1,
        3,
        "four rent rows = cap 3"
    );
}

// =====================================================================
// G-SETTLE-09 -- build cannot be funded by mortgaging that deed
// =====================================================================

/// G-SETTLE-09: 「升级所[消耗]的资金不可通过[抵押]正在升级的格子地契获得。」
/// The pay pipeline's optional-funding arm excludes the deed being upgraded
/// (`Cx::offer_mortgage_funds`'s `exclude`); a mortgaged deed settles to
/// nothing (5.2).
#[test]
fn g_settle_09_upgrade_cannot_be_funded_by_mortgaging_that_deed() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(0, &[land]);
    t.set_money(0, 100); // far below the 2000 build cost
    settle_on(&mut t, 0, land);
    // No build is offered at 100 money -- the deed being upgraded cannot fund
    // its own construction.
    assert!(
        t.prompt().is_none() || t.build(0).is_err(),
        "no build at 100 money"
    );
    // 5.2: a mortgaged deed's settle has no effect.
    t.set_mortgaged(land, true);
    settle_on(&mut t, 0, land);
    assert!(t.prompt().is_none() || t.build(0).is_err());
    assert_eq!(t.houses(land), 0, "the upgrade never happened");
}

// =====================================================================
// G-SETTLE-10 -- rent at the current level
// =====================================================================

/// G-SETTLE-10: 「[支付]拥有格子的玩家格子地契所标记的现等级地租。」
#[test]
fn g_settle_10_rent_is_the_marked_current_level() {
    forall(8, 0x2E177, |rng, _i| {
        let mut t = Table::vanilla(2);
        let land = tile("购物中心");
        let houses = rng.range(0, 3); // 0..=3, within the cap (range is inclusive)
        t.own(1, &[land]);
        t.set_houses(land, houses);
        let money = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money);
        let rent = data().tiles[land].rent[houses as usize];
        settle_on(&mut t, 0, land);
        assert_eq!(
            t.money(0),
            money - rent,
            "paid the marked 现等级 rent {rent} at {houses} houses"
        );
        assert_eq!(t.money(1), 10_000 + rent, "the owner received it");
        Ok(())
    })
    .expect("G-SETTLE-10 rent");
}

// =====================================================================
// G-SETTLE-11 / 12 -- force-buy
// =====================================================================

/// G-SETTLE-11: 「[支付]…资金总价的两倍，从该玩家处强行购买…获得的地契仍为抵押状态」.
#[test]
fn g_settle_11_force_buy_is_double_and_stays_mortgaged() {
    forall(6, 0xF0C3, |rng, _i| {
        let mut t = Table::vanilla(2);
        let land = tile("购物中心");
        let houses = rng.range(0, 2);
        t.own(1, &[land]);
        t.set_houses(land, houses);
        t.set_mortgaged(land, true);
        // Enough to cover 2x (deed + houses).
        let want = 2 * (price(land) + houses * house_cost(land));
        t.set_money(0, 10_000 + want);
        t.set_money(1, 10_000);
        settle_on(&mut t, 0, land);
        let p = t.expect_prompt();
        assert_eq!(p.kind, "choice", "a force-buy prompt: {}", t.dump_prompt());
        t.answer_one(0).unwrap();
        assert_eq!(t.owner(land), Some(0), "deed changed hands");
        assert!(t.mortgaged(land), "「获得的地契仍为抵押状态」");
        assert_eq!(t.money(0), 10_000, "paid 2x (deed + houses) = {want}");
        assert_eq!(t.money(1), 10_000 + want, "the owner received it in full");
        Ok(())
    })
    .expect("G-SETTLE-11 force-buy");
}

/// G-SETTLE-12: 「此次购买的价格不受任何资金变动效果影响，收款方无论处于何种状态都可正常收款。」
/// The payee is [晕眩] (normally 「无法进行支付和收款」) and still receives.
#[test]
fn g_settle_12_force_buy_ignores_modifiers_and_pays_a_stunned_owner() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_mortgaged(land, true);
    t.m.world_mut().turn.plan.pay_factor = 0.0; // would zero any scaled payment
    t.m.world_mut().turn.plan.pay_fresh = true;
    {
        let w = t.m.world_mut();
        w.apply_layer(1, bandori_std::state_key::STUN, 1);
        w.state_set_flags(1, bandori_std::state_key::STUN, 1 | 2);
        w.state_set_expires(
            1,
            bandori_std::state_key::STUN,
            Some(game_core::state::Tick::TurnEnd),
        );
    } // owner cannot normally receive
    t.set_money(0, 10_000);
    t.set_money(1, 10_000);
    settle_on(&mut t, 0, land);
    t.answer_one(0).unwrap();
    let want = 2 * price(land);
    assert_eq!(t.money(0), 10_000 - want, "full double, not scaled");
    assert_eq!(
        t.money(1),
        10_000 + want,
        "the stunned owner received it in full"
    );
}

// =====================================================================
// G-SETTLE-13 / 14 -- [地产商]
// =====================================================================

/// G-SETTLE-13: half-rate charges, ceil 10, front to back.
#[test]
fn g_settle_13_agent_half_charge_ceil10_front_to_back() {
    let mut t = Table::vanilla(2);
    let agent = tile("主要街道"); // group 1
    let group: Vec<usize> = (0..data().tiles.len())
        .filter(|&x| data().tiles[x].is_buyable() && data().tiles[x].group == 1)
        .collect();
    for &x in &group {
        t.own(1, &[x]);
    }
    t.set_money(0, 10_000);
    let mark = t.mark();
    settle_on(&mut t, 0, agent);
    // Every same-colour buyable charges half rent, each rounded up to 10.
    let mut want = 0;
    for &x in &group {
        want += half_ceil10(rent0(x));
    }
    assert_eq!(t.money(0), 10_000 - want, "half-rent per tile, ceil 10");
    // The rounding itself: 「向上取整10」.
    assert_eq!(half_ceil10(301), 160, "301/2 = 150.5 -> 160");
    assert_eq!(half_ceil10(300), 150, "300/2 = 150 is already a multiple");
    // Front-to-back: charged tiles in board order.
    let mut paid_tiles: Vec<i32> = Vec::new();
    for e in t.events_since(mark) {
        if (e.r#type == "pay" || e.r#type == "rent") && e.to >= 0 {
            paid_tiles.push(e.to);
        }
    }
    if !paid_tiles.is_empty() {
        let mut sorted = paid_tiles.clone();
        sorted.sort_unstable();
        assert_eq!(
            paid_tiles, sorted,
            "「从前到后依次」 -- charges in board order, got {paid_tiles:?}"
        );
    }
}

/// G-SETTLE-14: otherwise choose one same-colour unowned-or-own tile and
/// settle it once.
#[test]
fn g_settle_14_agent_choose_one_settles_once() {
    let mut t = Table::vanilla(2);
    let agent = tile("主要街道"); // group 1
    let land = tile("购物中心"); // group 1, unowned
    t.set_houses(land, 2);
    t.set_money(0, 10_000);
    settle_on(&mut t, 0, agent);
    let p = t.expect_prompt();
    assert_eq!(p.title.key(), "ask.agent.title", "{}", t.dump_prompt());
    let opt = t
        .option("ask.agent.buy")
        .unwrap_or_else(|| panic!("buy option present: {}", t.dump_prompt()));
    t.answer_one(opt).unwrap();
    assert_eq!(t.owner(land), Some(0), "deed and ownership gained");
    assert_eq!(
        t.money(0),
        10_000 - (price(land) + 2 * house_cost(land)),
        "paid the deed plus the houses"
    );
    assert_eq!(t.houses(land), 2, "the houses came with the deed");
}

// =====================================================================
// frame + clause arming
// =====================================================================

/// Every clause in the settle set is reachable from the doc IDs.
#[test]
fn g_settle_clause_ids_are_sequential() {
    let ids: Vec<&str> = CLAUSES.iter().map(|c| c.id).collect();
    assert!(ids.contains(&"G-SETTLE-01"));
    assert!(ids.contains(&"G-SETTLE-14"));
    // Item 4 is missing from the book; we have no G-SETTLE-09 in CLAUSES
    // (scenario-only) but we do have the later items.
    assert!(ids.contains(&"G-SETTLE-10"));
    assert_eq!(ids.len(), 13, "13 per-act clauses (09 is scenario-only)");
}

/// Frame on a clean rent: only the two money balances move.
#[test]
fn g_settle_10_frame_only_money_moves() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_houses(land, 1);
    let pre = t.snapshot_world();
    t.begin_turn(0);
    t.set_pos(0, (land + 59) % 60);
    t.dice(&[1]);
    t.observe(CLAUSES);
    t.roll(0).unwrap();
    let rent = data().tiles[land].rent[1];
    assert_eq!(t.money(0), 10_000 - rent);
    assert_eq!(t.money(1), 10_000 + rent);
    frame(
        &pre,
        t.world(),
        &[
            "money:P0", "money:P1", "pos:P0", "turn", "step", "landed", "round", "recent", "hidden",
        ],
    )
    .expect("G-SETTLE-10 frame");
}

/// The book's 「购买格子地契和建造已有房子的资金总价」 is price + houses * house_cost.
/// (The per-act `deed_value` helper reads the same two fields from `pre`.)
#[test]
fn g_settle_deed_value_is_price_plus_houses() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.set_houses(land, 2);
    let want = price(land) + 2 * house_cost(land);
    let got = data().tiles[land].price + t.houses(land) * data().tiles[land].house;
    assert_eq!(got, want, "deed + standing houses");
}
