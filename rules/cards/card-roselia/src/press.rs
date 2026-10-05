//! `R:[衍生] 压` -- C# `CardPress`: gain 1,000.

use card_sdk::{ctx, key, CardDef, Msg};

pub const PRESS: CardDef = CardDef { id: "R:[衍生] 压", play: Some(press), can_react: None, react: None, why_not: None };

fn press(seat: i32) {
    ctx::gain(seat, 1000, &Msg::new(key!("press_why")));
}
