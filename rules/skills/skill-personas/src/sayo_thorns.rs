//! `skill:冰川纱夜:踏上荆棘之路的觉悟`
//!
//! 规则书（skill sheet, 冰川纱夜）:
//! > （1）每次[经过]CiRCLE时获得2个[火罐]，每次被别的玩家[经过]时获得1个
//! > [火罐]（初始10，上限10）
//! > （2）一回合一次，你可以在进入移动阶段时消耗6个[火罐]将非[传送]的主要移动
//! > 添加1或2格
//! > （3）第一个玩家被淘汰，经过Circle时获得的火罐加1
//!
//! （1） is two `Pass` halves of one clause: your own pass onto CiRCLE pays 2,
//! being passed by anyone pays 1. 「被别的玩家[经过]」 is the *player* being
//! passed, which is `passPlayer`, not `pass`.
//!
//! （2） 「一回合一次」 is the `skill.sayoThorns.used` turn-key latch (reset at
//! turn start, including an extra turn). 「进入移动阶段时」 is read as the
//! move-phase entry that already carries the resolved main move -- the
//! `MoveBefore` counteraction after the final roll/counteractions and before
//! the first step -- because 「添加1或2格」 extends the *determined* distance
//! (the offer previews each landing) and the sheet's added 「一回合一次」
//! confirms the once-per-turn window. 「非[传送]的主要移动」 is the walk, not
//! a teleport.
//!
//! （3） is a standing modifier to （1） that lights up once anyone is out.

use card_sdk::abi::{prop, ChainKind, ActionKind};
use ruleset_core::ctx::{self, plan, state, trigger};
use card_sdk::{key, CardDef, Msg, On};

const USED: &str = "skill.sayoThorns.used";

pub const SAYO_THORNS: CardDef = CardDef::new(
    "skill:冰川纱夜:踏上荆棘之路的觉悟",
    &[
        On::Play("", "", Some(cant_press), offer),
        On::Action(
            &[ActionKind::TurnStartBefore, ActionKind::DeckAtGameStart],
            "", "",
            None,
            declare_cap,
        ),
        On::Action(&[ActionKind::Pass], ruleset_core::guards::OWN_START, "", None, on_pass),
        // （1）「每次被别的玩家[经过]时」 -- 行动阶段 12 [经过]
        // (`SETTLE-STAGES.md` §4 M4), the passer's step onto this player's
        // tile -- not the end-tile [重叠].
        On::Action(
            &[ActionKind::PassTile],
            "actor != owner && tile.id == owner.pos", "",
            None,
            on_passed,
        ),
        // （2）「将非[传送]的主要移动添加1或2格」 -- once per turn, only after a
        // non-teleport main move is determined. The category owns `MoveBefore`;
        // the residual is the any-band `skill_blocked`.
        On::Counteract(
            &[ChainKind::MoveBefore],
            "actor == owner && move.main && move.kind == Walk && slot('skill.sayoThorns.used') != turn_key && charge(owner) >= 6",
            "",
            Some(can_offer),
            offer,
        ),
    ],
)
.props(&[
    (prop::COUNTERACT_FROM_FIELD, 1),
    (prop::COUNTERACT_GROUP, 1),
    (prop::COUNTERACT_CHARGE_COST, 6),
])
.silent(&[1]);

/// 「初始10，上限10」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 10, 10);
    // One handler per hook: reset alongside the existing turn-start cap
    // declaration, including an extra turn by the same player.
    state::set(player_id, USED, 0);
    Ok(())
}

/// The manual skill button must not arm this before seeing the roll.
fn cant_press(_player_id: i32) -> Option<Msg> {
    Some(Msg::new(key!("sayo_thorns_timing")))
}

/// （3）「第一个玩家被淘汰，经过Circle时获得的火罐加1」.
fn circle_gain() -> i32 {
    let mut n = 2;
    for p in 0..ctx::player_count() {
        if ctx::player_out(p) {
            n += 1;
            break;
        }
    }
    n
}

/// （1）「每次[经过]CiRCLE时获得2个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    rules_status::fire::gain(
        player_id,
        circle_gain(),
        &Msg::new(key!("sayo_thorns_gain")),
    )?;
    Ok(())
}

/// 「被别的玩家[经过]」 -- another player's step onto **my** tile (行动阶段 12,
/// `SETTLE-STAGES.md` §4 M4). `actor != owner && tile.id == owner.pos` is the pre.

/// （1）「每次被别的玩家[经过]时获得1个[火罐]」 -- someone passed *this* player.
fn on_passed(player_id: i32) -> card_sdk::Asked {
    rules_status::fire::gain(player_id, 1, &Msg::new(key!("sayo_thorns_passed")))?;
    Ok(())
}

/// Residual guard for （2）'s offer -- `skill_blocked` (any-band) stays here
/// (not yet in the condition vocabulary). `move.main && move.kind == Walk &&
/// slot('skill.sayoThorns.used') != turn_key && charge(owner) >= 6` is the pre.
fn can_offer(player_id: i32) -> bool {
    !ctx::skill_blocked(player_id, "")
}

/// （2）「将非[传送]的主要移动添加1或2格」.
fn offer(player_id: i32) -> card_sdk::Asked {
    // Other MoveBefore counters may already have extended
    // the plan. Apply the preselected extension, retaining earlier extra steps.
    let base = trigger::value().max(0);
    let n = trigger::move_tag(card_sdk::abi::COUNTERACT_MOVE_EXTENSION);
    if !matches!(n, 1 | 2) {
        return Ok(());
    }
    if !rules_status::fire::spend(player_id, 6, &Msg::new(key!("sayo_thorns_spend")))? {
        return Ok(());
    }
    state::set(player_id, USED, ctx::turn_key());
    // The dice are already final. Extend the base distance and retain any
    // extra steps on this move; adding dice now would affect a later roll.
    plan::set_steps(base + n);
    ctx::log(
        player_id,
        &Msg::new(key!("sayo_thorns_added")).i("n", n as i64),
    );
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn offer_kani(player_id: i32) -> card_sdk::Asked {
    offer(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

// Kani-only direct-call exports (ppcost). Production builds omit these.

#[cfg(kani)]
#[inline(always)]
pub fn on_passed_kani(player_id: i32) -> card_sdk::Asked {
    on_passed(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}
