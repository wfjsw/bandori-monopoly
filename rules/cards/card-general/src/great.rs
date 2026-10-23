//! `通用:GREAT` -- C# `CardGreat`: remove, PERFECT to the deck, +2,000.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:GREAT`）:
//! > GREAT：
//! > [手]：
//! > 依次进行以下操作：
//! > 1. [移除]此卡；
//! > 2. 将一张“PERFECT“加入抽卡区并洗切；
//! > 3. [获得]2000资金。
//!

use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{CardText, para, sec, steps};

pub const GREAT: CardDef = CardDef::new("通用:GREAT", &[On::Play("", None, None, great)]).meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"GREAT"#)
        .raw_name(r#"GREAT"#)
        .tags(&[r#"手"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"依次进行以下操作："#),
                steps(&[
                    r#"[移除]此卡；"#,
                    r#"将一张“PERFECT”加入抽卡区并洗切；"#,
                    r#"[获得]2000资金。"#,
                ]),
            ]),
        ]))
        // sheet cell A4
        .fields(&[("group", r#"通用"#), ("owner", "")]),
);

fn great(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「1. [移除]此卡」
    ctx::set_dest(ctx::Dest::Banished);
    // 规则书[手]: 「2. 将一张“PERFECT“加入抽卡区并洗切」
    ctx::add_to_deck(player_id, "通用:[衍生]PERFECT", true);
    ctx::log(
        player_id,
        &Msg::new(key!("great_added")).player_id("who", player_id),
    );
    // 规则书[手]: 「3. [获得]2000资金」 -- `ctx::n(1, …)` so
    // 「后勤人员的努力」 (「将打出的那张卡[手]效果中的一个数字变为两倍」)
    // can double it. Index 1 is this effect's only tagged number.
    ctx::gain(player_id, ctx::n(1, 2000), &Msg::new(key!("great_why")))?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn great_kani(player_id: i32) -> card_sdk::Asked {
    great(player_id)
}
