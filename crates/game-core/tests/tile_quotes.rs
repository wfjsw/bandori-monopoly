//! `Match::tile_quotes` -- the per-viewer board-caption figures.
//!
//! One quote per buyable tile, from the viewer's seat (or the seat-independent
//! reading for a spectator / unknown member). See `purchase::tile_quotes`.

use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::purchase::{self, BuyQuery, Quote};
use game_core::engine::rules::CardRules;
use game_core::engine::{Match, StubRules, World};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{prop, FieldCard, MoneyFlow, TileQuote, TileQuoteKind};
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string())).unwrap(),
    )
}

fn members(n: i32) -> Vec<RoomMember> {
    (1..=n)
        .map(|i| RoomMember {
            id: i,
            player: format!("P{i}"),
            bot: true,
            ..Default::default()
        })
        .collect()
}

fn solo(seed: u64, rules: Arc<dyn CardRules>) -> Match {
    let mut m = Match::new(
        data(),
        rules,
        &members(3),
        seed,
        MatchMode::Solo,
        ScoreWeights::default(),
    );
    m.quick_start();
    m
}

fn stub(seed: u64) -> Match {
    solo(seed, Arc::new(StubRules))
}

/// The seat `member` is sitting at. Setup shuffles seats, so never assume
/// member 1 is player 0.
fn seat(m: &Match, member: i32) -> i32 {
    let s = m.state().player_of(member);
    assert!(s >= 0, "member {member} has a seat");
    s
}

/// Some other seated member (not `member`).
fn other_member(m: &Match, member: i32) -> i32 {
    m.state()
        .players
        .iter()
        .map(|p| p.member)
        .find(|&id| id != member)
        .expect("another seat")
}

fn first_tile(f: impl Fn(&game_core::data::TileData) -> bool) -> usize {
    let d = data();
    d.tiles.iter().position(f).expect("tile exists")
}

/// A property with a real build ladder (`rent.len() >= 2`).
fn prop_tile() -> usize {
    first_tile(|t| t.kind == "property" && t.rent.len() >= 2)
}

fn ring_tile() -> usize {
    first_tile(|t| t.kind == "ring")
}

fn all_rings() -> Vec<usize> {
    let d = data();
    (0..d.tiles.len())
        .filter(|&i| d.tiles[i].kind == "ring")
        .collect()
}

/// Tile `t`'s quote for `member`, or panic with the whole vector for debugging.
fn q(m: &Match, member: i32, t: usize) -> TileQuote {
    let qs = m.tile_quotes(member);
    qs.get(t)
        .and_then(|x| x.as_ref().copied())
        .unwrap_or_else(|| panic!("tile {t} has a quote: {:?}", qs))
}

#[test]
fn unowned_shows_the_buy_price() {
    let m = stub(1);
    let t = prop_tile();
    let tile = &data().tiles[t];
    let qq = q(&m, 1, t);
    assert_eq!(qq.kind, TileQuoteKind::Buy);
    assert_eq!(qq.flow, MoneyFlow::MayPay);
    assert_eq!(qq.value, tile.price, "land price, no houses standing");
    assert_eq!(qq.max, None);
}

#[test]
fn mine_with_a_next_building_shows_the_build_cost() {
    let mut m = stub(2);
    let t = prop_tile();
    let me = seat(&m, 1);
    m.world_mut().st.owners[t] = me;
    let house = data().tiles[t].house;
    let qq = q(&m, 1, t);
    assert_eq!(qq.kind, TileQuoteKind::Build);
    assert_eq!(qq.flow, MoneyFlow::MayPay);
    assert_eq!(qq.value, house);
}

