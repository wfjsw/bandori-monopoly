//! Strategy book format, back-off lookup and plumbing (`docs/BOT.md` §3.8 S1).
//!
//! The gate for S1 is byte-identical behaviour with the empty book (bar the
//! [反击] default, which user ruling 2026-10-08 moved from "never declare" to
//! `DEFAULT_COUNTERACT_PROPENSITY_MILLI` -- "bots must be able to counteract");
//! the tests below pin the lookup contract, the stale-book fallback, the
//! partial-params merge, and that a live parameter change is actually read by
//! the standard bot (the plumbing proof).

use std::collections::BTreeMap;
use std::sync::Arc;

use game_core::data::{CharacterData, GameData};
use game_core::engine::{Match, StubRules, BUY_RESERVE};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::strategy::{
    self, CardPlayParams, CounterParams, SkillParams, StrategyBook, StrategyEntry, StrategyParams,
    DEFAULT_COUNTERACT_PROPENSITY_MILLI, PARAMS_VERSION, POLICY_STANDARD, STRATEGY_BOOK_VERSION,
};
use game_core::MatchMode;

fn data() -> GameData {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
        .expect("web/data should load -- run tools/asset-pipe/extract.py")
}

fn me<'a>(d: &'a GameData) -> &'a CharacterData {
    d.character("户山香澄").unwrap()
}

/// A parameter set that only names `field`, everything else default.
fn params_with(f: impl FnOnce(&mut StrategyParams)) -> StrategyParams {
    let mut p = StrategyParams::default();
    f(&mut p);
    p
}

fn entry(me: &str, band: &str, opponent_bands: &[&str], params: StrategyParams) -> StrategyEntry {
    StrategyEntry {
        me: me.into(),
        band: band.into(),
        opponent_bands: opponent_bands.iter().map(|s| s.to_string()).collect(),
        deck: Vec::new(),
        params,
    }
}

fn book(entries: Vec<StrategyEntry>) -> StrategyBook {
    let mut b = StrategyBook::default();
    b.version = STRATEGY_BOOK_VERSION;
    b.ruleset_sha256 = "stub".into();
    b.policy = POLICY_STANDARD.into();
    b.generated_at = "test".into();
    b.params_version = PARAMS_VERSION;
    for e in entries {
        if !e.me.is_empty() && !e.opponent_bands.is_empty() {
            b.char_bands.push(e);
        } else if !e.me.is_empty() {
            b.me.push(e);
        } else {
            b.band.push(e);
        }
    }
    b
}

// ---------------------------------------------------------------- defaults

/// Every default equals today's constant in `engine/ai.rs` -- an empty book
/// must be byte-identical to the pre-book policy.
#[test]
fn defaults_equal_todays_constants() {
    let p = StrategyParams::default();
    assert_eq!(p.buy_reserve, BUY_RESERVE);
    assert_eq!(p.buy_reserve_mid, BUY_RESERVE);
    assert_eq!(p.buy_reserve_late, BUY_RESERVE);
    assert_eq!(p.build_reserve, 3_500);
    assert_eq!(p.redeem_reserve, 4_000);
    assert_eq!(p.force_buy_reserve, 4_000);
    assert_eq!(p.play_card_reserve, BUY_RESERVE);
    assert_eq!(p.play_card_chance_milli, 700);
    assert_eq!(p.max_plays_per_turn, 2);
    assert_eq!(p.auction_worth_lo_milli, 600);
    assert_eq!(p.auction_worth_span_milli, 700);
    assert_eq!(p.auction_cash_margin, 1_000);
    assert_eq!(p.bid_step, 100);
    assert_eq!(p.bid_nudge_steps, 3);
    assert_eq!(p.bid_frac_milli, 750);
    assert_eq!(p.mortgage_house_key_milli, 1_000);
    assert_eq!(p.mortgage_price_key_milli, 1_000);
    // Sparse maps are empty = neutral / the [反击] base rate (see below).
    assert!(p.cards.is_empty());
    assert!(p.skills.is_empty());
    assert!(p.counteract.is_empty());
    assert_eq!(p.card("anything"), CardPlayParams::default());
    assert_eq!(p.skill("anything"), SkillParams::default());
    // Ruling 2026-10-08 ("bots must be able to counteract"): the [反击] default
    // is the one default that is NOT one of the old constants -- it moved from
    // "never declare" to `DEFAULT_COUNTERACT_PROPENSITY_MILLI`.
    assert_eq!(
        p.counteract_propensity("anything", None),
        DEFAULT_COUNTERACT_PROPENSITY_MILLI
    );
    assert_eq!(
        p.counteract_propensity_milli,
        DEFAULT_COUNTERACT_PROPENSITY_MILLI
    );
}

