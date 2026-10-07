//! Bot decisions (`AiStep` and friends). Also drives humans who ran out of time.
//!
//! Every seat carries a [`BotMentality`] (only `bot` seats take one): **standard**
//! is the ported C# policy and **chaos** is legal but maximally disruptive. The
//! thresholds below are the standard policy; chaos replaces the money reserves
//! with [`CHAOS_RESERVE`] and drops the "sometimes" rolls. The web client's 托管
//! (auto-play) toggle ports both policies to TypeScript over the public match
//! view (`webui/src/game/autopilot.ts`); keep the two in sync.
//!
//! A human who times out or disconnects is answered with the **standard**
//! policy even though `ai` flips on -- see [`Cx::bot_mentality`].

use super::cx::{Cx, Flow};
use crate::state::{stage, BotMentality};

/// `AiWantsBuy` -- keep at least this much money after buying.
pub const BUY_RESERVE: i32 = 2_000;
/// `AiWantsBuild` -- keep at least this much money after building.
pub const BUILD_RESERVE: i32 = 3_500;
/// `AiRedeemChoice` -- keep at least this much money after redeeming.
pub const REDEEM_RESERVE: i32 = 4_000;
/// `OfferForceBuy` -- buy out a mortgaged deed only while it leaves this much.
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

/// `AiWantsBuy` -- would buying `price` leave [`BUY_RESERVE`]?
pub fn wants_buy(money: i32, price: i32) -> bool {
    money - price >= BUY_RESERVE
}

/// `AiWantsBuild` -- would building for `cost` leave [`BUILD_RESERVE`]?
pub fn wants_build(money: i32, cost: i32) -> bool {
    money - cost >= BUILD_RESERVE
}

/// Would redeeming for `cost` leave [`REDEEM_RESERVE`]?
pub fn wants_redeem(money: i32, cost: i32) -> bool {
    money - cost >= REDEEM_RESERVE
}

/// Mentality-aware buy test: standard keeps [`BUY_RESERVE`], chaos only
/// [`CHAOS_RESERVE`].
pub fn bot_wants_buy(m: BotMentality, money: i32, price: i32) -> bool {
    match m {
        BotMentality::Standard => wants_buy(money, price),
        BotMentality::Chaos => money - price >= CHAOS_RESERVE,
    }
}

/// Mentality-aware build test.
pub fn bot_wants_build(m: BotMentality, money: i32, cost: i32) -> bool {
    match m {
        BotMentality::Standard => wants_build(money, cost),
        BotMentality::Chaos => money - cost >= CHAOS_RESERVE,
    }
}

/// Mentality-aware redeem test.
pub fn bot_wants_redeem(m: BotMentality, money: i32, cost: i32) -> bool {
    match m {
        BotMentality::Standard => wants_redeem(money, cost),
        BotMentality::Chaos => money - cost >= CHAOS_RESERVE,
    }
}

