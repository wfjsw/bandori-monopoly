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

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const GREAT: CardDef = CardDef::new("通用:GREAT", &[On::Play(None, great)]);

fn great(player_id: i32) {
    // 规则书[手]: 「1. [移除]此卡」
    ctx::set_dest(ctx::Dest::Banished);
    // 规则书[手]: 「2. 将一张“PERFECT“加入抽卡区并洗切」
    ctx::add_to_deck(player_id, "通用:[衍生]PERFECT", true);
    ctx::log(player_id, &Msg::new(key!("great_added")).player_id("who", player_id));
    // 规则书[手]: 「3. [获得]2000资金」
    ctx::gain(player_id, 2000, &Msg::new(key!("great_why")));
}