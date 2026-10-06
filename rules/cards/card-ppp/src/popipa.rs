//! `PPP:Popipa` -- C# `CardPopipa`: gain 1,000, Pipopa joins the draw pile,
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:Popipa`）:
//! > Popipa：
//! > [手]：
//! > 获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]
//!
//! draw 1.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const POPIPA: CardDef = CardDef::new("PPP:Popipa", &[On::Play(None, popipa)]);

fn popipa(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Banished);
    ctx::gain(player_id, 1000, &Msg::new(key!("popipa_why")));
    ctx::add_to_deck(player_id, "PPP:[衍生]Pipopa", true);
    ctx::log(
        player_id,
        &Msg::new(key!("popipa_added")).player_id("who", player_id),
    );
    ctx::draw(player_id, 1);
    Ok(())
}
