//! `通用:10次招募（1回限定）` -- C# `CardGacha10`: spend 1,500, draw 1.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:10次招募（1回限定）`）:
//! > 10次招募（1回限定）：
//! > [手]：
//! > [消耗]1500资金，抽1张卡。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const GACHA10: CardDef = CardDef {
    id: "通用:10次招募（1回限定）",
    play: Some(gacha10),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `CardGacha10.WhyNot`: refuses the card with less than 1,500 (`资金不够 1,500`).
fn why_not(seat: i32) -> Option<Msg> {
    if ctx::money(seat) >= 1500 {
        return None;
    }
    Some(Msg::new(key!("x_no_money_1500")))
}

fn gacha10(seat: i32) {
    // 规则书[手]: 「[消耗]1500资金」 -- C# `PayCtx { kind: "lose", must: false }`
    // pays what the seat has; the C# draws whenever that payment was not
    // cancelled (`p.paid`).
    let paid = ctx::pay(seat, 1500, &Msg::new(key!("gacha10_why")));
    // 规则书[手]: 「抽1张卡」
    if paid > 0 {
        ctx::draw(seat, 1);
    }
}