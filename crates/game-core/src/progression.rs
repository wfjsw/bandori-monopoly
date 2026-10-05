//! `Progression.cs` + `MatchReward.cs` -- levels, EXP, fire (stamina), coins.

use serde::{Deserialize, Serialize};

use crate::MatchMode;

pub const MAX_LEVEL: i32 = 500;
/// Fire refilled each day.
pub const FIRE_DAILY_MAX: i32 = 5;
/// Fire a player may spend on one match (each multiplies EXP).
pub const FIRE_MAX_PER_GAME: i32 = 3;
pub const STARS_PER_GAME: i32 = 1;

const TOP_COINS: [i32; 3] = [500, 300, 100];
const BOTTOM_COINS: [i32; 3] = [-500, -300, -100];

/// EXP needed to go from `level` to `level + 1`; 0 at the cap.
pub fn exp_to_next(level: i32) -> i32 {
    if level < MAX_LEVEL {
        100 + 15 * level
    } else {
        0
    }
}

/// EXP for finishing `rank` (1 = first) of `players`, before the fire multiplier.
pub fn base_exp(ranked: bool, rank: i32, players: i32) -> i32 {
    let beaten = (players - rank).max(0);
    if ranked {
        100 + 20 * beaten
    } else {
        80 + 10 * beaten
    }
}

pub fn exp_multiplier(fire_used: i32) -> i32 {
    1 + fire_used.max(0)
}

/// Ranked coin change. Top three gain 500/300/100; otherwise the last three lose
/// 500/300/100 counting from the bottom.
///
/// Faithful to the C#, including its overlap in small games: with 4 players, 4th
/// place is "last" and loses 500 even though 3rd gains 100.
pub fn ranked_coins(rank: i32, players: i32) -> i32 {
    if rank >= 1 && rank as usize <= TOP_COINS.len() && rank <= players {
        return TOP_COINS[rank as usize - 1];
    }
    let from_bottom = players - rank;
    if from_bottom >= 0 && (from_bottom as usize) < BOTTOM_COINS.len() {
        return BOTTOM_COINS[from_bottom as usize];
    }
    0
}

/// `MatchReward.cs` -- what one finished match gave the player.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchReward {
    pub mode: MatchMode,
    pub rank: i32,
    pub players: i32,
    pub base_exp: i32,
    pub fire_used: i32,
    pub multiplier: i32,
    pub exp: i32,
    pub level_before: i32,
    pub level_after: i32,
    pub progress_before: f32,
    pub progress_after: f32,
    pub coins: i32,
    pub stars: i32,
}

impl MatchReward {
    pub fn ranked(&self) -> bool {
        self.mode == MatchMode::Ranked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_curve() {
        assert_eq!(exp_to_next(0), 100);
        assert_eq!(exp_to_next(1), 115);
        assert_eq!(exp_to_next(499), 7585);
        assert_eq!(exp_to_next(500), 0);
    }

    #[test]
    fn base_exp_by_mode() {
        assert_eq!(base_exp(false, 1, 4), 110);
        assert_eq!(base_exp(true, 1, 6), 200);
        assert_eq!(base_exp(true, 6, 6), 100);
        assert_eq!(
            base_exp(false, 9, 4),
            80,
            "rank past players clamps to 0 beaten"
        );
        assert_eq!(exp_multiplier(3), 4);
        assert_eq!(exp_multiplier(-1), 1);
    }

    #[test]
    fn ranked_coins_six_players() {
        let got: Vec<i32> = (1..=6).map(|r| ranked_coins(r, 6)).collect();
        assert_eq!(got, [500, 300, 100, -100, -300, -500]);
    }

    #[test]
    fn ranked_coins_small_game_quirk_is_preserved() {
        let got: Vec<i32> = (1..=4).map(|r| ranked_coins(r, 4)).collect();
        assert_eq!(got, [500, 300, 100, -500]);
        let got: Vec<i32> = (1..=5).map(|r| ranked_coins(r, 5)).collect();
        assert_eq!(got, [500, 300, 100, -300, -500]);
    }
}
