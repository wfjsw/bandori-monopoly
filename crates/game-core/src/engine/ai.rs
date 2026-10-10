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

use super::cx::{Cx, Flow};
use crate::state::{stage, BotMentality};
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
/// Chaos's coin reserve: voluntary spending only while it leaves this much.
/// The one knob the chaos policy spends to -- without it a chaos bot burns out
/// in a few turns and stops being disruptive.
pub const CHAOS_RESERVE: i32 = 1_000;
/// Chaos declares a [反击] on this share of the counteract offers it gets
/// (a random offered card); the rest of the offers it passes. Rolled per offer
/// from the match RNG, so a repeated offer ring naturally stops after a skip.
pub const CHAOS_COUNTER_CHANCE: f64 = 0.3;

/// Would buying `price` leave [`BUY_RESERVE`]?
pub fn wants_buy(money: i32, price: i32) -> bool {
    money - price >= BUY_RESERVE
}

/// Would building for `cost` leave [`BUILD_RESERVE`]?
pub fn wants_build(money: i32, cost: i32) -> bool {
    money - cost >= BUILD_RESERVE
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

/// Mentality-aware 「可选择[支付]…两倍…强行购买」 test.
pub fn bot_wants_force_buy(m: BotMentality, money: i32, price: i32) -> bool {
    match m {
        BotMentality::Standard | BotMentality::Advanced => money - price >= FORCE_BUY_RESERVE,
        BotMentality::Chaos => money - price >= CHAOS_RESERVE,
    }
}

impl Cx<'_> {
    /// The policy this seat actually plays under. A human seat (including one
    /// taken over after a time-out / disconnect) is always [`BotMentality::Standard`]
    /// -- a mentality is a property of a *bot*, not of whoever is driving it.
    ///
    /// Inlined and branch-light: this sits on every bot decision, and the
    /// standard path must not pay for the chaos one.
    #[inline]
    pub(crate) fn bot_mentality(&self, i: usize) -> BotMentality {
        let p = &self.w.st.players[i];
        if p.bot {
            p.mentality
        } else {
            BotMentality::Standard
        }
    }

    #[inline]
    pub(crate) fn is_chaos(&self, i: usize) -> bool {
        let p = &self.w.st.players[i];
        p.bot && p.mentality == BotMentality::Chaos
    }

    /// The standard policy's parameters for seat `i` (`docs/BOT.md` §3.8).
    /// Resolved from public state only (own character + the other seats'
    /// characters); chaos never reads them.
    #[inline]
    pub(crate) fn strategy_of(&self, i: usize) -> StrategyParams {
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
    pub(crate) fn ai_wants_buy(&self, i: usize, t: usize) -> bool {
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
    pub(crate) fn ai_wants_build(&self, i: usize, t: usize) -> bool {
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

    /// Standard: most valuable mortgaged deed that leaves
    /// the reserve (the original sort-then-`find`, which short-circuits).
    /// Chaos: any affordable one, at random.
    fn ai_redeem_choice(&mut self, i: usize) -> Option<usize> {
        if self.is_chaos(i) {
            let mut deeds: Vec<usize> = (0..self.data.tiles.len())
                .filter(|&t| self.w.st.owners[t] == i as i32 && self.w.st.mortgaged[t])
                .filter(|&t| {
                    self.w.st.players[i].money - self.redeem_cost(t) >= CHAOS_RESERVE
                })
                .collect();
            if deeds.is_empty() {
                return None;
            }
            let k = self.w.rng.below(deeds.len());
            return Some(deeds[k]);
        }
        let mut deeds: Vec<usize> = (0..self.data.tiles.len())
            .filter(|&t| self.w.st.owners[t] == i as i32 && self.w.st.mortgaged[t])
            .collect();
        let p = self.strategy_of(i);
        deeds.sort_by_key(|&t| p.redeem_key(self.tile(t).price));
        deeds
            .into_iter()
            .find(|&t| p.wants_redeem(self.w.st.players[i].money, self.redeem_cost(t)))
    }

    /// A random playable card the rules say a bot would play.
    /// Chaos ignores the rule's `ai_play` heuristic (it will even fire [反击]
    /// cards from hand) and only asks `cant_play`.
    ///
    /// Standard additionally gates on the card's [`crate::strategy::CardPlayParams`]
    /// (round window, `hold_for_counteract`, weight `> 0`, per-card cash floor
    /// over `estCost`). All defaults are neutral, so the candidate set and the
    /// uniform pick are exactly the old ones.
    fn ai_card_choice(&mut self, i: usize) -> Option<String> {
        let chaos = self.is_chaos(i);
        let mentality = self.bot_mentality(i);
        let money = self.w.st.players[i].money;
        let mut hand = self.w.hidden[i].hand.clone();
        hand.dedup();
        // Bot-only reserve check against the card's estimated execution cost
        // (`prop::EST_COST`, user ruling 2026-10-07). Never legality -- `cant_play`
        // is the only gate for that.
        let affordable = |id: &str| {
            let est = self.rules.card_prop(id, crate::state::prop::EST_COST);
            bot_wants_play_card(mentality, money, est)
        };
        if chaos {
            let ok: Vec<String> = hand
                .into_iter()
                .filter(|id| self.cant_play(i, id, false).is_none() && affordable(id))
                .collect();
            return if ok.is_empty() {
                None
            } else {
                let k = self.w.rng.below(ok.len());
                Some(ok[k].clone())
            };
        }
        let p = self.strategy_of(i);
        let round = self.w.st.round;
        let mut ok: Vec<String> = Vec::new();
        let mut weights: Vec<i32> = Vec::new();
        for id in hand {
            if self.cant_play(i, &id, false).is_some() || !self.rules.ai_play(self, i, &id) {
                continue;
            }
            let cp = p.card(&id);
            if cp.play_weight_milli <= 0
                || cp.hold_for_counteract
                || round < cp.min_round
                || round > cp.max_round
            {
                continue;
            }
            let est = self.rules.card_prop(&id, crate::state::prop::EST_COST);
            if !p.wants_play_card(money, est, &id) {
                continue;
            }
            ok.push(id);
            weights.push(cp.play_weight_milli);
        }
        if ok.is_empty() {
            return None;
        }
        // Uniform among equal weights (the default): the original
        // `ok[rng.below(len)]`, same draw, same mapping. A non-uniform weight
        // set switches to one cumulative draw.
        let uniform = weights.windows(2).all(|w| w[0] == w[1]);
        let k = if uniform {
            self.w.rng.below(ok.len())
        } else {
            let total: i64 = weights.iter().map(|&w| w.max(0) as i64).sum();
            if total <= 0 {
                self.w.rng.below(ok.len())
            } else {
                let mut r = (self.w.rng.below(total as usize)) as i64;
                let mut k = ok.len() - 1;
                for (j, &w) in weights.iter().enumerate() {
                    r -= w.max(0) as i64;
                    if r < 0 {
                        k = j;
                        break;
                    }
                }
                k
            }
        };
        Some(ok[k].clone())
    }

    /// A skill button the seat would press right now: a placed character /
    /// band skill rule the player may activate.
    ///
    /// Chaos presses every usable one (at most one press per skill per turn --
    /// a skill whose body is a no-op must not park the bot in a press loop).
    /// Standard only presses when the seat's [`crate::strategy::SkillParams`]
    /// say so; the default entry is "never", which is the old standard policy
    /// (skills were left to the player). The gate is the rule's own
    /// `cant_play` (`why_not_act` asks the same one before honouring a `skill`
    /// command).
    fn ai_skill_choice(&mut self, i: usize) -> Option<String> {
        let placed = self.w.placed_cards(i as i32);
        let chaos = self.is_chaos(i);
        let params = if chaos {
            None
        } else {
            Some(self.strategy_of(i))
        };
        let candidates: Vec<String> = self
            .data
            .skill_rules_of(&self.w.st.players[i].character)
            .into_iter()
            .filter(|id| {
                placed.contains(id)
                    && !self.w.turn.played.contains(id)
                    && self.rules_cant_play(i, id).is_none()
            })
            .collect();
        let round = self.w.st.round;
        let mut ok: Vec<String> = Vec::new();
        for id in candidates {
            if let Some(p) = &params {
                let sk = p.skill(&id);
                // `min_fires` / `min_crystals` / `keep_markers` are tuner
                // metadata in S1 -- the S1 heuristic enforces the weight and
                // the round window only (`docs/BOT.md` §3.8).
                if sk.play_weight_milli <= 0 || round < sk.min_round || round > sk.max_round {
                    continue;
                }
                // Odds per offer; the "always" band draws nothing extra, and a
                // zero weight (the default) short-circuits above so the RNG
                // stream of the old policy is untouched.
                if sk.play_weight_milli < crate::strategy::NEUTRAL_WEIGHT {
                    let roll = self.w.rng.f64() * 1000.0;
                    if roll >= sk.play_weight_milli as f64 {
                        continue;
                    }
                }
            }
            ok.push(id);
        }
        if ok.is_empty() {
            None
        } else {
            let k = self.w.rng.below(ok.len());
            Some(ok[k].clone())
        }
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

    /// One decision for the player whose turn it is.
    ///
    /// The standard path below is the standard bot policy -- the chaos branch is
    /// taken *before* it and returns, so a standard seat never pays for it.
    pub(crate) fn ai_step(&mut self, i: usize) -> Flow<()> {
        let bot = self.w.st.players[i].ai;
        if self.w.st.step == stage::OPS {
            if bot {
                if self.is_chaos(i) {
                    // Cards first (every legal opportunity, no cap), then every
                    // usable skill, then redeem down to the reserve. Only then
                    // does the turn move on.
                    if let Some(card) = self.ai_card_choice(i) {
                        self.play_from_hand(i, &card)?;
                        self.wait(1.2);
                        return Ok(());
                    }
                    if let Some(sk) = self.ai_skill_choice(i) {
                        self.w.turn.played.push(sk.clone());
                        self.use_skill(i, &sk)?;
                        self.wait(1.2);
                        return Ok(());
                    }
                    if let Some(t) = self.ai_redeem_choice(i) {
                        self.redeem(i, t);
                        self.wait(1.2);
                        return Ok(());
                    }
                } else {
                    let p = self.strategy_of(i);
                    if let Some(t) = self.ai_redeem_choice(i) {
                        self.redeem(i, t);
                        self.wait(1.2);
                        return Ok(());
                    }
                    if self.w.turn.played.len() < p.max_plays_per_turn.max(0) as usize
                        && self.w.rng.chance(p.play_card_chance())
                    {
                        if let Some(card) = self.ai_card_choice(i) {
                            self.play_from_hand(i, &card)?;
                            self.wait(1.2);
                            return Ok(());
                        }
                    }
                    // A skill press only when the seat's `SkillParams` ask for
                    // one (the default entry is "never", the old policy). No
                    // RNG is drawn otherwise, so the stream above is untouched.
                    if let Some(sk) = self.ai_skill_choice(i) {
                        self.w.turn.played.push(sk.clone());
                        self.use_skill(i, &sk)?;
                        self.wait(1.2);
                        return Ok(());
                    }
                }
            }
            if self.w.st.skip_move {
                return self.end_turn_cmd(i);
            }
            let roller = if self.w.st.roller >= 0 {
                self.w.st.roller as usize
            } else {
                i
            };
            return self.main_move(i, roller);
        }
        let pos = self.w.st.players[i].pos as usize;
        // Bots do not mortgage to buy land (user ruling 2026-10-08): require
        // **cash alone** to cover the quoted price. `can_buy_here` is funded
        // by `purchase_funds` (cash + mortgageable deeds) -- legality for
        // humans is unchanged; the bot just declines (end-turn) instead of
        // auto-mortgaging. `ai_wants_buy` already reads cash, but the gate is
        // explicit here so a policy whose reserve is 0 cannot slip past it.
        let cash = self.w.st.players[i].money;
        let quote = self.buy_quote_for(i, pos, super::purchase::BuyKind::Land);
        let cash_covers_buy = quote.eligible && cash >= quote.price.max(0);
        if bot && self.can_buy_here(i) && cash_covers_buy && self.ai_wants_buy(i, pos) {
            self.w.st.bought = true;
            self.buy(i, pos, super::purchase::BuyKind::Land)?;
            self.wait(1.2);
        } else if bot && self.can_build_here(i) && self.ai_wants_build(i, pos) {
            self.w.st.built = true;
            self.build(i, pos)?;
            self.wait(1.2);
        } else if self.over_hand(i) {
            let hand_len = self.w.hidden[i].hand.len();
            let k = self.w.rng.below(hand_len);
            let card = self.w.hidden[i].hand[k].clone();
            self.discard(i, &card)?;
            self.wait(0.4);
        } else {
            self.end_turn_cmd(i)?;
        }
        Ok(())
    }
}