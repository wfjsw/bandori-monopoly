//!
//! 规则书（docs/rulebook/cards.json, id `R:[衍生] 觉悟`）:
//! > [衍生] 觉悟：
//! > [特]：此卡[移除]时获得1000资金。
//! > [手]：[移除]此卡。

use bandori_std::zones::ZoneChangeCause;
use card_sdk::{ctx, key, Action, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const RESOLVE: CardDef = CardDef::new(
    "R:[衍生] 觉悟",
    &[
        On::Play("", None, resolve).quiet_when_empty(),
        // 规则书[特]: 「此卡[移除]时获得1000资金」 -- a leave from the field; the
        // card is [移除] rather than discarded.
        On::Action(Action::ZoneLeft("card.placed", None, on_removed)),
    ],
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
    // 规则书: 「[手]：[移除]此卡。」 -- the play's own removal is a hand card, so
    // it pays here; a later field removal pays through `zoneLeft`.
    // 规则书: 「[特]：此卡[移除]时获得1000资金。」
    ctx::gain(player_id, 1000, &Msg::new(key!("resolve_reason")))?;
    ctx::set_dest(ctx::Dest::Banished, ZoneChangeCause::Effect);
    Ok(())
}

/// 规则书[特]: 「此卡[移除]时获得1000资金」 -- any removal from the field pays
/// and files the card out of the game.
fn on_removed(player_id: i32) -> card_sdk::Asked {
    // Instance kind: another copy sees this raise too.
    if ctx::trigger::instance() != ctx::self_uid() {
        return Ok(());
    }
    bandori_std::zones::set_leave_destination(ctx::Dest::Banished);
    ctx::gain(player_id, 1000, &Msg::new(key!("resolve_reason")))?;
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

#[cfg(kani)]
#[inline(always)]
pub fn on_removed_kani(player_id: i32) -> card_sdk::Asked {
    on_removed(player_id)
}
