//! Per-match clock / timer settings, adjustable by the room host before start.
//!
//! Every waiting situation draws on one of these knobs:
//!
//! | knob | default | meaning |
//! |---|---|---|
//! | [`ClockSettings::base_time`] | 15 | initial per-seat 恢复时间 pool (s) |
//! | [`ClockSettings::bonus_time`] | 0 | initial per-seat 免费时间 pool (s) |
//! | [`ClockSettings::base_refill`] | 15 | per-turn base refill (s), capped at `base_time` |
//! | [`ClockSettings::ban_seconds`] | 20 | Ranked ban decision window (s) |
//! | [`ClockSettings::pick_seconds`] | 25 | pick decision window (s) |
//! | [`ClockSettings::deck_seconds`] | 45 | deck-build window (s) |
//! | [`ClockSettings::bankrupt_auction_time`] | 10 | bankruptcy leftover auction window (s) |
//!
//! The acting seat's **pool** (`base_time` + `bonus_time`) is consumed for
//! ordinary turn waiting *and* ordinary prompts. Bonus drains first, then base.
//! Ban / pick / deck and the bankruptcy leftover auction keep their own fixed
//! windows and never touch a seat pool. Solo never expires any of them.
//!
//! Defaults reproduce the pre-settings feel: base resets to 15 each turn
//! (`base_refill` == `base_time` == 15), no starting bonus, setup windows
//! unchanged, bankrupt auction = the old hard-coded 10 s.
//!
//! Host-side sanitization clamps every field to [`Self::MIN`]..=[`Self::MAX`]
//! seconds. The initial `bonus_time` may exceed [`Self::BONUS_MAX`]; the refill
//! cap is then lifted to the initial value so a deliberately large starting
//! bonus is not immediately erased (documented in `docs/SERVER.md` §Turn clock).

use serde::{Deserialize, Serialize};

/// Turn-clock / timer settings for one match. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClockSettings {
    /// Initial per-seat 恢复时间 pool, seconds. Also the refill cap for
    /// [`Self::base_refill`] (a `turnBaseTime` hook that grants a larger refill
    /// lifts the cap to at least that refill -- see `engine` `StartTimer`).
    pub base_time: f32,
    /// Initial per-seat 免费时间 pool, seconds. May exceed [`Self::BONUS_MAX`];
    /// the refill cap lifts to this value so the pool can hold it.
    pub bonus_time: f32,
    /// Per-turn base refill, seconds (the `turnBaseTime` hook's default value).
    /// Applied as `min(base + refill, max(base_time, refill))` at turn start.
    pub base_refill: f32,
    /// Ranked ban decision window, seconds (`setup::BAN_SECONDS` default).
    pub ban_seconds: f32,
    /// Pick decision window, seconds (`setup::PICK_SECONDS` default).
    pub pick_seconds: f32,
    /// Deck-build window, seconds (`setup::DECK_SECONDS` default).
    pub deck_seconds: f32,
    /// Bankruptcy leftover-auction window, seconds. **Not** drawn from any
    /// seat pool: a liquidation auction is a shared board event with its own
    /// timer (the old hard-coded 10 s `Ask::auction` allotment).
    pub bankrupt_auction_time: f32,
}

impl Default for ClockSettings {
    fn default() -> Self {
        Self {
            base_time: 15.0,
            bonus_time: 0.0,
            base_refill: 15.0,
            ban_seconds: 20.0,
            pick_seconds: 25.0,
            deck_seconds: 45.0,
            bankrupt_auction_time: 10.0,
        }
    }
}

impl ClockSettings {
    /// Lower bound for every knob, seconds. `0` disables that waiting budget
    /// (the engine then times the seat out immediately where a timer applies).
    pub const MIN: f32 = 0.0;
    /// Upper bound for every knob, seconds.
    pub const MAX: f32 = 300.0;
    /// Default / nominal cap on the 免费时间 pool when refilling. Lifted to
    /// [`Self::bonus_time`] when the initial bonus is deliberately larger.
    pub const BONUS_MAX: f32 = 30.0;

    /// The 免费时间 refill cap for this match.
    pub fn bonus_max(&self) -> f32 {
        Self::BONUS_MAX.max(self.bonus_time)
    }

    /// Clamp every field to [`Self::MIN`]..=[`Self::MAX`]; non-finite values
    /// fall back to the default for that field. Server-side gate before a
    /// room accepts a host edit (`ScoreWeights::sanitized`'s sibling).
    pub fn sanitized(&self) -> Self {
        fn fix(v: f32, dflt: f32) -> f32 {
            if !v.is_finite() {
                dflt
            } else {
                v.clamp(ClockSettings::MIN, ClockSettings::MAX)
            }
        }
        let d = Self::default();
        Self {
            base_time: fix(self.base_time, d.base_time),
            bonus_time: fix(self.bonus_time, d.bonus_time),
            base_refill: fix(self.base_refill, d.base_refill),
            ban_seconds: fix(self.ban_seconds, d.ban_seconds),
            pick_seconds: fix(self.pick_seconds, d.pick_seconds),
            deck_seconds: fix(self.deck_seconds, d.deck_seconds),
            bankrupt_auction_time: fix(self.bankrupt_auction_time, d.bankrupt_auction_time),
        }
    }

    /// True when every field equals [`Default`] -- used so the fairness canon
    /// string stays byte-identical to pre-settings records when a match runs
    /// on defaults (`fair::CanonSettings::canon`).
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_legacy_constants() {
        let c = ClockSettings::default();
        assert_eq!(c.base_time, 15.0);
        assert_eq!(c.bonus_time, 0.0);
        assert_eq!(c.base_refill, 15.0);
        assert_eq!(c.ban_seconds, 20.0);
        assert_eq!(c.pick_seconds, 25.0);
        assert_eq!(c.deck_seconds, 45.0);
        assert_eq!(c.bankrupt_auction_time, 10.0);
        assert_eq!(c.bonus_max(), 30.0);
        assert!(c.is_default());
    }

    #[test]
    fn sanitize_clamps_and_repairs() {
        let c = ClockSettings {
            base_time: 10_000.0,
            bonus_time: f32::NAN,
            base_refill: -5.0,
            ban_seconds: 0.0,
            pick_seconds: 250.0,
            deck_seconds: 300.5,
            bankrupt_auction_time: f32::INFINITY,
        }
        .sanitized();
        assert_eq!(c.base_time, ClockSettings::MAX);
        assert_eq!(c.bonus_time, ClockSettings::default().bonus_time);
        assert_eq!(c.base_refill, ClockSettings::MIN);
        assert_eq!(c.ban_seconds, 0.0);
        assert_eq!(c.pick_seconds, 250.0);
        assert_eq!(c.deck_seconds, ClockSettings::MAX);
        // Non-finite falls back to the field default, not the clamp bound.
        assert_eq!(c.bankrupt_auction_time, ClockSettings::default().bankrupt_auction_time);
    }

    #[test]
    fn large_initial_bonus_lifts_the_refill_cap() {
        let c = ClockSettings {
            bonus_time: 45.0,
            ..Default::default()
        };
        assert_eq!(c.bonus_max(), 45.0);
        assert_eq!(ClockSettings::default().bonus_max(), 30.0);
    }
}