//! `PP:梦在前方，结彩当下` -- C# `CardDreamAhead` (MatchHost.cs:7479-7609):
//! placed before the game starts; crystals grow on draws and settle tax grows
//! with X.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:梦在前方，结彩当下`）:
//! > 梦在前方，结彩当下：
//! > [特]：
//! > 游戏开始前将此卡放置在[使用者]的[场地]且初始手牌减1。
//! > [持续]：
//! >
//! > （1）[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]（上限5），此卡每获得一个超出上限的[奇迹水晶]就为此卡的X加1（X初始0）。
//! >
//! > （2）[拥有者]不可盖房且手卡上限数量减1。
//! >
//! > （3）[拥有者]以外的玩家在[拥有者]拥有的格子[结算]时额外[支付][拥有者]“[拥有者]拥有的[P✽P粉丝]数量”×X+MIN(X×30, 300)。
//! >
//! > （4）[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格以内有无主的[可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意[拥有者]所在格子前后3格以内的无主的[可购买格子]。
//!
//! Not a hand play (C# `Normal => false`, no `Play` override): the card is
//! placed by `DeckBeforeGame` and every [持续] line needs an Fx hook the ABI
//! lacks (below).

use card_sdk::CardDef;

pub const DREAM_AHEAD: CardDef = CardDef {
    id: "PP:梦在前方，结彩当下",
    play: None,
    can_react: None,
    react: None,
    why_not: None,
};

// TODO(规则书): [特]「游戏开始前将此卡放置在[使用者]的[场地]且初始手牌减1」 -- needs
// the Fx.DeckBeforeGame hook (C# `Card.DeckBeforeGame`: pull the id out of the
// draw pile, `H.PlaceCard(seat, seat, Id, -1, 0, ...)`, `H.IncV(seat,
// "startHandMinus")`).
// TODO(规则书): [持续]（1）「[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]（上限5），此卡每
// 获得一个超出上限的[奇迹水晶]就为此卡的X加1（X初始0）」 -- needs the Fx.Drew hook
// (C# `Card.Drew(int, List<string>)`), the per-card crystal counter capped at 5
// (`Card.MaxCrystals = 5`, `AddCrystals`), and per-card Mem state for X
// (C# `Mem["x"]`).
// TODO(规则书): [持续]（2）「[拥有者]不可盖房且手卡上限数量减1」 -- needs the
// Fx.CanBuild hook (C# `Card.CanBuild` returning `seat != Seat`) and
// Fx.HandLimitDelta (C# `Card.HandLimitDelta` returning -1 for the owner).
// TODO(规则书): [持续]（3）「[拥有者]以外的玩家在[拥有者]拥有的格子[结算]时额外[支付]
// [拥有者]“[拥有者]拥有的[P✽P粉丝]数量”×X+MIN(X×30, 300)」 -- needs the Fx.SettleAfter
// hook (C# `Card.SettleAfter(MoveCtx)`); the amount is
// `tok(P✽P粉丝(正))+tok(P✽P粉丝(反)) * X + min(X*30, 300)` paid via `H.PayR`.
// TODO(规则书): [持续]（4）「[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格
// 以内有无主的[可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意[拥有者]所在格子
// 前后3格以内的无主的[可购买格子]」 -- needs the Fx.TurnEnd hook (C#
// `Card.TurnEnd(int)`), H.TryResonance, the per-card crystal counter to spend 3,
// `H.Within3Free` (unowned buyable tiles within 3 ring steps either way -- the
// ABI's `tile_steps_ahead` is forward-only), and the buy routine
// (`H.BuyRoutine(Seat, tile, free: false, ...)`).