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
//! rides on the plan's extra steps -- a bonus on top of the face.
//!
//! （3） is a standing modifier to （1） that lights up once anyone is out.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

pub const SAYO_THORNS: CardDef = CardDef::new(
    "skill:冰川纱夜:踏上荆棘之路的觉悟",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        On::Hook(&[HookKind::Pass], card_sdk::pre::MINE, None, on_pass),
        // （1）「每次被别的玩家[经过]时」 -- 行动阶段 12 [经过]
        // (`SETTLE-STAGES.md` §4 M4), the passer's step onto this player's
        // tile -- not the end-tile [重叠].
        On::Hook(&[HookKind::PassTile], "", Some(passed_by), on_passed),
    ],
)
    .legacy(&[(2, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「初始10，上限10」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 10, 10);
    Ok(())
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

/// （2） 「你可以消耗6个[火罐]」.
fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 6 {
        return Some(Msg::new(key!("sayo_thorns_no_fire")));
    }
    None
}

/// （2）「将非[传送]的主要移动添加1或2格」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    // 「非[传送]的主要移动」 -- extra steps on a teleport plan are inert (a
    // teleport has no route to extend), so the clause is satisfied by the
    // bonus simply not applying there rather than by a separate gate.
    let add = ctx::ask_pick(
        player_id,
        &Msg::new(key!("sayo_thorns_title")),
        &Msg::new(key!("sayo_thorns_ask")),
        &[
            Msg::new(key!("sayo_thorns_add1")),
            Msg::new(key!("sayo_thorns_add2")),
        ],
    )?;
    let n = if add == 1 { 2 } else { 1 };
    if !ctx::spend_fire(player_id, 6, &Msg::new(key!("sayo_thorns_spend")))? {
        return Ok(());
    }
    plan::add_extra_dice(n, 0, "踏上荆棘之路的觉悟");
    ctx::log(
        player_id,
        &Msg::new(key!("sayo_thorns_added")).i("n", n as i64),
    );
    Ok(())
}
