//! `skill:上原绯玛丽:大家一起迈出新的一步`
//!
//! 规则书（skill sheet, 上原绯玛丽）:
//! > （1）每四回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）
//! > （2）移动阶段前，你可以消耗一个火罐指定你的此次移动仅在单/双数格上进行。
//!
//! （1） is the shared Afterglow rest counter (see [`super::ran_red`]).
//!
//! （2） 「仅在单/双数格上进行」 is the move plan's parity filter: the walk
//! lands only on odd- or even-numbered tiles. `plan::set_parity` is that field.

use bandori_std::ctx::{self, plan, state};
use bandori_std::state_key;
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};

use super::ran_red::REST_TURNS;

pub const HIMARI_STEP: CardDef = CardDef::new(
    "skill:上原绯玛丽:大家一起迈出新的一步",
    &[
        On::Play("", Some(can_use), use_skill).label("（2）"),
        On::Action(Action::TurnStartBefore("", None, declare_cap)),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)),
        On::Action(Action::TurnEnd("", Some(afterglow), tick)),
    ],
)
.silent(&[1, 2])
// 「（初始1，上限1）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 1),
    (bandori_std::fire::FIRE_CAP, 1),
]);

fn afterglow(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id && ctx::in_group(player_id, "Afterglow")
}

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// （1）「每四回合没有使用Afterglow角色的（2）技能获得一个[火罐]」.
fn tick(player_id: i32) -> card_sdk::Asked {
    super::ran_red::rest_tick(player_id)
}

fn can_use(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::CHARGE) < 1 {
        return Some(Msg::new(key!("himari_step_no_fire")));
    }
    None
}

/// （2）「指定你的此次移动仅在单/双数格上进行」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let odd = ctx::ask_pick(
        player_id,
        &Msg::new(key!("himari_step_title")),
        &Msg::new(key!("himari_step_ask")),
        &[
            Msg::new(key!("himari_step_odd")),
            Msg::new(key!("himari_step_even")),
        ],
    )?;
    if !bandori_std::fire::spend(player_id, 1, &Msg::new(key!("himari_step_spend")))? {
        return Ok(());
    }
    state::set(player_id, REST_TURNS, 0);
    // 1 = odd, 2 = even (the plan's parity field).
    plan::set_parity(if odd == 1 { 1 } else { 2 });
    ctx::log(player_id, &Msg::new(key!("himari_step_set")));
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn use_skill_kani(player_id: i32) -> card_sdk::Asked {
    use_skill(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn tick_kani(player_id: i32) -> card_sdk::Asked {
    tick(player_id)
}