/// The default params reproduce the old free functions and formulas exactly.
#[test]
fn default_params_are_the_old_formulas() {
    use game_core::engine::{wants_buy, wants_build, wants_redeem};
    let p = StrategyParams::default();
    for money in [0, 1_999, 2_000, 2_001, 10_000] {
        for price in [0, 1, 500, 2_000, 5_000] {
            assert_eq!(p.wants_buy(money, price), wants_buy(money, price));
            assert_eq!(p.wants_build(money, price), wants_build(money, price));
            assert_eq!(p.wants_redeem(money, price), wants_redeem(money, price));
            // Tile-aware: neutral group weight, no set bonus, no ratio cap.
            assert_eq!(p.wants_buy_tile(money, price, 7, 0, true), wants_buy(money, price));
            assert_eq!(p.wants_buy_tile(money, price, 7, 99, false), wants_buy(money, price));
            assert_eq!(p.wants_build_tile(money, price, 7, 0), wants_build(money, price));
        }
    }
    // The auction formula is the old float expression, bit for bit.
    for roll in [0.0, 0.25, 0.5, 0.999] {
        for base in [0, 1, 999, 1_000, 2_400, 9_999] {
            for money in [0, 3_000, 10_000] {
                let old = ((base as f64 * (0.6 + roll * 0.7) / 100.0) as i32) * 100;
                let old = old.min(money - 1000);
                assert_eq!(p.auction_worth(roll, base, money), old, "roll={roll} base={base}");
            }
        }
    }
    // The mortgage key is the old `(houses > 0, price, t)` lexicographic order.
    let mut a = vec![0usize, 1, 2];
    let houses = [0, 1, 0];
    let prices = [2_000, 100, 500];
    a.sort_by_key(|&t| p.mortgage_key(houses[t], prices[t], t));
    // Bare cheap (2) before bare expensive (0) before housed (1).
    assert_eq!(a, vec![2, 0, 1]);
    let mut old = a.clone();
    old.sort_by_key(|&t| (houses[t] > 0, prices[t], t));
    assert_eq!(a, old);
}

// ---------------------------------------------------------------- lookup

#[test]
fn lookup_backs_off_in_order() {
    let d = data();
    let c = me(&d); // 户山香澄, Poppin' Party
    let char_bands = params_with(|p| p.buy_reserve = 1_111);
    let me_only = params_with(|p| p.buy_reserve = 2_222);
    let band = params_with(|p| p.buy_reserve = 3_333);
    let b = book(vec![
        entry("户山香澄", "", &["Afterglow"], char_bands.clone()),
        entry("户山香澄", "", &[], me_only.clone()),
        entry("", "Poppin' Party", &[], band.clone()),
    ]);
    let lookup = |who: &str, opponents: &[&str]| {
        let opp: Vec<String> = opponents.iter().map(|s| s.to_string()).collect();
        let c = d.character(who).unwrap();
        b.lookup(&d, c, &opp, None, Some("stub"))
    };

    // 1. (me, opponents' bands multiset)
    assert_eq!(lookup("户山香澄", &["美竹兰"]), Some(char_bands));
    // 2. same character, other bands.
    assert_eq!(lookup("户山香澄", &["花园多惠"]), Some(me_only));
    // 3. band level: another Poppin' Party character the `me` levels do not name.
    assert_eq!(lookup("花园多惠", &["美竹兰"]), Some(band));
    // 4. defaults: an Afterglow character the `me` levels do not name.
    assert_eq!(lookup("美竹兰", &["花园多惠"]), None);
    // Resolve with nothing matching at all falls to the defaults.
    let empty = StrategyBook::default();
    let opp: Vec<String> = vec!["美竹兰".to_string()];
    assert_eq!(
        empty.resolve(&d, c, &opp, None, Some("stub")),
        StrategyParams::default()
    );
}

