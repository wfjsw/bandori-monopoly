//! The bot **strategy book** (`docs/BOT.md` §3.8): a flat, versioned parameter
//! set per character / band, derived offline and looked up here.
//!
//! Every tunable constant the standard heuristic uses lives in
//! [`StrategyParams`], with a default equal to today's constant in
//! `engine/ai.rs` / `bot-core` -- so an empty book (the shipped placeholder)
//! changes nothing. The engine heuristic (standard mentality only; chaos is
//! fixed), the ISMCTS rollout policy / priors and the browser 托管 all read the
//! seat's resolved params instead of hard-coded thresholds.
//!
//! * **Key = public information only**, with back-off, first hit wins
//!   ([`StrategyBook::lookup`]): `(character, opponents' bands multiset)` →
//!   `(character)` → `(band)` → [`StrategyParams::default`].
//! * **Validity.** The book records the ruleset hash (the same string
//!   [`crate::record::EngineStamp::ruleset_sha256`] carries), the policy and
//!   [`PARAMS_VERSION`]. A mismatch at lookup ignores the whole book (once, on
//!   stderr): a stale book would silently mis-tune a changed ruleset.
//! * **Partial params merge over defaults.** An entry's `params` object may
//!   name only the fields it retunes; the rest take [`StrategyParams::default`]
//!   (`#[serde(default)]` on every field). Unknown fields are ignored.
//! * **Pure.** Lookup uses no RNG.
//! * **Not in `data_sha256`** -- like [`crate::deck_book`], it tunes bots, not
//!   the match, so replay stamps do not move with it.
//!
//! File: `data/strategy_book.json` (optional -- a missing or unparsable file is
//! an empty book). Field names are snake_case, like [`crate::deck_book`].

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::data::{CharacterData, GameData};
use crate::state::MatchState;

/// File format version (`StrategyBook::version`). Bump on any schema change; a
/// book with a different version is ignored entirely.
pub const STRATEGY_BOOK_VERSION: u32 = 1;

/// `StrategyParams` schema version (`StrategyBook::params_version`). A book
/// tuned against a different parameter layout is ignored: its field names /
/// meanings may not line up with this build's defaults.
pub const PARAMS_VERSION: u32 = 1;

/// The policy a book may be tuned for -- the standard bot / 托管 policy.
/// Chaos never reads the book.
pub use crate::deck_book::POLICY_STANDARD;

/// The file name `GameData::load` looks the book up under, relative to the
/// other `data/*.json` tables. Absent = empty book.
pub const STRATEGY_BOOK_FILE: &str = "strategy_book.json";

/// Neutral per-group / per-card weight (`1000` = 1.0, no preference). Every
/// sparse weight defaults to this, so an empty map is exactly today's uniform
/// behaviour.
pub const NEUTRAL_WEIGHT: i32 = 1_000;

/// Default [反击] declare propensity, in milli (`600` = 60 % per offered card
/// per offer; see [`StrategyParams::counteract_propensity`]).
///
/// User ruling 2026-10-08: *"bots must be able to counteract"* -- the old
/// default was `0` (never declare, the C# parity gap), which meant a standard
/// bot held every [反击] card forever. The rules expose no per-card counteract
/// usefulness value (`Card.AiPlay` is a play-window gate and `CardDef` has no
/// `H.AiPlay` hook yet), so the policy is this propensity, drawn from the
/// match RNG exactly like chaos's `CHAOS_COUNTER_CHANCE`. A card the book
/// wants held back gets an explicit `propensity_milli: 0`.
pub const DEFAULT_COUNTERACT_PROPENSITY_MILLI: i32 = 600;

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

/// Per-card card-play knobs (sparse map; absent = [`Self::default`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CardPlayParams {
    /// Selection weight in the play window. `1000` = neutral (every candidate
    /// equal, exactly today's uniform pick); `0` = never play this card from
    /// the heuristic.
    pub play_weight_milli: i32,
    /// Hold this card back from the 运营 play window (keep it for a [反击]
    /// window). Default false -- today's policy holds nothing back.
    pub hold_for_counteract: bool,
    /// Earliest round the heuristic will play it (`0` = any).
    pub min_round: i32,
    /// Latest round the heuristic will play it (`i32::MAX` = any).
    pub max_round: i32,
    /// Per-card cash floor after the card's `estCost`, replacing the global
    /// [`StrategyParams::play_card_reserve`] when set.
    pub min_cash_after_est: Option<i32>,
}

