//! `RAS:UNSTOPPABLE` -- C# `CardUnstoppable` (MatchHost.cs:9823-9849).
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:UNSTOPPABLE`）:
//! > UNSTOPPABLE：
//! > 投掷1d6，根据结果1-6分别传送至白雪学园，艺术学院高中，瑟罗希亚国际学校，银河拉面馆，旭汤澡堂，CHUCHU的公寓。本次传送不触发结算，视为你的主要移动。且若骰点为1-3获得2000资金，若为4-6则获得1000资金。
//!

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const UNSTOPPABLE: CardDef =
    CardDef::new("RAS:UNSTOPPABLE", &[On::Play(Some(cant_play), unstoppable)]);

/// C# `CardUnstoppable.WhyNot` = `H.MoveWhyNot(seat)` -- the teleport is the
/// turn's main move.
fn cant_play(player_id: i32) -> Option<Msg> {
    ctx::cant_move(player_id)
}

/// C# `CardUnstoppable.Spots` -- the six destinations, indexed by the 1d6.
const SPOTS: [&str; 6] = [
    "白雪学园",
    "艺术学院高中",
    "瑟罗希亚国际学校",
    "银河拉面馆",
    "旭汤澡堂",
    "CHUCHU的公寓",
];

fn unstoppable(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「投掷1d6，根据结果1-6分别传送至白雪学园，艺术学院高中，瑟罗希亚国际学校，银河拉面馆，旭汤澡堂，CHUCHU的公寓」
    let r = ctx::roll(player_id, 1, 6);
    let spot = SPOTS[(r.clamp(1, 6) - 1) as usize];
    let to = ctx::tile_named(spot);
    if to >= 0 {
        // 规则书: 「本次传送不触发结算，视为你的主要移动」 -- C#
        // `H.CardMove(c, new MoveCtx { TeleportTo = ..., Resolve = false })`:
        // a teleport to the chosen tile that consumes the turn's main move and
        // does not settle.
        ctx::plan::set_teleport_to(to);
        ctx::plan::set_resolve(false);
        ctx::card_move(player_id);
    }
    // 规则书: 「且若骰点为1-3获得2000资金，若为4-6则获得1000资金」
    if !ctx::player_out(player_id) {
        let money = if r <= 3 { 2000 } else { 1000 };
        ctx::gain(
            player_id,
            money,
            &Msg::new(key!("unstoppable_why")).i("roll", r as i64),
        );
    }
    Ok(())
}
