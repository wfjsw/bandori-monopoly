//! `PPP:[衍生]Pipopa` -- C# `CardPipopa`: gain 1,000 and a Popipapapipopa
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:[衍生]Pipopa`）:
//! > [衍生]Pipopa：
//! > [手]：
//! > 获得1000资金，然后将一张“Popipapapipopa”放置在自身场上，然后此卡[移除]
//!
//! stays in play.

use card_sdk::{ctx, key, CardDef, Msg};

pub const PIPOPA: CardDef = CardDef { id: "PPP:[衍生]Pipopa", play: Some(pipopa), can_react: None, react: None, why_not: None };

fn pipopa(seat: i32) {
    ctx::set_dest(ctx::Dest::Banished);
    ctx::gain(seat, 1000, &Msg::new(key!("pipopa_why")));
    ctx::place_card(seat, "PPP:[衍生]Popipapapipopa", &Msg::new(key!("pipopa_note")));
}