impl Default for CardPlayParams {
    fn default() -> Self {
        Self {
            play_weight_milli: NEUTRAL_WEIGHT,
            hold_for_counteract: false,
            min_round: 0,
            max_round: i32::MAX,
            min_cash_after_est: None,
        }
    }
}

/// Per-skill press knobs (sparse map; absent = [`Self::default`]).
///
/// The default is **never press** -- today's standard bot leaves skills to the
/// player (chaos presses every usable one and is not parameterised).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillParams {
    /// Press weight in the skill window. `0` = never (today's standard
    /// policy); `1000` = always when usable; in between = odds per offer.
    pub play_weight_milli: i32,
    /// Press only with at least this many fires in hand / on the field
    /// (`0` = no threshold). Meaning is the tuner's; the heuristic only
    /// enforces the numeric floor.
    pub min_fires: i32,
    /// Press only with at least this many crystals (`0` = none).
    pub min_crystals: i32,
    /// Markers to keep unspent (`0` = none).
    pub keep_markers: i32,
    pub min_round: i32,
    pub max_round: i32,
}

impl Default for SkillParams {
    fn default() -> Self {
        Self {
            play_weight_milli: 0,
            min_fires: 0,
            min_crystals: 0,
            keep_markers: 0,
            min_round: 0,
            max_round: i32::MAX,
        }
    }
}

/// Per-card [反击] response knobs (sparse map; absent = the seat's
/// [`StrategyParams::counteract_propensity_milli`]). An entry is the card's
/// own spec -- it does **not** inherit the base rate, so a card the book lists
/// and wants held back writes `propensity_milli: 0` explicitly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CounterParams {
    /// Propensity to declare this card on a counteract offer, 0..=1000
    /// (`0` = never -- hold this card back).
    pub propensity_milli: i32,
    /// Propensity override per window kind (`"pay"`, `"move"`, `"turnEnd"`,
    /// ...). Consulted before [`Self::propensity_milli`] when the caller knows
    /// the window kind; the engine's prompt does not carry it yet, so the
    /// base value is what applies today.
    pub by_kind: BTreeMap<String, i32>,
}

impl Default for CounterParams {
    fn default() -> Self {
        Self {
            // An entry that omits `propensity_milli` holds the card back at
            // the base level (a `by_kind` override can still fire). Absent
            // cards do not use this -- they take the seat's
            // `counteract_propensity_milli`.
            propensity_milli: 0,
            by_kind: BTreeMap::new(),
        }
    }
}

