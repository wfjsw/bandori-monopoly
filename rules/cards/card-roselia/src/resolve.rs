//! `R:[衍生] 觉悟` -- C# `CardResolve`: gain 1,000 and the card is removed from
//!
//! 规则书（docs/rulebook/cards.json, id `R:[衍生] 觉悟`）:
//! > [衍生] 觉悟：
//! > [特]：此卡[移除]时获得1000资金。
//! > [手]：[移除]此卡。
//!
//! the game.

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const RESOLVE: CardDef = CardDef::new("R:[衍生] 觉悟", &[On::Play(None, resolve)]);

fn resolve(player_id: i32) {
    ctx::set_dest(ctx::Dest::Banished);
    ctx::gain(player_id, 1000, &Msg::new(key!("resolve_why")));
}
