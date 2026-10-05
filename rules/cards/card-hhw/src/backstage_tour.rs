//! `HHW:出发！后台之旅！` -- C# `CardBackstageTour` (MatchHost.cs:3718-3779): look
//! at the top two deck cards, keep 0-2 in order, discard the rest for 1,000 each.
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:出发！后台之旅！`）:
//! > 出发！后台之旅！：
//! >  看牌堆顶2张牌，选择0~2张以任意顺序放回，剩余的翻入弃牌堆，每翻入一张获得1000资金
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const BACKSTAGE_TOUR: CardDef = CardDef {
    id: "HHW:出发！后台之旅！",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // C# `CardBackstageTour.WhyNot` refuses the play when the draw pile and the
    // discard pile are both empty ("抽卡区没有卡").
    if ctx::deck_count(seat) + ctx::discard_size(seat) == 0 {
        return Some(Msg::new(key!("x_no_deck_cards")));
    }
    None
}

fn play(_seat: i32) {
    // 规则书: 「看牌堆顶2张牌，选择0~2张以任意顺序放回，剩余的翻入弃牌堆，每翻入一张获得1000资金」
    // TODO(规则书): the body needs deck-top inspection (C#
    // `H._hidden[seat].draw` top 2), `H.RevealSeen`, the per-card keep/discard
    // prompt (`H.AskYes(..., id, "放回", "翻入弃卡区")`), `H.ToDiscard` per card,
    // `H.GainR(i, 1000, ...)`, and reshuffling the discard back into the draw
    // pile when the draw pile has fewer than 2 cards. `discard_size` /
    // `deck_count` are now queryable, and `sweep_to_deck` / `to_discard` / `gain`
    // exist, but full hand/deck *enumeration* is still missing -- neither the
    // top-of-deck peek nor the per-card selection can be offered.
}