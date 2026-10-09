//! `skill:冰川纱夜:踏上荆棘之路的觉悟`
//!
//! 规则书（skill sheet, 冰川纱夜）:
//! > （1）每次[经过]CiRCLE时获得2个[火罐]，每次被别的玩家[经过]时获得1个
//! > [火罐]（初始10，上限10）
//! > （2）你可以消耗6个[火罐]将非[传送]的主要移动添加1或2格
//! > （3）第一个玩家被淘汰，经过Circle时获得的火罐加1
//!
//! （1） is two `Pass` halves of one clause: your own pass onto CiRCLE pays 2,
//! being passed by anyone pays 1. 「被别的玩家[经过]」 is the *player* being
//! passed, which is `passPlayer`, not `pass`.
//!
//! （2） 「非[传送]的主要移动」 is the walk, not a teleport, and 「添加1或2格」
//! is offered once per turn after the final roll/counteractions, before the
//! first step (the confirmed timing; the skill sheet text remains unchanged).
//!
//! （3） is a standing modifier to （1） that lights up once anyone is out.

use card_sdk::abi::{prop, state_key, ChainKind, HookKind, MoveKind};
use card_sdk::ctx::{self, plan, state, trigger};
use card_sdk::{key, CardDef, Msg, On};

const USED: &str = "skill.sayoThorns.used";

pub const SAYO_THORNS: CardDef = CardDef::new(
    "skill:冰川纱夜:踏上荆棘之路的觉悟",
    &[
        On::Play("", Some(cant_press), offer),
        On::Hook(
            &[HookKind::TurnStartBefore, HookKind::DeckAtGameStart],
            "",
            None,
            declare_cap,
        ),
        On::Hook(&[HookKind::Pass], card_sdk::pre::MINE, None, on_pass),
        // （1）「每次被别的玩家[经过]时」 -- 行动阶段 12 [经过]
        // (`SETTLE-STAGES.md` §4 M4), the passer's step onto this player's
        // tile -- not the end-tile [重叠].
        On::Hook(&[HookKind::PassTile], "", Some(passed_by), on_passed),
        On::Counteract(
            &[ChainKind::MoveBefore],
            card_sdk::pre::MINE,
            Some(can_offer),
            offer,
        ),
    ],
)
.props(&[
    (prop::COUNTERACT_FROM_FIELD, 1),
    (prop::COUNTERACT_GROUP, 1),
    (prop::COUNTERACT_FIRE_COST, 6),
])
.legacy(&[(2, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

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
    if !ctx::is_circle(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain_fire(
        player_id,
        circle_gain(),
        &Msg::new(key!("sayo_thorns_gain")),
    )?;
    Ok(())
}

/// 「被别的玩家[经过]」 -- another player's step onto **my** tile (行动阶段 12,
/// `SETTLE-STAGES.md` §4 M4).
fn passed_by(player_id: i32) -> bool {
    ctx::trigger::player_id() != player_id && ctx::trigger::tile() == ctx::player_pos(player_id)
}

/// （1）「每次被别的玩家[经过]时获得1个[火罐]」 -- someone passed *this* player.
fn on_passed(player_id: i32) -> card_sdk::Asked {
    ctx::gain_fire(player_id, 1, &Msg::new(key!("sayo_thorns_passed")))?;
    Ok(())
}

/// Once per turn, and only after a non-teleport main move is determined.
fn can_offer(player_id: i32) -> bool {
    trigger::move_is_main()
        && trigger::move_kind() == Some(MoveKind::Walk)
        && !ctx::skill_blocked(player_id, "")
        && state::get(player_id, USED) != ctx::turn_key()
        && state::get(player_id, state_key::FIRE) >= 6
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
    if !ctx::spend_fire(player_id, 6, &Msg::new(key!("sayo_thorns_spend")))? {
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
