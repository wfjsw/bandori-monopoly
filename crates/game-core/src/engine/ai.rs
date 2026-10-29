//! Bot decisions. Also drives humans who ran out of time.
//!
//! Every seat carries a [`BotMentality`] (only `bot` seats take one): **standard**
//! is the standard policy (`docs/BOT.md`) and **chaos** is legal but maximally
//! disruptive. The thresholds below are the standard policy; chaos replaces the
//! money reserves with [`CHAOS_RESERVE`] and drops the "sometimes" rolls. The web
//! client's 托管 (auto-play) toggle ports both policies to TypeScript over the
//! public match view (`webui/src/game/autopilot.ts`); keep the two in sync.
//!
//! A human who times out or disconnects is answered with the **standard**
//! policy even though `ai` flips on -- see [`Cx::bot_mentality`].
//!
//! **Standard reads its thresholds from the seat's resolved
//! [`StrategyParams`]** (`docs/BOT.md` §3.8): the constants below are their
//! defaults, so an empty strategy book is exactly the old policy. Chaos is
//! not parameterised -- it keeps [`CHAOS_RESERVE`] and [`CHAOS_COUNTER_CHANCE`]
//! as literals.

use super::cx::Cx;
use crate::state::BotMentality;
use crate::strategy::StrategyParams;

/// Keep at least this much money after buying.
pub const BUY_RESERVE: i32 = 2_000;
/// Keep at least this much money after building.
pub const BUILD_RESERVE: i32 = 3_500;
/// Keep at least this much money after redeeming.
pub const REDEEM_RESERVE: i32 = 4_000;
/// Buy out a mortgaged deed only while it leaves this much.
pub const FORCE_BUY_RESERVE: i32 = 4_000;
/// Odds a bot plays a hand card rather than rolling in 运营.
pub const PLAY_CARD_CHANCE: f64 = 0.7;
/// Hand cards a bot plays in one turn before it rolls.
pub const MAX_PLAYS_PER_TURN: usize = 2;
/// [pass-reward] card odds ramp (user ruling 2026-10-28). At or below [`PASS_REWARD_CARD_MIN_MONEY`] the
/// seat takes the 2 000 money print; the card odds then rise superlinearly to
/// 1 at [`PASS_REWARD_CARD_MONEY`], above which the seat always draws. The
/// print's value falls as the stack grows, so the ramp is `t²`, not linear.
pub const PASS_REWARD_CARD_MIN_MONEY: i32 = 8_000;
pub const PASS_REWARD_CARD_MONEY: i32 = 80_000;
/// Chaos's coin reserve: voluntary spending only while it leaves this much.
/// The one knob the chaos policy spends to -- without it a chaos bot burns out
/// in a few turns and stops being disruptive.
pub const CHAOS_RESERVE: i32 = 1_000;
/// Chaos declares a counteraction on this share of the counteract offers it gets
/// (a random offered card); the rest of the offers it passes. Rolled per offer
/// from the match RNG, so a repeated offer cycle naturally stops after a skip.
pub const CHAOS_COUNTER_CHANCE: f64 = 0.3;

/// Would buying `price` leave [`BUY_RESERVE`]?
pub fn wants_buy(money: i32, price: i32) -> bool {
    money - price >= BUY_RESERVE
}

/// Would building for `cost` leave [`BUILD_RESERVE`]?
pub fn wants_build(money: i32, cost: i32) -> bool {
    money - cost >= BUILD_RESERVE
}

/// Standard's [pass-reward] card odds at `money` -- 0 at or below
/// [`PASS_REWARD_CARD_MIN_MONEY`], 1 at or above [`PASS_REWARD_CARD_MONEY`],
/// superlinear (`t²`) in between.
pub fn pass_reward_card_chance(money: i32) -> f64 {
    if money <= PASS_REWARD_CARD_MIN_MONEY {
        return 0.0;
    }
    if money >= PASS_REWARD_CARD_MONEY {
        return 1.0;
    }
    let t = f64::from(money - PASS_REWARD_CARD_MIN_MONEY)
        / f64::from(PASS_REWARD_CARD_MONEY - PASS_REWARD_CARD_MIN_MONEY);
    t * t
}

