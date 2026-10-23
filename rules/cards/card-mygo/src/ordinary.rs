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
use card_sdk::{CardText};
use card_sdk::{key, CardDef, Msg, On};

pub const ORDINARY: CardDef = CardDef::new(
    "MyGO:普通与理所当然",
    &[On::Counteract(
        &[ChainKind::Effect],
        "target == owner && effect.has(Abnormal) && slot('lastWalk') > 0", Some(key!("ordinary_when")),
        None,
        counteract,
    )],
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
    // that NextStepsFx: the engine takes the stored count when planning the move.
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
