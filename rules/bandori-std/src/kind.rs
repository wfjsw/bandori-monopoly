//! Ruleset-declared kind ids and kind/class strings (opaque to the engine).
//!
//! The engine fires and dispatches by numeric id; the names of this game's
//! raise points live here so the engine crates and card-sdk carry no game
//! vocabulary. The values share the trigger-kind wire id space. Each
//! rule-defined kind is declared on its typed side -- today every one is an
//! action kind (an event-reaction listener).
//!
//! A future rule-defined filter kind must declare `FilterKind(id)` here and
//! also register that id as a filter in the trigger-kind filter test, so the
//! typed sets keep mirroring that test exactly. No rule-defined filter kinds
//! exist yet.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use card_sdk::abi::ActionKind;

// ------------------------------------------------------- raise-point ids

/// Pass-reward half of the pass reward. Wire 26.
pub const PASS_REWARD_AFFECTED: i32 = 26;
/// After a fire-pot charge spend commits. Wire 73.
pub const FIRE_SPENT: i32 = 73;
/// After a skill press. Wire 74.
pub const SKILL_USED: i32 = 74;
/// After a house count is written. Wire 75.
pub const HOUSE_ADDED: i32 = 75;
/// Before one charge spend commits. Wire 78.
pub const CHARGE_PAYING: i32 = 78;
/// After the fan-flip request is known, before the dict writes. Wire 107.
pub const FANS_FLIP: i32 = 107;

/// Action / Counteraction window. The pass-reward pick is known, before the
/// payout. Payload: player_id = the mover, value = the pick (0 = money,
/// 1 = card). A counteraction may cancel and rewrite the reward.
/// Dual-role: also a counteraction-window selector.
pub const PassRewardAffected: ActionKind = ActionKind(PASS_REWARD_AFFECTED);
/// Action / Counteraction window. A fire-pot charge spend just committed.
/// Payload: player_id, value = how many pots. A listener may emit effects.
pub const FireSpent: ActionKind = ActionKind(FIRE_SPENT);
/// Action / Counteraction window. A skill press is about to run its body.
/// Payload: card = the skill id, cards = that id, by_card = its player.
/// A counteraction may negate the press. Dual-role: also a counteraction-window
/// selector.
pub const SkillUsed: ActionKind = ActionKind(SKILL_USED);
/// Action / Counteraction window. A house count has just been written.
/// Payload: tile, value = the house count. A listener may emit effects.
pub const HouseAdded: ActionKind = ActionKind(HOUSE_ADDED);
/// Action / Counteraction window. Before one charge spend commits, so a
/// listener may substitute the cost. Payload: player_id, value = the cost.
/// May prompt; not a pure filter. Dual-role: also a counteraction-window
/// selector.
pub const ChargePaying: ActionKind = ActionKind(CHARGE_PAYING);
/// Action. A fan-flip request is known, before the dict writes, so a listener
/// sees the pre-flip face-down count. Payload: player_id = the seat,
/// value = the intended flip count.
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