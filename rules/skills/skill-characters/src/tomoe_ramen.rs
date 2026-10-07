//! `skill:宇田川巴:豚骨酱油拉面大姐`
//!
//! 规则书（skill sheet, 宇田川巴）:
//! > （1）每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）
//! > （2）移动阶段前，你可以使用一个[火罐]，使此次移动的起点向绝对距离"银河拉面馆"
//! > 更近的方向移动10格。若你因此效果导致起点向前移动并超过了"银河拉面馆"，你补充
//! > 一个火罐。
//! > （3）若你位于银河拉面馆上，你无法使用（2）效果，你可以选择停留一回合，此效果
//! > 每圈限一次
//!
//! （1） is the shared Afterglow rest counter (see [`super::ran_red`]).
//!
//! （2） 「使此次移动的起点…移动10格」 is `plan::set_start`, and 「向绝对距离
//! "银河拉面馆"更近的方向」 is whichever of the two directions shortens the
//! absolute distance to the named tile. 「超过了」 is the start stepping past it
//! -- which the direction check makes impossible on the approach side, so the
//! pot refund fires on the wrap-around overshoot.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

use super::ran_red::REST_TURNS;

/// 「此效果每圈限一次」 -- latched per lap, cleared when the player laps.
const USED: &str = "skill.tomoeRamen.used";

fn shop() -> i32 {
    ctx::tile_named("银河拉面馆")
}

pub const TOMOE_RAMEN: CardDef = CardDef::new(
    "skill:宇田川巴:豚骨酱油拉面大姐",
    &[
        On::Play(Some(can_use), use_skill),
        On::Hook(
            &[HookKind::TurnStartBefore, HookKind::DeckAtGameStart],
            |_| true,
            declare_cap,
        ),
        On::Hook(&[HookKind::TurnEnd], afterglow, tick),
    ],
);

fn afterglow(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id && ctx::in_band(player_id, "Afterglow")
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

/// （2） 「你可以使用一个[火罐]」 + （3） 「若你位于银河拉面馆上，你无法使用（2）效果」.
fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if ctx::player_pos(player_id) == shop() {
        return Some(Msg::new(key!("tomoe_ramen_at_shop")));
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("tomoe_ramen_no_fire")));
    }
    None
}

/// （2）「使此次移动的起点向绝对距离"银河拉面馆"更近的方向移动10格」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let shop = shop();
    if shop < 0 {
        return Ok(());
    }
    let here = ctx::player_pos(player_id);
    if here == shop {
        return Ok(());
    }
    // 「向绝对距离…更近的方向」 -- whichever way shortens `dist`.
    let fwd = ctx::tile_steps_ahead(player_id, 10);
    let back = ctx::tile_steps_ahead(player_id, -10);
    let toward = if ctx::dist(fwd, shop) <= ctx::dist(back, shop) {
        fwd
    } else {
        back
    };
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("tomoe_ramen_spend"))) {
        return Ok(());
    }
    state::set(player_id, REST_TURNS, 0);
    plan::set_start(toward, "豚骨酱油拉面大姐");
    // 「若你因此效果导致起点向前移动并超过了"银河拉面馆"，你补充一个火罐」 --
    // the overshoot is the start landing strictly past the shop on the approach
    // side, which the direction pick makes a wrap. Refund when it wrapped past.
    let before = ctx::dist(here, shop);
    let after = ctx::dist(toward, shop);
    if after > before {
        ctx::gain_fire(player_id, 1, &Msg::new(key!("tomoe_ramen_refund")));
    }
    ctx::log(
        player_id,
        &Msg::new(key!("tomoe_ramen_moved")).tile("tile", toward),
    );
    Ok(())
}
