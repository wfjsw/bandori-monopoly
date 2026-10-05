//! `PPP:Returns` -- C# `CardReturns` (MatchHost.cs:8403-8581): a [特] card that
//! swells the starting deck, sits on the field from game start, and copies another
//! player's band card at a sticker price.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:Returns`）:
//! > Returns：
//! >  [特]：
//! >
//! > （1）如果此卡被加入初始卡组则卡组卡数添加8。
//! >
//! > （2）游戏开始时此卡从卡组放置到拥有此卡的玩家的[场地]上并获得一个其他存活玩家的团卡。
//! > [持续]：
//! >
//! > （1）无效[拥有者]Poppin' Party团卡的
//! > （4）效果。
//! >
//! > （2）[拥有者]每回合开始时选择一个其他存活玩家的团卡，如果和当前因此卡获得的团卡不一样则替换并移除上面的所有[奇迹水晶]。
//! >
//! > （3）使用因此卡获得的团卡的主动效果时需要支付1星星贴纸。
//! >
//! > （4）[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸。
//!
//! A pure [特] card (C# has no `Play`): deck setup, game-start placement, band-card
//! copying and the four [持续] clauses all sit on hooks the ABI does not carry
//! yet (TODO below).

use card_sdk::CardDef;

pub const RETURNS: CardDef = CardDef {
    id: "PPP:Returns",
    play: None,
    can_react: None,
    react: None,
    why_not: None,
};

// TODO(ABI)（1）: 「如果此卡被加入初始卡组则卡组卡数添加8。」
//   -- needs the `DeckBeforeGame` hook (C# `CardReturns.DeckBeforeGame` -> `AddEight`):
//   eight ids from `DeckRules.Pool` go into the draw pile (prompted pick or random,
//   C# `H.AskCard` / `H.AskYes`), then the pile is shuffled.
// TODO(ABI)（2）: 「游戏开始时此卡从卡组放置到拥有此卡的玩家的[场地]上并获得一个
//   其他存活玩家的团卡。」 -- needs the `DeckAtGameStart` hook (C#
//   `CardReturns.DeckAtGameStart` -> `Setup`, `H.PlaceCard`) and the band-copy
//   machinery (`H.BandOf` / `H.MakeBand(..., extra: true)` / `BandBase.Attach`,
//   C# `CardReturns.Choose`).
// TODO(ABI)[持续]（1）: 「无效[拥有者]Poppin' Party团卡的（4）效果。」
//   -- needs the band-card build gate (C# `BandPPP.NoBuildRule`, flipped off in
//   `CardReturns.Attach` / back on in `Detach`): while Returns is in play the PPP
//   band card's main-settle build restriction is lifted. No ABI surface for band
//   skill modifiers.
// TODO(ABI)[持续]（2）: 「[拥有者]每回合开始时选择一个其他存活玩家的团卡，如果和当前
//   因此卡获得的团卡不一样则替换并移除上面的所有[奇迹水晶]。」
//   -- needs the Fx.TurnStart hook (C# `CardReturns.TurnStart` -> `Choose(first: false)`)
//   plus `Drop` (detach the copied band, wipe `BandBase.OwnCrystals`).
// TODO(ABI)[持续]（3）: 「使用因此卡获得的团卡的主动效果时需要支付1星星贴纸。」
//   -- needs the band active-effect cost gate (C# pays the `星星贴纸` token before a
//   copied band's actives run; `H.AddTok(seat, "星星贴纸", -1)`).
// TODO(ABI)[持续]（4）: 「[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸。」
//   -- needs the Fx.CardPlayed hook (C# `CardReturns.CardPlayed`, `c.Effective` and
//   `H.Db.Card(c.Id)?.band == "通用"`) to `ctx::add_tok(seat, "星星贴纸", 1, i32::MAX)`.