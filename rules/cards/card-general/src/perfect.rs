//! `通用:[衍生]PERFECT` -- C# `CardPerfect`: remove, FEVER! to the discard, +3,000.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:[衍生]PERFECT`）:
//! > [衍生]PERFECT：
//! > [手]：
//! > 依次进行以下操作：
//! > 1. [移除]此卡；
//! > 2. 将一张“FEVER!“加入弃卡区；
//! > 3. [获得]3000资金。
//!

use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{para, sec, steps, CardText};

pub const PERFECT: CardDef = CardDef::new("通用:[衍生]PERFECT", &[On::Play("", None, perfect)])
    .meta(
        &card_sdk::meta::CardMeta::new()
            .name(r#"PERFECT"#)
            .raw_name(r#"[衍生]PERFECT"#)
            .derived(true)
            .tags(&[r#"手"#, r#"衍生"#])
            .text(CardText::of(&[sec(
                r#"[手]："#,
                &[
                    para(r#"依次进行以下操作："#),
                    steps(&[
                        r#"[移除]此卡；"#,
                        r#"将一张“FEVER!“加入弃卡区；"#,
                        r#"[获得]3000资金。"#,
                    ]),
                ],
            )]))
            // sheet cell A5
            .fields(&[("group", r#"通用"#), ("owner", "")]),
    );

fn perfect(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「1. [移除]此卡」
    ctx::set_dest(ctx::Dest::Banished);
    // 规则书[手]: 「2. 将一张“FEVER!“加入弃卡区」 -- `ctx::add_to_dest` is the
    // create-only push; `ctx::to_discard` would first consume a same-named hand copy.
    // TODO(规则书)（将一张“FEVER!“加入弃卡区）: 加入仍记进 `discard_log`，会触发
    //   「进入弃卡区」钩子 -- 书是「加入」，并未「弃置」.
    ctx::add_to_dest(player_id, "通用:[衍生]FEVER!", ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("perfect_added")).player_id("who", player_id),
    );
    // 规则书[手]: 「3. [获得]3000资金」 -- `ctx::n(1, …)` so
    // 「后勤人员的努力」 (「将打出的那张卡[手]效果中的一个数字变为两倍」)
    // can double it. Index 1 is this effect's only tagged number.
    ctx::gain(player_id, ctx::n(1, 3000), &Msg::new(key!("perfect_why")))?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn perfect_kani(player_id: i32) -> card_sdk::Asked {
    perfect(player_id)
}
