//! `PPP:[衍生]Pipopa` -- C# `CardPipopa`: gain 1,000 and a Popipapapipopa
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:[衍生]Pipopa`）:
//! > [衍生]Pipopa：
//! > [手]：
//! > 获得1000资金，然后将一张“Popipapapipopa”放置在自身场上，然后此卡[移除]
//!
//! stays in play.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const PIPOPA: CardDef = CardDef::new("PPP:[衍生]Pipopa", &[On::Play(None, pipopa)]);

fn pipopa(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Banished);
    ctx::gain(player_id, 1000, &Msg::new(key!("pipopa_why")))?;
    ctx::place_card(
        player_id,
        "PPP:[衍生]Popipapapipopa",
        &Msg::new(key!("pipopa_note")),
    );
    Ok(())
}
