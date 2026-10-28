//! `PPP:（多惠）寻找更好的声音` -- C# `CardTaeSound`: teleport to Edogawa
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（多惠）寻找更好的声音`）:
//! > （多惠）寻找更好的声音：
//! >  传送到“江户川乐器店”并获得一个[火罐]
//!
//! bare 传送 -- `teleport_to` is a position write with `resolve: false` (no 结算
//! on arrival) -- then +1 [火罐].

use bandori_std::ctx;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const TAE_SOUND: CardDef = CardDef::new(
    "PPP:（多惠）寻找更好的声音",
    &[On::Play("", None, tae_sound)],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"寻找更好的声音"#)
        .raw_name(r#"（多惠）寻找更好的声音"#)
        .tags(&[])
        .text(CardText::para(r#"传送到“江户川乐器店”并获得一个[火罐]"#))
        // sheet cell B17
        .fields(&[("group", r#"Poppin' Party"#), ("owner", r#"花园多惠"#)]),
);

fn tae_sound(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「传送到“江户川乐器店”」 -- the bare (settle-free) teleport.
    let shop = ctx::tile_named("江户川乐器店");
    // TODO(规则书)（传送到“江户川乐器店”）: the book teleports unconditionally -- we
    // silently skip an absent tile (the fire is still paid).
    if shop >= 0 {
        ctx::teleport_to(player_id, shop);
    }
    // 规则书: 「并获得一个[火罐]」
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("tae_sound_why")))?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn tae_sound_kani(player_id: i32) -> card_sdk::Asked {
    tae_sound(player_id)
}
