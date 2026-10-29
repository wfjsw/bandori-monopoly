//! `R:[衍生] 觉悟` -- C# `CardResolve`: gain 1,000 and the card is removed from
//! the game.
//!
//! 规则书（docs/rulebook/cards.json, id `R:[衍生] 觉悟`）:
//! > [衍生] 觉悟：
//! > [特]：此卡[移除]时获得1000资金。
//! > [手]：[移除]此卡。

use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const RESOLVE: CardDef = CardDef::new(
    "R:[衍生] 觉悟",
    &[On::Play("", None, resolve).quiet_when_empty()],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"觉悟"#)
        .raw_name(r#"[衍生] 觉悟"#)
        .derived(true)
        .tags(&[r#"手"#, r#"特"#, r#"衍生"#])
        .text(CardText::of(&[
            sec("[特]：", &[para("此卡[移除]时获得1000资金。")]),
            sec("[手]：", &[para("[移除]此卡。")]),
        ]))
        // sheet cell E17
        .fields(&[("group", r#"Roselia"#), ("owner", "")]),
);

fn resolve(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「[手]：[移除]此卡。」
    ctx::set_dest(ctx::Dest::Banished);
    // TODO(规则书)（此卡[移除]时获得1000资金）: [特] pays on any [移除] (from any
    //   zone) -- only this play pays; no removal event exists to key on.
    // 规则书: 「[特]：此卡[移除]时获得1000资金。」
    ctx::gain(player_id, 1000, &Msg::new(key!("resolve_why")))?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn resolve_kani(player_id: i32) -> card_sdk::Asked {
    resolve(player_id)
}
