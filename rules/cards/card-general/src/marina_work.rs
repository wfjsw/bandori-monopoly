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
        // `condition::MINE` is the 「[使用者]」 half (`actor == owner`).
        On::Counteract(
            &[bandori_std::kind::PassRewardAffected.as_trigger()],
            card_sdk::condition::MINE,
            // TODO(规则书)（[经过]#1格子且#1格子受到其他效果影响时）: neither remaining
            //   conjunct of the window is checked -- the passed tile is not tested for
            //   being #1 (or its re-homed form) and 「受到其他效果影响」 is not tested
            //   either, so the card is offered on every `passRewardAffected`
            //   (OPEN-GEN-marina-extra); the raise point is also too narrow -- a pass
            //   onto #1 from #1, or one vetoed by `NO_REWARD`, earns no reward and
            //   never raises, so its settle cannot be answered either.
            None,
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

fn counteract(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("marina_work_normal")).player_id("who", player_id),
    );
    // TODO(规则书)[judgement](ABI): The trigger does carry the passed tile
    // (`trigger::tile()`, readable in this window); the log above just names it
    // statically as #1.
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
