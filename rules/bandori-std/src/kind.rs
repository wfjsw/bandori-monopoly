//! Ruleset-declared kind ids and kind/class strings (opaque to the engine).
//!
//! The engine fires and dispatches by numeric id; the names of this game's
//! raise points live here so the engine crates and card-sdk carry no game
//! vocabulary. The values live in `KindNs::Ruleset(RULESET_KEY)` and cannot
//! collide with engine ids -- or with another ruleset's ids. Each rule-defined
//! kind is declared on its typed side -- today every one is an action kind (an
//! event-reaction listener).
//!
//! Only four kinds are genuinely ruleset-emitted (raised from `rules/` with
//! this game's vocabulary): `Exile` (21), `PassRewardAffected` (26),
//! `ChargePaying` (78), `FansFlip` (107). The three universal raises this file
//! used to declare -- marker-spend commit, field-card activation, house-count
//! write -- moved to the engine namespace in ABI v90 as `markerSpent` /
//! `cardUsed` / `houseAdded` (`card_sdk::abi::ActionKind::MarkerSpent` etc.).
//!
//! A future rule-defined filter kind must declare
//! `FilterKind::ruleset(RULESET_KEY, id)` here and also register that id as a
//! filter in the trigger-kind filter test, so the typed sets keep mirroring
//! that test exactly. No rule-defined filter kinds exist yet.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use card_sdk::abi::{ActionKind, TriggerKind};

/// Stable wire name of this ruleset's kind namespace. `KindNs::Ruleset` is
/// keyed by it, so two rulesets cannot collide on the wire.
pub const RULESET_KEY: &str = "bandori";

// ------------------------------------------------------- raise-point ids

/// Pass-reward half of the pass reward. Wire 26.
pub const PASS_REWARD_AFFECTED: i32 = 26;
/// Before one charge spend commits. Wire 78.
pub const CHARGE_PAYING: i32 = 78;
/// After the fan-flip request is known, before the dict writes. Wire 107.
pub const FANS_FLIP: i32 = 107;
/// 「获得[除外]」 post-commit raise. Wire 21.
pub const EXILE_I32: i32 = 21;

/// Action / Counteraction window. The pass-reward pick is known, before the
/// payout. Payload: player_id = the mover, value = the pick (0 = money,
/// 1 = card). A counteraction may cancel and rewrite the reward.
/// Dual-role: also a counteraction-window selector.
pub const PassRewardAffected: ActionKind = ActionKind(TriggerKind::ruleset(RULESET_KEY, 26));
/// Action / Counteraction window. Before one charge spend commits, so a
/// listener may substitute the cost. Payload: player_id, value = the cost.
/// May prompt; not a pure filter. Dual-role: also a counteraction-window
/// selector.
pub const ChargePaying: ActionKind = ActionKind(TriggerKind::ruleset(RULESET_KEY, 78));
/// Action. A fan-flip request is known, before the dict writes, so a listener
/// sees the pre-flip face-down count. Payload: player_id = the seat,
/// value = the intended flip count.
pub const FansFlip: ActionKind = ActionKind(TriggerKind::ruleset(RULESET_KEY, 107));
/// Action / Counteraction window. 「任意玩家获得[除外]…时」 -- an exile layer
/// has been granted by any path (the ruleset's post-commit `raise_after`;
/// wire 21). Payload: value = 1, by_card. Distinct from `ActionKind::Abnormal`,
/// which is the landing itself.
pub const Exile: ActionKind = ActionKind(TriggerKind::ruleset(RULESET_KEY, 21));

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

/// Which ruleset kinds open a counteraction window (`false`) vs. are
/// field-card hooks only (`true`).
///
/// `ChargePaying` / `FansFlip` are hook-only: no hand counteraction window
/// opens at them. `Exile` / `PassRewardAffected` are [反击] windows.
/// The host never names these ids -- it reads this declaration at load
/// (`ruleset_kind_flags`, same path as [`class_defs`]).
pub fn kind_hook_only() -> Vec<(i32, bool)> {
    vec![
        (EXILE_I32, false),
        (PASS_REWARD_AFFECTED, false),
        (CHARGE_PAYING, true),
        (FANS_FLIP, true),
    ]
}

/// Abnormal-effect kind names (「异常效果」). The ids are the frozen webui
/// wire values of `card_sdk::abi::AbKind`; the readable names live here so
/// `card-sdk` / `crates/` carry no game status vocabulary.
pub mod abnormal {
    use card_sdk::abi::AbKind;

    /// [停留]. Wire 1.
    pub const STAY: i32 = 1;
    /// [晕眩]. Wire 2.
    pub const STUN: i32 = 2;
    /// [除外]. Wire 3.
    pub const EXILE: i32 = 3;
    /// [传送]. Wire 4.
    pub const TELEPORT: i32 = 4;
    /// [强制移动]. Wire 5.
    pub const FORCED: i32 = 5;
    /// [强制停下]. Wire 6.
    pub const STOP: i32 = 6;
    /// 反方向移动. Wire 7.
    pub const REVERSE: i32 = 7;

    /// [停留] -- 「处于该状态时[无法移动]」，可叠加，每回合结束时移除一层.
    pub const Stay: AbKind = AbKind(STAY);
    /// [晕眩] -- 「无法移动」，无法使用[主动]技能和手卡的[手]效果，无法收付款.
    pub const Stun: AbKind = AbKind(STUN);
    /// [除外] -- 无法移动 / 指定 / 收付款，每回合开始前移除一层.
    pub const Exile: AbKind = AbKind(EXILE);
    /// [传送] -- an abnormal move that is not the main move.
    pub const Teleport: AbKind = AbKind(TELEPORT);
    /// [强制移动] -- the abnormal force-move.
    pub const Forced: AbKind = AbKind(FORCED);
    /// [强制停下] -- the abnormal stop.
    pub const Stop: AbKind = AbKind(STOP);
    /// 反方向移动 -- the abnormal reverse.
    pub const Reverse: AbKind = AbKind(REVERSE);
}