//! `RAS:UNSTOPPABLE` -- C# `CardUnstoppable` (MatchHost.cs:9823-9849).
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:UNSTOPPABLE`）:
//! > UNSTOPPABLE：
//! > 投掷1d6，根据结果1-6分别传送至白雪学园，艺术学院高中，瑟罗希亚国际学校，银河拉面馆，旭汤澡堂，CHUCHU的公寓。本次传送不触发结算，视为你的主要移动。且若骰点为1-3获得2000资金，若为4-6则获得1000资金。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const UNSTOPPABLE: CardDef = CardDef {
    id: "RAS:UNSTOPPABLE",
    play: Some(unstoppable),
    can_react: None,
    react: None,
    why_not: None,
};

/// C# `CardUnstoppable.Spots` -- the six destinations, indexed by the 1d6.
const SPOTS: [&str; 6] = [
    "白雪学园",
    "艺术学院高中",
    "瑟罗希亚国际学校",
    "银河拉面馆",
    "旭汤澡堂",
    "CHUCHU的公寓",
];

fn unstoppable(seat: i32) {
    // 规则书: 「投掷1d6，根据结果1-6分别传送至白雪学园，艺术学院高中，瑟罗希亚国际学校，银河拉面馆，旭汤澡堂，CHUCHU的公寓」
    let r = ctx::roll(seat, 1, 6);
    let spot = SPOTS[(r.clamp(1, 6) - 1) as usize];
    let to = ctx::tile_named(spot);
    if to >= 0 {
        // 规则书: 「本次传送不触发结算」 -- C# `H.ForceTeleport(..., resolve: false)`.
        ctx::teleport_to(seat, to);
    }
    // 规则书: 「且若骰点为1-3获得2000资金，若为4-6则获得1000资金」
    if !ctx::seat_out(seat) {
        let money = if r <= 3 { 2000 } else { 1000 };
        ctx::gain(seat, money, &Msg::new(key!("unstoppable_why")).i("roll", r as i64));
    }
    // TODO(规则书): 「视为你的主要移动」 -- needs the H.CardMove / main-move
    // routine so this teleport consumes the turn's main move (C#
    // `H.CardMove(c, new MoveCtx { TeleportTo = ..., Resolve = false })`).
    // Until then the seat still gets its normal main move after the teleport.
}