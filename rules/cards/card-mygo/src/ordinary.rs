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
        // The refusals (status gate / `immuneAll`) run **before** this window
        // (G06 §6.7, engine `abnormal_gate`), so a protected player is no
        // longer offered; 「影响后」 stays 结算前 within the landed pipeline
        // (the `abnormal` hook itself is the outcome and cannot be countered).
        On::Counteract(
            &[ChainKind::Effect],
            // TODO(规则书)（[反击] 受到异常移动效果影响后可打出）:
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
    // TODO(规则书)（使你下一次主要移动的格数变为…）: `ctx::set_next_steps` is that NextStepsFx: the
    // engine takes the stored count when planning the next **main** move (`lastWalk`
    // itself is written only by a non-teleport main walk, so the source is the "last
    // non-teleport main move" the clause names). 书里的「下一次主要移动」是要被 消耗的 --
    // 下一次主要移动若是[传送]，`main_move` 只在非传送分支取这个数， 它会被留到之后再走的一次主要移动。
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
