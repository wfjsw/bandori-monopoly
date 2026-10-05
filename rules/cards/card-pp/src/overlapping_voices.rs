//! `PP:[衍生]重叠的声音` -- C# `CardOverlappingVoices` (MatchHost.cs:7290-7331):
//! remove itself and 「初次演出事故」, teleport to Bandori车站 without settling.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[衍生]重叠的声音`）:
//! > [衍生]重叠的声音：
//! > [特]：
//! > 此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的[场地]上的“初次演出事故”（如果有）。
//! > [手]：
//! > 依次进行以下操作：
//! > 1. [移除]此卡和[使用者][场地]上的“初次演出事故”（如果有）；
//! > 2. 进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”且不[结算]；
//! > 3. 回合结束后获得1层[停留]和1个正面的[P✽P粉丝]，将1张“明天见”加入手卡。
//!
//! The teleport is approximated with `teleport_to` (see the TODO on the main
//! move); the rest needs hooks the ABI lacks.

use card_sdk::{ctx, CardDef};

pub const OVERLAPPING_VOICES: CardDef = CardDef {
    id: "PP:[衍生]重叠的声音",
    play: Some(overlapping_voices),
    can_react: None,
    react: None,
    why_not: None,
};

fn overlapping_voices(seat: i32) {
    // 规则书[手]1: 「[移除]此卡」 -- C# `c.Dest = "removed"`.
    ctx::set_dest(ctx::Dest::Banished);
    // TODO(规则书)[手]1: 「和[使用者][场地]上的“初次演出事故”（如果有）」 -- needs
    // unplace-another-field-card (C# `H.Unplace(card, "removed", "重叠的声音")` on
    // the placed `CardFirstLiveAccident`); `unplace_card` only removes this card.
    // 规则书[手]2: 「[传送]到“bandori车站”且不[结算]」 -- C#
    // `H.TileNamed("Bandori车站")` + `H.ForceTeleport(..., resolve: false)`.
    let to = ctx::tile_named("Bandori车站");
    if to >= 0 {
        ctx::teleport_to(seat, to);
    }
    // TODO(规则书)[手]2: 「进入移动阶段并将本回合的[主要移动]改为」 -- needs the
    // H.CardMove / main-move routine so this teleport consumes the turn's main
    // move (C# `H.CardMove(c, new MoveCtx { TeleportTo = ..., Resolve = false })`).
    // Until then the seat still gets its normal main move after the teleport.
    // TODO(规则书)[手]3: 「回合结束后获得1层[停留]和1个正面的[P✽P粉丝]，将1张“明天见”
    // 加入手卡」 -- needs the turn-end AfterEnd hook (C#
    // `H._turnCtx.AfterEnd.Add(() => After(i))`) to `give_stay(seat, 1)`,
    // `add_tok("P✽P粉丝(正)", 1)`, `add_to_hand(seat, "PP:[衍生]明天见")`.
    // TODO(规则书): [特]「此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的[场地]上
    // 的“初次演出事故”（如果有）」 -- needs the Fx.OnDiscarded hook (C#
    // `Card.OnDiscarded(int)`).
    // C# `WhyNot` is `H.MoveWhyNot(seat)` (refuses while the seat cannot move);
    // that hook is still missing, so `why_not` stays `None` (over-permissive).
}