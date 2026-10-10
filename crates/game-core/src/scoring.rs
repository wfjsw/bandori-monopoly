//! End-of-match score weights, adjustable per room.
//!
//! score = money * w.money + property * w.property + houses * w.houses
//! (property = land prices, mortgaged at half; houses = build costs).
//! The RiNG rent multiplier is a separate mechanic ([`MatchRulesData::ring_multiplier`]).

use serde::{Deserialize, Serialize};

use crate::data::MatchRulesData;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScoreWeights {
    pub money: f32,
    pub property: f32,
    pub houses: f32,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            money: 1.0,
            property: 1.0,
            houses: 1.0,
        }
    }
}

impl ScoreWeights {
    /// UI step for each weight.
    pub const STEP: f32 = 0.5;
    /// Upper bound for each weight.
    pub const MAX: f32 = 5.0;

    /// Defaults from `match_rules.json`.
    pub fn from_rules(r: &MatchRulesData) -> Self {
        Self {
            money: r.money,
            property: r.property,
            houses: r.houses,
        }
    }

    /// `this[k]`: 0 = money, 1 = property, anything else = houses.
    pub fn get(&self, k: i32) -> f32 {
        match k {
            0 => self.money,
            1 => self.property,
            _ => self.houses,
        }
    }

    pub fn set(&mut self, k: i32, v: f32) {
        match k {
            0 => self.money = v,
            1 => self.property = v,
            _ => self.houses = v,
        }
    }

    /// Snap each weight to a multiple of 0.5 within [0, 5]. If everything ends up 0,
    /// fall back to `default` (the rules-file weights).
    pub fn sanitized(&self, default: ScoreWeights) -> Self {
        fn fix(v: f32) -> f32 {
            if !v.is_finite() {
                return 0.0;
            }
            // Banker's rounding: ties go to even (IEEE-754-style).
            ((v / ScoreWeights::STEP).round_ties_even() * ScoreWeights::STEP)
                .clamp(0.0, ScoreWeights::MAX)
        }
        let w = Self {
            money: fix(self.money),
            property: fix(self.property),
            houses: fix(self.houses),
        };
        if w.money + w.property + w.houses > 0.0 {
            w
        } else {
            default
        }
    }

    /// Approximate equality: each weight within a relative epsilon.
    pub fn same_as(&self, o: &ScoreWeights) -> bool {
        let approx = |a: f32, b: f32| (a - b).abs() < f32::EPSILON.max(1e-6 * a.abs().max(b.abs()));
        approx(self.money, o.money)
            && approx(self.property, o.property)
            && approx(self.houses, o.houses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_snaps_clamps_and_falls_back() {
        let d = ScoreWeights::default();
        let w = ScoreWeights {
            money: 1.3,
            property: 9.0,
            houses: -2.0,
        }
        .sanitized(d);
        assert_eq!(
            w,
            ScoreWeights {
                money: 1.5,
                property: 5.0,
                houses: 0.0
            }
        );
        // 0.25 / 0.5 = 0.5 -> ties to even -> 0 (banker's rounding)
        let z = ScoreWeights {
            money: 0.25,
            property: 0.0,
            houses: 0.0,
        }
        .sanitized(d);
        assert_eq!(z, d, "all-zero falls back to default");
    }
}