/// Mentality-aware 「可选择[支付]…两倍…强行购买」 test.
pub fn bot_wants_force_buy(m: BotMentality, money: i32, price: i32) -> bool {
    match m {
        BotMentality::Standard => money - price >= FORCE_BUY_RESERVE,
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

    /// `AiWantsBuy`.
    #[inline]
    pub(crate) fn ai_wants_buy(&self, i: usize, t: usize) -> bool {
        if self.is_chaos(i) {
            self.w.st.players[i].money - self.buy_price(t) >= CHAOS_RESERVE
        } else {
            wants_buy(self.w.st.players[i].money, self.buy_price(t))
        }
    }

    /// `AiWantsBuild`.
    #[inline]
    pub(crate) fn ai_wants_build(&self, i: usize, t: usize) -> bool {
        if self.is_chaos(i) {
            self.w.st.players[i].money - self.build_cost(t) >= CHAOS_RESERVE
        } else {
            wants_build(self.w.st.players[i].money, self.build_cost(t))
        }
    }

    /// `AiAgentChoice` -- standard: first affordable purchase, else first build,
    /// else none. Chaos: a random option it can pay for, never "none" while one
    /// exists.
    pub(crate) fn ai_agent_choice(&mut self, p: usize, options: &[usize]) -> i32 {
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
        if let Some(k) = options
            .iter()
            .position(|&t| self.w.st.owners[t] < 0 && wants_buy(self.w.st.players[p].money, self.buy_price(t)))
        {
            return k as i32;
        }
        if let Some(k) = options.iter().position(|&t| {
            self.w.st.owners[t] == p as i32
                && wants_build(self.w.st.players[p].money, self.build_cost(t))
        }) {
            return k as i32;
        }
        options.len() as i32
    }

    /// `AiRedeemChoice` -- standard: most valuable mortgaged deed that leaves
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
        deeds.sort_by_key(|&t| std::cmp::Reverse(self.tile(t).price));
        deeds
            .into_iter()
            .find(|&t| wants_redeem(self.w.st.players[i].money, self.redeem_cost(t)))
    }

    /// `AiCardChoice` -- a random playable card the rules say a bot would play.
    /// Chaos ignores the rule's `ai_play` heuristic (it will even fire [反击]
    /// cards from hand) and only asks `cant_play`.
    fn ai_card_choice(&mut self, i: usize) -> Option<String> {
        let chaos = self.is_chaos(i);
        let mut hand = self.w.hidden[i].hand.clone();
        hand.dedup();
        let ok: Vec<String> = if chaos {
            hand.into_iter()
                .filter(|id| self.cant_play(i, id, false).is_none())
                .collect()
        } else {
            hand.into_iter()
                .filter(|id| self.cant_play(i, id, false).is_none() && self.rules.ai_play(self, i, id))
                .collect()
        };
        if ok.is_empty() {
            None
        } else {
            let k = self.w.rng.below(ok.len());
            Some(ok[k].clone())
        }
    }

    /// A skill button chaos would press right now: a placed character / band
    /// skill rule the player may activate. Standard bots never press skills
    /// (the C# left that to the player). The gate is the rule's own `cant_play`
    /// (`why_not_act` asks the same one before honouring a `skill` command),
    /// plus at most one press per skill per turn -- a skill whose body is a
    /// no-op must not park the bot in a press loop.
    fn ai_skill_choice(&mut self, i: usize) -> Option<String> {
        let placed = self.w.placed_cards(i as i32);
        let ok: Vec<String> = self
            .data
            .skill_rules_of(&self.w.st.players[i].character)
            .into_iter()
            .filter(|id| {
                placed.contains(id)
                    && !self.w.turn.played.contains(id)
                    && self.rules.cant_play(self, i, id).is_none()
            })
            .collect();
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
        let v = ((base as f64 * (0.6 + self.w.rng.f64() * 0.7) / 100.0) as i32) * 100;
        v.min(money - 1000)
    }

    /// `AiStep` -- one decision for the player whose turn it is.
    ///
    /// The standard path below is the C# bot unchanged -- the chaos branch is
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
                    if let Some(t) = self.ai_redeem_choice(i) {
                        self.redeem(i, t);
                        self.wait(1.2);
                        return Ok(());
                    }
                    if self.w.turn.played.len() < MAX_PLAYS_PER_TURN
                        && self.w.rng.chance(PLAY_CARD_CHANCE)
                    {
                        if let Some(card) = self.ai_card_choice(i) {
                            self.play_from_hand(i, &card)?;
                            self.wait(1.2);
                            return Ok(());
                        }
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
        if bot && self.can_buy_here(i) && self.ai_wants_buy(i, pos) {
            self.w.st.bought = true;
            self.buy(i, pos)?;
            self.wait(1.2);
        } else if bot && self.can_build_here(i) && self.ai_wants_build(i, pos) {
            self.w.st.built = true;
            self.build(i, pos)?;
            self.wait(1.2);
        } else if self.over_hand(i) {
            let k = self.w.rng.below(self.w.hidden[i].hand.len());
            let card = self.w.hidden[i].hand[k].clone();
            self.discard(i, &card)?;
            self.wait(0.4);
        } else {
            self.end_turn_cmd(i)?;
        }
        Ok(())
    }
}