//! `通用:GREAT` -- C# `CardGreat`: remove, PERFECT to the deck, +2,000.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:GREAT`）:
//! > GREAT：
//! > [手]：
//! > 依次进行以下操作：
//! > 1. [移除]此卡；
//! > 2. 将一张“PERFECT“加入抽卡区并洗切；
//! > 3. [获得]2000资金。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const GREAT: CardDef = CardDef { id: "通用:GREAT", play: Some(great), can_react: None, react: None, why_not: None };

fn great(seat: i32) {
    // 规则书[手]: 「1. [移除]此卡」
    ctx::set_dest(ctx::Dest::Banished);
    // 规则书[手]: 「2. 将一张“PERFECT“加入抽卡区并洗切」
    ctx::add_to_deck(seat, "通用:[衍生]PERFECT", true);
    ctx::log(seat, &Msg::new(key!("great_added")).seat("who", seat));
    // 规则书[手]: 「3. [获得]2000资金」
    ctx::gain(seat, 2000, &Msg::new(key!("great_why")));
}