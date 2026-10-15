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
use ruleset_core::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

pub const SATO_RED: CardDef = CardDef::new(
    "skill:佐藤益木:与燃烧的红色一起驰骋",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        // （1）「每次[经过]"白雪学园"或"银河拉面馆"时」 -- the two named tiles
        // are the hook's applicability (`tile_named` is in the condition
        // vocabulary).
        On::Hook(
            &[HookKind::Pass],
            "actor == owner && (tile.id == tile_named('白雪学园') || tile.id == tile_named('银河拉面馆'))",
            None,
            on_pass,
        ),
    ],
)
    .silent(&[1]);

/// 「初始0，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 0, 2);
    Ok(())
}

/// （1）「每次[经过]"白雪学园"或"银河拉面馆"时获得1个[火罐]」. The two named
/// tiles are the pre.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    rules_status::fire::gain(player_id, 1, &Msg::new(key!("sato_red_gain")))?;
    Ok(())
}

/// （2） 「移动阶段前可使用X个[火罐]」 -- X is what the player holds.
fn can_use(player_id: i32) -> Option<Msg> {
    if ruleset_core::ctx::skill_blocked(player_id, "") {
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
    if x < 1 || !rules_status::fire::spend(player_id, x, &Msg::new(key!("sato_red_spend")))? {
        return Ok(());
    }
    plan::add_extra_dice(x, 10, "与燃烧的红色一起驰骋");
    ctx::log(player_id, &Msg::new(key!("sato_red_dice")).i("n", x as i64));
    Ok(())
}

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: ruleset_core::roster::PersonaDef =
ruleset_core::roster::PersonaDef::new(r#"佐藤益木"#)
    .display(r#"佐藤益木"#)
    .group(r#"RAISE A SUILEN"#)
    .color(r#"#EEBB44"#)
    .skill(r#"与燃烧的红色一起驰骋"#)
    .text(r#"（1）每次[经过]“白雪学园”或“银河拉面馆”时获得1个[火罐]（初始0，上限2）（2）移动阶段前可使用X个[火罐]，本回合主要移动掷骰额外添加Xd10"#)
    .cn_id(r#"033"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"CRUSH ON THE DRUM!!!"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");

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
