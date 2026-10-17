//! Ruleset-declared kind ids and kind/class strings (opaque to the engine).
//!
//! `card-sdk`'s [`TriggerKind`] is a transparent `i32` newtype. The engine
//! fires and dispatches by numeric id; the *names* of this game's raise points
//! live here so `crates/` and `rules/card-sdk` carry no game vocabulary.
//! Wire codes are stable (ABI v70).
//!
//! Call sites read `ruleset_core::kind::PassRewardAffected` (etc.). The values
//! share `TriggerKind`'s id space. Each rule-defined kind is declared on its
//! typed side -- today every one is [`ActionKind`] (an `On::Action` listener).
//!
//! A future rule-defined **FILTER** kind would declare `FilterKind(id)` here
//! **and** teach `TriggerKind::is_filter()` (or a `CUSTOM_FILTERS` table beside
//! `CUSTOM_LINKS` in `card-sdk::abi`) about that id, so the typed sets keep
//! mirroring `is_filter()` exactly. Known gap: no rule-defined filter kinds yet.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use card_sdk::abi::ActionKind;

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
/// 「将X个反面[P㏄P粉丝]变正」 (after the flip request is known, before the
/// dict writes). Wire 107. `trigger::value()` is the effect's intended X.
pub const FANS_FLIP: i32 = 107;

/// 「经过奖励受影响」 -- a [反击] to the pass-reward pick, before the payout.
/// Dual-role: also a [`card_sdk::abi::ChainKind`] (`CUSTOM_LINKS` 26).
pub const PassRewardAffected: ActionKind = ActionKind(PASS_REWARD_AFFECTED);
/// 「每当你消耗火罐时」 -- after the spend commits.
pub const FireSpent: ActionKind = ActionKind(FIRE_SPENT);
/// 「使用自己原有的技能（2）时」 -- after the press.
/// Dual-role: also a [`card_sdk::abi::ChainKind`] (`CUSTOM_LINKS` 74).
pub const SkillUsed: ActionKind = ActionKind(SKILL_USED);
/// 「盖房」 -- after the house count is written.
pub const HouseAdded: ActionKind = ActionKind(HOUSE_ADDED);
/// Before one charge spend commits; cancel to substitute the cost. Action
/// (prompts; not a FilterFlag).
pub const ChargePaying: ActionKind = ActionKind(CHARGE_PAYING);
/// 「将X个反面[P㏄P粉丝]变正」 -- raised by [`rule fans::flip_up`] before the
/// dict writes so a listener sees the pre-flip face-down count. Action kind.
pub const FansFlip: ActionKind = ActionKind(FANS_FLIP);

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