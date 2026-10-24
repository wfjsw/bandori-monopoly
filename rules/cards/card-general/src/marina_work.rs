//! `通用:[月岛麻里奈]今天也要加油工作喔` -- C# `CardMarinaWork`
//!
//! 规则书（docs/rulebook/cards.json, id `通用:[月岛麻里奈]今天也要加油工作喔`）:
//! > [月岛麻里奈]今天也要加油工作喔：
//! > [手]：
//! > [反击][使用者][经过]#1格子且#1格子受到其他效果影响时：[使用者]本次对#1格子的[经过]或[结算]正常进行而不受到其上的额外效果。
//!
//! (MatchHost.cs:2409-2429): [反击] let a CiRCLE [经过]/[结算] proceed normally.

use bandori_std::ctx;
use card_sdk::abi::ChainKind;
use card_sdk::{CardText, para, sec};
use card_sdk::{key, CardDef, Msg, On};

pub const MARINA_WORK: CardDef = CardDef::new(
    "通用:[月岛麻里奈]今天也要加油工作喔",
    &[On::Counteract(
        &[bandori_std::kind::PassRewardAffected.as_trigger()],
        card_sdk::pre::MINE,
        None,
        counteract).counter_reason(key!("marina_work_when"))],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"今天也要加油工作喔"#)
        .raw_name(r#"[月岛麻里奈]今天也要加油工作喔"#)
        .tags(&[r#"手"#, r#"反击"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"[反击][使用者][经过]#1格子且#1格子受到其他效果影响时：[使用者]本次对#1格子的[经过]或[结算]正常进行而不受到其上的额外效果。"#),
            ]),
        ]))
        // sheet cell A14
        .fields(&[("group", r#"通用"#), ("owner", r#"月岛麻里奈"#)]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「[使用者]本次对#1格子的[经过]或[结算]正常进行而不受到其上的额外效果」
    // C# `c.Trigger.Move.Tags["circleNormal"] = 1` then
    // 「今天也要加油工作喔：这次 CiRCLE 的 [经过] / [结算] 正常进行，不受其上的额外效果影响」.
    ctx::log(
        player_id,
        &Msg::new(key!("marina_work_normal")).player_id("who", player_id),
    );
    // TODO(规则书)[judgement](ABI): 「正常进行而不受到其上的额外效果」 -- needs a move-tag hook
    //   the clause under-specifies -- see the note above it
    // (C# `c.Trigger.Move.Tags["circleNormal"] = 1`) so the CiRCLE [经过]/[结算]
    // ignores the tile's extra effects; until then they still apply. The trigger
    // also carries no tile id for the log line (#1格子).
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
