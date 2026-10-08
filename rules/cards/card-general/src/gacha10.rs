//! `通用:10次招募（1回限定）` -- C# `CardGacha10`: spend 1,500, draw 1.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:10次招募（1回限定）`）:
//! > 10次招募（1回限定）：
//! > [手]：
//! > [消耗]1500资金，抽1张卡。
//!

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const GACHA10: CardDef = CardDef::new(
    "通用:10次招募（1回限定）",
    // TODO(规则书) NEGATION-AUDIT V2: no activation cost. The 「[消耗]1500资金」
    // is effect content (rulebook L13-14), so there is no `money >= 1500` play
    // gate -- an unaffordable in-body payment takes the Q1 shortfall path
    // (mortgage, then bankruptcy). C# `CardGacha10.WhyNot` had one.
    &[On::Play(None, gacha10, "")],
).props(&[(card_sdk::abi::prop::EST_COST, 1500)]);

fn gacha10(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「[消耗]1500资金」 -- effect content; `Pay::must = true` runs
    // the Q1 shortfall path (规则书 L16/L76) when the payer cannot cover it.
    let paid = ctx::pay(player_id, 1500, &Msg::new(key!("gacha10_why")))?;
    // 规则书[手]: 「抽1张卡」
    if paid > 0 {
        ctx::draw(player_id, 1)?;
    }
    Ok(())
}
