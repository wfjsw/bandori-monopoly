//! `R:[衍生] 觉悟` -- C# `CardResolve`: gain 1,000 and the card is removed from
//!
//! 规则书（docs/rulebook/cards.json, id `R:[衍生] 觉悟`）:
//! > [衍生] 觉悟：
//! > [特]：此卡[移除]时获得1000资金。
//! > [手]：[移除]此卡。
//!
//! the game.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const RESOLVE: CardDef = CardDef::new("R:[衍生] 觉悟", &[On::Play("", None, None, resolve)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"觉悟"#)
        .raw_name(r#"[衍生] 觉悟"#)
        .derived(true)
        .tags(&[r#"手"#, r#"特"#, r#"衍生"#])
        .text(r#"[特]：此卡[移除]时获得1000资金。
[手]：[移除]此卡。"#)
        .cell(r#"E17"#)
        .fields(&[("group", r#"Roselia"#), ("owner", "")]))
    .quiet_when_empty(&[0]);

fn resolve(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Banished);
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
