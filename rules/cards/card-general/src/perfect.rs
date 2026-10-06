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

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const PERFECT: CardDef = CardDef::new("通用:[衍生]PERFECT", &[On::Play(None, perfect)]);

fn perfect(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「1. [移除]此卡」
    ctx::set_dest(ctx::Dest::Banished);
    // 规则书[手]: 「2. 将一张“FEVER!“加入弃卡区」
    ctx::to_discard(player_id, "通用:[衍生]FEVER!");
    ctx::log(player_id, &Msg::new(key!("perfect_added")).player_id("who", player_id));
    // 规则书[手]: 「3. [获得]3000资金」
    ctx::gain(player_id, 3000, &Msg::new(key!("perfect_why")));
    Ok(())
}