//! `R:[衍生] 觉悟` -- C# `CardResolve`: gain 1,000 and the card is removed from
//!
//! 规则书（docs/rulebook/cards.json, id `R:[衍生] 觉悟`）:
//! > [衍生] 觉悟：
//! > [特]：此卡[移除]时获得1000资金。
//! > [手]：[移除]此卡。
//!
//! the game.

use card_sdk::{ctx, key, CardDef, Msg};

pub const RESOLVE: CardDef = CardDef { id: "R:[衍生] 觉悟", play: Some(resolve), can_react: None, react: None, why_not: None };

fn resolve(seat: i32) {
    ctx::set_dest(ctx::Dest::Banished);
    ctx::gain(seat, 1000, &Msg::new(key!("resolve_why")));
}
