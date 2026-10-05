//! `Sumimi:兼顾偶像与乐队` -- C# `CardIdolAndBand` (MatchHost.cs:11000-11036): reset
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:兼顾偶像与乐队`）:
//! > 兼顾偶像与乐队：
//! >  当你本回合未进行过赎回操作时可打出，将你的资金重设为3000。（若你的资金为3000以上则无效果）
//!
//! money to 3,000.

use card_sdk::{ctx, key, CardDef, Msg};

pub const IDOL_AND_BAND: CardDef = CardDef {
    id: "Sumimi:兼顾偶像与乐队",
    play: Some(idol_and_band),
    can_react: None,
    react: None,
    why_not: None,
};

fn idol_and_band(seat: i32) {
    // TODO(规则书): 「当你本回合未进行过赎回操作时可打出」-- the C#
    //   `CardIdolAndBand.WhyNot` is only the `H._turnCtx.Redeemed` check
    //   （「本回合进行过赎回操作」）; that turn-context flag is still off the ABI,
    //   so `why_not` has nothing expressible to refuse here.
    let have = ctx::money(seat);
    if have >= 3000 {
        // 规则书: 「若你的资金为3000以上则无效果」
        ctx::log(seat, &Msg::new(key!("idol_and_band_no_effect")));
        return;
    }
    // 规则书: 「将你的资金重设为3000」
    // C# H.Money(..., fixedAmount: true) -- the host's gain carries no such flag yet.
    ctx::gain(seat, 3000 - have, &Msg::new(key!("idol_and_band_why")));
}