/// Every tunable the standard heuristic reads (`docs/BOT.md` §3.8).
///
/// Flat and versioned; integers / milli-fractions only (no floats in the
/// file). **Every default equals today's constant** in `engine/ai.rs` /
/// `bot-core` / `autopilot.ts`, so an empty book is byte-identical to the
/// pre-book behaviour -- with one ruling exception: the [反击] default moved
/// from "never declare" to [`DEFAULT_COUNTERACT_PROPENSITY_MILLI`] (user
/// 2026-10-08, "bots must be able to counteract"; `defaults_equal_todays_constants`
/// pins the rest). Sparse maps (`cards`, `skills`, `counteract`) are
/// absent = default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StrategyParams {
    // -- buying -------------------------------------------------------------
    /// Cash left after a land buy (`BUY_RESERVE`). Early phase.
    pub buy_reserve: i32,
    /// [`Self::buy_reserve`] from [`Self::phase_mid_round`] on. Same default --
    /// today's policy has no phases.
    pub buy_reserve_mid: i32,
    /// [`Self::buy_reserve`] from [`Self::phase_late_round`] on.
    pub buy_reserve_late: i32,
    /// First round of the mid phase (meaningless while the reserves agree).
    pub phase_mid_round: i32,
    /// First round of the late phase.
    pub phase_late_round: i32,
    /// Per colour-group buy priority (`TileData::group` index → weight,
    /// milli). Shorter / missing entries are [`NEUTRAL_WEIGHT`]; the empty
    /// default is uniform, exactly today's behaviour.
    pub buy_group_weight: Vec<i32>,
    /// Discount, in milli of the price, for a buy that completes the buyer's
    /// colour group. `0` = off (today's policy ignores set completion).
    pub set_complete_bonus_milli: i32,
    /// Cap on price / cash, in milli. `0` = off (today's policy has no ratio
    /// cap).
    pub max_price_ratio_milli: i32,

    // -- building -----------------------------------------------------------
    /// Cash left after building (`BUILD_RESERVE`).
    pub build_reserve: i32,
    /// Per colour-group build priority (milli; empty = neutral).
    pub build_group_weight: Vec<i32>,
    /// Soft cap on houses per colour group (group index → target). Empty =
    /// no cap beyond the rulebook's. A group at or over its target is not
    /// built on.
    pub target_houses: Vec<i32>,

    // -- auctions / force-buy ------------------------------------------------
    /// Cash left after 「可选择[支付]…两倍…强行购买」 (`FORCE_BUY_RESERVE`).
    pub force_buy_reserve: i32,
    /// Auction ceiling's low end as a fraction of the quoted worth, milli
    /// (`0.6` today).
    pub auction_worth_lo_milli: i32,
    /// Width of the uniform range above the low end, milli (`0.7` today).
    pub auction_worth_span_milli: i32,
    /// The ceiling is also capped at `money - this` (`1000` today).
    pub auction_cash_margin: i32,
    /// Minimum raise / bid rounding step (`100` today).
    pub bid_step: i32,
    /// Standard's bid nudge: `rng.below(this)` steps of [`Self::bid_step`]
    /// above the minimum (`3` today = `+0..=+200`).
    pub bid_nudge_steps: i32,
    /// Bid level offered by the search abstraction, as a fraction of the
    /// quoted price, milli (`3/4` today).
    pub bid_frac_milli: i32,

    // -- card play -----------------------------------------------------------
    /// Odds of playing a hand card rather than rolling in 运营, milli
    /// (`PLAY_CARD_CHANCE` = 0.7).
    pub play_card_chance_milli: i32,
    /// Hand cards played in one turn before rolling (`MAX_PLAYS_PER_TURN`).
    pub max_plays_per_turn: i32,
    /// Cash left after a card's `estCost` (`BUY_RESERVE` today -- the card
    /// gate shares the buy reserve; a separate field so the tuner can split
    /// them).
    pub play_card_reserve: i32,
    /// Per-card sparse map (`docs/BOT.md` §3.8 "card play").
    pub cards: BTreeMap<String, CardPlayParams>,

    // -- skills --------------------------------------------------------------
    /// Per-skill-entry sparse map. Default entry = never press (today's
    /// standard policy).
    pub skills: BTreeMap<String, SkillParams>,

    // -- counteraction -------------------------------------------------------
    /// Base [反击] declare propensity for a card with **no** `counteract[id]`
    /// entry, 0..=1000 ([`DEFAULT_COUNTERACT_PROPENSITY_MILLI`] = 600). Drawn
    /// per offer from the match RNG, like chaos's `CHAOS_COUNTER_CHANCE`.
    pub counteract_propensity_milli: i32,
    /// Per-card [反击] response propensity, sparse. A listed card is its own
    /// spec (0 = hold back); an unlisted card takes
    /// [`Self::counteract_propensity_milli`].
    pub counteract: BTreeMap<String, CounterParams>,

    // -- mortgage / redeem ---------------------------------------------------
    /// Mortgage order: weight of the "has houses" component of the sort key,
    /// milli (`1000` = bare land first, today). `0` = ignore houses;
    /// negative = housed land first.
    pub mortgage_house_key_milli: i32,
    /// Mortgage order: weight of the price component, milli (`1000` = cheapest
    /// first, today). Negative = most expensive first.
    pub mortgage_price_key_milli: i32,
    /// Cash left after redeeming (`REDEEM_RESERVE`).
    pub redeem_reserve: i32,

    // -- search priors (bot-core `action_priors`) ----------------------------
    /// Buy prior when `wants_buy` holds (`0.8` today).
    pub prior_buy_yes_milli: i32,
    /// Buy prior when it does not (`0.2` today).
    pub prior_buy_no_milli: i32,
    /// Build prior when `wants_build` holds (`0.8` today).
    pub prior_build_yes_milli: i32,
    /// Build prior when it does not (`0.2` today).
    pub prior_build_no_milli: i32,
    /// Prior for the non-preferred alternate on a menu (`0.3` today: decline
    /// when it is not the heuristic's pick, offer / pick / mortgage / bid
    /// pass).
    pub prior_alt_milli: i32,
    /// Base prior of a card play the heuristic does not single out (`0.4`).
    pub prior_play_base_milli: i32,
    /// Bonus added for a cheap `estCost` (up to `0.3`).
    pub prior_play_bonus_milli: i32,
    /// Prior of a bid when the heuristic's ceiling is unknown (`0.4`).
    pub prior_bid_neutral_milli: i32,
    /// Floor of the distance-shaped bid prior (`0.15`).
    pub prior_bid_floor_milli: i32,
    /// Ceiling of the distance-shaped bid prior (`0.9`).
    pub prior_bid_ceil_milli: i32,
    /// Prior of a [反击] skip when it is the heuristic default (`0.7`).
    pub prior_counter_skip_milli: i32,
    /// Prior of a [反击] declaration the heuristic did not pick (`0.5`).
    pub prior_counter_declare_milli: i32,
}

