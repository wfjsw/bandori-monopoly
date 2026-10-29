//! `MyGO:普通与理所当然` -- C# `CardOrdinary` (MatchHost.cs:7087-7113): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:普通与理所当然`）:
//! > 普通与理所当然：
//! >  [反击] 受到异常移动效果影响后可打出，使你下一次主要移动的格数变为移动你最近一次非传送的主要移动的移动格数。
//!
//! after an abnormal-move effect, copy the step count of your last non-teleport
//! main move onto the next one.

use bandori_std::ctx;
use card_sdk::abi::ChainKind;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const ORDINARY: CardDef = CardDef::new(
    "MyGO:普通与理所当然",
    &[
        // 规则书: 「[反击] 受到异常移动效果影响后可打出」
        // TODO(规则书)（受到异常移动效果影响后）: 是效果落地**后** -- 现在的
        //   `ChainKind::Effect` 窗口在结算前开（`immuneAll` 的拒绝还没跑），免疫者
        //   也会被提供并使用；落地后没有可[反击]的 abnormal 窗口。
        On::Counteract(
            &[ChainKind::Effect],
            // TODO(规则书)（[反击] 受到异常移动效果影响后可打出）: 书没有前置移动
            //   要求 -- `lastWalk > 0` 是 C# 多加的门，没走过非传送主要移动就永不提供。
            "target == owner && effect.has(Abnormal) && slot('lastWalk') > 0",
            None,
            counteract,
        ).counter_reason(key!("ordinary_when")),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"普通与理所当然"#)
        .raw_name(r#"普通与理所当然"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 受到异常移动效果影响后可打出，使你下一次主要移动的格数变为移动你最近一次非传送的主要移动的移动格数。"#))
        // sheet cell I16
        .fields(&[("group", r#"MyGO!!!!!"#), ("owner", "")]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「使你下一次主要移动的格数变为移动你最近一次非传送的主要移动的
    // 移动格数」 -- C# `Counteract` reads `H.V(seat, "lastWalk") - 1` (the `NoteWalk`
    // count) and installs it as `H.ExtraOf<NextStepsFx>(c.Seat).Steps = steps`,
    // which `MoveBefore` applies to the next main move. `ctx::set_next_steps` is
    // that NextStepsFx: the engine takes the stored count when planning the next
    // **main** move (`lastWalk` itself is written only by a non-teleport main
    // walk, so the source is the "last non-teleport main move" the clause names).
    // TODO(规则书)（使你下一次主要移动的格数变为…）: 书里的「下一次主要移动」是要被
    //   消耗的 -- 下一次主要移动若是[传送]，`main_move` 只在非传送分支取这个数，
    //   它会被留到之后再走的一次主要移动。
    let steps = ctx::slot(player_id, "lastWalk") - 1;
    ctx::set_next_steps(player_id, steps);
    ctx::log(
        player_id,
        &Msg::new(key!("ordinary_log")).i("n", steps as i64),
    );
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
