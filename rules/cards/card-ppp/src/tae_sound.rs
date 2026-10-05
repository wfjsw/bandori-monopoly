//! `PPP:（多惠）寻找更好的声音` -- C# `CardTaeSound`: teleport to Edogawa
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（多惠）寻找更好的声音`）:
//! > （多惠）寻找更好的声音：
//! >  传送到“江户川乐器店”并获得一个[火罐]
//!
//! Instruments (no settle), +1 fire.

use card_sdk::{ctx, key, CardDef, Msg};

pub const TAE_SOUND: CardDef = CardDef { id: "PPP:（多惠）寻找更好的声音", play: Some(tae_sound), can_react: None, react: None, why_not: None };

fn tae_sound(seat: i32) {
    let shop = ctx::tile_named("江户川乐器店");
    if shop >= 0 {
        ctx::teleport_to(seat, shop);
    }
    ctx::gain_fire(seat, 1, &Msg::new(key!("tae_sound_why")));
}
