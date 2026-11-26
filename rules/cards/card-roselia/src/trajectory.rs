//! bankruptcy: take one of the bankrupt player's deeds, free and clear.
//!
//! 规则书（docs/rulebook/cards.json, id `R:轨迹`）:
//! > 轨迹：
//! > [反击][特]
//!
//! > （1）场上有玩家[破产]时，依次进行以下操作：
//! > 1. 展示此卡。
//! > 2. 获得那名玩家的任意1张地契，自动免费赎回。
//! > 3. 拆除该地契对应格子上的所有房屋。
//!

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::{it, it_sub, items, para, sec, CardText};
use card_sdk::{key, CardDef, Msg, On};

pub const TRAJECTORY: CardDef = CardDef::new(
    "R:轨迹",
    &[On::Counteract(&[ChainKind::Bankrupt], "", Some(can_counteract), counteract).counter_reason(key!("trajectory_when"))],
)
.props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"轨迹"#)
        .raw_name(r#"轨迹"#)
        .tags(&[r#"反击"#, r#"特"#])
        .text(CardText::of(&[sec(r#""#, &[para(r#"[反击][特]"#), items(&[it_sub(r#"场上有玩家[破产]时，依次进行以下操作："#, &[r#"展示此卡。"#, r#"获得那名玩家的任意1张地契，自动免费赎回。"#, r#"拆除该地契对应格子上的所有房屋。"#])])])]))
        // sheet cell E15
        .fields(&[("group", r#"Roselia"#), ("owner", "")]));

/// 规则书（1）[反击]: 「在场上有玩家破产时，展示此卡」
fn can_counteract(player_id: i32) -> bool {
    let who = trigger::target();
    if who < 0 || who == player_id {
        return false;
    }
    ctx::owned_count(who) > 0
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let who = trigger::target();
    if who < 0 {
        return Ok(());
    }
    let deeds: Vec<i32> = ctx::owned_tiles(who);
    if deeds.is_empty() {
        return Ok(());
    }
    let tile = ctx::ask_tile(
        player_id,
        &Msg::new(key!("trajectory_ask_title")),
        &Msg::new(key!("trajectory_ask_text")).player_id("who", who),
        &deeds,
    )?;
    ctx::set_owner(tile, player_id);
    // 规则书（1）[反击]: 「（自动免费赎回）」
    ctx::set_mortgaged(tile, false);
    // 规则书（1）[反击]: 「并拆除那个对应格子的所有房屋」
    ctx::set_houses(tile, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("trajectory_take"))
            .player_id("who", player_id)
            .player_id("them", who)
            .tile("tile", tile),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