impl Default for StrategyParams {
    fn default() -> Self {
        use crate::engine::{
            BUILD_RESERVE, BUY_RESERVE, FORCE_BUY_RESERVE, MAX_PLAYS_PER_TURN, PLAY_CARD_CHANCE,
            REDEEM_RESERVE,
        };
        Self {
            // buying
            buy_reserve: BUY_RESERVE,
            buy_reserve_mid: BUY_RESERVE,
            buy_reserve_late: BUY_RESERVE,
            phase_mid_round: 20,
            phase_late_round: 40,
            buy_group_weight: Vec::new(),
            set_complete_bonus_milli: 0,
            max_price_ratio_milli: 0,
            // building
            build_reserve: BUILD_RESERVE,
            build_group_weight: Vec::new(),
            target_houses: Vec::new(),
            // auctions / force-buy
            force_buy_reserve: FORCE_BUY_RESERVE,
            auction_worth_lo_milli: 600,
            auction_worth_span_milli: 700,
            auction_cash_margin: 1_000,
            bid_step: 100,
            bid_nudge_steps: 3,
            bid_frac_milli: 750,
            // card play
            // `PLAY_CARD_CHANCE` is 0.7; the milli form is exactly 700.
            play_card_chance_milli: (PLAY_CARD_CHANCE * 1000.0).round() as i32,
            max_plays_per_turn: MAX_PLAYS_PER_TURN as i32,
            play_card_reserve: BUY_RESERVE,
            cards: BTreeMap::new(),
            // skills
            skills: BTreeMap::new(),
            // counteraction
            counteract_propensity_milli: DEFAULT_COUNTERACT_PROPENSITY_MILLI,
            counteract: BTreeMap::new(),
            // mortgage / redeem
            mortgage_house_key_milli: NEUTRAL_WEIGHT,
            mortgage_price_key_milli: NEUTRAL_WEIGHT,
            redeem_reserve: REDEEM_RESERVE,
            // search priors (the f64s of `bot-core::action::action_priors`)
            prior_buy_yes_milli: 800,
            prior_buy_no_milli: 200,
            prior_build_yes_milli: 800,
            prior_build_no_milli: 200,
            prior_alt_milli: 300,
            prior_play_base_milli: 400,
            prior_play_bonus_milli: 300,
            prior_bid_neutral_milli: 400,
            prior_bid_floor_milli: 150,
            prior_bid_ceil_milli: 900,
            prior_counter_skip_milli: 700,
            prior_counter_declare_milli: 500,
        }
    }
}

impl StrategyParams {
    /// Scale `price` by a milli weight. [`NEUTRAL_WEIGHT`] is a strict no-op
    /// (`price * 1000 / 1000 == price`), so the default path is the old
    /// arithmetic exactly.
    fn weighted(&self, price: i32, weight: i32) -> i32 {
        if weight == NEUTRAL_WEIGHT {
            return price;
        }
        ((price as i64) * (weight as i64) / 1000) as i32
    }

    /// The buy reserve in force at `round` (phase split; all three default
    /// equal, so the split is invisible until a book retunes one).
    pub fn buy_reserve_at(&self, round: i32) -> i32 {
        if round >= self.phase_late_round {
            self.buy_reserve_late
        } else if round >= self.phase_mid_round {
            self.buy_reserve_mid
        } else {
            self.buy_reserve
        }
    }

    /// `AiWantsBuy` under these params (no tile / round context).
    pub fn wants_buy(&self, money: i32, price: i32) -> bool {
        money - price >= self.buy_reserve
    }

    /// `AiWantsBuy` with the tile's colour group and the current round.
    ///
    /// * `group` -- [`crate::data::TileData::group`] of the tile.
    /// * `completes_set` -- the buy would complete the buyer's colour group
    ///   (only consulted when [`Self::set_complete_bonus_milli`] is on).
    pub fn wants_buy_tile(
        &self,
        money: i32,
        price: i32,
        group: i32,
        round: i32,
        completes_set: bool,
    ) -> bool {
        let mut price = self.weighted(price, self.buy_group_weight_lookup(group));
        if completes_set && self.set_complete_bonus_milli != 0 {
            let keep = (NEUTRAL_WEIGHT - self.set_complete_bonus_milli).clamp(0, NEUTRAL_WEIGHT);
            price = self.weighted(price, keep);
        }
        if self.max_price_ratio_milli > 0
            && (price as i64) * 1000 > (money.max(0) as i64) * (self.max_price_ratio_milli as i64)
        {
            return false;
        }
        money - price >= self.buy_reserve_at(round)
    }

