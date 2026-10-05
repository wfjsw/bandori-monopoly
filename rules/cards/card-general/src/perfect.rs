//! `通用:[衍生]PERFECT` -- C# `CardPerfect`: remove, FEVER! to the discard, +3,000.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:[衍生]PERFECT`）:
//! > [衍生]PERFECT：
//! > [手]：
//! > 依次进行以下操作：
//! > 1. [移除]此卡；
//! > 2. 将一张“FEVER!“加入弃卡区；
//! > 3. [获得]3000资金。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const PERFECT: CardDef = CardDef { id: "通用:[衍生]PERFECT", play: Some(perfect), can_react: None, react: None, why_not: None };

fn perfect(seat: i32) {
    // 规则书[手]: 「1. [移除]此卡」
    ctx::set_dest(ctx::Dest::Banished);
    // 规则书[手]: 「2. 将一张“FEVER!“加入弃卡区」
    ctx::to_discard(seat, "通用:[衍生]FEVER!");
    ctx::log(seat, &Msg::new(key!("perfect_added")).seat("who", seat));
    // 规则书[手]: 「3. [获得]3000资金」
    ctx::gain(seat, 3000, &Msg::new(key!("perfect_why")));
}