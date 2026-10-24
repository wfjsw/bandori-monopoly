//! `PPP:Popipa` -- C# `CardPopipa`: gain 1,000, Pipopa joins the draw pile,
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:Popipa`）:
//! > Popipa：
//! > [手]：
//! > 获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]
//!
//! draw 1.

use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{CardText, para, sec};

pub const POPIPA: CardDef = CardDef::new("PPP:Popipa", &[On::Play("", None, popipa)]).meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"Popipa"#)
        .raw_name(r#"Popipa"#)
        .tags(&[r#"手"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]"#),
            ]),
        ]))
        // sheet cell B2
        .fields(&[("group", r#"Poppin' Party"#), ("owner", "")]),
);

fn popipa(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Banished);
    ctx::gain(player_id, 1000, &Msg::new(key!("popipa_why")))?;
    ctx::add_to_dest(player_id, "PPP:[衍生]Pipopa", ctx::Dest::DeckRandom);
    ctx::log(
        player_id,
        &Msg::new(key!("popipa_added")).player_id("who", player_id),
    );
    ctx::draw(player_id, 1)?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn popipa_kani(player_id: i32) -> card_sdk::Asked {
    popipa(player_id)
}
