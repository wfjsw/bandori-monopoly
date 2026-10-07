//! `skill:白金燐子:即使1cm也要前进`
//!
//! 规则书（skill sheet, 白金燐子）:
//! > （1）每次[经过]CiRCLE时获得三个[火罐]（初始3，上限3）
//! > （2）此技能可以正常使用或在[晕眩]状态下使用。你可以在移动掷骰前消耗X个
//! > 火罐，使这回合移动掷骰的结果固定为X*6。本次移动不受异常移动效果影响。
//!
//! （1） is `Pass` onto CiRCLE, three pots at a time.
//!
//! （2） has three parts and they are all one press: spend X, pin the face at
//! `X*6`, and arm the move against abnormal movement. 「固定」 is the fixed roll
//! the move plan already carries; 「不受异常移动效果影响」 is `unstoppable`,
//! which the abnormal gate already reads.
//!
//! 「此技能可以正常使用或在[晕眩]状态下使用」 is a *play waiver*: a stunned
//! player may press this. The engine's generic `cannot_play` refuses every
//! action while stunned, and there is no per-card way to say otherwise yet --
//! see the TODO below.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

pub const RINKO_1CM: CardDef = CardDef::new(
    "skill:白金燐子:即使1cm也要前进",
    &[
        On::Play(Some(can_use), use_skill),
        On::Hook(
            &[HookKind::TurnStartBefore, HookKind::DeckAtGameStart],
            |_| true,
            declare_cap,
        ),
        On::Hook(&[HookKind::Pass], mine, on_pass),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「初始3，上限3」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 3, 3);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得三个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_circle(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain_fire(player_id, 3, &Msg::new(key!("rinko_1cm_gain")));
    Ok(())
}

/// （2） 「你可以在移动掷骰前消耗X个[火罐]」 -- X is what the player holds.
fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("rinko_1cm_no_fire")));
    }
    // TODO(规则书)[judgement]: 「此技能可以正常使用或在[晕眩]状态下使用」 --
    //   the clause under-specifies -- a per-card waiver of the engine's generic
    //   stun gate needs a shape (a `CardDef` flag, or the play guard running
    //   first) and neither exists yet.
    None
}

/// （2）「使这回合移动掷骰的结果固定为X*6。本次移动不受异常移动效果影响」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let have = state::get(player_id, state_key::FIRE);
    if have < 1 {
        return Ok(());
    }
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("rinko_1cm_title")),
        &Msg::new(key!("rinko_1cm_ask")),
        1,
        have,
    )?;
    if x < 1 || !ctx::spend_fire(player_id, x, &Msg::new(key!("rinko_1cm_spend"))) {
        return Ok(());
    }
    // 「固定为X*6」 -- the face is pinned, not rolled.
    ctx::set_fixed_roll(x * 6);
    // 「本次移动不受异常移动效果影响」 -- `unstoppable` is exactly that gate.
    state::add(player_id, state_key::UNSTOPPABLE, 1);
    state::set_expires(player_id, state_key::UNSTOPPABLE, ctx::state::TURN_END);
    ctx::log(
        player_id,
        &Msg::new(key!("rinko_1cm_fixed")).i("n", (x * 6) as i64),
    );
    Ok(())
}
