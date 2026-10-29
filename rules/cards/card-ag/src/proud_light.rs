//! `AG:刻入天穹傲岸的烈光` -- C# `CardProudLight` (MatchHost.cs:1681-1721):
//!
//! 规则书（docs/rulebook/cards.json, id `AG:刻入天穹傲岸的烈光`）:
//! > 刻入天穹傲岸的烈光：
//! > [反击] 当你经过一名角色时，你可以打出此卡，你从对方处获得等于对方最贵格子基础购买价格一半数额的资金，之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金。
//!
//! swap half of each side's most expensive deed price with **each** character
//! passed (every occupant counts as 「一名角色」).

use alloc::vec::Vec;
use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const PROUD_LIGHT: CardDef = CardDef::new(
    "AG:刻入天穹傲岸的烈光",
    // 规则书[反击]: 「当你经过一名角色时」 -- 行动阶段 12 [经过]
    // (`SETTLE-STAGES.md` §4 M4), the step onto a tile a character stands on.
    // The walk's last step raises it too (移动终点触发[经过], before stage 13
    // raises [重叠]). `ChainKind::PassTile` is the [反击] key for one [经过]
    // step (ABI v43).
    // G4: kind (PassTile) is the category; `mine` is the condition. The
    // "someone stands on the tile" check is a derived list (GUARDS.md §6) and
    // stays in the residual guard.
    &[On::Counteract(
        &[ChainKind::PassTile],
        card_sdk::pre::MINE,
        Some(can_counteract),
        counteract).counter_reason(key!("proud_light_when"))],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"刻入天穹傲岸的烈光"#)
        .raw_name(r#"刻入天穹傲岸的烈光"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 当你经过一名角色时，你可以打出此卡，你从对方处获得等于对方最贵格子基础购买价格一半数额的资金，之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金。"#))
        // sheet cell C13
        .fields(&[("group", r#"Afterglow"#), ("owner", "")]));

/// The player's most expensive deed's base purchase price (C# `CardProudLight.Best`).
fn best_price(player_id: i32) -> i32 {
    // 规则书[反击]: 「对方最贵格子基础购买价格」-- max of `H._tiles[t].price`
    // (`ctx::tile_price`, the land price alone).
    ctx::owned_tiles(player_id)
        .into_iter()
        .map(ctx::tile_price)
        .max()
        .unwrap_or(0)
}

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「当你经过一名角色时」 -- the actor rel is the condition
    // (`pre::MINE`); 「一名角色」 is a derived list (GUARDS.md §6) and stays here.
    let _ = player_id;
    // 「一名角色」 -- a player standing on the tile being entered.
    ctx::players_on(trigger::tile(), player_id)
        .into_iter()
        .any(|o| o >= 0 && !ctx::player_out(o))
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 「对方」 -- the characters being passed. A `passTile` payload has no
    // `target`, so the tile's occupants are it; every occupant is 「一名角色」,
    // so both legs run once per passed character.
    // TODO(规则书)（一名角色）+ TODO(ABI): we re-read the tile at resolution (a
    //   later counter moving an occupant off makes that pair fall through for
    //   them); `passTile` carries no `target` for a pick.
    let passed: Vec<i32> = ctx::players_on(trigger::tile(), player_id)
        .into_iter()
        .filter(|&o| o >= 0 && !ctx::player_out(o))
        .collect();
    for o in passed {
        // 规则书[反击]: 「你从对方处获得等于对方最贵格子基础购买价格一半数额的资金」
        let from_them = best_price(o) / 2;
        if from_them > 0 {
            ctx::transfer(o, player_id, from_them, &Msg::new(key!("proud_light_why")))?;
        }
        // 规则书[反击]: 「之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金」
        let from_us = best_price(player_id) / 2;
        if from_us > 0 && !ctx::player_out(o) {
            ctx::transfer(player_id, o, from_us, &Msg::new(key!("proud_light_why")))?;
        }
    }
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
