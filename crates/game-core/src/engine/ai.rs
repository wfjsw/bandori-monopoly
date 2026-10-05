//! Bot decisions (`AiStep` and friends). Also drives humans who ran out of time.

use super::cx::{Cx, Flow};

impl Cx<'_> {
    /// `AiWantsBuy` -- keep at least 2,000 after buying.
    pub(crate) fn ai_wants_buy(&self, i: usize, t: usize) -> bool {
        self.w.st.players[i].money - self.buy_price(t) >= 2000
    }

    /// `AiWantsBuild` -- keep at least 3,500 after building.
    pub(crate) fn ai_wants_build(&self, i: usize, t: usize) -> bool {
        self.w.st.players[i].money - self.build_cost(t) >= 3500
    }

    /// `AiAgentChoice` -- first affordable purchase, else first build, else none.
    pub(crate) fn ai_agent_choice(&self, p: usize, options: &[usize]) -> i32 {
        if let Some(k) = options.iter().position(|&t| self.w.st.owners[t] < 0 && self.ai_wants_buy(p, t)) {
            return k as i32;
        }
        if let Some(k) = options.iter().position(|&t| self.w.st.owners[t] == p as i32 && self.ai_wants_build(p, t)) {
            return k as i32;
        }
        options.len() as i32
    }

    /// `AiRedeemChoice` -- most valuable mortgaged deed that leaves 4,000.
    fn ai_redeem_choice(&self, i: usize) -> Option<usize> {
        let mut deeds: Vec<usize> = (0..self.data.tiles.len()).filter(|&t| self.w.st.owners[t] == i as i32 && self.w.st.mortgaged[t]).collect();
        deeds.sort_by_key(|&t| std::cmp::Reverse(self.tile(t).price));
        deeds.into_iter().find(|&t| self.w.st.players[i].money - self.redeem_cost(t) >= 4000)
    }

    /// `AiCardChoice` -- a random playable card the rules say a bot would play.
    fn ai_card_choice(&mut self, i: usize) -> Option<String> {
        let mut hand = self.w.hidden[i].hand.clone();
        hand.dedup();
        let ok: Vec<String> = hand
            .into_iter()
            .filter(|id| self.cant_play(i, id, false).is_none() && self.rules.ai_play(self, i, id))
            .collect();
        if ok.is_empty() {
            None
        } else {
            let k = self.w.rng.below(ok.len());
            Some(ok[k].clone())
        }
    }

    /// `AiStep` -- one decision for the player whose turn it is.
    pub(crate) fn ai_step(&mut self, i: usize) -> Flow<()> {
        let bot = self.w.st.players[i].ai;
        if self.w.st.step == 1 {
            if bot {
                if let Some(t) = self.ai_redeem_choice(i) {
                    self.redeem(i, t);
                    self.wait(1.2);
                    return Ok(());
                }
                if self.w.turn.played.len() < 2 && self.w.rng.chance(0.7) {
                    if let Some(card) = self.ai_card_choice(i) {
                        self.play_from_hand(i, &card)?;
                        self.wait(1.2);
                        return Ok(());
                    }
                }
            }
            if self.w.st.skip_move {
                return self.end_turn_cmd(i);
            }
            let roller = if self.w.st.roller >= 0 { self.w.st.roller as usize } else { i };
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