#[test]
fn mine_at_the_build_cap_shows_own_rent() {
    let mut m = stub(3);
    let t = prop_tile();
    let tile = &data().tiles[t];
    let cap = tile.rent.len() - 1;
    let me = seat(&m, 1);
    m.world_mut().st.owners[t] = me;
    m.world_mut().st.houses[t] = cap as i32;
    let qq = q(&m, 1, t);
    assert_eq!(qq.kind, TileQuoteKind::OwnRent);
    assert_eq!(qq.flow, MoneyFlow::Receive);
    assert_eq!(qq.value, tile.rent[cap], "capped rent, collected");
    assert_eq!(qq.max, None);
}

#[test]
fn mine_on_a_ring_shows_own_rent_because_rings_cannot_upgrade() {
    let mut m = stub(4);
    let t = ring_tile();
    let me = seat(&m, 1);
    // Own every RiNG so the dice range is the full-board one.
    for &r in &all_rings() {
        m.world_mut().st.owners[r] = me;
    }
    let qq = q(&m, 1, t);
    assert_eq!(qq.kind, TileQuoteKind::OwnRent);
    assert_eq!(qq.flow, MoneyFlow::Receive);
    let mult = data().match_rules.ring_multiplier.max(1);
    let n = all_rings().len() as i32;
    assert_eq!(qq.value, n * mult, "min of rings x mult x 1d20");
    assert_eq!(qq.max, Some(n * mult * 20));
}

#[test]
fn others_tile_shows_the_rent_id_pay() {
    let mut m = stub(5);
    let t = prop_tile();
    let tile = &data().tiles[t];
    let other = seat(&m, other_member(&m, 1));
    m.world_mut().st.owners[t] = other;
    m.world_mut().st.houses[t] = 1;
    let qq = q(&m, 1, t);
    assert_eq!(qq.kind, TileQuoteKind::Rent);
    assert_eq!(qq.flow, MoneyFlow::MustPay);
    assert_eq!(qq.value, tile.rent[1]);
}

#[test]
fn rent_houses_override_is_honoured() {
    let mut m = stub(6);
    let t = prop_tile();
    let tile = &data().tiles[t];
    let other_member = other_member(&m, 1);
    let other = seat(&m, other_member);
    m.world_mut().st.owners[t] = other;
    m.world_mut().st.houses[t] = 0;
    // 「房屋数视为 2」 on the owner's field.
    let mut fc = FieldCard::default();
    fc.owner = other;
    fc.user = other;
    fc.props.insert(prop::RENT_HOUSES.to_string(), 2);
    m.world_mut().st.players[other as usize].field.push(fc);
    let qq = q(&m, 1, t);
    assert_eq!(qq.kind, TileQuoteKind::Rent);
    assert_eq!(
        qq.value, tile.rent[2],
        "counted houses, not the standing ones"
    );
}

#[test]
fn ring_rent_is_a_dice_range() {
    let mut m = stub(7);
    let t = ring_tile();
    let other = seat(&m, other_member(&m, 1));
    m.world_mut().st.owners[t] = other;
    let qq = q(&m, 1, t);
    assert_eq!(qq.kind, TileQuoteKind::Rent);
    assert_eq!(qq.flow, MoneyFlow::MustPay);
    let mult = data().match_rules.ring_multiplier.max(1);
    assert_eq!(qq.value, mult, "1 ring: mult x 1d20 min");
    assert_eq!(qq.max, Some(mult * 20));
}

#[test]
fn mortgaged_others_shows_the_force_buy_price() {
    let mut m = stub(8);
    let t = prop_tile();
    let tile = &data().tiles[t];
    let other = seat(&m, other_member(&m, 1));
    m.world_mut().st.owners[t] = other;
    m.world_mut().st.mortgaged[t] = true;
    let qq = q(&m, 1, t);
    assert_eq!(qq.kind, TileQuoteKind::ForceBuy);
    assert_eq!(qq.flow, MoneyFlow::MayPay);
    assert_eq!(qq.value, 2 * tile.price, "2x deed value");
}

