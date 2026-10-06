//! `Sumimi:兼顾偶像与乐队` -- C# `CardIdolAndBand` (MatchHost.cs:11000-11036): reset
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:兼顾偶像与乐队`）:
//! > 兼顾偶像与乐队：
//! >  当你本回合未进行过赎回操作时可打出，将你的资金重设为3000。（若你的资金为3000以上则无效果）
//!
//! money to 3,000.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const IDOL_AND_BAND: CardDef = CardDef::new(
    "Sumimi:兼顾偶像与乐队",
    &[On::Play(Some(cant_play), idol_and_band)],
);

/// 规则书: 「当你本回合未进行过赎回操作时可打出」 -- the redeem step records
/// itself and clears at the turn end, so this is just asking that record.
fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::state::get(player_id, "redeemed") != 0 {
        return Some(Msg::new(key!("idol_and_band_redeemed")));
    }
    None
}

fn idol_and_band(player_id: i32) -> card_sdk::Asked {
    let have = ctx::money_of(player_id);
    if have >= 3000 {
        // 规则书: 「若你的资金为3000以上则无效果」
        ctx::log(player_id, &Msg::new(key!("idol_and_band_no_effect")));
        return Ok(());
    }
    // 规则书: 「将你的资金重设为3000」
    // C# H.Money(..., fixedAmount: true) -- the host's gain carries no such flag yet.
    ctx::gain(player_id, 3000 - have, &Msg::new(key!("idol_and_band_why")));
    Ok(())
}
