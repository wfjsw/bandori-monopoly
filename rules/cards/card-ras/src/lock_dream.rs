//! `RAS:（LOCK）追逐梦想的步伐` -- C# `CardLockDream` (MatchHost.cs:10003-10065):
//! pre-game field card that exiles to 东京外 and reroutes a Bandori车站 pass.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（LOCK）追逐梦想的步伐`）:
//! > （LOCK）追逐梦想的步伐：
//! >
//! > （1）抽取游戏开始的2手牌前将此卡从卡组展示给所有玩家并放置在自身场上，游戏开始时[传送]到“东京外”获得3层[除外]
//! >
//! > （2）如果此卡拥有者的主要移动[经过]了“Bandori车站”则在触发结算前将行动终点改为“旭汤澡堂”，然后此卡[移除]
//!

use card_sdk::CardDef;

/// C# `Normal => false` with no `Play`/`React`: the card is shown out of the
/// deck before the opening hands and lives on the field from there.
pub const LOCK_DREAM: CardDef = CardDef {
    id: "RAS:（LOCK）追逐梦想的步伐",
    play: None,
    can_react: None,
    react: None,
    why_not: None,
};

// 规则书（1）: 「抽取游戏开始的2手牌前将此卡从卡组展示给所有玩家并放置在自身场上」
// TODO(规则书): needs the Fx.DeckBeforeGame hook (C# `CardLockDream.DeckBeforeGame`
// pulls the card out of the draw pile and `H.PlaceCard(seat, seat, Id)` onto the
// owner's field, revealed to everyone).
// 规则书（1）: 「游戏开始时[传送]到“东京外”获得3层[除外]」
// TODO(规则书): needs the Fx.AtGameStart hook (C# `CardLockDream.AtGameStart` ->
// `H.Teleport` to `H.TileNamed("东京外")` with `resolve: false` and
// `H.GiveExile(Seat, 3, to)`). `ctx::teleport_to` + `ctx::give_exile` are ready
// once the hook runs.
// 规则书（2）: 「如果此卡拥有者的主要移动[经过]了“Bandori车站”则在触发结算前将行动终点改为“旭汤澡堂”，然后此卡[移除]」
// TODO(规则书): needs the Fx.PassTile + Fx.SettleBefore hooks (C#
// `CardLockDream.PassTile` tags `lockStation` when the owner's main move passes
// `H.TileNamed("Bandori车站")`, `SettleBefore` moves `Me.pos` to
// `H.TileNamed("旭汤澡堂")` and `H.Unplace(this, "removed")`).