//! `PPP:（多惠）寻找更好的声音` -- C# `CardTaeSound`: teleport to Edogawa
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（多惠）寻找更好的声音`）:
//! > （多惠）寻找更好的声音：
//! >  传送到“江户川乐器店”并获得一个[火罐]
//!
//! Instruments (no settle), +1 fire.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const TAE_SOUND: CardDef =
    CardDef::new("PPP:（多惠）寻找更好的声音", &[On::Play(None, tae_sound, "")]);

fn tae_sound(player_id: i32) -> card_sdk::Asked {
    let shop = ctx::tile_named("江户川乐器店");
    if shop >= 0 {
        ctx::teleport_to(player_id, shop);
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("tae_sound_why")))?;
    Ok(())
}
