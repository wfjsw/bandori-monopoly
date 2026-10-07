//! `skill:佐藤益木:与燃烧的红色一起驰骋`
//!
//! 规则书（skill sheet, 佐藤益木）:
//! > （1）每次[经过]"白雪学园"或"银河拉面馆"时获得1个[火罐]（初始0，上限2）
//! > （2）移动阶段前可使用X个[火罐]，本回合主要移动掷骰额外添加Xd10
//!
//! （1） is `Pass` onto one of the two named tiles.
//!
//! （2） 「移动阶段前」 is the operations phase, the same window every other
//! 「移动阶段前可使用」 skill opens in; the engine already refuses an action
//! outside it. The extra dice are `add_extra_dice`, which rides along on the
//! plan rather than replacing its base -- 「额外添加」, not 「变更为」.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

pub const SATO_RED: CardDef = CardDef::new(
    "skill:佐藤益木:与燃烧的红色一起驰骋",
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

/// 「初始0，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 0, 2);
    Ok(())
}

/// （1）「每次[经过]"白雪学园"或"银河拉面馆"时获得1个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    if t != ctx::tile_named("白雪学园") && t != ctx::tile_named("银河拉面馆") {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("sato_red_gain")));
    Ok(())
}

/// （2） 「移动阶段前可使用X个[火罐]」 -- X is what the player holds.
fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("sato_red_no_fire")));
    }
    None
}

/// （2）「本回合主要移动掷骰额外添加Xd10」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let have = state::get(player_id, state_key::FIRE);
    if have < 1 {
        return Ok(());
    }
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("sato_red_title")),
        &Msg::new(key!("sato_red_ask")),
        1,
        have,
    )?;
    if x < 1 || !ctx::spend_fire(player_id, x, &Msg::new(key!("sato_red_spend"))) {
        return Ok(());
    }
    plan::add_extra_dice(x, 10, "与燃烧的红色一起驰骋");
    ctx::log(player_id, &Msg::new(key!("sato_red_dice")).i("n", x as i64));
    Ok(())
}