/// Would redeeming for `cost` leave [`REDEEM_RESERVE`]?
pub fn wants_redeem(money: i32, cost: i32) -> bool {
    money - cost >= REDEEM_RESERVE
}

/// Mentality-aware buy test: standard keeps [`BUY_RESERVE`], chaos only
/// [`CHAOS_RESERVE`]. Advanced is standard here -- it is the same policy
/// whenever the engine is the one driving the seat (`docs/BOT.md` B5).
pub fn bot_wants_buy(m: BotMentality, money: i32, price: i32) -> bool {
    match m {
        BotMentality::Standard | BotMentality::Advanced => wants_buy(money, price),
        BotMentality::Chaos => money - price >= CHAOS_RESERVE,
    }
}

/// Mentality-aware card-play reserve check against the card's bot-only
/// **estimated execution cost** (`prop::EST_COST`, user ruling 2026-10-07).
///
/// This is a *bot policy*, never legality: a human (and `cant_play`) may still
/// play the card and take the Q1 shortfall path. Standard keeps
/// [`BUY_RESERVE`]; chaos keeps its own [`CHAOS_RESERVE`]. A cost of `0`
/// (unknown / assume free) always passes.
pub fn bot_wants_play_card(m: BotMentality, money: i32, est_cost: i32) -> bool {
    if est_cost <= 0 {
        return true;
    }
    match m {
        BotMentality::Standard | BotMentality::Advanced => money - est_cost >= BUY_RESERVE,
        BotMentality::Chaos => money - est_cost >= CHAOS_RESERVE,
    }
}

/// Mentality-aware build test.
pub fn bot_wants_build(m: BotMentality, money: i32, cost: i32) -> bool {
    match m {
        BotMentality::Standard | BotMentality::Advanced => wants_build(money, cost),
        BotMentality::Chaos => money - cost >= CHAOS_RESERVE,
    }
}

/// Mentality-aware redeem test.
pub fn bot_wants_redeem(m: BotMentality, money: i32, cost: i32) -> bool {
    match m {
        BotMentality::Standard | BotMentality::Advanced => wants_redeem(money, cost),
        BotMentality::Chaos => money - cost >= CHAOS_RESERVE,
    }
}

/// Mentality-aware 「可选择pay…两倍…强行购买」 test.
pub fn bot_wants_force_buy(m: BotMentality, money: i32, price: i32) -> bool {
    match m {
        BotMentality::Standard | BotMentality::Advanced => money - price >= FORCE_BUY_RESERVE,
        BotMentality::Chaos => money - price >= CHAOS_RESERVE,
    }
}

