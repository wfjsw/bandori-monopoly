//! `R:[衍生] 压` -- gain 1,000.

// 规则书: `压` 是 Roselia 的衍生手卡，不在 docs/rulebook/cards.json 内。
// 规则书: [手]：获得1000资金。

use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const PRESS: CardDef = CardDef::new("R:[衍生] 压", &[On::Play("", None, press)]).meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"压"#)
        .raw_name(r#"[衍生] 压"#)
        .derived(true)
        .tags(&[r#"手"#, r#"衍生"#])
        .text(CardText::para(r#"[手]：获得1000资金。"#))
        .fields(&[("group", r#"Roselia"#), ("owner", "")]),
);

fn press(player_id: i32) -> card_sdk::Asked {
    ctx::gain(player_id, 1000, &Msg::new(key!("press_reason")))?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn press_kani(player_id: i32) -> card_sdk::Asked {
    press(player_id)
}
