//! Ruleset-declared kind ids and kind/class strings (opaque to the engine).
//!
//! `card-sdk`'s [`TriggerKind`] is a transparent `i32` newtype. The engine
//! fires and dispatches by numeric id; the *names* of this game's raise points
//! live here so `crates/` and `rules/card-sdk` carry no game vocabulary.
//! Wire codes are stable (ABI v70).
//!
//! Call sites read `ruleset_core::kind::PassRewardAffected` (etc.). The values
//! share `TriggerKind`'s id space (`HookKind` / `ChainKind` are aliases).

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use card_sdk::abi::TriggerKind;

// ------------------------------------------------------- raise-point ids

/// 「经过奖励受影响」 (pass-reward half). Wire 26.
pub const PASS_REWARD_AFFECTED: i32 = 26;
/// 「每当你消耗火罐时」 (after the spend commits). Wire 73.
pub const FIRE_SPENT: i32 = 73;
/// 「使用自己原有的技能（2）时」. Wire 74.
pub const SKILL_USED: i32 = 74;
/// 「盖房」 (after the house count is written). Wire 75.
pub const HOUSE_ADDED: i32 = 75;
/// 「以代替此次技能的火罐消耗」 (before one charge spend commits). Wire 78.
pub const CHARGE_PAYING: i32 = 78;

/// 「经过奖励受影响」 -- a [反击] to the pass-reward pick, before the payout.
pub const PassRewardAffected: TriggerKind = TriggerKind(PASS_REWARD_AFFECTED);
/// 「每当你消耗火罐时」 -- after the spend commits.
pub const FireSpent: TriggerKind = TriggerKind(FIRE_SPENT);
/// 「使用自己原有的技能（2）时」 -- after the press.
pub const SkillUsed: TriggerKind = TriggerKind(SKILL_USED);
/// 「盖房」 -- after the house count is written.
pub const HouseAdded: TriggerKind = TriggerKind(HOUSE_ADDED);
/// Before one charge spend commits; cancel to substitute the cost.
pub const ChargePaying: TriggerKind = TriggerKind(CHARGE_PAYING);

// ------------------------------------------------------- tile kind strings

/// Start square wire kind.
pub const START: &str = "circle";
/// Flat deed wire kind.
pub const DEED: &str = "ring";
/// 地产商 wire kind.
pub const AGENT: &str = "agent";

/// Tile-class wire names (`tile_in` classes) and their definitions.
pub mod class {
    /// Deed / buyable class.
    pub const BUYABLE: &str = "buyable";
    /// 「商店街」 class.
    pub const SHOP: &str = "shop";
    /// Venue class.
    pub const VENUE: &str = "livehouse";
    /// Colour group of the 「商店街」 deeds.
    pub const SHOP_GROUP: i32 = 10;
    /// Colour group of the venue deeds.
    pub const VENUE_GROUP: i32 = 6;
}

/// Class definitions the engine's `tile_in` table is seeded with at boot:
/// `(name, requires_buyable, matching colour groups)`.
pub fn class_defs() -> Vec<(String, bool, Vec<i32>)> {
    vec![
        (class::BUYABLE.into(), true, vec![]),
        (class::SHOP.into(), true, vec![class::SHOP_GROUP]),
        (class::VENUE.into(), true, vec![class::VENUE_GROUP]),
    ]
}