    /// Per-group buy weight (milli, [`NEUTRAL_WEIGHT`] when absent).
    pub fn buy_group_weight_lookup(&self, group: i32) -> i32 {
        self.buy_group_weight
            .get(group.max(0) as usize)
            .copied()
            .unwrap_or(NEUTRAL_WEIGHT)
    }

    /// Per-group build weight (milli, [`NEUTRAL_WEIGHT`] when absent).
    pub fn build_group_weight_lookup(&self, group: i32) -> i32 {
        self.build_group_weight
            .get(group.max(0) as usize)
            .copied()
            .unwrap_or(NEUTRAL_WEIGHT)
    }

    /// Per-group house target (`i32::MAX` when absent = no soft cap).
    pub fn target_houses_for(&self, group: i32) -> i32 {
        self.target_houses
            .get(group.max(0) as usize)
            .copied()
            .unwrap_or(i32::MAX)
    }

    /// `AiWantsBuild` under these params (no tile context).
    pub fn wants_build(&self, money: i32, cost: i32) -> bool {
        money - cost >= self.build_reserve
    }

    /// `AiWantsBuild` with the tile's colour group and the houses already up.
    pub fn wants_build_tile(&self, money: i32, cost: i32, group: i32, houses: i32) -> bool {
        if houses >= self.target_houses_for(group) {
            return false;
        }
        let cost = self.weighted(cost, self.build_group_weight_lookup(group));
        money - cost >= self.build_reserve
    }

    /// Would redeeming for `cost` leave [`Self::redeem_reserve`]?
    pub fn wants_redeem(&self, money: i32, cost: i32) -> bool {
        money - cost >= self.redeem_reserve
    }

    /// 「可选择[支付]…两倍…强行购买」 under these params.
    pub fn wants_force_buy(&self, money: i32, price: i32) -> bool {
        money - price >= self.force_buy_reserve
    }

    /// The card-play reserve in force for `card` (the global
    /// [`Self::play_card_reserve`], or the card's own floor).
    pub fn play_card_reserve_for(&self, card: &str) -> i32 {
        self.cards
            .get(card)
            .and_then(|c| c.min_cash_after_est)
            .unwrap_or(self.play_card_reserve)
    }

    /// Card-play reserve check against the card's bot-only estimated
    /// execution cost. A cost of `0` always passes.
    pub fn wants_play_card(&self, money: i32, est_cost: i32, card: &str) -> bool {
        if est_cost <= 0 {
            return true;
        }
        money - est_cost >= self.play_card_reserve_for(card)
    }

    /// `PLAY_CARD_CHANCE` as an f64 probability (`700 / 1000.0 == 0.7`
    /// exactly in IEEE-754, so the default draws the same odds as the old
    /// literal).
    pub fn play_card_chance(&self) -> f64 {
        self.play_card_chance_milli as f64 / 1000.0
    }

    /// Per-card play knobs (absent = [`CardPlayParams::default`]).
    pub fn card(&self, id: &str) -> CardPlayParams {
        self.cards.get(id).cloned().unwrap_or_default()
    }

    /// Per-skill press knobs (absent = [`SkillParams::default`] = never).
    pub fn skill(&self, id: &str) -> SkillParams {
        self.skills.get(id).cloned().unwrap_or_default()
    }

    /// The [反击] propensity for declaring `card`, 0..=1000. `window_kind` is
    /// the trigger kind when the caller knows it (the engine's prompt does not
    /// carry it yet, so `None` is the common case and only the base
    /// propensity applies).
    ///
    /// A card with no `counteract[id]` entry takes the seat's
    /// [`Self::counteract_propensity_milli`] (default
    /// [`DEFAULT_COUNTERACT_PROPENSITY_MILLI`] = 600, user ruling 2026-10-08:
    /// "bots must be able to counteract"). A listed card is its own spec:
    /// `by_kind[window]` if set, else `propensity_milli` (`0` = hold it back).
    pub fn counteract_propensity(&self, card: &str, window_kind: Option<&str>) -> i32 {
        let Some(e) = self.counteract.get(card) else {
            return self.counteract_propensity_milli.clamp(0, NEUTRAL_WEIGHT);
        };
        if let Some(k) = window_kind {
            if let Some(&v) = e.by_kind.get(k) {
                return v.clamp(0, NEUTRAL_WEIGHT);
            }
        }
        e.propensity_milli.clamp(0, NEUTRAL_WEIGHT)
    }