impl Cx<'_> {
    /// The policy this seat actually plays under: the seat's own mentality.
    /// A human seat carries [`BotMentality::Standard`] unless a host set
    /// otherwise -- 托管's 混沌 mode tags the human seat `chaos`, and the
    /// page's own frame rewrite (and this fill) must agree with the driver.
    ///
    /// Inlined and branch-light: this sits on every bot decision, and the
    /// standard path must not pay for the chaos one.
    #[inline]
    pub fn bot_mentality(&self, i: usize) -> BotMentality {
        self.w.st.players[i].mentality
    }

    #[inline]
    pub fn is_chaos(&self, i: usize) -> bool {
        self.w.st.players[i].mentality == BotMentality::Chaos
    }

    /// The standard policy's parameters for seat `i` (`docs/BOT.md` §3.8).
    /// Resolved from public state only (own persona + the other seats'
    /// personas); chaos never reads them.
    #[inline]
    pub fn strategy_of(&self, i: usize) -> StrategyParams {
        crate::strategy::for_seat_sha(
            self.data,
            &self.w.st,
            i,
            Some(self.rules.ruleset_sha256().unwrap_or("stub")),
        )
    }

    /// Would buying `t` complete seat `i`'s colour group? Only computed when
    /// [`StrategyParams::set_complete_bonus_milli`] is on (today it is 0).
    fn buy_completes_set(&self, i: usize, t: usize) -> bool {
        let group = self.tile(t).group;
        (0..self.data.tiles.len()).all(|u| {
            u == t
                || self.tile(u).group != group
                || !self.tile(u).is_buyable()
                || self.w.st.owners[u] == i as i32
        })
    }

    /// Would buying leave the reserve? Uses the quoted price (`docs/PURCHASE.md`), so the AI
    /// decides on the figure a hook-aware ruleset would actually charge; for
    /// `StubRules` the quote is the plain rulebook formula.
    #[inline]
    pub fn ai_wants_buy(&self, i: usize, t: usize) -> bool {
        let quote = self.buy_quote_for(i, t, super::purchase::BuyKind::Land);
        if !quote.eligible {
            return false;
        }
        let price = quote.price.max(0);
        if self.is_chaos(i) {
            self.w.st.players[i].money - price >= CHAOS_RESERVE
        } else {
            let p = self.strategy_of(i);
            let completes = p.set_complete_bonus_milli != 0 && self.buy_completes_set(i, t);
            p.wants_buy_tile(
                self.w.st.players[i].money,
                price,
                self.tile(t).group,
                self.w.st.round,
                completes,
            )
        }
    }

    /// Would building leave the reserve?
    #[inline]
    pub fn ai_wants_build(&self, i: usize, t: usize) -> bool {
        if self.is_chaos(i) {
            self.w.st.players[i].money - self.build_cost(t) >= CHAOS_RESERVE
        } else {
            let p = self.strategy_of(i);
            p.wants_build_tile(
                self.w.st.players[i].money,
                self.build_cost(t),
                self.tile(t).group,
                self.w.st.houses.get(t).copied().unwrap_or(0),
            )
        }
    }

    /// Would a force-buy at `price` leave the reserve? Mirrors
    /// `offer_force_buy`'s `Ask::with_ai` fill: standard reads
    /// [`StrategyParams::wants_force_buy`], chaos keeps [`CHAOS_RESERVE`].
    /// `price` is the quoted force-buy figure (the prompt's `view.price`),
    /// which is not the plain land quote.
    #[inline]
    pub fn ai_wants_force_buy(&self, i: usize, price: i32) -> bool {
        let money = self.w.st.players[i].money;
        if self.is_chaos(i) {
            bot_wants_force_buy(self.bot_mentality(i), money, price)
        } else {
            self.strategy_of(i).wants_force_buy(money, price)
        }
    }

    /// Standard: first affordable purchase, else first build,
    /// else none. Chaos: a random option it can pay for, never "none" while one
    /// exists.
    pub fn ai_agent_choice(&mut self, p: usize, options: &[usize]) -> i32 {
        if self.is_chaos(p) {
            let ok: Vec<usize> = options
                .iter()
                .copied()
                .enumerate()
                .filter(|&(_, t)| {
                    (self.w.st.owners[t] < 0 && self.ai_wants_buy(p, t))
                        || (self.w.st.owners[t] == p as i32 && self.ai_wants_build(p, t))
                })
                .map(|(k, _)| k)
                .collect();
            return match ok.len() {
                0 => options.len() as i32,
                n => ok[self.w.rng.below(n)] as i32,
            };
        }
        let params = self.strategy_of(p);
        let round = self.w.st.round;
        if let Some(k) = options.iter().position(|&t| {
            if self.w.st.owners[t] >= 0 {
                return false;
            }
            let quote = self.buy_quote_for(p, t, super::purchase::BuyKind::Agent);
            quote.eligible
                && params.wants_buy_tile(
                    self.w.st.players[p].money,
                    quote.price.max(0),
                    self.tile(t).group,
                    round,
                    false,
                )
        }) {
            return k as i32;
        }
        if let Some(k) = options.iter().position(|&t| {
            self.w.st.owners[t] == p as i32
                && params.wants_build_tile(
                    self.w.st.players[p].money,
                    self.build_cost(t),
                    self.tile(t).group,
                    self.w.st.houses.get(t).copied().unwrap_or(0),
                )
        }) {
            return k as i32;
        }
        options.len() as i32
    }

    /// The auction ceiling for seat `s` on a tile of price `base`. Standard
    /// randomises around the price; chaos will spend down to [`CHAOS_RESERVE`].
    pub(crate) fn ai_auction_worth(&mut self, s: usize, base: i32) -> i32 {
        let money = self.w.st.players[s].money;
        if self.is_chaos(s) {
            return (money - CHAOS_RESERVE).max(0);
        }
        let p = self.strategy_of(s);
        let roll = self.w.rng.f64();
        p.auction_worth(roll, base, money)
    }
}