#[test]
fn mortgaged_mine_shows_the_redeem_price() {
    // Decision: force-buy does not apply to one's own deed; the natural cost
    // is the redeem price (60% of the land price, ties even).
    let mut m = stub(9);
    let t = prop_tile();
    let tile = &data().tiles[t];
    let me = seat(&m, 1);
    m.world_mut().st.owners[t] = me;
    m.world_mut().st.mortgaged[t] = true;
    let qq = q(&m, 1, t);
    assert_eq!(qq.kind, TileQuoteKind::Redeem);
    assert_eq!(qq.flow, MoneyFlow::MayPay);
    assert_eq!(qq.value, (tile.price as f64 * 0.6).round_ties_even() as i32);
}

#[test]
fn spectator_sees_the_seat_independent_reading() {
    let mut m = stub(10);
    let t = prop_tile();
    let tile = &data().tiles[t];
    // Unowned: base land price, no player-scoped modifiers.
    let qq = q(&m, 99, t);
    assert_eq!(qq.kind, TileQuoteKind::Buy);
    assert_eq!(qq.value, tile.price);

    // Owned by someone: the rent a visitor would pay (MustPay).
    let other = seat(&m, other_member(&m, 1));
    m.world_mut().st.owners[t] = other;
    let qq = q(&m, 99, t);
    assert_eq!(qq.kind, TileQuoteKind::Rent);
    assert_eq!(qq.flow, MoneyFlow::MustPay);

    // Mortgaged: the force-buy list price.
    m.world_mut().st.mortgaged[t] = true;
    let qq = q(&m, 99, t);
    assert_eq!(qq.kind, TileQuoteKind::ForceBuy);
    assert_eq!(qq.value, 2 * tile.price);
}

#[test]
fn a_buy_price_modifier_is_reflected() {
    /// A ruleset whose `buy_quote` takes 1000 off every quote -- the shape of
    /// a `BuyAdd` stage, enough to prove the caption rides `rules.buy_quote`
    /// and not a UI re-derivation.
    struct DiscountRules;
    impl CardRules for DiscountRules {
        fn play(
            &self,
            cx: &mut game_core::engine::Cx,
            player_id: usize,
            card: &str,
        ) -> game_core::engine::Flow<game_core::engine::Dest> {
            StubRules.play(cx, player_id, card)
        }
        fn event(
            &self,
            cx: &mut game_core::engine::Cx,
            player_id: usize,
            id: &str,
        ) -> game_core::engine::Flow<bool> {
            StubRules.event(cx, player_id, id)
        }
        fn buy_quote(&self, w: &World, d: &GameData, q: &BuyQuery) -> Vec<Quote> {
            q.tiles
                .iter()
                .map(|&t| {
                    let base = purchase::base_quote(d, &w.st, t, q.kind);
                    Quote {
                        price: (base - 1000).max(0),
                        eligible: true,
                    }
                })
                .collect()
        }
    }

    let m = solo(11, Arc::new(DiscountRules));
    let t = prop_tile();
    let tile = &data().tiles[t];
    let qq = q(&m, 1, t);
    assert_eq!(qq.kind, TileQuoteKind::Buy);
    assert_eq!(qq.value, tile.price - 1000, "BuyAdd-style discount lands");

    // Force-buy goes through the same quote path.
    let mut m = solo(11, Arc::new(DiscountRules));
    let other = seat(&m, other_member(&m, 1));
    m.world_mut().st.owners[t] = other;
    m.world_mut().st.mortgaged[t] = true;
    let qq = q(&m, 1, t);
    assert_eq!(qq.kind, TileQuoteKind::ForceBuy);
    assert_eq!(qq.value, (2 * tile.price) - 1000);
}

#[test]
fn non_buyable_tiles_carry_no_quote() {
    let m = stub(12);
    let d = data();
    let qs = m.tile_quotes(1);
    for (i, tile) in d.tiles.iter().enumerate() {
        if tile.is_buyable() {
            continue;
        }
        assert!(
            qs.get(i).and_then(|x| x.as_ref()).is_none(),
            "tile {} ({}) is not a quote tile",
            i,
            tile.kind
        );
    }
}