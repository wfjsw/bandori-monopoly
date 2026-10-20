//! `Mor:（小白）` -- C# `CardMashiroPay` (MatchHost.cs:4996-5028): rewrite a pay
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（小白）`）:
//! > （小白）：
//! > [反击]当你将要向其他玩家支付时打出此卡，此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金
//!
//! into a pure loss and make the payee lose half.
//!
//! Counteraction-only (`Normal => false`).

use card_sdk::abi::ChainKind;
use bandori_std::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const MASHIRO_PAY: CardDef = CardDef::new(
    "Mor:（小白）",
    &[On::Counteract(
        &[ChainKind::Effect],
        "actor == owner && effect.has(Pay) && target >= 0 && target != owner && value > 0",
        r#"你将要向其他玩家支付"#,
        None,
        counteract,
    )],
).meta(&card_sdk::meta::CardMeta::new()
        .name("")
        .raw_name(r#"（小白）"#)
        .tags(&[r#"反击"#])
        .text(r#"[反击]当你将要向其他玩家支付时打出此卡，此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金"#)
        .cell(r#"G9"#)
        .fields(&[("group", r#"Morfonica"#), ("owner", r#"仓田真白"#)]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value();
    let to = trigger::target();
    // 规则书[反击]: 「此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金」
    // C# rewrites the pending `PayCtx` (`pay.to = -1; pay.kind = "lose"`) and then
    // `H.LoseR(to, amount / 2, ..., must: true)`. Redirecting to the bank is what
    // turns the transfer into a pure loss; the forced half-loss is a separate
    // card-driven pay, which is itself a [反击] point.
    trigger::set_pay_target(-1);
    ctx::log(
        player_id,
        &Msg::new(key!("mashiro_pay_rewritten"))
            .player_id("who", player_id)
            .player_id("to", to)
            .n("money", amount as i64)
            .n("half", (amount / 2) as i64),
    );
    if to >= 0 {
        ctx::pay(
            to,
            amount / 2,
            &Msg::new(key!("mashiro_pay_half"))
                .player_id("who", to)
                .n("n", (amount / 2) as i64),
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
