//!
//! 规则书（docs/rulebook/cards.json, id `通用:[月岛麻里奈]今天也要加油工作喔`）:
//! > [月岛麻里奈]今天也要加油工作喔：
//! > [手]：
//! > [反击][使用者][经过]#1格子且#1格子受到其他效果影响时：[使用者]本次对#1格子的[经过]或[结算]正常进行而不受到其上的额外效果。
//!

use bandori_std::ctx;
use card_sdk::abi::ChainKind;
use card_sdk::{key, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const MARINA_WORK: CardDef = CardDef::new(
    "通用:[月岛麻里奈]今天也要加油工作喔",
    &[
        // 规则书[反击]: 「[反击][使用者][经过]#1格子且#1格子受到其他效果影响时」
        // The condition spells `condition::MINE` (`actor == owner`; the half
        // is a `&'static str`, so it cannot be a constant reference) and adds
        // the 「#1格子」 test -- 「受到其他效果影响」 is what the window is:
        // an extra to suppress. `tile.id` is the governing tile, so a swapped
        // start tile still passes.
        On::Counteract(
            &[ChainKind::TileEffectBefore],
            "actor == owner && tile_is(tile.id, \"circle\")",
            Some(can_counteract),
            counteract,
        )
        .counter_reason(key!("marina_work_when")),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"今天也要加油工作喔"#)
        .raw_name(r#"[月岛麻里奈]今天也要加油工作喔"#)
        // 规则书: 「[手]：」 -- and 「[反击]」, the [手] card's own counter window.
        .tags(&[r#"手"#, r#"反击"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"[反击][使用者][经过]#1格子且#1格子受到其他效果影响时：[使用者]本次对#1格子的[经过]或[结算]正常进行而不受到其上的额外效果。"#),
            ]),
        ]))
        // sheet cell A14
        .fields(&[("group", r#"通用"#), ("owner", r#"月岛麻里奈"#)]));

/// 「#1格子受到其他效果影响」 -- there must be an extra to suppress; the
/// tile's own base rule runs either way.
fn can_counteract(_owner: i32) -> bool {
    ctx::tile_extra_count(ctx::trigger::tile()) > 0
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    // Hide this tile's extras for the rest of the move: both the pass step
    // and its settle run clean (设计 5.2).
    ctx::plan::set_tag(card_sdk::abi::tag::SUPPRESS_TILE_EFFECTS, t);
    ctx::log(
        player_id,
        &Msg::new(key!("marina_work_normal"))
            .player_id("who", player_id)
            .tile("tile", t),
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