    /// The auction ceiling for a tile of quoted price `base` and `money` in
    /// hand, given one uniform draw `roll` in `[0, 1)`. The caller draws (and
    /// so controls RNG order): this is pure arithmetic.
    ///
    /// `((base * (lo + roll * span)) / 100 | 0) * 100`, capped at
    /// `money - auction_cash_margin` -- the original formula with the
    /// constants parameterised. Defaults are bit-identical to it.
    pub fn auction_worth(&self, roll: f64, base: i32, money: i32) -> i32 {
        let lo = self.auction_worth_lo_milli as f64 / 1000.0;
        let span = self.auction_worth_span_milli as f64 / 1000.0;
        let v = ((base as f64 * (lo + roll * span) / 100.0) as i32) * 100;
        v.min(money - self.auction_cash_margin)
    }

    /// The minimum raise at a live bid of `bid` ([`Self::bid_step`] when the
    /// auction has not opened).
    pub fn bid_min(&self, bid: i32) -> i32 {
        if bid <= 0 {
            self.bid_step.max(0)
        } else {
            bid + self.bid_step.max(0)
        }
    }

    /// Mortgage sort key: `(has_houses · house_key, price · price_key, tile)`.
    ///
    /// Lexicographic, like today's `(houses > 0, price, t)` -- with both keys
    /// at [`NEUTRAL_WEIGHT`] it **is** that order exactly: the house component
    /// is `1000` vs `0` (any bare deed before any housed one) and
    /// `price * 1000 / 1000 == price`. The keys only turn each component up,
    /// down or off (`0`); they do not trade one against the other, which would
    /// change the order the rulebook's `MortgageOrder` fixes.
    pub fn mortgage_key(&self, houses: i32, price: i32, tile: usize) -> (i64, i64, usize) {
        let house = i64::from(houses > 0) * i64::from(self.mortgage_house_key_milli);
        let price = i64::from(price) * i64::from(self.mortgage_price_key_milli) / 1000;
        (house, price, tile)
    }

    /// Redeem order key: most valuable first at the default price key
    /// (`Reverse(price)` today). Ascending sort puts the smallest key first.
    pub fn redeem_key(&self, price: i32) -> i64 {
        -(i64::from(price) * i64::from(self.mortgage_price_key_milli) / 1000)
    }

    // -- search priors (bot-core `action_priors`) ---------------------------
    /// Preferred-action prior is always `1.0`; these are the alternates.

    /// Buy prior when `wants_buy` holds.
    pub fn prior_buy_yes(&self) -> f64 {
        self.prior_buy_yes_milli as f64 / 1000.0
    }
    /// Buy prior when it does not.
    pub fn prior_buy_no(&self) -> f64 {
        self.prior_buy_no_milli as f64 / 1000.0
    }
    /// Build prior when `wants_build` holds.
    pub fn prior_build_yes(&self) -> f64 {
        self.prior_build_yes_milli as f64 / 1000.0
    }
    /// Build prior when it does not.
    pub fn prior_build_no(&self) -> f64 {
        self.prior_build_no_milli as f64 / 1000.0
    }
    /// The generic non-preferred alternate.
    pub fn prior_alt(&self) -> f64 {
        self.prior_alt_milli as f64 / 1000.0
    }
    /// Base prior of a non-singled-out card play.
    pub fn prior_play_base(&self) -> f64 {
        self.prior_play_base_milli as f64 / 1000.0
    }
    /// Bonus for a cheap `estCost`.
    pub fn prior_play_bonus(&self) -> f64 {
        self.prior_play_bonus_milli as f64 / 1000.0
    }
    /// Prior of a bid with an unknown ceiling.
    pub fn prior_bid_neutral(&self) -> f64 {
        self.prior_bid_neutral_milli as f64 / 1000.0
    }
    /// Floor of the distance-shaped bid prior.
    pub fn prior_bid_floor(&self) -> f64 {
        self.prior_bid_floor_milli as f64 / 1000.0
    }
    /// Ceiling of the distance-shaped bid prior.
    pub fn prior_bid_ceil(&self) -> f64 {
        self.prior_bid_ceil_milli as f64 / 1000.0
    }
    /// Prior of a [反击] skip that is the heuristic default.
    pub fn prior_counter_skip(&self) -> f64 {
        self.prior_counter_skip_milli as f64 / 1000.0
    }
    /// Prior of a [反击] declaration the heuristic did not pick.
    pub fn prior_counter_declare(&self) -> f64 {
        self.prior_counter_declare_milli as f64 / 1000.0
    }
}

