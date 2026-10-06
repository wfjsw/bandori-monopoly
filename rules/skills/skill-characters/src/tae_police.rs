//! `skill:花园多惠:花园警察，出警！`
//!
//! 规则书（skill sheet, 花园多惠）:
//! > （1）每次你领取[CiRCLE奖励]时投掷3d20，如果结果对应的格子不是"星空齿科"则在
//! > 结果对应的格子放置一个[多惠兔子]；当你经过有[多惠兔子]的格子时[移除]该格子上
//! > 的所有[多惠兔子]并获得等量的[火罐]（初始2，上限4）
//! > （2）在其他玩家使用角色/团技能的[主]效果或使用手牌的[手]效果时你可以使用4个
//! > [火罐]将其抵消，然后你获得500资金且被抵消的玩家获得2000资金
//!
//! （1）'s 「[多惠兔子]」 is a tile mark, which is what `add_mark` / `bump_mark`
//! carry; 「[移除]该格子上的所有[多惠兔子]并获得等量的[火罐]」 is a count-and-
//! clear on the pass.
//!
//! （2） 「将其抵消」 is a [反击]: the effect is named and then voided, which is
//! `trigger::set_cancelled`. 「使用角色/团技能的[主]效果或使用手牌的[手]效果」
//! is the `card` chain for a hand play and `skillUsed` for a skill press.

use card_sdk::abi::{state_key, ChainKind, HookKind};
use card_sdk::ctx::{self, state, trigger};
use card_sdk::{key, CardDef, Msg, On};

/// The mark kind 「[多惠兔子]」.
const RABBIT: &str = "多惠兔子";

pub const TAE_POLICE: CardDef = CardDef::new(
    "skill:花园多惠:花园警察，出警！",
    &[
        On::Hook(&[HookKind::TurnStartBefore], |_| true, declare_cap),
        On::Hook(&[HookKind::CircleAffected], mine, on_circle),
        On::Hook(&[HookKind::Pass], mine, on_pass),
        On::CounterAct(&[ChainKind::Effect], can_negate, negate),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「初始2，上限4」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    state::set_bounds(player_id, state_key::FIRE, 0, 4);
    Ok(())
}

/// （1）「每次你领取[CiRCLE奖励]时投掷3d20，如果结果对应的格子不是"星空齿科"
/// 则在结果对应的格子放置一个[多惠兔子]」.
fn on_circle(player_id: i32) -> card_sdk::Asked {
    let n = ctx::roll(player_id, 3, 20);
    let total = ctx::tile_count();
    if total <= 0 {
        return Ok(());
    }
    let t = (n - 1).rem_euclid(total);
    if t == ctx::tile_named("星空齿科") {
        return Ok(());
    }
    ctx::add_mark(t, player_id, RABBIT, &Msg::new(key!("tae_police_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("tae_police_placed"))
            .tile("tile", t)
            .i("n", n as i64),
    );
    Ok(())
}

/// （1）「当你经过有[多惠兔子]的格子时[移除]该格子上的所有[多惠兔子]并获得
/// 等量的[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    let n = ctx::count_marks(t, RABBIT, -2);
    if n <= 0 {
        return Ok(());
    }
    ctx::remove_marks(t, RABBIT, -2);
    ctx::gain_fire(player_id, n, &Msg::new(key!("tae_police_gain")));
    Ok(())
}

/// （2） 「你可以使用4个[火罐]将其抵消」 -- only against someone else's effect.
fn can_negate(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() != player_id
        && state::get(player_id, state_key::FIRE) >= 4
        && trigger::value() != 0
}

/// （2）「将其抵消，然后你获得500资金且被抵消的玩家获得2000资金」.
fn negate(player_id: i32) -> card_sdk::Asked {
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("tae_police_title")),
        &Msg::new(key!("tae_police_ask")),
    )? {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 4, &Msg::new(key!("tae_police_spend"))) {
        return Ok(());
    }
    trigger::set_cancelled();
    let src = trigger::player_id();
    ctx::gain(player_id, 500, &Msg::new(key!("tae_police_gain_self")));
    if src >= 0 && !ctx::player_out(src) {
        ctx::gain(src, 2000, &Msg::new(key!("tae_police_gain_other")));
    }
    Ok(())
}
