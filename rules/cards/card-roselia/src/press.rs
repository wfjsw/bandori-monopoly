//! `R:[衍生] 压` -- C# `CardPress`: gain 1,000.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const PRESS: CardDef = CardDef::new("R:[衍生] 压", &[On::Play(None, press)]);

fn press(player_id: i32) -> card_sdk::Asked {
    ctx::gain(player_id, 1000, &Msg::new(key!("press_why")));
    Ok(())
}