/// The parameters a standard seat at `seat` plays under, from public state
/// only (own character + the other seats' characters). The running ruleset's
/// hash is not in hand here, so only version / policy / `params_version` gate
/// the book -- see [`for_seat_sha`] for the hash-checked lookup.
pub fn for_seat(data: &GameData, st: &MatchState, seat: usize) -> StrategyParams {
    for_seat_sha(data, st, seat, None)
}

/// [`for_seat`] with the running ruleset's hash (`"stub"` without card
/// modules) so a stale book is ignored.
pub fn for_seat_sha(
    data: &GameData,
    st: &MatchState,
    seat: usize,
    ruleset_sha256: Option<&str>,
) -> StrategyParams {
    let Some(p) = st.players.get(seat) else {
        return StrategyParams::default();
    };
    let me = p.character.as_str();
    let opponents: Vec<String> = st
        .players
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != seat)
        .map(|(_, p)| p.character.clone())
        .collect();
    match data.character(me) {
        Some(c) => data
            .strategy_book
            .resolve(data, c, &opponents, None, ruleset_sha256),
        None => StrategyParams::default(),
    }
}

// ---------------------------------------------------------------------------
// Book
// ---------------------------------------------------------------------------

/// One book row. Which fields matter depends on the level array it sits in
/// (`docs/BOT.md` §3.8):
///
/// | level | key fields |
/// |---|---|
/// | `char_bands` | `me`, `opponent_bands` (sorted multiset) |
/// | `me` | `me` |
/// | `band` | `band` (the character's band) |
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StrategyEntry {
    /// Own character (`CharacterData::name`). Used by the two character levels.
    pub me: String,
    /// Own band (`CharacterData::band`). Used by the band level.
    pub band: String,
    /// Sorted multiset of the opponents' band names (char_bands level).
    pub opponent_bands: Vec<String>,
    /// The deck-book entry the parameters were tuned with (card ids). Empty =
    /// any deck. A non-empty entry whose deck differs from the seat's actual
    /// deck falls through to the next level (see [`StrategyBook::lookup`]).
    pub deck: Vec<String>,
    /// Partial parameters; missing fields take [`StrategyParams::default`].
    pub params: StrategyParams,
}

/// `data/strategy_book.json` -- header plus one entry list per back-off level.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StrategyBook {
    pub version: u32,
    /// `Ruleset::sha256()` of the ruleset the entries were tuned against, or
    /// `"stub"`. Same string as [`crate::record::EngineStamp::ruleset_sha256`].
    pub ruleset_sha256: String,
    /// [`POLICY_STANDARD`]; any other policy makes the book inert.
    pub policy: String,
    /// Free-form generation stamp (date, tool version); display only.
    pub generated_at: String,
    /// [`PARAMS_VERSION`] the entries were tuned against.
    pub params_version: u32,
    /// 1. `(me, sorted multiset of opponents' bands)`.
    pub char_bands: Vec<StrategyEntry>,
    /// 2. `(me)`.
    pub me: Vec<StrategyEntry>,
    /// 3. `(band)`.
    pub band: Vec<StrategyEntry>,
    /// Warned once about being stale / mismatched (never serialized).
    #[serde(skip)]
    warned: Arc<AtomicBool>,
}

impl PartialEq for StrategyBook {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
            && self.ruleset_sha256 == other.ruleset_sha256
            && self.policy == other.policy
            && self.generated_at == other.generated_at
            && self.params_version == other.params_version
            && self.char_bands == other.char_bands
            && self.me == other.me
            && self.band == other.band
    }
}

