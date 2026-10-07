//! Pre-game phases: turn order, ban (Ranked only), pick, deck.
//!
//! These never prompt, so they run directly on the world without replay.

use super::cx::Cx;
use crate::deck;
use crate::msg::{Arg, Msg};

/// Seconds per ban / pick / deck decision (`StartChoice`).
pub const BAN_SECONDS: f32 = 20.0;
pub const PICK_SECONDS: f32 = 25.0;
pub const DECK_SECONDS: f32 = 45.0;

impl Cx<'_> {
    /// `RollOrder` -- 1d20 each, ties re-roll; players are reordered high to low.
    pub(crate) fn roll_order(&mut self) {
        let n = self.w.player_count();
        for i in 0..n {
            self.w.st.players[i].roll = self.w.rng.d(20);
        }
        let mut rerolled = false;
        loop {
            let tied: Vec<usize> = (0..n)
                .filter(|&i| {
                    (0..n).any(|j| j != i && self.w.st.players[j].roll == self.w.st.players[i].roll)
                })
                .collect();
            if tied.is_empty() {
                break;
            }
            rerolled = true;
            for i in tied {
                self.w.st.players[i].roll = self.w.rng.d(20);
            }
        }
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(self.w.st.players[i].roll));
        let players = std::mem::take(&mut self.w.st.players);
        let hidden = std::mem::take(&mut self.w.hidden);
        self.w.st.players = order.iter().map(|&i| players[i].clone()).collect();
        self.w.hidden = order.iter().map(|&i| hidden[i].clone()).collect();
        let list = (0..self.w.st.players.len())
            .map(|p| {
                Arg::Msg(Box::new(
                    Msg::new("log.part.player_roll")
                        .player_id("who", p)
                        .i("n", self.w.st.players[p].roll),
                ))
            })
            .collect();
        let key = if rerolled {
            "log.order_rerolled"
        } else {
            "log.order"
        };
        self.w.log("text", -1, Msg::new(key).list("rolls", list));
    }

    pub(crate) fn bannable(&self, character: &str) -> bool {
        self.data.character(character).is_some() && !self.w.st.bans.iter().any(|b| b == character)
    }

    pub(crate) fn pickable(&self, character: &str) -> bool {
        self.bannable(character) && self.w.st.players.iter().all(|s| s.character != character)
    }

    /// `RandomCharacter` -- prefers characters with their own art set (`cnId`).
    pub(crate) fn random_character(&mut self, for_pick: bool) -> String {
        let all: Vec<&str> = self
            .data
            .characters
            .iter()
            .map(|c| c.name.as_str())
            .filter(|c| {
                if for_pick {
                    self.pickable(c)
                } else {
                    self.bannable(c)
                }
            })
            .collect();
        let with_art: Vec<&str> = all
            .iter()
            .copied()
            .filter(|c| self.data.character(c).is_some_and(|x| !x.cn_id.is_empty()))
            .collect();
        let list = if with_art.is_empty() { all } else { with_art };
        if list.is_empty() {
            return String::new();
        }
        let k = self.w.rng.below(list.len());
        list[k].to_string()
    }

    /// `BeginBan` -- back to front.
    pub(crate) fn begin_ban(&mut self) {
        self.w.st.phase = "ban".into();
        self.w.st.turn = self.w.player_count() as i32 - 1;
        self.w.st.time_left = BAN_SECONDS;
        self.w.log("text", -1, Msg::new("log.ban_phase"));
    }

    /// `BeginPick` -- front to back.
    pub(crate) fn begin_pick(&mut self) {
        self.w.st.phase = "pick".into();
        self.w.st.turn = 0;
        self.w.st.time_left = PICK_SECONDS;
        self.w.log("text", -1, Msg::new("log.pick_phase"));
    }

    /// `DoBan`; empty `character` = no ban.
    pub(crate) fn do_ban(&mut self, i: usize, character: &str) {
        let s = &mut self.w.st.players[i];
        s.ban_done = true;
        s.ban = character.into();
        if !character.is_empty() {
            self.w.st.bans.push(character.into());
        }
        let text = if character.is_empty() {
            Msg::new("log.no_ban").player_id("who", i)
        } else {
            Msg::new("log.ban")
                .player_id("who", i)
                .chara("chara", character)
        };
        self.w.log("ban", i as i32, text);
        self.w.st.turn -= 1;
        if self.w.st.turn < 0 {
            self.begin_pick();
        } else {
            self.w.st.time_left = BAN_SECONDS;
        }
    }

    /// `DoPick`. Bots submit their deck as soon as the deck phase opens.
    pub(crate) fn do_pick(&mut self, i: usize, character: &str) {
        self.w.st.players[i].character = character.into();
        let d = self.data;
        self.w.bind_skills(d, self.rules, i as i32);
        self.w.log(
            "pick",
            i as i32,
            Msg::new("log.pick")
                .player_id("who", i)
                .chara("chara", character),
        );
        self.w.st.turn += 1;
        if self.w.st.turn as usize >= self.w.player_count() {
            self.w.st.phase = "deck".into();
            self.w.st.turn = -1;
            self.w.log(
                "text",
                -1,
                Msg::new("log.deck_phase").i("n", crate::deck::SIZE as i64),
            );
            for p in 0..self.w.player_count() {
                if self.w.st.players[p].ai {
                    self.submit_deck(p, None);
                }
            }
            self.w.st.time_left = DECK_SECONDS;
        } else {
            self.w.st.time_left = PICK_SECONDS;
        }
    }

    /// `SubmitDeck` -- a complete legal deck, or the character's preset.
    pub(crate) fn submit_deck(&mut self, i: usize, cards: Option<&[String]>) {
        let character = self.w.st.players[i].character.clone();
        let list = match self.data.character(&character) {
            Some(c) => match cards {
                Some(ids) if deck::is_complete(self.data, c, ids) => deck::clean(self.data, c, ids),
                _ => deck::preset(self.data, c),
            },
            None => vec![],
        };
        self.w.hidden[i].draw = list;
        self.w.st.players[i].deck_ready = true;
        if !self.w.st.players[i].ai {
            self.w.log(
                "deck",
                i as i32,
                Msg::new("log.deck_ready").player_id("who", i),
            );
        }
    }
}
