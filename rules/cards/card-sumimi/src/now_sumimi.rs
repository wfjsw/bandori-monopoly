//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:现在她是Sumimi的小初啦`）:
//! > 现在她是Sumimi的小初啦：
//! > [反击] 当你在你的本回合开始后到下回合开始前之间失去资金的总额即将超过你所在格子的[收费标价]时打出此卡，获得相当于你所在格子[收费标价]数额的资金（RiNG则为其基础乘数）
//!
//! [反击] on a pay, gain the tile's price tag.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const NOW_SUMIMI: CardDef = CardDef::new(
    "Sumimi:现在她是Sumimi的小初啦",
    &[On::Counteract(
        &[ChainKind::Effect],
        // 规则书[反击]: 「失去资金」 -- the `pay` effect entry, from the holder,
        // of a positive amount. The causer's own `abnormal` also rides an
        // `effect` link with `player_id` = the causer and `value` = its `AbKind`
        // (> 0); `chain_has(Pay)` is what keeps it out (its effect kind is
        // "abnormal"), not the value test.
        // `value > 0` covers only a payment an earlier stage already set to 0
        // (`set_pay_amount(0)`).
        // A payment an earlier counter *cancelled* would still read its
        // declared value; the residual guard reads `pending_negation()` so the
        // card is not offered over an already-declared negation (design G06
        // §5.5, item 11).
        "chain_has(Pay) && actor == owner && value > 0",
        Some(can_counteract),
        counteract).counter_reason(key!("now_sumimi_when"))],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"现在她是Sumimi的小初啦"#)
        .raw_name(r#"现在她是Sumimi的小初啦"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 当你在你的本回合开始后到下回合开始前之间失去资金的总额即将超过你所在格子的[收费标价]时打出此卡，获得相当于你所在格子[收费标价]数额的资金（RiNG则为其基础乘数）"#))
        // sheet cell L5
        .fields(&[("group", r#"Sumimi"#), ("owner", "")]));

fn is_deed(t: i32) -> bool {
    ctx::is_deed(t)
}

// TODO(规则书)（[收费标价]）: 「收费标价」 is an open term (OPEN-收费标价: the
//   purchase price or the charge); we read the current rent (RiNG: its 基础乘数).
//   A non-buyable tile (地产商 / CiRCLE / café / 江户川 / 流星堂) has no rent row
//   and answers 0, though the sheet puts no buyable qualifier on 「你所在格子」.
fn price_tag(player_id: i32) -> i32 {
    let t = ctx::player_pos(player_id);
    if t < 0 || !ctx::is_buyable(t) {
        return 0;
    }
    if is_deed(t) {
        return ctx::ring_multiplier();
    }
    // `rent_of` is the rent table at the current house count (0 for empty tables).
    ctx::rent_of(t)
}

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「当…失去资金的总额即将超过你所在格子的[收费标价]时打出此卡」
    // The payment shape is the condition; the price-tag comparison is the residual.
    // A 0 tag (a non-buyable tile) still lets any positive payment through.
    // 规则书: the payment must still be coming -- a declared negation on the
    // link means it will not settle (「即将超过」 was already the point of no
    // return for a negation guard).
    if trigger::pending_negation() != 0 {
        return false;
    }
    let tag = price_tag(player_id);
    // TODO(规则书)[judgement] The guard here is `amount > Tag` alone: it drops both the
    // running total and the 「即将超过」 window, so it also fires once the total has
    // already crossed the tag.
    trigger::value() > tag
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let tag = price_tag(player_id);
    if tag > 0 {
        ctx::gain(
            player_id,
            tag,
            &Msg::new(key!("now_sumimi_reason")).n("n", tag as i64),
        )?;
    }
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