impl StrategyBook {
    /// Parse the file body. Header mismatches are **not** checked here -- see
    /// [`Self::usable`], which lookup applies against the running ruleset.
    pub fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())
    }

    /// Parse, or fall back to an empty book with the reason on stderr. The
    /// optional-file loader ([`GameData::load`]) uses this so a corrupt book
    /// never breaks the game.
    pub fn parse_or_empty(text: &str) -> Self {
        Self::parse(text).unwrap_or_else(|e| {
            eprintln!("{STRATEGY_BOOK_FILE}: {e} -- bots fall back to the default parameters");
            Self::default()
        })
    }

    /// Does the book hold any entries at all? An empty book (the shipped
    /// placeholder, or a missing file) is the pre-derivation state and never
    /// warns about being stale -- there is nothing to go stale.
    pub fn is_empty(&self) -> bool {
        self.char_bands.is_empty() && self.me.is_empty() && self.band.is_empty()
    }

    /// Is the book tuned for this ruleset / policy / parameter schema? A
    /// mismatch means every entry is silently mis-tuned, so the whole book is
    /// ignored (logged once) and the caller falls back to
    /// [`StrategyParams::default`].
    ///
    /// `ruleset_sha256: None` skips only the hash comparison (the view-only
    /// fallback has no ruleset in hand); version, policy and
    /// [`PARAMS_VERSION`] are always enforced.
    pub fn usable(&self, ruleset_sha256: Option<&str>) -> bool {
        let hash_ok = match ruleset_sha256 {
            Some(sha) => self.ruleset_sha256 == sha,
            None => true,
        };
        if self.version != STRATEGY_BOOK_VERSION
            || self.policy != POLICY_STANDARD
            || self.params_version != PARAMS_VERSION
            || !hash_ok
        {
            if !self.is_empty()
                && self
                    .warned
                    .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
            {
                eprintln!(
                    "{STRATEGY_BOOK_FILE}: version {} / policy {} / params {} / ruleset {} does not \
                     match this build (want version {STRATEGY_BOOK_VERSION} / {POLICY_STANDARD} / \
                     {PARAMS_VERSION} / {}) -- bots fall back to the default parameters",
                    self.version,
                    self.policy,
                    self.params_version,
                    self.ruleset_sha256,
                    ruleset_sha256.unwrap_or("(any)"),
                );
            }
            return false;
        }
        true
    }

    /// Back-off lookup over the public table key. Pure: no RNG, first hit per
    /// level wins. An entry that names a deck only applies when the seat's
    /// actual deck matches (or is unknown).
    ///
    /// * `me` -- own character.
    /// * `opponents` -- the other seats' characters in seat order (only their
    ///   bands are used).
    /// * `deck` -- the seat's deck card ids, when the caller knows them.
    /// * `ruleset_sha256` -- the running ruleset's hash; `None` skips the hash
    ///   comparison (see [`Self::usable`]).
    pub fn lookup(
        &self,
        data: &GameData,
        me: &CharacterData,
        opponents: &[String],
        deck: Option<&[String]>,
        ruleset_sha256: Option<&str>,
    ) -> Option<StrategyParams> {
        if !self.usable(ruleset_sha256) {
            return None;
        }
        let bands = opponent_bands(data, opponents);
        let hits = |e: &StrategyEntry| deck_matches(e, deck);
        // 1. (me, sorted multiset of opponents' bands)
        if let Some(e) = self
            .char_bands
            .iter()
            .find(|e| e.me == me.name && e.opponent_bands == bands && hits(e))
        {
            return Some(e.params.clone());
        }
        // 2. (me)
        if let Some(e) = self.me.iter().find(|e| e.me == me.name && hits(e)) {
            return Some(e.params.clone());
        }
        // 3. (band)
        if let Some(e) = self.band.iter().find(|e| e.band == me.band && hits(e)) {
            return Some(e.params.clone());
        }
        // 4. defaults
        None
    }

    /// The parameters a standard seat with this public table plays under:
    /// [`Self::lookup`], else [`StrategyParams::default`]. Pure -- no RNG.
    /// Chaos never calls this.
    pub fn resolve(
        &self,
        data: &GameData,
        me: &CharacterData,
        opponents: &[String],
        deck: Option<&[String]>,
        ruleset_sha256: Option<&str>,
    ) -> StrategyParams {
        self.lookup(data, me, opponents, deck, ruleset_sha256)
            .unwrap_or_default()
    }
}

/// An entry that names a deck only applies to that deck. An unknown deck
/// (`None`) cannot disprove the pair, so the entry applies.
fn deck_matches(e: &StrategyEntry, deck: Option<&[String]>) -> bool {
    if e.deck.is_empty() {
        return true;
    }
    match deck {
        None => true,
        Some(d) => e.deck.len() == d.len() && e.deck.iter().all(|c| d.iter().any(|x| x == c)),
    }
}

/// Sorted multiset of the opponents' band names (`CharacterData::band`). An
/// unknown character contributes `""` -- deterministic, and it only ever makes
/// a band-level key miss.
fn opponent_bands(data: &GameData, opponents: &[String]) -> Vec<String> {
    let mut bands: Vec<String> = opponents
        .iter()
        .map(|c| data.character(c).map(|c| c.band.clone()).unwrap_or_default())
        .collect();
    bands.sort();
    bands
}