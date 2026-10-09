//! Replayable console cheats -- debug builds only.
//!
//! Compiled in only when `cfg(debug_assertions)` (the `mod debug` declaration
//! in `engine/mod.rs` is gated the same way), so a `--release` binary or the
//! production browser glue has no cheat handling at all and a `debug` act is
//! refused as an unknown command. Solo matches only: online rooms are always
//! Casual / Ranked and must never take a cheat input (`docs/SERVER.md`).
//!
//! Validate everything before changing the world; a suspended routine would
//! otherwise overwrite changes when it resumes.

#![cfg(debug_assertions)]

use super::Match;
use crate::{msg::Msg, net::NetMessage, state::stage, MatchMode};

impl Match {
    pub(super) fn debug_act(&mut self, actor: usize, m: &NetMessage) -> Result<(), Msg> {
        if self.mode != MatchMode::Solo {
            return Err(Msg::new("err.debug_solo_only"));
        }
        if self.world.st.phase != "play" {
            return Err(Msg::new("err.debug_play_only"));
        }
        if self.pending.is_some() || self.world.st.step == stage::MOVE {
            return Err(Msg::new("err.debug_busy"));
        }
        let target = if m.target == -1 {
            actor
        } else {
            usize::try_from(m.target).map_err(|_| Msg::new("err.debug_target"))?
        };
        if self.world.st.players.get(target).is_none_or(|p| p.out()) {
            return Err(Msg::new("err.debug_target"));
        }
        let valid = match m.debug.as_str() {
            "money" => (0..=100_000_000).contains(&m.value),
            "tp" => (0..self.data.tiles.len() as i32).contains(&m.value),
            "give" => (1..=100).contains(&m.value) && self.data.card(&m.card).is_some(),
            "draw" => (1..=100).contains(&m.value),
            "state" => {
                (-1_000_000..=1_000_000).contains(&m.value)
                    && self.world.st.players[target]
                        .state
                        .contains_key(&m.character)
            }
            _ => false,
        };
        if !valid {
            return Err(Msg::new("err.debug_argument"));
        }
        self.direct(|cx| {
            match m.debug.as_str() {
                "money" => cx.w.st.players[target].money = m.value,
                "tp" => {
                    cx.w.teleport_to(target as i32, m.value);
                    if cx.w.st.turn == target as i32 && cx.w.st.step == stage::END {
                        cx.w.st.landed = m.value;
                    }
                }
                "give" => {
                    for _ in 0..m.value {
                        cx.w.add_to_hand(target as i32, &m.card);
                    }
                }
                "draw" => {
                    cx.w.draw_cards(target as i32, m.value, true);
                }
                "state" => {
                    cx.w.st.players[target].state_set(&m.character, m.value);
                }
                _ => unreachable!(),
            }
            cx.w.st.debug_open = true;
            cx.log(
                actor as i32,
                Msg::new("log.debug")
                    .player_id("who", target as i32)
                    .text("command", &m.debug)
                    .i("value", m.value)
                    .text(
                        "detail",
                        if m.debug == "give" {
                            &m.card
                        } else {
                            &m.character
                        },
                    ),
            );
        });
        Ok(())
    }
}