#[test]
fn first_hit_within_a_level_wins() {
    let d = data();
    let c = me(&d);
    let first = params_with(|p| p.buy_reserve = 1);
    let second = params_with(|p| p.buy_reserve = 2);
    let b = book(vec![
        entry("户山香澄", "", &[], first.clone()),
        entry("户山香澄", "", &[], second),
    ]);
    assert_eq!(b.lookup(&d, c, &[], None, Some("stub")), Some(first));
}

// ---------------------------------------------------------------- validity

#[test]
fn stale_hash_policy_and_params_version_ignore_the_book() {
    let d = data();
    let c = me(&d);
    let p = params_with(|p| p.buy_reserve = 42);
    let opp: Vec<String> = vec!["美竹兰".to_string()];

    // Stale ruleset hash: the whole book is ignored.
    let mut b = book(vec![entry("户山香澄", "", &[], p.clone())]);
    b.ruleset_sha256 = "some-other-ruleset".into();
    assert_eq!(b.lookup(&d, c, &opp, None, Some("stub")), None);
    assert_eq!(b.resolve(&d, c, &opp, None, Some("stub")), StrategyParams::default());

    // Policy mismatch: same story.
    let mut b = book(vec![entry("户山香澄", "", &[], p.clone())]);
    b.policy = "chaos".into();
    assert_eq!(b.lookup(&d, c, &opp, None, Some("stub")), None);

    // A different parameter schema: same story.
    let mut b = book(vec![entry("户山香澄", "", &[], p.clone())]);
    b.params_version = PARAMS_VERSION + 1;
    assert_eq!(b.lookup(&d, c, &opp, None, Some("stub")), None);

    // A caller without a ruleset in hand only skips the hash check.
    let ok = book(vec![entry("户山香澄", "", &[], p.clone())]);
    assert_eq!(ok.lookup(&d, c, &opp, None, None), Some(p));
    let mut b = book(vec![entry("户山香澄", "", &[], params_with(|p| p.buy_reserve = 42))]);
    b.policy = "chaos".into();
    assert_eq!(b.lookup(&d, c, &opp, None, None), None);
}

#[test]
fn an_empty_book_is_inert_whatever_it_claims() {
    let d = data();
    let mut b = book(vec![]);
    b.ruleset_sha256 = "nonsense".into();
    assert!(b.is_empty());
    assert_eq!(
        b.resolve(&d, me(&d), &[], None, Some("stub")),
        StrategyParams::default()
    );
}

// ---------------------------------------------------------------- format

