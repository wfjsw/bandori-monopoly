//! `skill:青叶摩卡:我行我素`
//!
//! 规则书（skill sheet, 青叶摩卡）:
//! > （1）每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）
//! > （2）当他人使用角色技能或打出手牌时，你可以消耗一个[火罐]传送到其所在格子
//! > （不触发结算且该传送本身不引发其他被动效果（如玩家重叠），该效果对每个玩家
//! > 的被动技能和卡存在2次使用cd）
//!
//! （1） is the shared Afterglow rest counter (see [`super::ran_red`]).
//!
//! （2） is a counteraction to *someone else's* act -- a skill press or a hand play --
//! and the teleport it buys is inert: 「不触发结算」 and 「不引发其他被动效果
//! （如玩家重叠）」. Both are the plan's own flags (`resolve: false`) plus the
//! fact that a plain position write raises nothing.
//!
//! 「该效果对每个玩家的被动技能和卡存在2次使用cd」 is a per-*source* cooldown of
//! two uses, which is a counter keyed on the source rather than on the target.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

use super::ran_red::REST_TURNS;

/// Per-source use count, keyed `skill.mocaSelf.used:<player>`. Cap 2.
fn used_key(src: i32) -> alloc::string::String {
    alloc::format!("skill.mocaSelf.used:{src}")
}

pub const MOCA_SELF: CardDef = CardDef::new(
    "skill:青叶摩卡:我行我素",
    &[
        On::Hook(
            &[HookKind::TurnStartBefore, HookKind::DeckAtGameStart],
            |_| true,
            declare_cap,
        ),
        On::Hook(&[HookKind::TurnEnd], afterglow, tick),
        On::Hook(&[HookKind::CardPlayed], other, on_card),
        On::Hook(&[HookKind::SkillUsed], other, on_skill),
    ],
);

fn afterglow(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id && ctx::in_band(player_id, "Afterglow")
}

fn other(player_id: i32) -> bool {
    ctx::trigger::player_id() != player_id
}

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 1, 1);
    Ok(())
}

/// （1）「每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]」.
fn tick(player_id: i32) -> card_sdk::Asked {
    let n = state::get(player_id, REST_TURNS) + 1;
    state::set(player_id, REST_TURNS, n);
    if n >= 3 {
        state::set(player_id, REST_TURNS, 0);
        ctx::gain_fire(player_id, 1, &Msg::new(key!("afterglow_rest_gain")));
    }
    Ok(())
}

/// （2） 「当他人…打出手牌时」.
fn on_card(player_id: i32) -> card_sdk::Asked {
    offer(player_id, ctx::trigger::player_id())?;
    Ok(())
}

/// （2） 「当他人使用角色技能时」.
fn on_skill(player_id: i32) -> card_sdk::Asked {
    offer(player_id, ctx::trigger::player_id())?;
    Ok(())
}

/// （2） 「你可以消耗一个[火罐]传送到其所在格子」.
fn offer(player_id: i32, src: i32) -> card_sdk::Asked {
    if src < 0 || src == player_id {
        return Ok(());
    }
    // 「该效果对每个玩家的被动技能和卡存在2次使用cd」
    let key = used_key(src);
    if state::get(player_id, &key) >= 2 {
        return Ok(());
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("moca_self_title")),
        &Msg::new(key!("moca_self_ask")).player_id("who", src),
    )? {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("moca_self_spend"))) {
        return Ok(());
    }
    state::set(player_id, &key, state::get(player_id, &key) + 1);
    state::set(player_id, REST_TURNS, 0);
    // 「不触发结算且该传送本身不引发其他被动效果」 -- a plain position write:
    // no move plan, so no pass/arrive/settle and no player-overlap bookkeeping.
    let to = ctx::player_pos(src);
    if to >= 0 {
        ctx::teleport_to(player_id, to);
        ctx::log(
            player_id,
            &Msg::new(key!("moca_self_moved")).tile("tile", to),
        );
    }
    Ok(())
}
