//! `PPP:Popipa` -- C# `CardPopipa`: gain 1,000, Pipopa joins the draw pile,
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:Popipa`）:
//! > Popipa：
//! > [手]：
//! > 获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]
//!
//! draw 1.

use card_sdk::{ctx, key, CardDef, Msg};

pub const POPIPA: CardDef = CardDef { id: "PPP:Popipa", play: Some(popipa), can_react: None, react: None, why_not: None };

fn popipa(seat: i32) {
    ctx::set_dest(ctx::Dest::Banished);
    ctx::gain(seat, 1000, &Msg::new(key!("popipa_why")));
    ctx::add_to_deck(seat, "PPP:[衍生]Pipopa", true);
    ctx::log(seat, &Msg::new(key!("popipa_added")).seat("who", seat));
    ctx::draw(seat, 1);
}