#[test]
fn parses_the_documented_shape_and_merges_partial_params() {
    let body = r#"{
      "version": 1,
      "ruleset_sha256": "stub",
      "policy": "standard",
      "generated_at": "2026-10-08",
      "params_version": 1,
      "char_bands": [
        {
          "me": "户山香澄",
          "opponent_bands": ["Afterglow"],
          "params": { "buy_reserve": 5000 }
        }
      ],
      "me": [
        { "me": "户山香澄", "params": { "build_reserve": 1, "play_card_chance_milli": 250 } }
      ],
      "band": [
        {
          "band": "Poppin' Party",
          "deck": ["a", "b"],
          "params": {
            "cards": { "卡": { "play_weight_milli": 0 } },
            "counteract": { "卡": { "propensity_milli": 1000 } }
          }
        }
      ]
    }"#;
    let b = StrategyBook::parse(body).unwrap();
    assert_eq!(b.version, STRATEGY_BOOK_VERSION);
    assert_eq!(b.params_version, PARAMS_VERSION);
    assert!(b.usable(Some("stub")));
    assert!(!b.usable(Some("other")));

    let d = data();
    let c = me(&d);
    // Partial params merge over the defaults: only the named fields move.
    let opp = vec!["美竹兰".to_string()]; // Afterglow
    let p = b.lookup(&d, c, &opp, None, Some("stub")).unwrap();
    assert_eq!(p.buy_reserve, 5_000);
    assert_eq!(p.build_reserve, 3_500, "untouched fields take the default");
    assert_eq!(p.redeem_reserve, 4_000);
    assert_eq!(p.play_card_chance_milli, 700);

    // The next level's partial params, likewise.
    let opp: Vec<String> = vec!["花园多惠".to_string()]; // Poppin' Party
    let p = b.lookup(&d, c, &opp, None, Some("stub")).unwrap();
    assert_eq!(p.build_reserve, 1);
    assert_eq!(p.play_card_chance_milli, 250);
    assert_eq!(p.buy_reserve, BUY_RESERVE);

    // A deck-tagged entry only applies to that deck. The band level names
    // Poppin' Party -- use a character the `me` levels do not.
    let band_c = d.character("花园多惠").unwrap();
    let any: Vec<String> = vec![];
    assert!(b
        .lookup(&d, band_c, &[], Some(&any), Some("stub"))
        .is_none());
    let named = vec!["a".to_string(), "b".to_string()];
    let p = b
        .lookup(&d, band_c, &[], Some(&named), Some("stub"))
        .expect("deck matches the entry");
    assert_eq!(p.card("卡").play_weight_milli, 0);
    assert_eq!(p.card("other").play_weight_milli, 1_000);
    assert_eq!(p.counteract_propensity("卡", None), 1_000);
    assert_eq!(
        p.counteract_propensity("other", None),
        DEFAULT_COUNTERACT_PROPENSITY_MILLI
    );
    // Unknown deck cannot disprove the pair.
    assert!(b.lookup(&d, band_c, &[], None, Some("stub")).is_some());

    assert!(StrategyBook::parse("{").is_err());
}

#[test]
fn the_shipped_book_is_empty_and_inert() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let text = std::fs::read_to_string(dir.join("strategy_book.json")).expect("shipped placeholder");
    let b = StrategyBook::parse(&text).unwrap();
    assert!(b.is_empty(), "S2/S3 have not derived entries yet");
    let d = data();
    assert_eq!(
        b.resolve(&d, me(&d), &[], None, Some("stub")),
        StrategyParams::default()
    );
}

// ---------------------------------------------------------------- plumbing

fn member(i: i32, bot: bool) -> RoomMember {
    RoomMember {
        id: i,
        player: format!("Bot{i}"),
        bot,
        ..Default::default()
    }
}

fn new_match(data: Arc<GameData>, seed: u64) -> Match {
    let members: Vec<RoomMember> = (1..=4).map(|i| member(i, true)).collect();
    let mut m = Match::new(
        data,
        Arc::new(StubRules),
        &members,
        seed,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    m
}

/// Count the end-step purchases a seeded game makes.
fn buy_count(data: Arc<GameData>, seed: u64) -> usize {
    let mut m = new_match(data, seed);
    let mut last = 0;
    let mut buys = 0;
    while !m.ended() {
        m.tick(0.25);
        let st = m.state();
        for e in m.events_since(last) {
            last = e.id;
            // Auction wins log as `buy` too; only the land purchase tests
            // `AiWantsBuy`.
            if e.r#type == "buy" && e.player_id >= 0 && !e.msg.to_string().contains("auction") {
                buys += 1;
            }
        }
        if st.round > 200 {
            m.finish();
        }
    }
    buys
}

