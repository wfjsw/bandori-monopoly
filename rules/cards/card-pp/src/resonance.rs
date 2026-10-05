//! `PP:[衍生]共鸣` -- C# `CardResonance` (MatchHost.cs:7448-7452): the [共鸣] fuel
//! card, consumed by `H.TryResonance`.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[衍生]共鸣`）:
//! > [衍生]共鸣：
//! > [特]：
//! > 拥有此卡的玩家的卡可发动[共鸣]时将此卡放入弃卡区，发动那张卡的[共鸣]效果并为拥有此卡的玩家的Pastel✽Palettes乐队卡添加2[奇迹水晶]。
//!
//! Not a hand play (C# `Normal => false`, no `Play` override): the [特] is the
//! host routine `H.TryResonance`, which every card's [共鸣] branch calls.

use card_sdk::CardDef;

pub const RESONANCE: CardDef = CardDef {
    id: "PP:[衍生]共鸣",
    play: None,
    can_react: None,
    react: None,
    why_not: None,
};

// TODO(规则书): [特]「拥有此卡的玩家的卡可发动[共鸣]时将此卡放入弃卡区，发动那张卡的[共鸣]
// 效果并为拥有此卡的玩家的Pastel✽Palettes乐队卡添加2[奇迹水晶]」 -- this whole
// sentence is C# `H.TryResonance` (refuse when 「PP:[衍生]共鸣」 is not in hand;
// `AskYes` then `hand.Remove` + `discard.Add`; `H.AddBandCrystals(seat, 2, "共鸣")`
// when `H.InBand(seat, "Pastel✽Palettes")`; then the caller's resonance body runs).
// Needs a host `H.TryResonance` / `H.HasResonance` hook so the other cards'
// [共鸣] branches can offer it; every such branch is TODO'd at its call site.