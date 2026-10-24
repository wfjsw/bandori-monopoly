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
//! player may press this. The engine's generic stun gate (`Gate::Act` /
//! `cannot_play`) refuses every action while stunned; `prop::PLAYABLE_STUNNED`
//! is the documented exception (the same waiver 壱雫空 carries).

use bandori_std::ctx::{self, state};
use bandori_std::state_key;
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};

pub const RINKO_1CM: CardDef = CardDef::new(
    "skill:白金燐子:即使1cm也要前进",
    &[
        On::Play("", None, Some(can_use), use_skill),
        On::Action(Action::TurnStartBefore("", None, None, declare_cap)),
        On::Action(Action::DeckAtGameStart("", None, None, declare_cap)),
        On::Action(Action::Pass(
            bandori_std::guards::OWN_START,
            None,
            None,
            on_pass,
        )),
    ],
)
// 规则书: 「此技能可以正常使用或在[晕眩]状态下使用」 -- skip the stun gate
// (`bandori_std::prop::PLAYABLE_STUNNED`, the same waiver 壱雫空 carries).
// 「（初始3，上限3）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 3),
    (bandori_std::fire::FIRE_CAP, 3),
    (bandori_std::prop::PLAYABLE_STUNNED, 1),
])
.silent(&[1, 2]);

/// 「初始3，上限3」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得三个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    bandori_std::fire::gain(player_id, 3, &Msg::new(key!("rinko_1cm_gain")))?;
    Ok(())
}

/// （2） 「你可以在移动掷骰前消耗X个[火罐]」 -- X is what the player holds.
fn can_use(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if bandori_std::fire::get(player_id) < 1 {
        return Some(Msg::new(key!("rinko_1cm_no_fire")));
    }
    // 「可在[晕眩]状态下使用」 is `prop::PLAYABLE_STUNNED` on this CardDef;
    // the engine's stun gate reads it before this body runs.
    None
}

/// （2）「使这回合移动掷骰的结果固定为X*6。本次移动不受异常移动效果影响」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let have = bandori_std::fire::get(player_id);
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
    if x < 1 || !bandori_std::fire::spend(player_id, x, &Msg::new(key!("rinko_1cm_spend")))? {
        return Ok(());
    }
    // 「固定为X*6」 -- the face is pinned, not rolled.
    ctx::set_fixed_roll(x * 6);
    // 「本次移动不受异常移动效果影响」 -- the companion layer closes the
    // stay half of the move gate, and the generic walk-exempt flag cancels
    // the immobile clamp for this walk.
    state::add(player_id, state_key::UNSTOPPABLE, 1);
    bandori_std::status::declare_companion(player_id, state_key::UNSTOPPABLE);
    state::set_expires(player_id, state_key::UNSTOPPABLE, ctx::state::TURN_END);
    state::set(player_id, "moveExempt", 1);
    state::set_expires(player_id, "moveExempt", ctx::state::TURN_END);
    ctx::log(
        player_id,
        &Msg::new(key!("rinko_1cm_fixed")).i("n", (x * 6) as i64),
    );
    Ok(())
}

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
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}