/// An in-memory book that raises every seat's buy reserve to the sky makes the
/// standard bot never buy -- proving the engine heuristic reads the seat's
/// resolved `StrategyParams`, not a copy of the constants.
#[test]
fn a_live_parameter_change_moves_the_standard_bot() {
    let d = Arc::new(data());
    let baseline: usize = (1..=4).map(|s| buy_count(d.clone(), s)).sum();
    assert!(baseline > 0, "the default policy must buy something");

    // Same seeds, buy reserve (all three phases) so high no purchase can
    // leave it. Cover every character `quick_start` might deal out.
    let mut book = StrategyBook::default();
    book.version = STRATEGY_BOOK_VERSION;
    book.ruleset_sha256 = "stub".into();
    book.policy = POLICY_STANDARD.into();
    book.generated_at = "test".into();
    book.params_version = PARAMS_VERSION;
    for c in &d.characters {
        book.me.push(entry(
            &c.name,
            "",
            &[],
            params_with(|p| {
                p.buy_reserve = 1_000_000;
                p.buy_reserve_mid = 1_000_000;
                p.buy_reserve_late = 1_000_000;
            }),
        ));
    }
    let mut d2 = (*d).clone();
    d2.strategy_book = book;
    let d2 = Arc::new(d2);
    let blocked: usize = (1..=4).map(|s| buy_count(d2.clone(), s)).sum();
    assert_eq!(blocked, 0, "a 1 000 000 buy reserve must stop every purchase");
}

/// `strategy::for_seat` resolves from public state alone (own character + the
/// other seats' characters) -- the key the book is allowed to see.
#[test]
fn for_seat_reads_the_book_from_public_state() {
    let mut d = data();
    // One `me` entry per character, so whatever `quick_start` deals out hits.
    let mut b = StrategyBook::default();
    b.version = STRATEGY_BOOK_VERSION;
    b.ruleset_sha256 = "stub".into();
    b.policy = POLICY_STANDARD.into();
    b.params_version = PARAMS_VERSION;
    for c in &d.characters {
        b.me.push(entry(
            &c.name,
            "",
            &[],
            params_with(|p| p.buy_reserve = 7_777),
        ));
    }
    d.strategy_book = b;
    let m = new_match(Arc::new(d.clone()), 0);
    let st = m.state();
    for seat in 0..st.players.len() {
        let p = strategy::for_seat(&d, &st, seat);
        assert_eq!(p.buy_reserve, 7_777, "seat {seat} reads its own entry");
    }
}

/// The counteract propensity is per-card over a base rate. A listed card is
/// its own spec (`by_kind` first, then `propensity_milli`; 0 = hold it back);
/// an unlisted card takes the seat's `counteract_propensity_milli`.
#[test]
fn counteract_propensity_is_per_card_over_the_base_rate() {
    let p = params_with(|p| {
        p.counteract.insert(
            "反击卡".into(),
            CounterParams {
                propensity_milli: 400,
                by_kind: [("pay".to_string(), 900)].into_iter().collect(),
            },
        );
        p.counteract.insert(
            "扣着".into(),
            CounterParams {
                propensity_milli: 0,
                by_kind: BTreeMap::new(),
            },
        );
    });
    assert_eq!(p.counteract_propensity("反击卡", None), 400);
    assert_eq!(p.counteract_propensity("反击卡", Some("pay")), 900);
    assert_eq!(p.counteract_propensity("反击卡", Some("move")), 400);
    // Unlisted: the base rate (600‰, ruling 2026-10-08).
    assert_eq!(
        p.counteract_propensity("别的卡", None),
        DEFAULT_COUNTERACT_PROPENSITY_MILLI
    );
    // Listed at 0: held back even though the base rate would declare it.
    assert_eq!(p.counteract_propensity("扣着", None), 0);
    // The base rate is tunable.
    let tuned = params_with(|p| p.counteract_propensity_milli = 250);
    assert_eq!(tuned.counteract_propensity("别的卡", None), 250);
}